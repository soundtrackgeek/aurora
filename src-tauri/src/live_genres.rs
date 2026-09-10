//! Pending file genres are authoritative until Music Library catches up.
use crate::{catalog::TrackSummary, device_mode, state_store::StateStore};
use id3::TagLike;
use rusqlite::{Connection, params};
use std::{
    collections::HashMap,
    fs::File,
    io::{Cursor, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::SystemTime,
};

type GenreCache = HashMap<PathBuf, (u64, SystemTime, Option<String>)>;
static CACHE: OnceLock<Mutex<GenreCache>> = OnceLock::new();

fn read_genre(directory: &str, filename: &str) -> Option<Option<String>> {
    if Path::new(filename).components().count() != 1 {
        return None;
    }
    let path = device_mode::resolve_device_path(Path::new(directory)).join(filename);
    let metadata = path.metadata().ok()?;
    let modified = metadata.modified().ok()?;
    let mut cache = CACHE.get_or_init(Default::default).lock().ok()?;
    if let Some((size, timestamp, genre)) = cache.get(&path)
        && *size == metadata.len()
        && *timestamp == modified
    {
        return Some(genre.clone());
    }
    let genre = read_genre_frame(&path).or_else(|| {
        id3::Tag::read_from_path(&path)
            .ok()
            .map(|tag| normalized_genre(&tag))
    })?;
    // Do not cache a read that raced an external replacement.
    let after = path.metadata().ok()?;
    if after.len() != metadata.len() || after.modified().ok()? != modified {
        return None;
    }
    if cache.len() >= 20_000 {
        cache.clear();
    }
    cache.insert(path, (metadata.len(), modified, genre.clone()));
    Some(genre)
}

fn normalized_genre(tag: &id3::Tag) -> Option<String> {
    tag.genre()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn syncsafe(bytes: &[u8]) -> Option<u32> {
    bytes.iter().try_fold(0, |value, byte| {
        (*byte < 128).then_some((value << 7) | u32::from(*byte))
    })
}

// Skip artwork by seeking, then let the existing ID3 decoder interpret TCON.
// Unusual tag headers use the full decoder instead of guessing their layout.
fn read_genre_frame(path: &Path) -> Option<Option<String>> {
    let mut file = File::open(path).ok()?;
    let mut header = [0; 10];
    file.read_exact(&mut header).ok()?;
    if &header[..3] != b"ID3" || !matches!(header[3], 3 | 4) || header[5] != 0 {
        return None;
    }
    let end = 10 + u64::from(syncsafe(&header[6..])?);
    if end > file.metadata().ok()?.len() {
        return None;
    }
    let mut position = 10;
    while position + 10 <= end {
        let mut frame = [0; 10];
        file.read_exact(&mut frame).ok()?;
        if frame[..4] == [0; 4] {
            return Some(None);
        }
        if !frame[..4]
            .iter()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        {
            return None;
        }
        let size = if header[3] == 4 {
            syncsafe(&frame[4..8])?
        } else {
            u32::from_be_bytes(frame[4..8].try_into().ok()?)
        };
        position += 10 + u64::from(size);
        if size == 0 || position > end {
            return None;
        }
        if &frame[..4] == b"TCON" {
            // Bound allocations even for malformed files; larger frames use the full decoder.
            if size > 1_048_576 {
                return None;
            }
            let tag_size = size + 10;
            for (index, byte) in header[6..].iter_mut().enumerate() {
                *byte = ((tag_size >> (7 * (3 - index))) & 127) as u8;
            }
            let mut bytes = Vec::with_capacity(size as usize + 20);
            bytes.extend_from_slice(&header);
            bytes.extend_from_slice(&frame);
            bytes.resize(size as usize + 20, 0);
            file.read_exact(&mut bytes[20..]).ok()?;
            return id3::Tag::read_from2(Cursor::new(bytes))
                .ok()
                .map(|tag| normalized_genre(&tag));
        }
        file.seek(SeekFrom::Start(position)).ok()?;
    }
    Some(None)
}

pub(crate) const SCHEMA: &str = r#"
    CREATE TEMP TABLE IF NOT EXISTS aurora_live_track_genres (
      track_id INTEGER PRIMARY KEY, album_id TEXT, genre TEXT
    );
    CREATE TEMP TABLE IF NOT EXISTS aurora_live_album_genres (
      album_id TEXT PRIMARY KEY, genre TEXT
    );
    CREATE VIRTUAL TABLE IF NOT EXISTS temp.aurora_live_album_genre_fts USING fts5(
      album_id UNINDEXED, album, album_artist_display, canonical_genre, publisher,
      tokenize = 'unicode61 remove_diacritics 2'
    );
    CREATE VIRTUAL TABLE IF NOT EXISTS temp.aurora_live_genre_fts USING fts5(
      track_id UNINDEXED, album_id UNINDEXED, title, display_artist, album,
      album_artist_display, canonical_genre, publisher, file_path, filename,
      tokenize = 'unicode61 remove_diacritics 2'
    );
"#;

pub(crate) fn prepare(connection: &Connection) -> Result<(), String> {
    prepare_scope(connection, None)
}

pub(crate) fn prepare_album(connection: &Connection, album_id: &str) -> Result<(), String> {
    prepare_scope(connection, Some(album_id))
}

fn prepare_scope(connection: &Connection, album_id: Option<&str>) -> Result<(), String> {
    let query_only: bool = connection
        .pragma_query_value(None, "query_only", |r| r.get(0))
        .map_err(|e| e.to_string())?;
    connection
        .pragma_update(None, "query_only", false)
        .map_err(|e| e.to_string())?;
    let result = (|| -> rusqlite::Result<()> {
        connection.execute_batch(SCHEMA)?;
        connection.execute_batch("DELETE FROM temp.aurora_live_track_genres; DELETE FROM temp.aurora_live_album_genres; DELETE FROM temp.aurora_live_genre_fts; DELETE FROM temp.aurora_live_album_genre_fts;")?;
        let mut statement = connection.prepare("SELECT t.id, t.album_id, t.file_path, t.filename, t.canonical_genre FROM aurora_state.pending_library_folder_sync p CROSS JOIN tracks t ON t.file_path = p.directory AND (p.filename IS NULL OR p.filename = t.filename) WHERE (?1 IS NULL OR t.album_id = ?1)")?;
        let rows = statement.query_map([album_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })?;
        for row in rows {
            let (id, album, directory, filename, catalog_genre) = row?;
            if let Some(genre) = read_genre(&directory, &filename)
                && genre != catalog_genre
            {
                connection.execute(
                    "INSERT INTO temp.aurora_live_track_genres VALUES (?1, ?2, ?3)",
                    params![id, album, genre],
                )?;
            }
        }
        connection.execute_batch(r#"
          INSERT INTO temp.aurora_live_genre_fts
          SELECT t.id, t.album_id, t.title, t.display_artist, t.album, t.album_artist_display,
                 live.genre, t.publisher, t.file_path, t.filename
          FROM temp.aurora_live_track_genres live JOIN tracks t ON t.id = live.track_id;
          INSERT INTO temp.aurora_live_album_genres
          SELECT t.album_id, MIN(CASE WHEN live.track_id IS NOT NULL THEN live.genre ELSE t.canonical_genre END)
          FROM tracks t LEFT JOIN temp.aurora_live_track_genres live ON live.track_id = t.id
          WHERE t.album_id IN (SELECT album_id FROM temp.aurora_live_track_genres)
          GROUP BY t.album_id
          HAVING COUNT(DISTINCT COALESCE(CASE WHEN live.track_id IS NOT NULL THEN live.genre ELSE t.canonical_genre END, '')) = 1;
          INSERT INTO temp.aurora_live_album_genre_fts
          SELECT a.id, a.album, a.album_artist_display, live.genre, a.publisher
          FROM temp.aurora_live_album_genres live JOIN albums a ON a.id = live.album_id;
        "#)?;
        Ok(())
    })();
    connection
        .pragma_update(None, "query_only", query_only)
        .map_err(|e| e.to_string())?;
    result.map_err(|e| format!("Could not project pending file genres: {e}"))
}

pub(crate) fn apply(tracks: &mut [TrackSummary], store: &StateStore) -> Result<(), String> {
    let connection = store.open()?;
    let mut statement = connection.prepare("SELECT EXISTS(SELECT 1 FROM pending_library_folder_sync WHERE directory = ?1 AND (filename IS NULL OR filename = ?2))").map_err(|e| e.to_string())?;
    for track in tracks {
        let pending: bool = statement
            .query_row(params![track.directory, track.filename], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if pending
            && let Some(genre) = read_genre(&track.directory, &track.filename)
            && track.genre != genre
        {
            track.tag_sync_state = Some(crate::tag_model::TagSyncState::PendingImport);
            track.genre = genre;
        }
    }
    Ok(())
}

pub(crate) fn track_sql(alias: &str) -> String {
    format!(
        "CASE WHEN {alias}.id IN (SELECT track_id FROM temp.aurora_live_track_genres) THEN (SELECT genre FROM temp.aurora_live_track_genres WHERE track_id = {alias}.id) ELSE {alias}.canonical_genre END"
    )
}

pub(crate) fn album_sql(alias: &str) -> String {
    format!(
        "CASE WHEN {alias}.id IN (SELECT album_id FROM temp.aurora_live_album_genres) THEN (SELECT genre FROM temp.aurora_live_album_genres WHERE album_id = {alias}.id) ELSE {alias}.canonical_genre END"
    )
}

pub(crate) fn has_changes(connection: &Connection) -> bool {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM temp.aurora_live_track_genres)",
            [],
            |r| r.get(0),
        )
        .unwrap_or(false)
}

pub(crate) fn fts_matches(column: &str) -> String {
    format!(
        "SELECT {column} FROM track_search_fts WHERE track_search_fts MATCH ? AND CAST(track_id AS INTEGER) NOT IN (SELECT track_id FROM temp.aurora_live_track_genres) UNION ALL SELECT {column} FROM temp.aurora_live_genre_fts WHERE aurora_live_genre_fts MATCH ?"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genre_reader_matches_id3_with_large_artwork_and_empty_genres() {
        let directory = tempfile::tempdir().unwrap();
        for version in [
            id3::Version::Id3v22,
            id3::Version::Id3v23,
            id3::Version::Id3v24,
        ] {
            let path = directory.path().join(format!("{version:?}.mp3"));
            for genre in [
                Some("  Soundtrack  "),
                Some("Électronique"),
                Some("(12)"),
                None,
            ] {
                let mut tag = id3::Tag::new();
                tag.add_frame(id3::frame::Picture {
                    mime_type: "image/jpeg".into(),
                    picture_type: id3::frame::PictureType::CoverFront,
                    description: String::new(),
                    data: vec![7; 2_000_000],
                });
                if let Some(genre) = genre {
                    tag.set_genre(genre);
                }
                std::fs::write(&path, []).unwrap();
                tag.write_to_path(&path, version).unwrap();
                let expected = normalized_genre(&id3::Tag::read_from_path(&path).unwrap());
                assert_eq!(
                    read_genre(
                        directory.path().to_str().unwrap(),
                        path.file_name().unwrap().to_str().unwrap()
                    ),
                    Some(expected)
                );
                if version != id3::Version::Id3v22 {
                    assert_eq!(read_genre_frame(&path), Some(normalized_genre(&tag)));
                }
            }
        }
    }
}
