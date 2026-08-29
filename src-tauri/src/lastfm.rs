use image::{ImageFormat, imageops::FilterType};
use keyring::Entry;
use percent_encoding::percent_decode_str;
use reqwest::{Url, blocking::Client};
use serde::Deserialize;
use serde::Serialize;
use std::{
    fs,
    io::{Cursor, Read},
    path::Path,
    sync::{Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager, Runtime, http};

const CREDENTIAL_SERVICE: &str = "Aurora";
const API_KEY_USER: &str = "Last.fm API key";
const SHARED_SECRET_USER: &str = "Last.fm shared secret";
const MAX_ARTIST_CHARS: usize = 512;
const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_PAGE_BYTES: u64 = 2 * 1024 * 1024;
const LAST_FM_API_URL: &str = "https://ws.audioscrobbler.com/2.0/";
const LAST_FM_PAGE_HOST: &str = "www.last.fm";
const LAST_FM_IMAGE_HOST: &str = "lastfm-img.freetls.fastly.net";
const LAST_FM_DEFAULT_IMAGE: &str = "2a96cbd8b46e442fc41c2b86b821562f";
const USER_AGENT: &str = concat!(
    "Aurora/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/soundtrackgeek/aurora)"
);

static CLIENT: OnceLock<Result<Client, String>> = OnceLock::new();
static IMAGE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static POPULARITY_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

const AVAILABLE_TTL_MS: i64 = 7 * 24 * 60 * 60 * 1_000;
const UNAVAILABLE_TTL_MS: i64 = 30 * 24 * 60 * 60 * 1_000;

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AlbumPopularity {
    pub(crate) tracks: Vec<AlbumPopularTrack>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AlbumPopularTrack {
    pub(crate) track_key: String,
    pub(crate) rank: u8,
}

#[derive(Clone, Debug)]
struct PopularityRecord {
    track_key: String,
    track_name: String,
    listeners: Option<i64>,
    play_count: Option<i64>,
    available: bool,
    fresh: bool,
}

fn popularity_key(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .map(|character| match character {
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2212}' => '-',
            '\u{2018}' | '\u{2019}' => '\'',
            other => other,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn loose_popularity_key(value: &str) -> String {
    popularity_key(value)
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn matching_record<'a>(
    records: &'a [PopularityRecord],
    title: &str,
) -> Option<&'a PopularityRecord> {
    let exact = popularity_key(title);
    if let Some(record) = records.iter().find(|record| record.track_key == exact) {
        return Some(record);
    }
    let loose = loose_popularity_key(title);
    let mut matches = records
        .iter()
        .filter(|record| loose_popularity_key(&record.track_name) == loose);
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn popularity_cache(
    store: &crate::state_store::StateStore,
) -> Result<rusqlite::Connection, String> {
    let connection = rusqlite::Connection::open(store.lastfm_cache_path())
        .map_err(|error| format!("Could not open Aurora's Last.fm cache: {error}"))?;
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS track_popularity (
              artist_key TEXT NOT NULL,
              track_key TEXT NOT NULL,
              track_name TEXT NOT NULL,
              listeners INTEGER,
              play_count INTEGER,
              state TEXT NOT NULL CHECK (state IN ('available', 'unavailable')),
              expires_at_ms INTEGER NOT NULL,
              PRIMARY KEY (artist_key, track_key)
            );",
        )
        .map_err(|error| format!("Could not prepare Aurora's Last.fm cache: {error}"))?;
    Ok(connection)
}

fn local_records(artist: &str, store: &crate::state_store::StateStore) -> Vec<PopularityRecord> {
    let Ok(connection) = popularity_cache(store) else {
        return Vec::new();
    };
    let Ok(mut statement) = connection.prepare(
        "SELECT track_key, track_name, listeners, play_count, state, expires_at_ms FROM track_popularity WHERE artist_key = ?",
    ) else { return Vec::new() };
    let cutoff = now_ms();
    statement
        .query_map([popularity_key(artist)], |row| {
            Ok(PopularityRecord {
                track_key: row.get(0)?,
                track_name: row.get(1)?,
                listeners: row.get(2)?,
                play_count: row.get(3)?,
                available: row.get::<_, String>(4)? == "available",
                fresh: row.get::<_, i64>(5)? > cutoff,
            })
        })
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .collect()
}

fn catalog_records(artist: &str) -> Vec<PopularityRecord> {
    let Ok(path) = crate::catalog::default_catalog_path() else {
        return Vec::new();
    };
    let Ok(connection) = crate::catalog::open_catalog(&path) else {
        return Vec::new();
    };
    let Ok(mut statement) = connection.prepare(
        "SELECT track_key, track_name, listeners, play_count, state FROM lastfm_track_popularity WHERE artist_key = lower(trim(?))",
    ) else { return Vec::new() };
    statement
        .query_map([artist.trim()], |row| {
            Ok(PopularityRecord {
                track_key: popularity_key(&row.get::<_, String>(0)?),
                track_name: row.get(1)?,
                listeners: row.get(2)?,
                play_count: row.get(3)?,
                available: row.get::<_, String>(4)? == "available",
                fresh: true,
            })
        })
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .collect()
}

fn combined_records(artist: &str, store: &crate::state_store::StateStore) -> Vec<PopularityRecord> {
    let mut records = catalog_records(artist);
    for local in local_records(artist, store) {
        if matching_record(&records, &local.track_name).is_none() {
            records.push(local);
        }
    }
    records
}

fn rank_album(
    tracks: &[crate::catalog::TrackSummary],
    records: &[PopularityRecord],
) -> AlbumPopularity {
    let mut ranked = tracks
        .iter()
        .enumerate()
        .filter_map(|(index, track)| {
            let record = matching_record(records, &track.title)?;
            (record.available && record.listeners.unwrap_or(0) > 0).then_some((
                track.track_key.clone(),
                record.listeners.unwrap_or(0),
                record.play_count.unwrap_or(0),
                index,
            ))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| right.2.cmp(&left.2))
            .then_with(|| left.3.cmp(&right.3))
    });
    AlbumPopularity {
        tracks: ranked
            .into_iter()
            .take(3)
            .enumerate()
            .map(|(index, (track_key, ..))| AlbumPopularTrack {
                track_key,
                rank: (index + 1) as u8,
            })
            .collect(),
    }
}

pub(crate) fn cached_album_popularity(
    artist: &str,
    tracks: &[crate::catalog::TrackSummary],
    store: &crate::state_store::StateStore,
) -> AlbumPopularity {
    rank_album(tracks, &combined_records(artist, store))
}

fn json_count(value: &serde_json::Value, key: &str) -> Option<i64> {
    value
        .get(key)
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
}

fn fetch_popularity(artist: &str, title: &str, api_key: &str) -> Result<PopularityRecord, String> {
    let response = client()?
        .get(LAST_FM_API_URL)
        .query(&[
            ("method", "track.getInfo"),
            ("artist", artist),
            ("track", title),
            ("autocorrect", "1"),
            ("api_key", api_key),
            ("format", "json"),
        ])
        .send()
        .map_err(|error| format!("Last.fm track lookup failed: {error}"))?;
    let status = response.status();
    let value: serde_json::Value = response
        .json()
        .map_err(|_| "Last.fm returned invalid track metadata.".to_owned())?;
    if value.get("error").and_then(serde_json::Value::as_i64) == Some(6) {
        return Ok(PopularityRecord {
            track_key: popularity_key(title),
            track_name: title.to_owned(),
            listeners: None,
            play_count: None,
            available: false,
            fresh: true,
        });
    }
    if value.get("error").is_some() {
        return Err(value
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("Last.fm rejected the track lookup.")
            .to_owned());
    }
    if !status.is_success() {
        return Err(format!("Last.fm returned {status}."));
    }
    let track = value
        .get("track")
        .ok_or_else(|| "Last.fm omitted track metadata.".to_owned())?;
    Ok(PopularityRecord {
        track_key: popularity_key(title),
        track_name: track
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(title)
            .to_owned(),
        listeners: json_count(track, "listeners"),
        play_count: json_count(track, "playcount"),
        available: true,
        fresh: true,
    })
}

fn save_record(artist: &str, record: &PopularityRecord, store: &crate::state_store::StateStore) {
    let Ok(connection) = popularity_cache(store) else {
        return;
    };
    let ttl = if record.available {
        AVAILABLE_TTL_MS
    } else {
        UNAVAILABLE_TTL_MS
    };
    let _ = connection.execute(
        "INSERT INTO track_popularity(artist_key, track_key, track_name, listeners, play_count, state, expires_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT(artist_key, track_key) DO UPDATE SET track_name=excluded.track_name, listeners=excluded.listeners, play_count=excluded.play_count, state=excluded.state, expires_at_ms=excluded.expires_at_ms",
        rusqlite::params![popularity_key(artist), record.track_key, record.track_name, record.listeners, record.play_count, if record.available { "available" } else { "unavailable" }, now_ms() + ttl],
    );
}

pub(crate) fn refresh_album_popularity(
    artist: &str,
    tracks: &[crate::catalog::TrackSummary],
    store: &crate::state_store::StateStore,
) -> AlbumPopularity {
    let Ok(_guard) = POPULARITY_LOCK.get_or_init(|| Mutex::new(())).lock() else {
        return cached_album_popularity(artist, tracks, store);
    };
    let Some(api_key) = api_key() else {
        return cached_album_popularity(artist, tracks, store);
    };
    let mut records = combined_records(artist, store);
    for track in tracks {
        if matching_record(&records, &track.title).is_some_and(|record| record.fresh) {
            continue;
        }
        let lookup_artist = track
            .display_artist
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(artist);
        match fetch_popularity(lookup_artist, &track.title, &api_key) {
            Ok(record) => {
                save_record(artist, &record, store);
                records.push(record);
            }
            Err(_) => break,
        }
        std::thread::sleep(Duration::from_millis(210));
    }
    rank_album(tracks, &combined_records(artist, store))
}

#[derive(Clone, Debug, Deserialize)]
#[serde(
    tag = "mode",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum LastFmCredentialsRequest {
    Save {
        api_key: String,
        shared_secret: String,
    },
    Clear,
}

#[derive(Debug, Deserialize)]
struct ArtistInfoResponse {
    artist: ArtistInfo,
}

#[derive(Debug, Deserialize)]
struct ArtistInfo {
    #[serde(default)]
    image: Vec<ArtistImage>,
    #[serde(default)]
    url: String,
}

#[derive(Debug, Deserialize)]
struct ArtistImage {
    #[serde(default)]
    size: String,
    #[serde(default, rename = "#text")]
    url: String,
}

#[derive(Debug, PartialEq)]
struct ArtistImageRequest {
    artist: String,
    size: u32,
}

fn credential_entry(user: &str) -> Result<Entry, String> {
    Entry::new(CREDENTIAL_SERVICE, user)
        .map_err(|error| format!("Could not open Aurora's credential vault: {error}"))
}

fn saved_credential(user: &str) -> Option<String> {
    credential_entry(user)
        .ok()
        .and_then(|entry| entry.get_password().ok())
        .filter(|value| !value.trim().is_empty())
}

fn api_key() -> Option<String> {
    saved_credential(API_KEY_USER).or_else(|| {
        cfg!(debug_assertions)
            .then(|| std::env::var("LAST_FM").ok())
            .flatten()
            .filter(|value| !value.trim().is_empty())
    })
}

fn shared_secret() -> Option<String> {
    saved_credential(SHARED_SECRET_USER).or_else(|| {
        cfg!(debug_assertions)
            .then(|| std::env::var("LAST_FM_SECRET").ok())
            .flatten()
            .filter(|value| !value.trim().is_empty())
    })
}

pub(crate) fn configured() -> bool {
    api_key().is_some()
}

pub(crate) fn secret_configured() -> bool {
    shared_secret().is_some()
}

fn validate_credential(value: String, label: &str) -> Result<String, String> {
    let value = value.trim().to_owned();
    if value.len() < 16 || value.len() > 256 || value.chars().any(char::is_whitespace) {
        return Err(format!("The Last.fm {label} is invalid."));
    }
    Ok(value)
}

fn delete_credential(user: &str) -> Result<(), String> {
    match credential_entry(user)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(format!(
            "Windows Credential Manager could not remove the Last.fm credentials: {error}"
        )),
    }
}

pub(crate) fn save_credentials(request: LastFmCredentialsRequest) -> Result<(), String> {
    match request {
        LastFmCredentialsRequest::Save {
            api_key,
            shared_secret,
        } => {
            let api_key = validate_credential(api_key, "API key")?;
            let shared_secret = validate_credential(shared_secret, "shared secret")?;
            credential_entry(API_KEY_USER)?
                .set_password(&api_key)
                .map_err(|error| {
                    format!(
                        "Windows Credential Manager could not update the Last.fm credentials: {error}"
                    )
                })?;
            if let Err(error) = credential_entry(SHARED_SECRET_USER)?.set_password(&shared_secret) {
                let _ = delete_credential(API_KEY_USER);
                return Err(format!(
                    "Windows Credential Manager could not update the Last.fm credentials: {error}"
                ));
            }
        }
        LastFmCredentialsRequest::Clear => {
            delete_credential(API_KEY_USER)?;
            delete_credential(SHARED_SECRET_USER)?;
        }
    }
    Ok(())
}

fn client() -> Result<&'static Client, String> {
    CLIENT
        .get_or_init(|| {
            Client::builder()
                .user_agent(USER_AGENT)
                .timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| "Aurora could not start Last.fm networking.".to_owned())
        })
        .as_ref()
        .map_err(Clone::clone)
}

fn parse_request(request: &http::Request<Vec<u8>>) -> Result<ArtistImageRequest, ()> {
    let encoded = request.uri().path().strip_prefix("/artist/").ok_or(())?;
    let artist = percent_decode_str(encoded)
        .decode_utf8()
        .map_err(|_| ())?
        .into_owned();
    if artist.trim().is_empty() || artist.chars().count() > MAX_ARTIST_CHARS {
        return Err(());
    }
    let size = request
        .uri()
        .query()
        .and_then(|query| {
            query.split('&').find_map(|pair| {
                let (key, value) = pair.split_once('=')?;
                (key == "size").then(|| value.parse::<u32>().ok()).flatten()
            })
        })
        .unwrap_or(64);
    if !matches!(size, 64 | 128) {
        return Err(());
    }
    Ok(ArtistImageRequest { artist, size })
}

fn choose_image_url(images: &[ArtistImage], size: u32) -> Option<Url> {
    let preferred = if size == 128 { "large" } else { "medium" };
    images
        .iter()
        .filter(|image| !image.url.contains(LAST_FM_DEFAULT_IMAGE))
        .filter_map(|image| Url::parse(image.url.trim()).ok().map(|url| (image, url)))
        .filter(|(_, url)| url.scheme() == "https" && url.host_str() == Some(LAST_FM_IMAGE_HOST))
        .min_by_key(|(image, _)| if image.size == preferred { 0 } else { 1 })
        .map(|(_, url)| url)
}

fn attribute_value<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let lower = tag.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut offset = 0;
    while let Some(relative) = lower[offset..].find(name) {
        let start = offset + relative;
        let before_is_name = start
            .checked_sub(1)
            .and_then(|index| bytes.get(index))
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'-');
        let after_name = start + name.len();
        let after_is_name = bytes
            .get(after_name)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'-');
        if before_is_name || after_is_name {
            offset = after_name;
            continue;
        }
        let mut cursor = after_name;
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b'=') {
            offset = after_name;
            continue;
        }
        cursor += 1;
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        let quote = *bytes.get(cursor)?;
        if !matches!(quote, b'\'' | b'"') {
            offset = after_name;
            continue;
        }
        let value_start = cursor + 1;
        let value_end = bytes[value_start..]
            .iter()
            .position(|byte| *byte == quote)?
            + value_start;
        return tag.get(value_start..value_end);
    }
    None
}

fn page_portrait_url(html: &str) -> Option<Url> {
    let lower = html.to_ascii_lowercase();
    let mut offset = 0;
    while let Some(relative) = lower[offset..].find("<meta") {
        let start = offset + relative;
        let end = lower[start..].find('>')? + start + 1;
        let tag = html.get(start..end)?;
        if attribute_value(tag, "property")
            .is_some_and(|value| value.eq_ignore_ascii_case("og:image"))
        {
            let url = Url::parse(attribute_value(tag, "content")?.trim()).ok()?;
            if url.scheme() == "https"
                && url.host_str() == Some(LAST_FM_IMAGE_HOST)
                && !url.as_str().contains(LAST_FM_DEFAULT_IMAGE)
            {
                return Some(url);
            }
        }
        offset = end;
    }
    None
}

fn page_image_url(page_url: &str) -> Result<Url, String> {
    let page_url = Url::parse(page_url)
        .map_err(|_| "Last.fm returned an invalid artist page URL.".to_owned())?;
    if page_url.scheme() != "https" || page_url.host_str() != Some(LAST_FM_PAGE_HOST) {
        return Err("Last.fm returned an unsafe artist page URL.".to_owned());
    }
    let response = client()?
        .get(page_url)
        .send()
        .map_err(|_| "Aurora could not load the Last.fm artist page.".to_owned())?;
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|length| length == 0 || length > MAX_PAGE_BYTES)
    {
        return Err("The Last.fm artist page is unavailable.".to_owned());
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_PAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Aurora could not read the Last.fm artist page.".to_owned())?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_PAGE_BYTES {
        return Err("The Last.fm artist page is outside Aurora's safe size range.".to_owned());
    }
    let html = String::from_utf8(bytes)
        .map_err(|_| "Last.fm returned an invalid artist page.".to_owned())?;
    page_portrait_url(&html).ok_or_else(|| "Last.fm has no usable artist portrait.".to_owned())
}

fn resolve_artist_image(artist: &str, size: u32) -> Result<Url, String> {
    let api_key = api_key().ok_or_else(|| "Last.fm is not configured.".to_owned())?;
    let response = client()?
        .get(LAST_FM_API_URL)
        .query(&[
            ("method", "artist.getinfo"),
            ("artist", artist),
            ("api_key", api_key.as_str()),
            ("autocorrect", "1"),
            ("format", "json"),
        ])
        .send()
        .map_err(|_| "Aurora could not connect to Last.fm.".to_owned())?;
    if !response.status().is_success() {
        return Err(format!(
            "Last.fm artist lookup failed with HTTP {}.",
            response.status()
        ));
    }
    let payload = response
        .json::<ArtistInfoResponse>()
        .map_err(|_| "Last.fm returned invalid artist metadata.".to_owned())?;
    choose_image_url(&payload.artist.image, size)
        .map(Ok)
        .unwrap_or_else(|| page_image_url(&payload.artist.url))
}

fn image_bytes(url: Url, size: u32) -> Result<Vec<u8>, String> {
    let response = client()?
        .get(url)
        .send()
        .map_err(|_| "Aurora could not download the Last.fm artist portrait.".to_owned())?;
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|length| length == 0 || length > MAX_IMAGE_BYTES)
    {
        return Err("The Last.fm artist portrait is unavailable.".to_owned());
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Aurora could not read the Last.fm artist portrait.".to_owned())?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_IMAGE_BYTES {
        return Err("The Last.fm artist portrait is outside Aurora's safe size range.".to_owned());
    }
    let image = image::load_from_memory(&bytes)
        .map_err(|_| "Aurora could not decode the Last.fm artist portrait.".to_owned())?;
    let thumbnail = image.resize_to_fill(size, size, FilterType::Lanczos3);
    let mut output = Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut output, ImageFormat::WebP)
        .map_err(|_| "Aurora could not encode the Last.fm artist portrait.".to_owned())?;
    Ok(output.into_inner())
}

fn cache_path<R: Runtime>(
    app: &AppHandle<R>,
    artist: &str,
    size: u32,
) -> Result<std::path::PathBuf, String> {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(artist.trim().to_lowercase().as_bytes());
    let filename = format!("{:x}-{size}.webp", digest);
    app.path()
        .app_cache_dir()
        .map(|root| root.join("artist-portraits").join(filename))
        .map_err(|_| "Aurora's cache directory is unavailable.".to_owned())
}

fn cache_image(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Aurora's artist portrait cache has no parent directory.".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|_| "Aurora could not create its artist portrait cache.".to_owned())?;
    let temporary = path.with_extension(format!("{}-{}.tmp", std::process::id(), bytes.len()));
    fs::write(&temporary, bytes)
        .map_err(|_| "Aurora could not stage the artist portrait cache.".to_owned())?;
    match fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(_) if path.is_file() => {
            let _ = fs::remove_file(&temporary);
            Ok(())
        }
        Err(_) => {
            let _ = fs::remove_file(&temporary);
            Err("Aurora could not finish caching the artist portrait.".to_owned())
        }
    }
}

fn load_artist_image<R: Runtime>(
    app: &AppHandle<R>,
    artist: &str,
    size: u32,
) -> Result<Vec<u8>, String> {
    let path = cache_path(app, artist, size)?;
    if let Ok(bytes) = fs::read(&path) {
        return Ok(bytes);
    }
    let _guard = IMAGE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "Aurora's Last.fm image loader stopped unexpectedly.".to_owned())?;
    if let Ok(bytes) = fs::read(&path) {
        return Ok(bytes);
    }
    let url = resolve_artist_image(artist, size)?;
    let bytes = image_bytes(url, size)?;
    cache_image(&path, &bytes)?;
    Ok(bytes)
}

fn response(
    status: http::StatusCode,
    content_type: &'static str,
    body: Vec<u8>,
) -> http::Response<Vec<u8>> {
    http::Response::builder()
        .status(status)
        .header(http::header::CONTENT_TYPE, content_type)
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(
            http::header::CACHE_CONTROL,
            if status.is_success() {
                "public, max-age=31536000, immutable"
            } else {
                "no-store"
            },
        )
        .body(body)
        .expect("valid Last.fm artist image response")
}

pub(crate) fn handle_artist_image_request<R: Runtime>(
    app: &AppHandle<R>,
    request: &http::Request<Vec<u8>>,
) -> http::Response<Vec<u8>> {
    let Ok(request) = parse_request(request) else {
        return response(http::StatusCode::BAD_REQUEST, "text/plain", Vec::new());
    };
    match load_artist_image(app, &request.artist, request.size) {
        Ok(bytes) => response(http::StatusCode::OK, "image/webp", bytes),
        Err(_) => response(http::StatusCode::NOT_FOUND, "text/plain", Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popularity_matching_handles_music_metadata_punctuation_without_guessing_ambiguously() {
        let records = vec![PopularityRecord {
            track_key: popularity_key("Love–Song"),
            track_name: "Love–Song".to_owned(),
            listeners: Some(42),
            play_count: Some(99),
            available: true,
            fresh: true,
        }];
        assert_eq!(
            matching_record(&records, " LOVE-SONG ").map(|record| record.listeners),
            Some(Some(42))
        );
    }

    #[test]
    fn artist_image_protocol_accepts_only_bounded_names_and_sizes() {
        let request = http::Request::builder()
            .uri("http://aurora-artist.localhost/artist/Dolly%20Parton?size=64")
            .body(Vec::new())
            .expect("request");
        assert_eq!(
            parse_request(&request),
            Ok(ArtistImageRequest {
                artist: "Dolly Parton".to_owned(),
                size: 64,
            })
        );

        let oversized = http::Request::builder()
            .uri("http://aurora-artist.localhost/artist/Dolly%20Parton?size=512")
            .body(Vec::new())
            .expect("request");
        assert_eq!(parse_request(&oversized), Err(()));
    }

    #[test]
    fn artist_image_selection_prefers_the_requested_safe_cdn_size() {
        let images = vec![
            ArtistImage {
                size: "large".to_owned(),
                url: "https://lastfm-img.freetls.fastly.net/i/u/174s/example.jpg".to_owned(),
            },
            ArtistImage {
                size: "medium".to_owned(),
                url: "https://lastfm-img.freetls.fastly.net/i/u/64s/example.jpg".to_owned(),
            },
            ArtistImage {
                size: "medium".to_owned(),
                url: "https://example.com/not-allowed.jpg".to_owned(),
            },
        ];
        assert_eq!(
            choose_image_url(&images, 64).map(|url| url.path().to_owned()),
            Some("/i/u/64s/example.jpg".to_owned())
        );
    }

    #[test]
    fn artist_image_selection_rejects_last_fm_placeholder_art() {
        let images = vec![ArtistImage {
            size: "medium".to_owned(),
            url: format!(
                "https://lastfm-img.freetls.fastly.net/i/u/64s/{LAST_FM_DEFAULT_IMAGE}.png"
            ),
        }];
        assert!(choose_image_url(&images, 64).is_none());
    }

    #[test]
    fn artist_page_fallback_accepts_only_last_fm_portrait_metadata() {
        let valid = r#"<meta content="https://lastfm-img.freetls.fastly.net/i/u/ar0/portrait.jpg" property="og:image">"#;
        assert_eq!(
            page_portrait_url(valid).map(|url| url.path().to_owned()),
            Some("/i/u/ar0/portrait.jpg".to_owned())
        );

        let unsafe_host =
            r#"<meta property="og:image" content="https://example.com/portrait.jpg">"#;
        let placeholder = format!(
            r#"<meta property='og:image' content='https://lastfm-img.freetls.fastly.net/i/u/ar0/{LAST_FM_DEFAULT_IMAGE}.png'>"#
        );
        assert!(page_portrait_url(unsafe_host).is_none());
        assert!(page_portrait_url(&placeholder).is_none());
    }

    #[test]
    #[ignore = "requires a live LAST_FM API key"]
    fn live_artist_image_pipeline_returns_a_bounded_square_webp() {
        let url = resolve_artist_image("Dolly Parton", 64).expect("resolve portrait");
        let bytes = image_bytes(url, 64).expect("download portrait");
        let image = image::load_from_memory(&bytes).expect("decode cached WebP");
        assert_eq!((image.width(), image.height()), (64, 64));
        assert!(bytes.len() < MAX_IMAGE_BYTES as usize);
    }
}
