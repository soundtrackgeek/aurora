use crate::{ArtworkSelectionState, InboxState, catalog, tagging::read_tag_for_write};
use id3::{
    TagLike,
    frame::{Picture, PictureType},
};
use image::{ImageFormat, ImageReader, imageops::FilterType};
use percent_encoding::percent_decode_str;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{VecDeque, hash_map::DefaultHasher},
    fs,
    hash::{Hash, Hasher},
    io::Cursor,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager, Runtime, http};

const MAX_COVER_BYTES: u64 = 32 * 1024 * 1024;
const MAX_COVER_PIXELS: u64 = 100_000_000;
const MAX_ARTWORK_SELECTIONS: usize = 16;
static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub(crate) struct CanonicalCover {
    pub(crate) picture: Picture,
    pub(crate) digest: [u8; 32],
}

#[derive(Clone)]
struct ArtworkSelection {
    token: String,
    path: PathBuf,
    digest: [u8; 32],
}

#[derive(Default)]
pub(crate) struct ArtworkSelectionRegistry {
    selections: VecDeque<ArtworkSelection>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SelectedArtwork {
    token: String,
    preview_url: String,
    file_name: String,
}

impl ArtworkSelectionRegistry {
    fn register(&mut self, path: PathBuf, digest: [u8; 32]) -> SelectedArtwork {
        let sequence = TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let token = format!("{}-{sequence}", now_ns());
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Selected cover")
            .to_owned();
        self.selections.push_back(ArtworkSelection {
            token: token.clone(),
            path,
            digest,
        });
        while self.selections.len() > MAX_ARTWORK_SELECTIONS {
            self.selections.pop_front();
        }
        SelectedArtwork {
            preview_url: format!("http://aurora-cover.localhost/selected/{token}?size=256"),
            token,
            file_name,
        }
    }

    fn resolve(&self, token: &str) -> Result<PathBuf, String> {
        let selection = self
            .selections
            .iter()
            .find(|selection| selection.token == token)
            .ok_or_else(|| {
                "The selected album cover expired. Choose it again before saving.".to_owned()
            })?;
        let cover = canonical_cover_from_image(&selection.path)?;
        if cover.digest != selection.digest {
            return Err(
                "The selected album-cover file changed. Choose it again before saving.".to_owned(),
            );
        }
        Ok(selection.path.clone())
    }
}

fn now_ns() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

pub(crate) fn register_selected_artwork<R: Runtime>(
    app: &AppHandle<R>,
    path: PathBuf,
) -> Result<SelectedArtwork, String> {
    let cover = canonical_cover_from_image(&path)?;
    let selected = app
        .state::<ArtworkSelectionState>()
        .lock()
        .map_err(|_| "Aurora's album-cover picker stopped unexpectedly.".to_owned())?
        .register(path, cover.digest);
    Ok(selected)
}

pub(crate) fn selected_cover<R: Runtime>(
    app: &AppHandle<R>,
    token: Option<&str>,
) -> Result<Option<CanonicalCover>, String> {
    token
        .map(|token| {
            let path = app
                .state::<ArtworkSelectionState>()
                .lock()
                .map_err(|_| "Aurora's album-cover picker stopped unexpectedly.".to_owned())?
                .resolve(token)?;
            canonical_cover_from_image(&path)
        })
        .transpose()
}

pub(crate) fn validate_cover_bytes(data: &[u8]) -> Result<([u8; 32], &'static str), String> {
    if data.is_empty() || data.len() as u64 > MAX_COVER_BYTES {
        return Err("The album cover is outside Aurora's safe size range.".to_owned());
    }
    let format = image::guess_format(data)
        .map_err(|_| "Aurora could not identify the album-cover image.".to_owned())?;
    let (width, height) = ImageReader::with_format(Cursor::new(data), format)
        .into_dimensions()
        .map_err(|_| "Aurora could not decode the album-cover image.".to_owned())?;
    if width == 0
        || height == 0
        || u64::from(width).saturating_mul(u64::from(height)) > MAX_COVER_PIXELS
    {
        return Err("The album-cover dimensions are outside Aurora's safe range.".to_owned());
    }
    ImageReader::with_format(Cursor::new(data), format)
        .decode()
        .map_err(|_| "Aurora could not fully decode the album-cover image.".to_owned())?;
    Ok((Sha256::digest(data).into(), format.to_mime_type()))
}

pub(crate) fn canonical_cover_from_image(path: &Path) -> Result<CanonicalCover, String> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("Could not open the selected album cover: {error}"))?;
    if !canonical.is_file() {
        return Err("The selected album cover is not a file.".to_owned());
    }
    let data = fs::read(&canonical)
        .map_err(|error| format!("Could not read the selected album cover: {error}"))?;
    canonical_cover_from_picture(&Picture {
        mime_type: String::new(),
        picture_type: PictureType::CoverFront,
        description: String::new(),
        data,
    })
}

pub(crate) fn canonical_cover_from_picture(picture: &Picture) -> Result<CanonicalCover, String> {
    let (digest, mime_type) = validate_cover_bytes(&picture.data)?;
    Ok(CanonicalCover {
        picture: Picture {
            mime_type: mime_type.to_owned(),
            picture_type: PictureType::CoverFront,
            description: String::new(),
            data: picture.data.clone(),
        },
        digest,
    })
}

pub(crate) fn cover_digest(picture: &Picture) -> Result<[u8; 32], String> {
    validate_cover_bytes(&picture.data).map(|(digest, _)| digest)
}

pub(crate) fn front_cover_matches(tag: &id3::Tag, expected_digest: &[u8; 32]) -> bool {
    let pictures = tag
        .pictures()
        .filter(|picture| picture.picture_type == PictureType::CoverFront)
        .collect::<Vec<_>>();
    pictures.len() == 1 && cover_digest(pictures[0]).is_ok_and(|digest| &digest == expected_digest)
}

pub(crate) fn front_cover_fingerprint(tag: &id3::Tag) -> [u8; 32] {
    let mut hasher = Sha256::new();
    let pictures = tag
        .pictures()
        .filter(|picture| picture.picture_type == PictureType::CoverFront)
        .collect::<Vec<_>>();
    hasher.update((pictures.len() as u64).to_le_bytes());
    for picture in pictures {
        hasher.update((picture.mime_type.len() as u64).to_le_bytes());
        hasher.update(picture.mime_type.as_bytes());
        hasher.update((picture.description.len() as u64).to_le_bytes());
        hasher.update(picture.description.as_bytes());
        hasher.update((picture.data.len() as u64).to_le_bytes());
        hasher.update(&picture.data);
    }
    hasher.finalize().into()
}

pub(crate) fn canonical_front_cover_fingerprint(cover: &CanonicalCover) -> [u8; 32] {
    let mut tag = id3::Tag::new();
    tag.add_frame(cover.picture.clone());
    front_cover_fingerprint(&tag)
}

fn response(
    status: http::StatusCode,
    content_type: &'static str,
    body: Vec<u8>,
) -> http::Response<Vec<u8>> {
    let cache_control = if status.is_success() {
        "public, max-age=31536000, immutable"
    } else {
        "no-store"
    };
    http::Response::builder()
        .status(status)
        .header(http::header::CONTENT_TYPE, content_type)
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(http::header::CACHE_CONTROL, cache_control)
        .body(body)
        .expect("valid cover response")
}

#[derive(Debug, PartialEq)]
enum CoverSource {
    Album(String),
    InboxTrack(String),
    Selected(String),
}

fn parse_request(request: &http::Request<Vec<u8>>) -> Result<(CoverSource, u32), ()> {
    let (kind, encoded, max_chars) = request
        .uri()
        .path()
        .strip_prefix("/album/")
        .map(|value| ("album", value, 512))
        .or_else(|| {
            request
                .uri()
                .path()
                .strip_prefix("/inbox/")
                .map(|value| ("inbox", value, 32_768))
        })
        .or_else(|| {
            request
                .uri()
                .path()
                .strip_prefix("/selected/")
                .map(|value| ("selected", value, 128))
        })
        .ok_or(())?;
    let identity = percent_decode_str(encoded)
        .decode_utf8()
        .map_err(|_| ())?
        .into_owned();
    if identity.trim().is_empty() || identity.chars().count() > max_chars {
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
        .unwrap_or(256);
    if !matches!(size, 64 | 128 | 256 | 512) {
        return Err(());
    }
    let source = match kind {
        "album" => CoverSource::Album(identity),
        "inbox" => CoverSource::InboxTrack(identity),
        _ => CoverSource::Selected(identity),
    };
    Ok((source, size))
}

fn source_fingerprint(
    album_id: &str,
    path: &Path,
    size: u32,
    max_source_bytes: Option<u64>,
) -> Result<String, String> {
    let metadata = fs::metadata(path).map_err(|_| "Album artwork is unavailable.".to_owned())?;
    if metadata.len() == 0 || max_source_bytes.is_some_and(|limit| metadata.len() > limit) {
        return Err("Album artwork is outside Aurora's safe size range.".to_owned());
    }
    let modified = metadata
        .modified()
        .unwrap_or(SystemTime::UNIX_EPOCH)
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut hasher = DefaultHasher::new();
    album_id.hash(&mut hasher);
    metadata.len().hash(&mut hasher);
    modified.hash(&mut hasher);
    size.hash(&mut hasher);
    Ok(format!("{:016x}-{size}.webp", hasher.finish()))
}

fn encode_thumbnail(source: &Path, size: u32) -> Result<Vec<u8>, String> {
    let image = ImageReader::open(source)
        .map_err(|_| "Aurora could not open this album cover.".to_owned())?
        .with_guessed_format()
        .map_err(|_| "Aurora could not identify this album cover.".to_owned())?
        .decode()
        .map_err(|_| "Aurora could not decode this album cover.".to_owned())?;
    let thumbnail = image.resize(size, size, FilterType::Lanczos3);
    let mut output = Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut output, ImageFormat::WebP)
        .map_err(|_| "Aurora could not encode this album-cover thumbnail.".to_owned())?;
    Ok(output.into_inner())
}

fn encode_embedded_thumbnail(source: &Path, size: u32) -> Result<Vec<u8>, String> {
    let (tag, _) = read_tag_for_write(source)?;
    let mut decoded = None;
    for prefer_front in [true, false] {
        for picture in tag
            .pictures()
            .filter(|picture| (picture.picture_type == PictureType::CoverFront) == prefer_front)
        {
            if picture.data.is_empty() || picture.data.len() as u64 > MAX_COVER_BYTES {
                continue;
            }
            if let Ok(image) = image::load_from_memory(&picture.data) {
                decoded = Some(image);
                break;
            }
        }
        if decoded.is_some() {
            break;
        }
    }
    let image = decoded.ok_or_else(|| "No usable embedded Inbox cover is available.".to_owned())?;
    let thumbnail = image.resize(size, size, FilterType::Lanczos3);
    let mut output = Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut output, ImageFormat::WebP)
        .map_err(|_| "Aurora could not encode this Inbox cover thumbnail.".to_owned())?;
    Ok(output.into_inner())
}

fn cache_thumbnail(cache_path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = cache_path
        .parent()
        .ok_or_else(|| "Aurora's cover cache has no parent directory.".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|_| "Aurora could not create its cover cache.".to_owned())?;
    let sequence = TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary: PathBuf =
        cache_path.with_extension(format!("{}-{sequence}.tmp", std::process::id()));
    fs::write(&temporary, bytes)
        .map_err(|_| "Aurora could not stage a cover thumbnail.".to_owned())?;
    match fs::rename(&temporary, cache_path) {
        Ok(()) => Ok(()),
        Err(_) if cache_path.is_file() => {
            let _ = fs::remove_file(&temporary);
            Ok(())
        }
        Err(_) => {
            let _ = fs::remove_file(&temporary);
            Err("Aurora could not finish caching a cover thumbnail.".to_owned())
        }
    }
}

fn load_thumbnail<R: Runtime>(
    app: &AppHandle<R>,
    album_id: &str,
    size: u32,
) -> Result<Vec<u8>, String> {
    let source = catalog::resolve_cover_path(album_id)?;
    let filename = source_fingerprint(album_id, &source, size, Some(MAX_COVER_BYTES))?;
    let cache_path = app
        .path()
        .app_cache_dir()
        .map_err(|_| "Aurora's cache directory is unavailable.".to_owned())?
        .join("covers")
        .join(filename);
    if let Ok(bytes) = fs::read(&cache_path) {
        return Ok(bytes);
    }
    let bytes = encode_thumbnail(&source, size)?;
    cache_thumbnail(&cache_path, &bytes)?;
    Ok(bytes)
}

fn load_inbox_thumbnail<R: Runtime>(
    app: &AppHandle<R>,
    track_path: &str,
    size: u32,
) -> Result<Vec<u8>, String> {
    let source = app
        .state::<InboxState>()
        .lock()
        .map_err(|_| "Aurora's Inbox stopped unexpectedly.".to_owned())?
        .resolve_cover_track(track_path)?;
    let filename = source_fingerprint(track_path, &source, size, None)?;
    let cache_path = app
        .path()
        .app_cache_dir()
        .map_err(|_| "Aurora's cache directory is unavailable.".to_owned())?
        .join("inbox-covers")
        .join(filename);
    if let Ok(bytes) = fs::read(&cache_path) {
        return Ok(bytes);
    }
    let bytes = encode_embedded_thumbnail(&source, size)?;
    cache_thumbnail(&cache_path, &bytes)?;
    Ok(bytes)
}

fn load_selected_thumbnail<R: Runtime>(
    app: &AppHandle<R>,
    token: &str,
    size: u32,
) -> Result<Vec<u8>, String> {
    let source = app
        .state::<ArtworkSelectionState>()
        .lock()
        .map_err(|_| "Aurora's album-cover picker stopped unexpectedly.".to_owned())?
        .resolve(token)?;
    let filename = source_fingerprint(token, &source, size, Some(MAX_COVER_BYTES))?;
    let cache_path = app
        .path()
        .app_cache_dir()
        .map_err(|_| "Aurora's cache directory is unavailable.".to_owned())?
        .join("selected-covers")
        .join(filename);
    if let Ok(bytes) = fs::read(&cache_path) {
        return Ok(bytes);
    }
    let bytes = encode_thumbnail(&source, size)?;
    cache_thumbnail(&cache_path, &bytes)?;
    Ok(bytes)
}

pub(crate) fn handle_cover_request<R: Runtime>(
    app: &AppHandle<R>,
    request: &http::Request<Vec<u8>>,
) -> http::Response<Vec<u8>> {
    let Ok((source, size)) = parse_request(request) else {
        return response(http::StatusCode::BAD_REQUEST, "text/plain", Vec::new());
    };
    let result = match source {
        CoverSource::Album(album_id) => load_thumbnail(app, &album_id, size),
        CoverSource::InboxTrack(track_path) => load_inbox_thumbnail(app, &track_path, size),
        CoverSource::Selected(token) => load_selected_thumbnail(app, &token, size),
    };
    match result {
        Ok(bytes) => response(http::StatusCode::OK, "image/webp", bytes),
        Err(_) => response(http::StatusCode::NOT_FOUND, "text/plain", Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use id3::{
        Tag, TagLike, Version,
        frame::{Picture, PictureType},
    };
    use std::{fs::File, io::Write};

    #[test]
    fn cover_protocol_accepts_only_bounded_thumbnail_sizes() {
        let good = http::Request::builder()
            .uri("http://aurora-cover.localhost/album/mb%3A42?size=256")
            .body(Vec::new())
            .expect("request");
        assert_eq!(
            parse_request(&good),
            Ok((CoverSource::Album("mb:42".to_owned()), 256))
        );

        let inbox = http::Request::builder()
            .uri("http://aurora-cover.localhost/inbox/C%3A%5CMusic%5C01.mp3?size=128")
            .body(Vec::new())
            .expect("request");
        assert_eq!(
            parse_request(&inbox),
            Ok((CoverSource::InboxTrack("C:\\Music\\01.mp3".to_owned()), 128))
        );

        let oversized = http::Request::builder()
            .uri("http://aurora-cover.localhost/album/mb%3A42?size=4096")
            .body(Vec::new())
            .expect("request");
        assert!(parse_request(&oversized).is_err());
    }

    #[test]
    fn embedded_inbox_cover_is_decoded_and_bounded() {
        let path = std::env::temp_dir().join(format!(
            "aurora-inbox-cover-{}-{}.mp3",
            std::process::id(),
            TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        File::create(&path)
            .expect("create track")
            .write_all(b"FAKE-MPEG-AUDIO")
            .expect("write track");

        let mut png = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(320, 180)
            .write_to(&mut png, ImageFormat::Png)
            .expect("encode picture");
        let mut tag = Tag::with_version(Version::Id3v24);
        tag.add_frame(Picture {
            mime_type: "image/png".to_owned(),
            picture_type: PictureType::CoverFront,
            description: String::new(),
            data: png.into_inner(),
        });
        tag.write_to_path(&path, Version::Id3v24)
            .expect("write embedded cover");

        let webp = encode_embedded_thumbnail(&path, 64).expect("thumbnail");
        let decoded = image::load_from_memory(&webp).expect("decode thumbnail");
        assert!(decoded.width() <= 64);
        assert!(decoded.height() <= 64);
        fs::remove_file(path).expect("remove fixture");
    }
}
