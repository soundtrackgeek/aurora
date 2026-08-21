use crate::catalog;
use image::{ImageFormat, ImageReader, imageops::FilterType};
use percent_encoding::percent_decode_str;
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    io::Cursor,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager, Runtime, http};

const MAX_COVER_BYTES: u64 = 32 * 1024 * 1024;
static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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

fn parse_request(request: &http::Request<Vec<u8>>) -> Result<(String, u32), ()> {
    let encoded = request.uri().path().strip_prefix("/album/").ok_or(())?;
    let album_id = percent_decode_str(encoded)
        .decode_utf8()
        .map_err(|_| ())?
        .into_owned();
    if album_id.trim().is_empty() || album_id.chars().count() > 512 {
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
    Ok((album_id, size))
}

fn source_fingerprint(album_id: &str, path: &Path, size: u32) -> Result<String, String> {
    let metadata = fs::metadata(path).map_err(|_| "Album artwork is unavailable.".to_owned())?;
    if metadata.len() == 0 || metadata.len() > MAX_COVER_BYTES {
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
    let filename = source_fingerprint(album_id, &source, size)?;
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

pub(crate) fn handle_cover_request<R: Runtime>(
    app: &AppHandle<R>,
    request: &http::Request<Vec<u8>>,
) -> http::Response<Vec<u8>> {
    let Ok((album_id, size)) = parse_request(request) else {
        return response(http::StatusCode::BAD_REQUEST, "text/plain", Vec::new());
    };
    match load_thumbnail(app, &album_id, size) {
        Ok(bytes) => response(http::StatusCode::OK, "image/webp", bytes),
        Err(_) => response(http::StatusCode::NOT_FOUND, "text/plain", Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_protocol_accepts_only_bounded_thumbnail_sizes() {
        let good = http::Request::builder()
            .uri("http://aurora-cover.localhost/album/mb%3A42?size=256")
            .body(Vec::new())
            .expect("request");
        assert_eq!(parse_request(&good), Ok(("mb:42".to_owned(), 256)));

        let oversized = http::Request::builder()
            .uri("http://aurora-cover.localhost/album/mb%3A42?size=4096")
            .body(Vec::new())
            .expect("request");
        assert!(parse_request(&oversized).is_err());
    }
}
