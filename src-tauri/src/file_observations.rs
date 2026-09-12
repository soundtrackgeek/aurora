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

// Drive the lookup from the small correction cache. Reordering this join can
// scan the entire million-track catalog even when there are no corrections.
pub(crate) const LOAD_SQL: &str = "INSERT INTO temp.aurora_verified_files
      SELECT t.id, o.missing, o.genre_known, o.genre FROM file_observations.observations o
      CROSS JOIN tracks t ON t.file_path = o.directory AND t.filename = o.filename AND t.import_run_id = o.import_run_id;";

pub(crate) fn cache_path(store: &StateStore) -> PathBuf {
    store
        .path()
        .with_file_name("aurora-file-observations.sqlite3")
}

const LOAD_ALBUM_SQL: &str = "INSERT INTO temp.aurora_verified_files
      SELECT t.id, o.missing, o.genre_known, o.genre FROM tracks t
      CROSS JOIN file_observations.observations o
        ON t.file_path = o.directory AND t.filename = o.filename AND t.import_run_id = o.import_run_id
      WHERE t.album_id = ?1;";

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
    load_scope(connection, store, album_id)?;
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
    load_scope(connection, store, album_id)
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

/// Only send saved, still-missing identities; unavailable directories remain unknown.
pub(crate) fn pending_deletion_paths(
    catalog: &Connection,
    store: &StateStore,
    directories: &[String],
) -> Result<Vec<String>, String> {
    let cache = open(store)?;
    let mut query = cache
        .prepare("SELECT filename FROM observations WHERE directory=?1 AND missing=1")
        .map_err(|e| e.to_string())?;
    let mut paths = Vec::new();
    for directory in directories {
        let Some(entries) = directory_entries(Path::new(directory)) else {
            continue;
        };
        let filenames = query
            .query_map([directory], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        for filename in filenames {
            let filename = filename.map_err(|e| e.to_string())?;
            let file = Path::new(&filename);
            if file.components().count() == 1
                && file
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("mp3"))
                && !entries.contains(&filename.to_lowercase())
            {
                let still_cataloged: bool = catalog
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM tracks WHERE file_path=?1 AND filename=?2)",
                        params![directory, filename],
                        |r| r.get(0),
                    )
                    .map_err(|e| e.to_string())?;
                if !still_cataloged {
                    continue;
                }
                paths.push(
                    Path::new(directory)
                        .join(file)
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    Ok(paths)
}

pub(crate) fn load(connection: &Connection, store: &StateStore) -> Result<(), String> {
    load_scope(connection, store, None)
}

pub(crate) fn load_album(
    connection: &Connection,
    store: &StateStore,
    album_id: &str,
) -> Result<(), String> {
    load_scope(connection, store, Some(album_id))
}

fn load_scope(
    connection: &Connection,
    store: &StateStore,
    album_id: Option<&str>,
) -> Result<(), String> {
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
    let result = connection
        .execute_batch(&format!(
            "{TEMP_SCHEMA}
      DELETE FROM temp.aurora_verified_files;"
        ))
        .and_then(|()| {
            if let Some(album_id) = album_id {
                connection.execute(LOAD_ALBUM_SQL, [album_id]).map(|_| ())
            } else {
                connection.execute_batch(LOAD_SQL)
            }
        });
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
    fn deletion_payload_uses_saved_absence_and_skips_restored_files() {
        let temp = tempfile::tempdir().unwrap();
        let store = StateStore::new(temp.path().join("state.sqlite3")).unwrap();
        let directory = temp.path().to_string_lossy().into_owned();
        let catalog = Connection::open_in_memory().unwrap();
        catalog
            .execute_batch("CREATE TABLE tracks(file_path TEXT, filename TEXT)")
            .unwrap();
        catalog
            .execute("INSERT INTO tracks VALUES (?1,'bonus.mp3')", [&directory])
            .unwrap();
        open(&store)
            .unwrap()
            .execute(
                "INSERT INTO observations VALUES (?1,'bonus.mp3',1,1,0,NULL)",
                [&directory],
            )
            .unwrap();
        assert_eq!(
            pending_deletion_paths(&catalog, &store, std::slice::from_ref(&directory))
                .unwrap()
                .len(),
            1
        );
        fs::write(temp.path().join("bonus.mp3"), []).unwrap();
        assert!(
            pending_deletion_paths(&catalog, &store, &[directory])
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn album_corrections_are_bounded_to_selected_tracks() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE tracks(id INTEGER PRIMARY KEY, album_id TEXT, file_path TEXT, filename TEXT, import_run_id INTEGER);
            CREATE INDEX idx_tracks_album_id ON tracks(album_id);
            WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<10000)
            INSERT INTO tracks SELECT x, CAST((x-1)/10 AS TEXT), 'folder', CAST(x AS TEXT), 2 FROM n;
            ATTACH DATABASE ':memory:' AS file_observations;
            CREATE TABLE file_observations.observations(directory TEXT, filename TEXT, import_run_id INTEGER,
                missing INTEGER, genre_known INTEGER, genre TEXT, PRIMARY KEY(directory, filename));
            INSERT INTO file_observations.observations SELECT file_path, filename, import_run_id, 0, 1, 'Soundtrack' FROM tracks;").unwrap();
        db.execute_batch(TEMP_SCHEMA).unwrap();
        let mut statement = db.prepare(LOAD_ALBUM_SQL).unwrap();
        assert_eq!(statement.execute(["5"]).unwrap(), 10);
        let steps = statement.get_status(rusqlite::StatementStatus::VmStep);
        assert!(steps < 1_000, "album corrections used {steps} VM steps");
        let bounds: (i64, i64) = db
            .query_row(
                "SELECT MIN(track_id), MAX(track_id) FROM temp.aurora_verified_files",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(bounds, (51, 60));
    }

    #[test]
    fn saved_corrections_use_indexed_identities_and_reject_stale_revisions() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE tracks(id INTEGER PRIMARY KEY, file_path TEXT, filename TEXT, import_run_id INTEGER);
            CREATE INDEX idx_tracks_file ON tracks(file_path, filename);
            WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<10000)
            INSERT INTO tracks SELECT x, 'album', CAST(x AS TEXT), 2 FROM n;
            ATTACH DATABASE ':memory:' AS file_observations;
            CREATE TABLE file_observations.observations(directory TEXT, filename TEXT, import_run_id INTEGER,
                missing INTEGER, genre_known INTEGER, genre TEXT, PRIMARY KEY(directory, filename));
            INSERT INTO file_observations.observations VALUES
                ('album', '10', 2, 0, 1, 'Soundtrack'),
                ('album', '20', 1, 1, 0, NULL),
                ('other', '30', 2, 1, 0, NULL);").unwrap();
        db.execute_batch(TEMP_SCHEMA).unwrap();
        for expected in [1, 0] {
            let mut statement = db.prepare(LOAD_SQL).unwrap();
            assert_eq!(statement.execute([]).unwrap(), expected);
            let steps = statement.get_status(rusqlite::StatementStatus::VmStep);
            assert!(steps < 1_000, "sparse corrections used {steps} VM steps");
            if expected == 1 {
                let correction: (i64, String) = db
                    .query_row(
                        "SELECT track_id, genre FROM temp.aurora_verified_files",
                        [],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .unwrap();
                assert_eq!(correction, (10, "Soundtrack".into()));
            }
            db.execute_batch("DELETE FROM temp.aurora_verified_files; DELETE FROM file_observations.observations;").unwrap();
        }
    }

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
