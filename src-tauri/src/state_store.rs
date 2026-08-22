use crate::{
    device_mode,
    tag_model::{LoveState, TagValues},
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

pub(crate) const SCHEMA_VERSION: i64 = 6;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StoredQueueEntry {
    pub(crate) track_id: String,
    pub(crate) track_key: Option<String>,
    pub(crate) directory: Option<String>,
    pub(crate) filename: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StoredPlaybackState {
    pub(crate) queue: Vec<StoredQueueEntry>,
    pub(crate) current_index: Option<usize>,
    pub(crate) position_seconds: f64,
    pub(crate) volume: f32,
    pub(crate) shuffle: bool,
    pub(crate) repeat_mode: String,
}

impl Default for StoredPlaybackState {
    fn default() -> Self {
        Self {
            queue: Vec::new(),
            current_index: None,
            position_seconds: 0.0,
            volume: 0.7,
            shuffle: false,
            repeat_mode: "off".to_owned(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TagOverlay {
    pub(crate) track_key: String,
    pub(crate) directory: String,
    pub(crate) filename: String,
    pub(crate) values: TagValues,
    pub(crate) catalog_values: TagValues,
    pub(crate) catalog_import_run_id: i64,
    pub(crate) last_operation_id: Option<i64>,
}

#[derive(Clone, Debug)]
pub(crate) struct TagOperation {
    pub(crate) id: i64,
    pub(crate) track_key: String,
    pub(crate) target_path: PathBuf,
    pub(crate) temp_path: Option<PathBuf>,
    pub(crate) backup_path: Option<PathBuf>,
    pub(crate) before: TagValues,
    pub(crate) after: TagValues,
    pub(crate) source_fingerprint: String,
    pub(crate) status: String,
}

#[derive(Clone)]
pub(crate) struct StateStore {
    path: PathBuf,
}

impl StateStore {
    pub(crate) fn new(path: PathBuf) -> Result<Self, String> {
        let parent = path
            .parent()
            .ok_or_else(|| "Aurora's state path has no parent directory.".to_owned())?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create Aurora's state directory: {error}"))?;
        let store = Self { path };
        store.migrate()?;
        Ok(store)
    }

    pub(crate) fn open(&self) -> Result<Connection, String> {
        let connection = Connection::open(&self.path)
            .map_err(|error| format!("Could not open Aurora's state database: {error}"))?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|error| format!("Could not configure Aurora's state database: {error}"))?;
        connection
            .pragma_update(None, "foreign_keys", true)
            .map_err(|error| format!("Could not enable state integrity checks: {error}"))?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|error| format!("Could not enable durable state journaling: {error}"))?;
        Ok(connection)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    fn migrate(&self) -> Result<(), String> {
        let mut connection = self.open()?;
        let current: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|error| format!("Could not read Aurora's state schema: {error}"))?;
        if current > SCHEMA_VERSION {
            return Err(format!(
                "Aurora's state database uses unsupported schema version {current}."
            ));
        }

        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not start Aurora's state migration: {error}"))?;
        transaction
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS playback_queue (
                  position INTEGER PRIMARY KEY,
                  track_id TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS playback_state (
                  singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                  current_index INTEGER,
                  position_seconds REAL NOT NULL DEFAULT 0,
                  volume REAL NOT NULL DEFAULT 0.7 CHECK (volume >= 0 AND volume <= 1),
                  shuffle INTEGER NOT NULL DEFAULT 0 CHECK (shuffle IN (0, 1)),
                  repeat_mode TEXT NOT NULL DEFAULT 'off' CHECK (repeat_mode IN ('off', 'all', 'one'))
                );
                INSERT OR IGNORE INTO playback_state(singleton) VALUES (1);
                "#,
            )
            .map_err(|error| format!("Could not ensure Aurora's playback schema: {error}"))?;

        let queue_columns = {
            let mut statement = transaction
                .prepare("PRAGMA table_info(playback_queue)")
                .map_err(|error| format!("Could not inspect Aurora's queue schema: {error}"))?;
            statement
                .query_map([], |row| row.get::<_, String>(1))
                .map_err(|error| format!("Could not inspect Aurora's queue columns: {error}"))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("Could not decode Aurora's queue columns: {error}"))?
        };
        if !queue_columns.iter().any(|column| column == "track_key") {
            transaction
                .execute("ALTER TABLE playback_queue ADD COLUMN track_key TEXT", [])
                .map_err(|error| {
                    format!("Could not migrate Aurora's durable queue identity: {error}")
                })?;
        }
        if !queue_columns.iter().any(|column| column == "directory") {
            transaction
                .execute("ALTER TABLE playback_queue ADD COLUMN directory TEXT", [])
                .map_err(|error| format!("Could not migrate Aurora's queue directory: {error}"))?;
        }
        if !queue_columns.iter().any(|column| column == "filename") {
            transaction
                .execute("ALTER TABLE playback_queue ADD COLUMN filename TEXT", [])
                .map_err(|error| format!("Could not migrate Aurora's queue filename: {error}"))?;
        }

        transaction
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS tag_overlays (
                  track_key TEXT PRIMARY KEY,
                  directory TEXT NOT NULL,
                  filename TEXT NOT NULL,
                  rating REAL,
                  love_state TEXT NOT NULL CHECK (love_state IN ('neutral', 'loved', 'banned')),
                  release_year INTEGER,
                  catalog_rating REAL,
                  catalog_love_state TEXT NOT NULL CHECK (catalog_love_state IN ('neutral', 'loved', 'banned')),
                  catalog_release_year INTEGER,
                  catalog_import_run_id INTEGER NOT NULL,
                  last_operation_id INTEGER,
                  updated_at_ms INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_tag_overlays_catalog_identity
                  ON tag_overlays(directory, filename);

                CREATE TABLE IF NOT EXISTS tag_edit_operations (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  track_key TEXT NOT NULL,
                  target_path TEXT NOT NULL,
                  temp_path TEXT,
                  backup_path TEXT,
                  before_rating REAL,
                  before_love_state TEXT NOT NULL CHECK (before_love_state IN ('neutral', 'loved', 'banned')),
                  before_release_year INTEGER,
                  after_rating REAL,
                  after_love_state TEXT NOT NULL CHECK (after_love_state IN ('neutral', 'loved', 'banned')),
                  after_release_year INTEGER,
                  source_fingerprint TEXT NOT NULL,
                  status TEXT NOT NULL CHECK (status IN ('prepared', 'replaced', 'verified', 'undoing', 'failed', 'rolledBack')),
                  created_at_ms INTEGER NOT NULL,
                  updated_at_ms INTEGER NOT NULL,
                  error_message TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_tag_edit_operations_track
                  ON tag_edit_operations(track_key, id DESC);
                CREATE INDEX IF NOT EXISTS idx_tag_edit_operations_status
                  ON tag_edit_operations(status, id);
                "#,
            )
            .map_err(|error| format!("Could not ensure Aurora's tag journal schema: {error}"))?;
        if current < 4 {
            let journal_sql: String = transaction
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'tag_edit_operations'",
                    [],
                    |row| row.get(0),
                )
                .map_err(|error| format!("Could not inspect Aurora's tag journal: {error}"))?;
            if !journal_sql.contains("'undoing'") {
                transaction
                    .execute_batch(
                        r#"
                        ALTER TABLE tag_edit_operations RENAME TO tag_edit_operations_v3;
                        CREATE TABLE tag_edit_operations (
                          id INTEGER PRIMARY KEY AUTOINCREMENT,
                          track_key TEXT NOT NULL,
                          target_path TEXT NOT NULL,
                          temp_path TEXT,
                          backup_path TEXT,
                          before_rating REAL,
                          before_love_state TEXT NOT NULL CHECK (before_love_state IN ('neutral', 'loved', 'banned')),
                          before_release_year INTEGER,
                          after_rating REAL,
                          after_love_state TEXT NOT NULL CHECK (after_love_state IN ('neutral', 'loved', 'banned')),
                          after_release_year INTEGER,
                          source_fingerprint TEXT NOT NULL,
                          status TEXT NOT NULL CHECK (status IN ('prepared', 'replaced', 'verified', 'undoing', 'failed', 'rolledBack')),
                          created_at_ms INTEGER NOT NULL,
                          updated_at_ms INTEGER NOT NULL,
                          error_message TEXT
                        );
                        INSERT INTO tag_edit_operations SELECT * FROM tag_edit_operations_v3;
                        DROP TABLE tag_edit_operations_v3;
                        CREATE INDEX idx_tag_edit_operations_track
                          ON tag_edit_operations(track_key, id DESC);
                        CREATE INDEX idx_tag_edit_operations_status
                          ON tag_edit_operations(status, id);
                        "#,
                    )
                    .map_err(|error| format!("Could not upgrade Aurora's undo journal: {error}"))?;
            }
        }
        transaction
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS musicbrainz_artist_decisions (
                  local_artist_key TEXT PRIMARY KEY,
                  display_artist TEXT NOT NULL,
                  decision TEXT NOT NULL CHECK (decision IN ('confirmed', 'ignored')),
                  artist_mbid TEXT,
                  canonical_name TEXT,
                  created_at_ms INTEGER NOT NULL,
                  updated_at_ms INTEGER NOT NULL,
                  CHECK (
                    (decision = 'confirmed' AND artist_mbid IS NOT NULL AND length(trim(artist_mbid)) > 0)
                    OR
                    (decision = 'ignored' AND artist_mbid IS NULL)
                  )
                );

                CREATE TABLE IF NOT EXISTS musicbrainz_release_decisions (
                  local_artist_key TEXT NOT NULL,
                  display_artist TEXT NOT NULL,
                  artist_mbid TEXT NOT NULL,
                  release_mbid TEXT NOT NULL,
                  decision TEXT NOT NULL CHECK (decision IN ('linked', 'not-in-scope', 'ignored')),
                  local_album_id TEXT,
                  created_at_ms INTEGER NOT NULL,
                  updated_at_ms INTEGER NOT NULL,
                  PRIMARY KEY (local_artist_key, artist_mbid, release_mbid),
                  CHECK (
                    (decision = 'linked' AND local_album_id IS NOT NULL AND length(trim(local_album_id)) > 0)
                    OR
                    (decision IN ('not-in-scope', 'ignored') AND local_album_id IS NULL)
                  )
                );
                CREATE UNIQUE INDEX IF NOT EXISTS idx_musicbrainz_release_local_album
                  ON musicbrainz_release_decisions(local_artist_key, local_album_id)
                  WHERE decision = 'linked';

                CREATE TABLE IF NOT EXISTS musicbrainz_curation_events (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  entity_kind TEXT NOT NULL CHECK (entity_kind IN ('artist', 'release')),
                  local_artist_key TEXT NOT NULL,
                  artist_mbid TEXT,
                  release_mbid TEXT,
                  before_json TEXT,
                  after_json TEXT,
                  created_at_ms INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_musicbrainz_curation_events_created
                  ON musicbrainz_curation_events(id DESC);
                "#,
            )
            .map_err(|error| format!("Could not ensure Aurora's MusicBrainz curation schema: {error}"))?;
        transaction
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS state_sync_meta (
                  singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                  lineage_id TEXT NOT NULL,
                  snapshot_id TEXT NOT NULL,
                  generation INTEGER NOT NULL DEFAULT 0 CHECK (generation >= 0),
                  content_revision INTEGER NOT NULL DEFAULT 0 CHECK (content_revision >= 0),
                  mirrored_revision INTEGER NOT NULL DEFAULT 0 CHECK (mirrored_revision >= 0),
                  last_synced_at_ms INTEGER
                );
                INSERT OR IGNORE INTO state_sync_meta(
                  singleton, lineage_id, snapshot_id, generation,
                  content_revision, mirrored_revision, last_synced_at_ms
                ) VALUES (1, '', '', 0, 0, 0, NULL);
                "#,
            )
            .map_err(|error| format!("Could not ensure Aurora's state-sync metadata: {error}"))?;
        for table in [
            "playback_queue",
            "playback_state",
            "tag_overlays",
            "tag_edit_operations",
            "musicbrainz_artist_decisions",
            "musicbrainz_release_decisions",
            "musicbrainz_curation_events",
        ] {
            for (operation, timing) in [
                ("insert", "INSERT"),
                ("update", "UPDATE"),
                ("delete", "DELETE"),
            ] {
                transaction
                    .execute_batch(&format!(
                        r#"
                        CREATE TRIGGER IF NOT EXISTS state_sync_{table}_{operation}
                        AFTER {timing} ON {table} BEGIN
                          UPDATE state_sync_meta
                          SET content_revision = content_revision + 1
                          WHERE singleton = 1;
                        END;
                        "#
                    ))
                    .map_err(|error| {
                        format!("Could not ensure Aurora's {table} state-sync trigger: {error}")
                    })?;
            }
        }
        transaction
            .pragma_update(None, "user_version", SCHEMA_VERSION)
            .map_err(|error| {
                format!("Could not mark Aurora's state migration complete: {error}")
            })?;
        transaction
            .commit()
            .map_err(|error| format!("Could not commit Aurora's state migration: {error}"))
    }

    pub(crate) fn load(&self) -> Result<StoredPlaybackState, String> {
        let connection = self.open()?;
        let mut queue_statement = connection
            .prepare(
                "SELECT track_id, track_key, directory, filename FROM playback_queue ORDER BY position",
            )
            .map_err(|error| format!("Could not prepare Aurora's queue restore: {error}"))?;
        let queue = queue_statement
            .query_map([], |row| {
                Ok(StoredQueueEntry {
                    track_id: row.get(0)?,
                    track_key: row.get(1)?,
                    directory: row.get(2)?,
                    filename: row.get(3)?,
                })
            })
            .map_err(|error| format!("Could not restore Aurora's queue: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Could not decode Aurora's queue: {error}"))?;
        connection
            .query_row(
                r#"
                SELECT current_index, position_seconds, volume, shuffle, repeat_mode
                FROM playback_state WHERE singleton = 1
                "#,
                [],
                |row| {
                    let current_index: Option<i64> = row.get(0)?;
                    Ok(StoredPlaybackState {
                        queue,
                        current_index: current_index.and_then(|value| usize::try_from(value).ok()),
                        position_seconds: row.get::<_, f64>(1)?.max(0.0),
                        volume: row.get::<_, f32>(2)?.clamp(0.0, 1.0),
                        shuffle: row.get::<_, i64>(3)? == 1,
                        repeat_mode: row.get(4)?,
                    })
                },
            )
            .map_err(|error| format!("Could not restore Aurora's playback state: {error}"))
    }

    pub(crate) fn save(&self, state: &StoredPlaybackState) -> Result<(), String> {
        let mut connection = self.open()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not begin saving Aurora's queue: {error}"))?;
        transaction
            .execute("DELETE FROM playback_queue", [])
            .map_err(|error| format!("Could not replace Aurora's queue: {error}"))?;
        {
            let mut insert = transaction
                .prepare(
                    "INSERT INTO playback_queue(position, track_id, track_key, directory, filename) VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .map_err(|error| format!("Could not prepare Aurora's queue save: {error}"))?;
            for (position, entry) in state.queue.iter().enumerate() {
                insert
                    .execute(params![
                        position as i64,
                        entry.track_id,
                        entry.track_key,
                        entry.directory,
                        entry.filename
                    ])
                    .map_err(|error| format!("Could not save Aurora's queue: {error}"))?;
            }
        }
        transaction
            .execute(
                r#"
                UPDATE playback_state
                SET current_index = ?1, position_seconds = ?2, volume = ?3,
                    shuffle = ?4, repeat_mode = ?5
                WHERE singleton = 1
                "#,
                params![
                    state.current_index.map(|value| value as i64),
                    state.position_seconds.max(0.0),
                    state.volume.clamp(0.0, 1.0),
                    i64::from(state.shuffle),
                    state.repeat_mode,
                ],
            )
            .map_err(|error| format!("Could not save Aurora's playback settings: {error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("Could not commit Aurora's playback state: {error}"))
    }

    pub(crate) fn overlays_for_keys(
        &self,
        track_keys: &[String],
    ) -> Result<Vec<TagOverlay>, String> {
        if track_keys.is_empty() {
            return Ok(Vec::new());
        }
        let connection = self.open()?;
        let mut statement = connection
            .prepare(
                r#"
                SELECT track_key, directory, filename, rating, love_state, release_year,
                       catalog_rating, catalog_love_state, catalog_release_year,
                       catalog_import_run_id, last_operation_id
                FROM tag_overlays WHERE track_key = ?1
                "#,
            )
            .map_err(|error| format!("Could not prepare Aurora's tag overlay lookup: {error}"))?;
        let mut overlays = Vec::new();
        for track_key in track_keys {
            if let Some(overlay) = statement
                .query_row(params![track_key], overlay_from_row)
                .optional()
                .map_err(|error| format!("Could not read Aurora's tag overlay: {error}"))?
            {
                overlays.push(overlay);
            }
        }
        Ok(overlays)
    }

    pub(crate) fn all_overlays(&self) -> Result<Vec<TagOverlay>, String> {
        let connection = self.open()?;
        let mut statement = connection
            .prepare(
                r#"
                SELECT track_key, directory, filename, rating, love_state, release_year,
                       catalog_rating, catalog_love_state, catalog_release_year,
                       catalog_import_run_id, last_operation_id
                FROM tag_overlays ORDER BY track_key
                "#,
            )
            .map_err(|error| format!("Could not prepare Aurora's tag reconciliation: {error}"))?;
        statement
            .query_map([], overlay_from_row)
            .map_err(|error| format!("Could not read Aurora's tag reconciliation: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Could not decode Aurora's tag reconciliation: {error}"))
    }

    pub(crate) fn pending_overlays(&self, limit: usize) -> Result<Vec<TagOverlay>, String> {
        if limit == 0 || limit > 201 {
            return Err("Aurora's pending-tag reconciliation batch is invalid.".to_owned());
        }
        let connection = self.open()?;
        let mut statement = connection
            .prepare(
                r#"
                SELECT track_key, directory, filename, rating, love_state, release_year,
                       catalog_rating, catalog_love_state, catalog_release_year,
                       catalog_import_run_id, last_operation_id
                FROM tag_overlays
                ORDER BY updated_at_ms, track_key
                LIMIT ?1
                "#,
            )
            .map_err(|error| {
                format!("Could not prepare Aurora's pending-tag reconciliation: {error}")
            })?;
        statement
            .query_map(params![limit as i64], overlay_from_row)
            .map_err(|error| {
                format!("Could not read Aurora's pending-tag reconciliation: {error}")
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                format!("Could not decode Aurora's pending-tag reconciliation: {error}")
            })
    }

    pub(crate) fn defer_overlay_reconciliation(&self, track_key: &str) -> Result<(), String> {
        let connection = self.open()?;
        let timestamp = now_ms();
        connection
            .execute(
                r#"
                UPDATE tag_overlays
                SET updated_at_ms = CASE
                  WHEN updated_at_ms >= ?2 THEN updated_at_ms + 1
                  ELSE ?2
                END
                WHERE track_key = ?1
                "#,
                params![track_key, timestamp],
            )
            .map_err(|error| format!("Could not defer a pending-tag retry: {error}"))?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn upsert_overlay(
        &self,
        track_key: &str,
        directory: &str,
        filename: &str,
        catalog_values: &TagValues,
        values: &TagValues,
        catalog_import_run_id: i64,
        last_operation_id: Option<i64>,
    ) -> Result<(), String> {
        let connection = self.open()?;
        if values == catalog_values {
            connection
                .execute(
                    "DELETE FROM tag_overlays WHERE track_key = ?1",
                    params![track_key],
                )
                .map_err(|error| format!("Could not reconcile Aurora's tag overlay: {error}"))?;
            return Ok(());
        }
        connection
            .execute(
                r#"
                INSERT INTO tag_overlays (
                  track_key, directory, filename, rating, love_state, release_year,
                  catalog_rating, catalog_love_state, catalog_release_year,
                  catalog_import_run_id, last_operation_id, updated_at_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                ON CONFLICT(track_key) DO UPDATE SET
                  directory = excluded.directory,
                  filename = excluded.filename,
                  rating = excluded.rating,
                  love_state = excluded.love_state,
                  release_year = excluded.release_year,
                  catalog_rating = excluded.catalog_rating,
                  catalog_love_state = excluded.catalog_love_state,
                  catalog_release_year = excluded.catalog_release_year,
                  catalog_import_run_id = excluded.catalog_import_run_id,
                  last_operation_id = COALESCE(excluded.last_operation_id, tag_overlays.last_operation_id),
                  updated_at_ms = excluded.updated_at_ms
                "#,
                params![
                    track_key,
                    directory,
                    filename,
                    values.rating,
                    values.love_state.as_db(),
                    values.release_year,
                    catalog_values.rating,
                    catalog_values.love_state.as_db(),
                    catalog_values.release_year,
                    catalog_import_run_id,
                    last_operation_id,
                    now_ms(),
                ],
            )
            .map_err(|error| format!("Could not save Aurora's tag overlay: {error}"))?;
        Ok(())
    }

    pub(crate) fn overlay_summary_deltas(
        &self,
        current_import_run_id: i64,
    ) -> Result<(i64, i64), String> {
        self.open()?
            .query_row(
                r#"
                SELECT
                  COALESCE(SUM((love_state = 'loved') - (catalog_love_state = 'loved')), 0),
                  COALESCE(SUM((rating IS NOT NULL) - (catalog_rating IS NOT NULL)), 0)
                FROM tag_overlays
                WHERE catalog_import_run_id = ?1
                "#,
                params![current_import_run_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|error| format!("Could not summarize Aurora's pending tag edits: {error}"))
    }

    pub(crate) fn begin_tag_operation(
        &self,
        track_key: &str,
        target_path: &str,
        before: &TagValues,
        after: &TagValues,
        source_fingerprint: &str,
    ) -> Result<i64, String> {
        let connection = self.open()?;
        let timestamp = now_ms();
        connection
            .execute(
                r#"
                INSERT INTO tag_edit_operations (
                  track_key, target_path,
                  before_rating, before_love_state, before_release_year,
                  after_rating, after_love_state, after_release_year,
                  source_fingerprint, status, created_at_ms, updated_at_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'prepared', ?10, ?10)
                "#,
                params![
                    track_key,
                    target_path,
                    before.rating,
                    before.love_state.as_db(),
                    before.release_year,
                    after.rating,
                    after.love_state.as_db(),
                    after.release_year,
                    source_fingerprint,
                    timestamp,
                ],
            )
            .map_err(|error| format!("Could not start Aurora's tag journal: {error}"))?;
        Ok(connection.last_insert_rowid())
    }

    pub(crate) fn set_operation_paths(
        &self,
        operation_id: i64,
        temp_path: &str,
        backup_path: &str,
    ) -> Result<(), String> {
        self.open()?
            .execute(
                r#"
                UPDATE tag_edit_operations
                SET temp_path = ?1, backup_path = ?2, updated_at_ms = ?3
                WHERE id = ?4 AND status = 'prepared'
                "#,
                params![temp_path, backup_path, now_ms(), operation_id],
            )
            .map_err(|error| format!("Could not checkpoint Aurora's tag paths: {error}"))?;
        Ok(())
    }

    pub(crate) fn mark_operation(
        &self,
        operation_id: i64,
        status: &str,
        error_message: Option<&str>,
    ) -> Result<(), String> {
        if !matches!(
            status,
            "replaced" | "verified" | "undoing" | "failed" | "rolledBack"
        ) {
            return Err("Aurora refused an invalid tag journal transition.".to_owned());
        }
        self.open()?
            .execute(
                r#"
                UPDATE tag_edit_operations
                SET status = ?1, error_message = ?2, updated_at_ms = ?3
                WHERE id = ?4
                "#,
                params![status, error_message, now_ms(), operation_id],
            )
            .map_err(|error| format!("Could not update Aurora's tag journal: {error}"))?;
        Ok(())
    }

    pub(crate) fn begin_undo(
        &self,
        operation_id: i64,
        current_backup_path: &str,
        source_fingerprint: &str,
    ) -> Result<(), String> {
        let updated = self
            .open()?
            .execute(
                r#"
                UPDATE tag_edit_operations
                SET status = 'undoing', temp_path = ?1, source_fingerprint = ?2,
                    updated_at_ms = ?3
                WHERE id = ?4 AND status = 'verified'
                "#,
                params![
                    current_backup_path,
                    source_fingerprint,
                    now_ms(),
                    operation_id
                ],
            )
            .map_err(|error| format!("Could not start Aurora's undo journal: {error}"))?;
        if updated != 1 {
            return Err("Aurora's saved undo changed before it could start.".to_owned());
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn finish_tag_operation(
        &self,
        operation_id: i64,
        track_key: &str,
        directory: &str,
        filename: &str,
        catalog_values: &TagValues,
        values: &TagValues,
        catalog_import_run_id: i64,
    ) -> Result<(), String> {
        let mut connection = self.open()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not finish Aurora's tag transaction: {error}"))?;
        if values == catalog_values {
            transaction
                .execute(
                    "DELETE FROM tag_overlays WHERE track_key = ?1",
                    params![track_key],
                )
                .map_err(|error| {
                    format!("Could not reconcile Aurora's tag transaction: {error}")
                })?;
        } else {
            transaction
                .execute(
                    r#"
                    INSERT INTO tag_overlays (
                      track_key, directory, filename, rating, love_state, release_year,
                      catalog_rating, catalog_love_state, catalog_release_year,
                      catalog_import_run_id, last_operation_id, updated_at_ms
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                    ON CONFLICT(track_key) DO UPDATE SET
                      directory = excluded.directory, filename = excluded.filename,
                      rating = excluded.rating, love_state = excluded.love_state,
                      release_year = excluded.release_year,
                      catalog_rating = excluded.catalog_rating,
                      catalog_love_state = excluded.catalog_love_state,
                      catalog_release_year = excluded.catalog_release_year,
                      catalog_import_run_id = excluded.catalog_import_run_id,
                      last_operation_id = excluded.last_operation_id,
                      updated_at_ms = excluded.updated_at_ms
                    "#,
                    params![
                        track_key,
                        directory,
                        filename,
                        values.rating,
                        values.love_state.as_db(),
                        values.release_year,
                        catalog_values.rating,
                        catalog_values.love_state.as_db(),
                        catalog_values.release_year,
                        catalog_import_run_id,
                        operation_id,
                        now_ms(),
                    ],
                )
                .map_err(|error| format!("Could not save Aurora's tag transaction: {error}"))?;
        }
        let updated = transaction
            .execute(
                r#"
                UPDATE tag_edit_operations
                SET status = 'verified', error_message = NULL, updated_at_ms = ?1
                WHERE id = ?2 AND status IN ('prepared', 'replaced')
                "#,
                params![now_ms(), operation_id],
            )
            .map_err(|error| format!("Could not verify Aurora's tag transaction: {error}"))?;
        if updated != 1 {
            return Err("Aurora's tag journal changed before verification.".to_owned());
        }
        transaction
            .commit()
            .map_err(|error| format!("Could not commit Aurora's tag transaction: {error}"))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn finish_undo_operation(
        &self,
        operation_id: i64,
        track_key: &str,
        directory: &str,
        filename: &str,
        catalog_values: &TagValues,
        values: &TagValues,
        catalog_import_run_id: i64,
    ) -> Result<(), String> {
        let mut connection = self.open()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not finish Aurora's undo transaction: {error}"))?;
        if values == catalog_values {
            transaction
                .execute(
                    "DELETE FROM tag_overlays WHERE track_key = ?1",
                    params![track_key],
                )
                .map_err(|error| format!("Could not reconcile Aurora's undo: {error}"))?;
        } else {
            transaction
                .execute(
                    r#"
                    INSERT INTO tag_overlays (
                      track_key, directory, filename, rating, love_state, release_year,
                      catalog_rating, catalog_love_state, catalog_release_year,
                      catalog_import_run_id, last_operation_id, updated_at_ms
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                    ON CONFLICT(track_key) DO UPDATE SET
                      directory = excluded.directory, filename = excluded.filename,
                      rating = excluded.rating, love_state = excluded.love_state,
                      release_year = excluded.release_year,
                      catalog_rating = excluded.catalog_rating,
                      catalog_love_state = excluded.catalog_love_state,
                      catalog_release_year = excluded.catalog_release_year,
                      catalog_import_run_id = excluded.catalog_import_run_id,
                      last_operation_id = excluded.last_operation_id,
                      updated_at_ms = excluded.updated_at_ms
                    "#,
                    params![
                        track_key,
                        directory,
                        filename,
                        values.rating,
                        values.love_state.as_db(),
                        values.release_year,
                        catalog_values.rating,
                        catalog_values.love_state.as_db(),
                        catalog_values.release_year,
                        catalog_import_run_id,
                        operation_id,
                        now_ms(),
                    ],
                )
                .map_err(|error| format!("Could not save Aurora's undo overlay: {error}"))?;
        }
        let updated = transaction
            .execute(
                r#"
                UPDATE tag_edit_operations
                SET status = 'rolledBack', error_message = NULL, updated_at_ms = ?1
                WHERE id = ?2 AND status = 'undoing'
                "#,
                params![now_ms(), operation_id],
            )
            .map_err(|error| format!("Could not verify Aurora's undo transaction: {error}"))?;
        if updated != 1 {
            return Err("Aurora's undo journal changed before verification.".to_owned());
        }
        transaction
            .commit()
            .map_err(|error| format!("Could not commit Aurora's undo transaction: {error}"))
    }

    pub(crate) fn interrupted_operations(&self) -> Result<Vec<TagOperation>, String> {
        let connection = self.open()?;
        let mut statement = connection
            .prepare(
                r#"
                SELECT id, track_key, target_path, temp_path, backup_path,
                       before_rating, before_love_state, before_release_year,
                       after_rating, after_love_state, after_release_year,
                       source_fingerprint, status
                FROM tag_edit_operations
                WHERE status IN ('prepared', 'replaced', 'undoing') ORDER BY id
                "#,
            )
            .map_err(|error| format!("Could not prepare Aurora's tag recovery: {error}"))?;
        statement
            .query_map([], operation_from_row)
            .map_err(|error| format!("Could not read Aurora's tag recovery journal: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Could not decode Aurora's tag recovery journal: {error}"))
    }

    pub(crate) fn latest_undo_operation(
        &self,
        track_key: &str,
    ) -> Result<Option<TagOperation>, String> {
        let connection = self.open()?;
        let operation = connection
            .query_row(
                r#"
                SELECT id, track_key, target_path, temp_path, backup_path,
                       before_rating, before_love_state, before_release_year,
                       after_rating, after_love_state, after_release_year,
                       source_fingerprint, status
                FROM tag_edit_operations
                WHERE track_key = ?1 AND status = 'verified' AND backup_path IS NOT NULL
                  AND id = (SELECT MAX(id) FROM tag_edit_operations WHERE track_key = ?1)
                "#,
                params![track_key],
                operation_from_row,
            )
            .optional()
            .map_err(|error| format!("Could not read Aurora's undo journal: {error}"))?;
        Ok(operation.filter(|operation| {
            operation
                .backup_path
                .as_ref()
                .is_some_and(|path| path.is_file())
        }))
    }

    pub(crate) fn can_undo(&self, track_key: &str) -> Result<bool, String> {
        Ok(self.latest_undo_operation(track_key)?.is_some())
    }

    pub(crate) fn undoable_keys(&self, track_keys: &[String]) -> Result<HashSet<String>, String> {
        let connection = self.open()?;
        let mut statement = connection
            .prepare(
                r#"
                SELECT backup_path FROM tag_edit_operations
                WHERE track_key = ?1 AND status = 'verified' AND backup_path IS NOT NULL
                  AND id = (SELECT MAX(id) FROM tag_edit_operations WHERE track_key = ?1)
                "#,
            )
            .map_err(|error| format!("Could not prepare Aurora's undo lookup: {error}"))?;
        let mut undoable = HashSet::new();
        for track_key in track_keys {
            let backup = statement
                .query_row(params![track_key], |row| row.get::<_, String>(0))
                .optional()
                .map_err(|error| format!("Could not read Aurora's undo lookup: {error}"))?;
            if backup
                .is_some_and(|path| device_mode::resolve_device_path(Path::new(&path)).is_file())
            {
                undoable.insert(track_key.clone());
            }
        }
        Ok(undoable)
    }

    pub(crate) fn prune_old_backups(&self, keep: usize) -> Result<(), String> {
        let connection = self.open()?;
        let mut statement = connection
            .prepare(
                r#"
                SELECT id, backup_path FROM tag_edit_operations
                WHERE status IN ('verified', 'rolledBack') AND backup_path IS NOT NULL
                ORDER BY id DESC LIMIT -1 OFFSET ?1
                "#,
            )
            .map_err(|error| format!("Could not prepare Aurora's backup retention: {error}"))?;
        let stale = statement
            .query_map(params![keep as i64], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    device_mode::resolve_device_path(Path::new(&row.get::<_, String>(1)?)),
                ))
            })
            .map_err(|error| format!("Could not read Aurora's backup retention list: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Could not decode Aurora's backup retention list: {error}"))?;
        drop(statement);

        for (operation_id, path) in stale {
            let looks_owned = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with('.') && name.contains(".aurora-") && name.ends_with(".backup")
                });
            if looks_owned && path.is_file() {
                fs::remove_file(&path).map_err(|error| {
                    format!("Could not remove an expired Aurora tag backup: {error}")
                })?;
            }
            connection
                .execute(
                    "UPDATE tag_edit_operations SET backup_path = NULL WHERE id = ?1",
                    params![operation_id],
                )
                .map_err(|error| format!("Could not record Aurora's backup retention: {error}"))?;
        }
        Ok(())
    }
}

fn overlay_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TagOverlay> {
    let love_state: String = row.get(4)?;
    let catalog_love_state: String = row.get(7)?;
    Ok(TagOverlay {
        track_key: row.get(0)?,
        directory: row.get(1)?,
        filename: row.get(2)?,
        values: TagValues {
            rating: row.get(3)?,
            love_state: love_state_from_row(&love_state, 4)?,
            release_year: row.get(5)?,
        },
        catalog_values: TagValues {
            rating: row.get(6)?,
            love_state: love_state_from_row(&catalog_love_state, 7)?,
            release_year: row.get(8)?,
        },
        catalog_import_run_id: row.get(9)?,
        last_operation_id: row.get(10)?,
    })
}

fn operation_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TagOperation> {
    let before_love_state: String = row.get(6)?;
    let after_love_state: String = row.get(9)?;
    Ok(TagOperation {
        id: row.get(0)?,
        track_key: row.get(1)?,
        target_path: device_mode::resolve_device_path(Path::new(&row.get::<_, String>(2)?)),
        temp_path: row
            .get::<_, Option<String>>(3)?
            .map(|path| device_mode::resolve_device_path(Path::new(&path))),
        backup_path: row
            .get::<_, Option<String>>(4)?
            .map(|path| device_mode::resolve_device_path(Path::new(&path))),
        before: TagValues {
            rating: row.get(5)?,
            love_state: love_state_from_row(&before_love_state, 6)?,
            release_year: row.get(7)?,
        },
        after: TagValues {
            rating: row.get(8)?,
            love_state: love_state_from_row(&after_love_state, 9)?,
            release_year: row.get(10)?,
        },
        source_fingerprint: row.get(11)?,
        status: row.get(12)?,
    })
}

fn love_state_from_row(value: &str, column: usize) -> rusqlite::Result<LoveState> {
    LoveState::from_db(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            std::io::Error::new(std::io::ErrorKind::InvalidData, error).into(),
        )
    })
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_state_path() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "aurora-state-{}-{unique}.sqlite3",
            std::process::id()
        ))
    }

    #[test]
    fn migrates_v1_queue_and_persists_stable_identity() {
        let path = temporary_state_path();
        {
            let connection = Connection::open(&path).expect("create v1 state");
            connection
                .execute_batch(
                    r#"
                    CREATE TABLE playback_queue(position INTEGER PRIMARY KEY, track_id TEXT NOT NULL);
                    CREATE TABLE playback_state(
                      singleton INTEGER PRIMARY KEY, current_index INTEGER,
                      position_seconds REAL NOT NULL DEFAULT 0, volume REAL NOT NULL DEFAULT 0.7,
                      shuffle INTEGER NOT NULL DEFAULT 0, repeat_mode TEXT NOT NULL DEFAULT 'off'
                    );
                    INSERT INTO playback_state(singleton) VALUES (1);
                    INSERT INTO playback_queue VALUES (0, '7');
                    PRAGMA user_version = 1;
                    "#,
                )
                .expect("seed v1 state");
        }

        let store = StateStore::new(path.clone()).expect("migrate state");
        let migrated = store.load().expect("load migrated state");
        assert_eq!(migrated.queue[0].track_id, "7");
        assert_eq!(migrated.queue[0].track_key, None);
        assert_eq!(migrated.queue[0].directory, None);

        let expected = StoredPlaybackState {
            queue: vec![StoredQueueEntry {
                track_id: "57".to_owned(),
                track_key: Some("h:\\music\\sæglópur.mp3".to_owned()),
                directory: Some("H:\\Music".to_owned()),
                filename: Some("Sæglópur.mp3".to_owned()),
            }],
            current_index: Some(0),
            position_seconds: 31.5,
            volume: 0.42,
            shuffle: true,
            repeat_mode: "all".to_owned(),
        };
        store.save(&expected).expect("save stable state");
        assert_eq!(store.load().expect("reload state"), expected);

        drop(store);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn overlays_are_durable_and_reconcile_when_catalog_matches() {
        let path = temporary_state_path();
        let store = StateStore::new(path.clone()).expect("state store");
        let catalog = TagValues {
            rating: Some(4.0),
            love_state: LoveState::Neutral,
            release_year: Some(2011),
        };
        let desired = TagValues {
            rating: Some(4.5),
            love_state: LoveState::Loved,
            release_year: Some(2011),
        };
        store
            .upsert_overlay(
                "h:\\music\\track.mp3",
                "H:\\Music",
                "Track.mp3",
                &catalog,
                &desired,
                52,
                Some(1),
            )
            .expect("save overlay");
        assert_eq!(
            store
                .overlays_for_keys(&["h:\\music\\track.mp3".to_owned()])
                .expect("load overlay")[0]
                .values,
            desired
        );
        assert_eq!(
            store.overlay_summary_deltas(52).expect("current delta"),
            (1, 0)
        );
        assert_eq!(
            store.overlay_summary_deltas(53).expect("stale delta"),
            (0, 0)
        );
        store
            .upsert_overlay(
                "h:\\music\\track.mp3",
                "H:\\Music",
                "Track.mp3",
                &desired,
                &desired,
                53,
                None,
            )
            .expect("reconcile overlay");
        assert!(
            store
                .overlays_for_keys(&["h:\\music\\track.mp3".to_owned()])
                .expect("load reconciled overlays")
                .is_empty()
        );

        drop(store);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn migrates_v3_journal_to_crash_recoverable_undo_status() {
        let path = temporary_state_path();
        {
            let connection = Connection::open(&path).expect("create v3 state");
            connection
                .execute_batch(
                    r#"
                    CREATE TABLE playback_queue(
                      position INTEGER PRIMARY KEY, track_id TEXT NOT NULL,
                      track_key TEXT, directory TEXT, filename TEXT
                    );
                    CREATE TABLE playback_state(
                      singleton INTEGER PRIMARY KEY, current_index INTEGER,
                      position_seconds REAL NOT NULL DEFAULT 0, volume REAL NOT NULL DEFAULT 0.7,
                      shuffle INTEGER NOT NULL DEFAULT 0, repeat_mode TEXT NOT NULL DEFAULT 'off'
                    );
                    INSERT INTO playback_state(singleton) VALUES (1);
                    CREATE TABLE tag_edit_operations (
                      id INTEGER PRIMARY KEY AUTOINCREMENT, track_key TEXT NOT NULL,
                      target_path TEXT NOT NULL, temp_path TEXT, backup_path TEXT,
                      before_rating REAL, before_love_state TEXT NOT NULL,
                      before_release_year INTEGER, after_rating REAL,
                      after_love_state TEXT NOT NULL, after_release_year INTEGER,
                      source_fingerprint TEXT NOT NULL,
                      status TEXT NOT NULL CHECK (status IN ('prepared','replaced','verified','failed','rolledBack')),
                      created_at_ms INTEGER NOT NULL, updated_at_ms INTEGER NOT NULL,
                      error_message TEXT
                    );
                    INSERT INTO tag_edit_operations(
                      track_key,target_path,before_love_state,after_love_state,
                      source_fingerprint,status,created_at_ms,updated_at_ms
                    ) VALUES ('track-key','H:\Music\Track.mp3','neutral','loved','fingerprint','verified',1,1);
                    PRAGMA user_version = 3;
                    "#,
                )
                .expect("seed v3 state");
        }

        let store = StateStore::new(path.clone()).expect("migrate v3 state");
        let connection = store.open().expect("open migrated state");
        let journal_sql: String = connection
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='tag_edit_operations'",
                [],
                |row| row.get(0),
            )
            .expect("read journal schema");
        assert!(journal_sql.contains("'undoing'"));
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM tag_edit_operations", [], |row| row
                    .get::<_, i64>(0))
                .expect("count migrated operations"),
            1
        );
        drop(connection);
        drop(store);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn migrates_v5_curation_rows_and_installs_sync_revision_triggers() {
        let path = temporary_state_path();
        {
            let connection = Connection::open(&path).expect("create v5 state");
            connection
                .execute_batch(
                    r#"
                    CREATE TABLE playback_queue(position INTEGER PRIMARY KEY, track_id TEXT NOT NULL);
                    CREATE TABLE playback_state(
                      singleton INTEGER PRIMARY KEY, current_index INTEGER,
                      position_seconds REAL NOT NULL DEFAULT 0, volume REAL NOT NULL DEFAULT 0.7,
                      shuffle INTEGER NOT NULL DEFAULT 0, repeat_mode TEXT NOT NULL DEFAULT 'off'
                    );
                    INSERT INTO playback_state(singleton) VALUES (1);
                    CREATE TABLE musicbrainz_artist_decisions(
                      local_artist_key TEXT PRIMARY KEY, display_artist TEXT NOT NULL,
                      decision TEXT NOT NULL, artist_mbid TEXT, canonical_name TEXT,
                      created_at_ms INTEGER NOT NULL, updated_at_ms INTEGER NOT NULL
                    );
                    INSERT INTO musicbrainz_artist_decisions VALUES(
                      'm83', 'M83', 'confirmed',
                      '6d7b7cd4-254b-4c25-83f6-dd20f98ceacd', 'M83', 1, 1
                    );
                    PRAGMA user_version = 5;
                    "#,
                )
                .expect("seed v5 state");
        }

        let store = StateStore::new(path.clone()).expect("migrate v5 state");
        let connection = store.open().expect("open migrated state");
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .expect("schema version"),
            6
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM musicbrainz_artist_decisions",
                    [],
                    |row| { row.get::<_, i64>(0) }
                )
                .expect("curation row count"),
            1
        );
        let before: i64 = connection
            .query_row(
                "SELECT content_revision FROM state_sync_meta WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .expect("revision before change");
        connection
            .execute(
                "UPDATE musicbrainz_artist_decisions SET canonical_name = 'M83 updated' WHERE local_artist_key = 'm83'",
                [],
            )
            .expect("update curated row");
        let after: i64 = connection
            .query_row(
                "SELECT content_revision FROM state_sync_meta WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .expect("revision after change");
        assert_eq!(after, before + 1);

        drop(connection);
        drop(store);
        let _ = fs::remove_file(path);
    }
}
