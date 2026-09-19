use crate::{lastfm, state_store::StateStore};
use reqwest::blocking::Response;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;

const MAX_RESPONSE_BYTES: u64 = 2 * 1024 * 1024;
const CACHE_TTL_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PopularTrack {
    name: String,
    play_count: Option<u64>,
    listeners: Option<u64>,
    url: Option<String>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SimilarArtist {
    name: String,
    match_score: Option<f64>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArtistDiscovery {
    biography: String,
    listeners: Option<u64>,
    play_count: Option<u64>,
    tags: Vec<String>,
    top_tracks: Vec<PopularTrack>,
    similar_artists: Vec<SimilarArtist>,
    warnings: Vec<String>,
}

pub(crate) fn validate_artist(artist: &str) -> Result<&str, String> {
    let artist = artist.trim();
    if artist.is_empty() || artist.chars().count() > 256 {
        return Err("Choose an artist name between 1 and 256 characters.".into());
    }
    Ok(artist)
}

fn provider_link(url: &str) -> Result<reqwest::Url, String> {
    let parsed = reqwest::Url::parse(url).map_err(|_| "Invalid artist link.".to_owned())?;
    if parsed.scheme() != "https"
        || !matches!(
            parsed.host_str(),
            Some("www.last.fm" | "musicbrainz.org" | "en.wikipedia.org" | "fanart.tv")
        )
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
    {
        return Err("This artist link is not supported.".into());
    }
    Ok(parsed)
}

pub(crate) fn open_link(url: &str) -> Result<(), String> {
    let url = provider_link(url)?;
    #[cfg(windows)]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut command = std::process::Command::new("rundll32.exe");
        command
            .arg("url.dll,FileProtocolHandler")
            .creation_flags(0x08000000);
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(not(any(windows, target_os = "macos")))]
    let mut command = std::process::Command::new("xdg-open");
    command
        .arg(url.as_str())
        .spawn()
        .map_err(|_| "Could not open the artist link in your browser.".to_owned())?;
    Ok(())
}

pub(crate) fn read_json(response: Response) -> Result<Value, String> {
    if !response.status().is_success() {
        return Err(format!(
            "The provider returned HTTP {}.",
            response.status().as_u16()
        ));
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_RESPONSE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "The provider response could not be read.".to_owned())?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err("The provider response was too large.".into());
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| "The provider returned invalid metadata.".to_owned())?;
    if value.get("error").is_some() {
        // Do not echo provider messages or request URLs: they may contain credentials.
        return Err(
            "The provider could not complete this request. Check your credentials or retry later."
                .into(),
        );
    }
    Ok(value)
}

/// Device-local provider cache. Failed refreshes never replace a successful response.
pub(crate) fn cached_json(
    store: &StateStore,
    key: &str,
    refresh: bool,
    fetch: impl FnOnce() -> Result<Value, String>,
) -> Result<(Value, bool), String> {
    let connection =
        rusqlite::Connection::open(store.path().with_file_name("aurora-artist-cache.sqlite3"))
            .map_err(|_| "The artist cache could not be opened.".to_owned())?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    connection.execute_batch("CREATE TABLE IF NOT EXISTS artist_provider_cache (cache_key TEXT PRIMARY KEY, payload TEXT NOT NULL, updated_at_ms INTEGER NOT NULL)")
        .map_err(|_| "The artist cache could not be initialized.".to_owned())?;
    let cached = connection
        .query_row(
            "SELECT payload, updated_at_ms FROM artist_provider_cache WHERE cache_key = ?1",
            [key],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(|_| "The artist cache could not be read.".to_owned())?
        .and_then(|(text, at)| {
            serde_json::from_str::<Value>(&text)
                .ok()
                .map(|value| (value, at))
        });
    let now = crate::state_sync::now_ms();
    if !refresh
        && let Some((value, at)) = &cached
        && now.saturating_sub(*at) < CACHE_TTL_MS
    {
        return Ok((value.clone(), false));
    }
    match fetch() {
        Ok(value) => {
            connection.execute("INSERT INTO artist_provider_cache VALUES (?1, ?2, ?3) ON CONFLICT(cache_key) DO UPDATE SET payload=excluded.payload, updated_at_ms=excluded.updated_at_ms", params![key, value.to_string(), now])
                .map_err(|_| "The artist cache could not be saved.".to_owned())?;
            Ok((value, false))
        }
        Err(error) => cached.map(|(value, _)| (value, true)).ok_or(error),
    }
}

fn number(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| value.as_str()?.parse().ok())
}

fn items(value: &Value) -> Vec<&Value> {
    match value {
        Value::Array(items) => items.iter().collect(),
        Value::Object(_) => vec![value],
        _ => vec![],
    }
}

fn lastfm_url(value: &Value) -> Option<String> {
    let mut url = reqwest::Url::parse(value.as_str()?).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str() != Some("www.last.fm") {
        return None;
    }
    url.set_scheme("https").ok()?;
    Some(url.into())
}

fn apply_payload(result: &mut ArtistDiscovery, method: &str, payload: Value) {
    match method {
        "artist.getinfo" => {
            let artist = &payload["artist"];
            result.biography = artist["bio"]["summary"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .take(8000)
                .collect();
            result.listeners = number(&artist["stats"]["listeners"]);
            result.play_count = number(&artist["stats"]["playcount"]);
            result.tags = items(&artist["tags"]["tag"])
                .iter()
                .filter_map(|tag| tag["name"].as_str().map(str::to_owned))
                .take(12)
                .collect();
        }
        "artist.gettoptracks" => {
            result.top_tracks = items(&payload["toptracks"]["track"])
                .iter()
                .filter_map(|track| {
                    Some(PopularTrack {
                        name: track["name"].as_str()?.to_owned(),
                        play_count: number(&track["playcount"]),
                        listeners: number(&track["listeners"]),
                        url: lastfm_url(&track["url"]),
                    })
                })
                .take(10)
                .collect();
        }
        "artist.getsimilar" => {
            result.similar_artists = items(&payload["similarartists"]["artist"])
                .iter()
                .filter_map(|artist| {
                    Some(SimilarArtist {
                        name: artist["name"].as_str()?.to_owned(),
                        match_score: artist["match"]
                            .as_f64()
                            .or_else(|| artist["match"].as_str()?.parse().ok()),
                    })
                })
                .take(12)
                .collect();
        }
        _ => {}
    }
}

pub(crate) fn load(
    artist: &str,
    refresh: bool,
    store: &StateStore,
) -> Result<ArtistDiscovery, String> {
    let artist = validate_artist(artist)?;
    let mut result = ArtistDiscovery::default();
    for method in ["artist.getinfo", "artist.gettoptracks", "artist.getsimilar"] {
        let response = cached_json(
            store,
            &format!("lastfm:{method}:{}", artist.to_lowercase()),
            refresh,
            || {
                let key =
                    lastfm::api_key().ok_or("Add Last.fm credentials in Settings → Metadata.")?;
                let response = lastfm::client()?
                    .get("https://ws.audioscrobbler.com/2.0/")
                    .query(&[
                        ("method", method),
                        ("artist", artist),
                        ("api_key", &key),
                        ("format", "json"),
                        ("autocorrect", "0"),
                        (
                            "limit",
                            if method == "artist.gettoptracks" {
                                "10"
                            } else {
                                "12"
                            },
                        ),
                    ])
                    .send()
                    .map_err(|_| "Last.fm is temporarily unavailable.".to_owned())?;
                let value = read_json(response)?;
                let root = match method {
                    "artist.getinfo" => "artist",
                    "artist.gettoptracks" => "toptracks",
                    _ => "similarartists",
                };
                if !value[root].is_object() {
                    return Err("Last.fm returned incomplete artist metadata.".into());
                }
                Ok(value)
            },
        );
        match response {
            Ok((payload, stale)) => {
                apply_payload(&mut result, method, payload);
                if stale {
                    result.warnings.push(
                        "Showing saved Last.fm data; the latest refresh was unavailable.".into(),
                    );
                }
            }
            Err(error) => result.warnings.push(error),
        }
    }
    result.warnings.sort();
    result.warnings.dedup();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn provider_rank_is_preserved_and_missing_counts_stay_unknown() {
        let mut result = ArtistDiscovery::default();
        apply_payload(
            &mut result,
            "artist.gettoptracks",
            json!({"toptracks":{"track":[{"name":"First","playcount":"100","listeners":"20","url":"http://www.last.fm/music/a/_/First"},{"name":"Second","playcount":"bad","url":"javascript:alert(1)"}]}}),
        );
        assert_eq!(result.top_tracks[0].name, "First");
        assert_eq!(result.top_tracks[0].play_count, Some(100));
        assert_eq!(result.top_tracks[1].play_count, None);
        assert!(result.top_tracks[1].url.is_none());
        assert!(
            result.top_tracks[0]
                .url
                .as_ref()
                .unwrap()
                .starts_with("https://")
        );
    }

    #[test]
    fn singleton_similar_artist_and_empty_results_are_supported() {
        let mut result = ArtistDiscovery::default();
        apply_payload(
            &mut result,
            "artist.getsimilar",
            json!({"similarartists":{"artist":{"name":"Other","match":"0.9"}}}),
        );
        assert_eq!(result.similar_artists.len(), 1);
        assert_eq!(result.similar_artists[0].match_score, Some(0.9));
        apply_payload(
            &mut result,
            "artist.getsimilar",
            json!({"similarartists":{"artist":[]}}),
        );
        assert!(result.similar_artists.is_empty());
    }

    #[test]
    fn cache_reuses_fresh_results_and_preserves_them_after_a_failed_refresh() {
        let directory = tempfile::tempdir().unwrap();
        let store = StateStore::new(directory.path().join("state.sqlite3")).unwrap();
        let first = cached_json(&store, "test", false, || Ok(json!({"value": 7}))).unwrap();
        assert!(!first.1);
        let fresh = cached_json(&store, "test", false, || {
            panic!("fresh cache must avoid networking")
        })
        .unwrap();
        assert_eq!(fresh.0["value"], 7);
        let stale = cached_json(&store, "test", true, || Err("offline".into())).unwrap();
        assert!(stale.1);
        assert_eq!(stale.0["value"], 7);
        assert!(cached_json(&store, "missing", true, || Err("offline".into())).is_err());
    }

    #[test]
    fn external_links_are_limited_to_https_provider_pages() {
        assert!(provider_link("https://www.last.fm/music/Superchunk").is_ok());
        for url in [
            "file:///C:/test",
            "javascript:alert(1)",
            "https://www.last.fm.evil.test/",
            "https://user@musicbrainz.org/",
            "https://musicbrainz.org:9999/",
        ] {
            assert!(provider_link(url).is_err());
        }
    }

    #[test]
    #[ignore = "requires local catalog and configured Last.fm/fanart.tv credentials"]
    fn live_artist_providers() {
        let _ = dotenvy::from_filename("../.env.local");
        let directory = tempfile::tempdir().unwrap();
        let store = StateStore::new(directory.path().join("state.sqlite3")).unwrap();
        let artist = "Superchunk";
        let discovery = load(artist, true, &store).unwrap();
        assert!(
            discovery.warnings.is_empty(),
            "Provider lookup must succeed"
        );
        assert_eq!(discovery.top_tracks.len(), 10);
        assert!(!discovery.similar_artists.is_empty());
        let artwork = crate::fanart::load(artist, true, &store).unwrap();
        let artwork_json = serde_json::to_value(&artwork).unwrap();
        assert!(artwork_json["backgroundUrl"].as_str().is_some());
        let detail = crate::explorer::load_artist_detail(artist.to_owned(), &store).unwrap();
        let intelligence =
            crate::musicbrainz::load_artist_intelligence_with_store(artist.to_owned(), &store)
                .unwrap();
        if let Ok(path) = std::env::var("AURORA_ARTIST_QA_FIXTURE") {
            let tracks = crate::explorer::load_track_page(
                crate::explorer::TrackPageRequest {
                    artist: Some(artist.to_owned()),
                    page_size: Some(100),
                    ..Default::default()
                },
                &store,
                true,
            )
            .unwrap();
            let fixture = json!({"artist": artist, "discovery": discovery, "artwork": artwork, "detail": detail, "intelligence": intelligence, "tracks": tracks});
            std::fs::write(path, serde_json::to_vec(&fixture).unwrap()).unwrap();
        }
    }
}
