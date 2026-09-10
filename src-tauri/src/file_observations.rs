//! Device-local, durable observations of pending music files. A failed share read
//! is unknown, never evidence of deletion. Catalog/state snapshots remain untouched.
use crate::{catalog, device_mode, live_genres, state_store::StateStore};
use rusqlite::{Connection, params};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

pub(crate) const TEMP_SCHEMA: &str = "CREATE TEMP TABLE IF NOT EXISTS aurora_verified_files (track_id INTEGER PRIMARY KEY, missing INTEGER NOT NULL, genre_known INTEGER NOT NULL, genre TEXT);";

pub(crate) fn cache_path(store: &StateStore) -> PathBuf {
    store
        .path()
        .with_file_name("aurora-file-observations.sqlite3")
}

fn open(store: &StateStore) -> Result<Connection, String> {
    let connection = Connection::open(cache_path(store)).map_err(|e| e.to_string())?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS observations (
      directory TEXT NOT NULL, filename TEXT NOT NULL, import_run_id INTEGER NOT NULL,
      missing INTEGER NOT NULL, genre_known INTEGER NOT NULL, genre TEXT,
      PRIMARY KEY(directory, filename)
    );",
        )
        .map_err(|e| e.to_string())?;
    Ok(connection)
}

fn directory_entries(path: &Path) -> Option<HashSet<String>> {
    // Collect the complete listing: an interrupted/failed enumeration must not
    // turn the unseen tail of a directory into deletions.
    fs::read_dir(path)
        .ok()?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().to_lowercase()))
        .collect::<Result<HashSet<_>, _>>()
        .ok()
}

pub(crate) fn confirmed_missing(directory: &str, filename: &str) -> bool {
    let path = device_mode::resolve_device_path(Path::new(directory));
    directory_entries(&path).is_some_and(|entries| !entries.contains(&filename.to_lowercase()))
}

pub(crate) fn refresh(
    connection: &Connection,
    store: &StateStore,
    album_id: Option<&str>,
) -> Result<(), String> {
    load(connection, store)?;
    let mut statement = connection
        .prepare(
            "WITH candidates(id) AS (
      SELECT t.id FROM aurora_state.pending_library_folder_sync p CROSS JOIN tracks t
        ON t.file_path=p.directory AND (p.filename IS NULL OR p.filename=t.filename)
      UNION SELECT track_id FROM temp.aurora_verified_files
    ) SELECT t.file_path, t.filename, t.import_run_id FROM candidates c JOIN tracks t ON t.id=c.id
      WHERE (?1 IS NULL OR t.album_id=?1)",
        )
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([album_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut directories = HashMap::new();
    let mut observations = Vec::new();
    // No database write lock is held while waiting for network files.
    for (directory, filename, revision) in rows {
        let file = Path::new(&filename);
        if file.components().count() != 1
            || !file
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("mp3"))
        {
            continue;
        }
        let entries = directories.entry(directory.clone()).or_insert_with(|| {
            directory_entries(&device_mode::resolve_device_path(Path::new(&directory)))
        });
        let Some(entries) = entries else {
            continue;
        };
        let missing = !entries.contains(&filename.to_lowercase());
        let genre = if missing {
            None
        } else {
            live_genres::read_genre(&directory, &filename)
        };
        observations.push((directory, filename, revision, missing, genre));
    }
    let mut cache = open(store)?;
    let transaction = cache.transaction().map_err(|e| e.to_string())?;
    for (directory, filename, revision, missing, genre) in observations {
        transaction.execute("INSERT INTO observations VALUES (?1, ?2, ?3, ?4, ?5, ?6)
          ON CONFLICT(directory, filename) DO UPDATE SET
          import_run_id = excluded.import_run_id, missing = excluded.missing,
          genre_known = CASE WHEN excluded.genre_known = 1 OR observations.import_run_id != excluded.import_run_id THEN excluded.genre_known ELSE observations.genre_known END,
          genre = CASE WHEN excluded.genre_known = 1 OR observations.import_run_id != excluded.import_run_id THEN excluded.genre ELSE observations.genre END",
          params![directory, filename, revision, missing, genre.is_some(), genre.flatten()]).map_err(|e| e.to_string())?;
    }
    transaction.commit().map_err(|e| e.to_string())?;
    load(connection, store)
}

pub(crate) fn record_deleted(
    store: &StateStore,
    track: &catalog::TrackSummary,
) -> Result<(), String> {
    open(store)?.execute("INSERT INTO observations VALUES (?1, ?2, ?3, 1, 0, NULL)
      ON CONFLICT(directory, filename) DO UPDATE SET import_run_id=excluded.import_run_id, missing=1",
      params![track.directory, track.filename, track.catalog_import_run_id]).map_err(|e| format!("Could not save the verified deletion: {e}"))?;
    Ok(())
}

pub(crate) fn load(connection: &Connection, store: &StateStore) -> Result<(), String> {
    open(store)?;
    connection
        .execute(
            "ATTACH DATABASE ?1 AS file_observations",
            [cache_path(store).to_string_lossy().as_ref()],
        )
        .map_err(|e| e.to_string())?;
    let query_only: bool = connection
        .pragma_query_value(None, "query_only", |r| r.get(0))
        .map_err(|e| e.to_string())?;
    connection
        .pragma_update(None, "query_only", false)
        .map_err(|e| e.to_string())?;
    let result = connection.execute_batch(&format!("{TEMP_SCHEMA}
      DELETE FROM temp.aurora_verified_files;
      INSERT INTO temp.aurora_verified_files
      SELECT t.id, o.missing, o.genre_known, o.genre FROM file_observations.observations o
      JOIN tracks t ON t.file_path = o.directory AND t.filename = o.filename AND t.import_run_id = o.import_run_id;"));
    connection
        .pragma_update(None, "query_only", query_only)
        .map_err(|e| e.to_string())?;
    connection
        .execute_batch("DETACH DATABASE file_observations")
        .map_err(|e| e.to_string())?;
    result.map_err(|e| format!("Could not load verified file corrections: {e}"))
}

pub(crate) fn deleted_keys(
    connection: &Connection,
    album_id: &str,
) -> Result<HashSet<String>, String> {
    let mut statement = connection.prepare("SELECT t.file_path, t.filename FROM tracks t JOIN temp.aurora_verified_files o ON o.track_id=t.id WHERE t.album_id=?1 AND o.missing=1").map_err(|e| e.to_string())?;
    statement
        .query_map([album_id], |r| {
            Ok(catalog::normalize_track_key(
                &r.get::<_, String>(0)?,
                &r.get::<_, String>(1)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<HashSet<_>, _>>()
        .map_err(|e| e.to_string())
}

pub(crate) fn apply_genres(
    connection: &Connection,
    tracks: &mut [catalog::TrackSummary],
) -> Result<(), String> {
    let mut statement = connection.prepare("SELECT genre FROM temp.aurora_verified_files WHERE track_id=?1 AND missing=0 AND genre_known=1").map_err(|e| e.to_string())?;
    for track in tracks {
        use rusqlite::OptionalExtension;
        if let Some(genre) = statement
            .query_row([&track.id], |r| r.get::<_, Option<String>>(0))
            .optional()
            .map_err(|e| e.to_string())?
            && track.genre != genre
        {
            track.genre = genre;
            track.tag_sync_state = Some(crate::tag_model::TagSyncState::PendingImport);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_directory_is_unknown_and_case_differences_are_not_deletions() {
        let directory = tempfile::tempdir().unwrap();
        let folder = directory.path().to_str().unwrap();
        assert!(confirmed_missing(folder, "bonus.mp3"));
        fs::write(directory.path().join("BONUS.MP3"), []).unwrap();
        assert!(!confirmed_missing(folder, "bonus.mp3"));
        assert!(!confirmed_missing(
            directory.path().join("offline").to_str().unwrap(),
            "bonus.mp3"
        ));
    }
}
