use crate::{
    catalog::{self, TrackSummary},
    state_store::{StateStore, StoredQueueEntry},
    state_sync,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

const HISTORY_SCHEMA_VERSION: i64 = 1;
const DEFAULT_PLAY_THRESHOLD_SECONDS: u32 = 30;
const MIN_PLAY_THRESHOLD_SECONDS: u32 = 1;
const MAX_PLAY_THRESHOLD_SECONDS: u32 = 3_600;
const HISTORY_SYNC_INTERVAL_MS: i64 = 60_000;
const MAX_HISTORY_SOURCES: usize = 16;
const MAX_HISTORY_PAGE_SIZE: usize = 100;

#[derive(Clone, Debug)]
struct HistoryMetadata {
    device_id: String,
    device_name: String,
    content_revision: i64,
    mirrored_revision: i64,
}

#[derive(Clone, Debug)]
struct HistoryRow {
    session_id: String,
    track_key: String,
    title: String,
    artist: String,
    album: String,
    genre: Option<String>,
    directory: String,
    filename: String,
    duration_seconds: Option<i64>,
    device_id: String,
    device_name: String,
    started_at_ms: i64,
    ended_at_ms: Option<i64>,
    listened_seconds: f64,
    registered_play: bool,
    registered_at_ms: Option<i64>,
    outcome: String,
}

#[derive(Clone, Debug)]
struct HistoryTopRow {
    track_key: String,
    title: String,
    artist: String,
    album: String,
    directory: String,
    filename: String,
    plays: i64,
    listened_seconds: f64,
    last_played_at_ms: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct HistoryPageRequest {
    page_size: usize,
    cursor: Option<String>,
    search: Option<String>,
    device_id: Option<String>,
    outcome: Option<String>,
    started_after_ms: Option<i64>,
    started_before_ms: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistoryItem {
    session_id: String,
    track_key: String,
    title: String,
    artist: String,
    album: String,
    genre: Option<String>,
    duration_seconds: Option<i64>,
    device_id: String,
    device_name: String,
    started_at_ms: i64,
    ended_at_ms: Option<i64>,
    listened_seconds: f64,
    registered_play: bool,
    registered_at_ms: Option<i64>,
    outcome: String,
    track: Option<TrackSummary>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistorySummary {
    sessions: i64,
    plays: i64,
    skips: i64,
    unique_tracks: i64,
    listened_seconds: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistoryDevice {
    device_id: String,
    device_name: String,
    sessions: i64,
    last_listened_at_ms: Option<i64>,
    is_this_device: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistoryTopTrack {
    track_key: String,
    title: String,
    artist: String,
    album: String,
    plays: i64,
    listened_seconds: f64,
    last_played_at_ms: i64,
    track: Option<TrackSummary>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistoryPage {
    items: Vec<HistoryItem>,
    summary: HistorySummary,
    top_tracks: Vec<HistoryTopTrack>,
    devices: Vec<HistoryDevice>,
    next_cursor: Option<String>,
    play_threshold_seconds: u32,
    sync_state: &'static str,
    sync_message: String,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrackHistoryInsight {
    sessions: i64,
    plays: i64,
    skips: i64,
    listened_seconds: f64,
    last_listened_at_ms: Option<i64>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct GenreHistoryInsight {
    pub(crate) sessions: i64,
    pub(crate) plays: i64,
    pub(crate) listened_seconds: f64,
    pub(crate) last_listened_at_ms: Option<i64>,
}

#[derive(Clone, Debug)]
pub(crate) struct ActiveHistorySession {
    session_id: String,
    listened_seconds: f64,
    last_position_seconds: f64,
    configured_threshold_seconds: u32,
    effective_threshold_seconds: f64,
    duration_seconds: Option<f64>,
    registered_play: bool,
    checkpoint_bucket: u64,
}

#[derive(Default)]
struct SharedHistoryState {
    last_publish_attempt_ms: Option<i64>,
    last_error: Option<String>,
    remote_write_blocked: bool,
}

#[derive(Clone)]
pub(crate) struct HistoryStore {
    path: PathBuf,
    remote_directory: PathBuf,
    remote_path: PathBuf,
    device_id: String,
    device_name: String,
    shared: Arc<Mutex<SharedHistoryState>>,
}

impl HistoryStore {
    pub(crate) fn local_device_id(path: &Path) -> Result<Option<String>, String> {
        if !path.is_file() {
            return Ok(None);
        }
        open_valid_history_source(path).map(|source| source.map(|(metadata, _)| metadata.device_id))
    }

    pub(crate) fn new(
        path: PathBuf,
        remote_directory: PathBuf,
        device_id: String,
        device_name: String,
    ) -> Result<Self, String> {
        if !valid_device_id(&device_id) {
            return Err("Aurora's listening-history device identity is invalid.".to_owned());
        }
        let parent = path
            .parent()
            .ok_or_else(|| "Aurora's listening-history path has no parent directory.".to_owned())?;
        fs::create_dir_all(parent).map_err(|error| {
            format!("Could not create Aurora's listening-history folder: {error}")
        })?;
        let remote_path = remote_directory.join(format!("aurora-history-{device_id}.sqlite3"));
        let restore_warning = if !path.is_file() && remote_path.is_file() {
            restore_local_history(&remote_path, &path, &device_id)
                .err()
                .map(|error| {
                    format!(
                        "Aurora could not restore this device's OneDrive history and will not overwrite it. Local listening history remains available after a restart resolves the snapshot: {error}"
                    )
                })
        } else {
            None
        };
        let remote_write_blocked = restore_warning.is_some();
        let store = Self {
            path,
            remote_directory,
            remote_path,
            device_id,
            device_name,
            shared: Arc::new(Mutex::new(SharedHistoryState {
                last_publish_attempt_ms: None,
                last_error: restore_warning,
                remote_write_blocked,
            })),
        };
        store.migrate()?;
        store.recover_interrupted_sessions()?;
        Ok(store)
    }

    fn open(&self) -> Result<Connection, String> {
        let connection = Connection::open(&self.path)
            .map_err(|error| format!("Could not open Aurora's listening history: {error}"))?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|error| format!("Could not configure Aurora's listening history: {error}"))?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|error| format!("Could not enable durable listening history: {error}"))?;
        Ok(connection)
    }

    fn migrate(&self) -> Result<(), String> {
        let mut connection = self.open()?;
        let current: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|error| format!("Could not read Aurora's history schema: {error}"))?;
        if current > HISTORY_SCHEMA_VERSION {
            return Err(format!(
                "Aurora's listening history uses unsupported schema version {current}."
            ));
        }
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not start Aurora's history migration: {error}"))?;
        transaction
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS history_meta (
                  singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                  device_id TEXT NOT NULL,
                  device_name TEXT NOT NULL,
                  play_threshold_seconds INTEGER NOT NULL DEFAULT 30
                    CHECK (play_threshold_seconds BETWEEN 1 AND 3600),
                  content_revision INTEGER NOT NULL DEFAULT 0 CHECK (content_revision >= 0),
                  mirrored_revision INTEGER NOT NULL DEFAULT 0 CHECK (mirrored_revision >= 0),
                  last_synced_at_ms INTEGER
                );
                CREATE TABLE IF NOT EXISTS listening_sessions (
                  session_id TEXT PRIMARY KEY,
                  track_key TEXT NOT NULL,
                  title TEXT NOT NULL,
                  artist TEXT NOT NULL,
                  album TEXT NOT NULL,
                  genre TEXT,
                  directory TEXT NOT NULL,
                  filename TEXT NOT NULL,
                  duration_seconds INTEGER,
                  started_at_ms INTEGER NOT NULL,
                  ended_at_ms INTEGER,
                  listened_seconds REAL NOT NULL DEFAULT 0 CHECK (listened_seconds >= 0),
                  registered_play INTEGER NOT NULL DEFAULT 0 CHECK (registered_play IN (0, 1)),
                  registered_at_ms INTEGER,
                  threshold_seconds INTEGER NOT NULL CHECK (threshold_seconds BETWEEN 1 AND 3600),
                  outcome TEXT NOT NULL DEFAULT 'active'
                    CHECK (outcome IN ('active', 'completed', 'skipped', 'interrupted'))
                );
                CREATE INDEX IF NOT EXISTS idx_history_started
                  ON listening_sessions(started_at_ms DESC, session_id DESC);
                CREATE INDEX IF NOT EXISTS idx_history_track_plays
                  ON listening_sessions(track_key, registered_play, started_at_ms DESC);
                CREATE INDEX IF NOT EXISTS idx_history_outcome
                  ON listening_sessions(outcome, started_at_ms DESC);
                "#,
            )
            .map_err(|error| format!("Could not ensure Aurora's history schema: {error}"))?;
        let existing_device: Option<String> = transaction
            .query_row(
                "SELECT device_id FROM history_meta WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| format!("Could not inspect Aurora's history identity: {error}"))?;
        if let Some(existing) = existing_device {
            if existing != self.device_id {
                return Err(
                    "Aurora refused to open listening history owned by another device.".to_owned(),
                );
            }
            transaction
                .execute(
                    "UPDATE history_meta SET device_name = ?1 WHERE singleton = 1",
                    [&self.device_name],
                )
                .map_err(|error| format!("Could not refresh Aurora's device label: {error}"))?;
        } else {
            transaction
                .execute(
                    r#"
                    INSERT INTO history_meta(
                      singleton, device_id, device_name, play_threshold_seconds,
                      content_revision, mirrored_revision, last_synced_at_ms
                    ) VALUES (1, ?1, ?2, ?3, 0, 0, NULL)
                    "#,
                    params![
                        self.device_id,
                        self.device_name,
                        DEFAULT_PLAY_THRESHOLD_SECONDS
                    ],
                )
                .map_err(|error| {
                    format!("Could not initialize Aurora's history identity: {error}")
                })?;
        }
        transaction
            .pragma_update(None, "user_version", HISTORY_SCHEMA_VERSION)
            .map_err(|error| {
                format!("Could not mark Aurora's history migration complete: {error}")
            })?;
        transaction
            .commit()
            .map_err(|error| format!("Could not commit Aurora's history migration: {error}"))
    }

    fn recover_interrupted_sessions(&self) -> Result<(), String> {
        let mut connection = self.open()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not start history recovery: {error}"))?;
        let now = state_sync::now_ms();
        let updated = transaction
            .execute(
                r#"
                UPDATE listening_sessions
                SET ended_at_ms = ?1, outcome = 'interrupted'
                WHERE outcome = 'active'
                "#,
                [now],
            )
            .map_err(|error| format!("Could not recover interrupted listening history: {error}"))?;
        if updated > 0 {
            transaction
                .execute(
                    "UPDATE history_meta SET content_revision = content_revision + 1 WHERE singleton = 1",
                    [],
                )
                .map_err(|error| format!("Could not checkpoint recovered history: {error}"))?;
        }
        transaction
            .commit()
            .map_err(|error| format!("Could not commit history recovery: {error}"))
    }

    pub(crate) fn play_threshold_seconds(&self) -> Result<u32, String> {
        let connection = self.open()?;
        let value: i64 = connection
            .query_row(
                "SELECT play_threshold_seconds FROM history_meta WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|error| format!("Could not read Aurora's played threshold: {error}"))?;
        u32::try_from(value)
            .ok()
            .filter(|value| {
                (MIN_PLAY_THRESHOLD_SECONDS..=MAX_PLAY_THRESHOLD_SECONDS).contains(value)
            })
            .ok_or_else(|| "Aurora's saved played threshold is invalid.".to_owned())
    }

    pub(crate) fn set_play_threshold_seconds(&self, value: u32) -> Result<u32, String> {
        if !(MIN_PLAY_THRESHOLD_SECONDS..=MAX_PLAY_THRESHOLD_SECONDS).contains(&value) {
            return Err(format!(
                "Played threshold must be between {MIN_PLAY_THRESHOLD_SECONDS} and {MAX_PLAY_THRESHOLD_SECONDS} seconds."
            ));
        }
        let connection = self.open()?;
        connection
            .execute(
                r#"
                UPDATE history_meta
                SET play_threshold_seconds = ?1,
                    content_revision = content_revision + 1
                WHERE singleton = 1 AND play_threshold_seconds != ?1
                "#,
                [i64::from(value)],
            )
            .map_err(|error| format!("Could not save Aurora's played threshold: {error}"))?;
        let _ = self.publish_if_due(true);
        Ok(value)
    }

    pub(crate) fn begin_session(
        &self,
        track: &TrackSummary,
        position_seconds: f64,
    ) -> Result<ActiveHistorySession, String> {
        let configured = self.play_threshold_seconds()?;
        let effective = effective_threshold_seconds(configured, track.duration_seconds);
        let started_at = state_sync::now_ms();
        let session_id = format!(
            "session-{}-{started_at}-{}",
            self.device_id,
            next_session_sequence()
        );
        let mut connection = self.open()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not start Aurora's listening session: {error}"))?;
        transaction
            .execute(
                r#"
                INSERT INTO listening_sessions(
                  session_id, track_key, title, artist, album, genre,
                  directory, filename, duration_seconds, started_at_ms,
                  ended_at_ms, listened_seconds, registered_play,
                  registered_at_ms, threshold_seconds, outcome
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                          NULL, 0, 0, NULL, ?11, 'active')
                "#,
                params![
                    session_id,
                    track.track_key,
                    track.title,
                    track.artist,
                    track.album,
                    track.genre,
                    track.directory,
                    track.filename,
                    track.duration_seconds,
                    started_at,
                    i64::from(configured),
                ],
            )
            .map_err(|error| format!("Could not start Aurora's listening history: {error}"))?;
        transaction
            .execute(
                "UPDATE history_meta SET content_revision = content_revision + 1 WHERE singleton = 1",
                [],
            )
            .map_err(|error| format!("Could not checkpoint Aurora's listening start: {error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("Could not commit Aurora's listening start: {error}"))?;
        Ok(ActiveHistorySession {
            session_id,
            listened_seconds: 0.0,
            last_position_seconds: position_seconds.max(0.0),
            configured_threshold_seconds: configured,
            effective_threshold_seconds: effective,
            duration_seconds: track.duration_seconds.map(|value| value.max(0) as f64),
            registered_play: false,
            checkpoint_bucket: 0,
        })
    }

    pub(crate) fn observe_position(
        &self,
        active: &mut ActiveHistorySession,
        position_seconds: f64,
    ) -> Result<(), String> {
        let position = position_seconds.max(0.0);
        let delta = (position - active.last_position_seconds).max(0.0);
        active.last_position_seconds = position;
        if delta <= 0.0 {
            return Ok(());
        }
        active.listened_seconds += delta;
        let became_played = !active.registered_play
            && active.listened_seconds + f64::EPSILON >= active.effective_threshold_seconds;
        if became_played {
            active.registered_play = true;
        }
        let bucket = (active.listened_seconds / 10.0).floor() as u64;
        if !became_played && bucket == active.checkpoint_bucket {
            return Ok(());
        }
        active.checkpoint_bucket = bucket;
        let mut connection = self.open()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not start a listening-progress checkpoint: {error}"))?;
        let updated = transaction
            .execute(
                r#"
                UPDATE listening_sessions
                SET listened_seconds = ?1,
                    registered_play = ?2,
                    registered_at_ms = CASE
                      WHEN registered_at_ms IS NULL AND ?2 = 1 THEN ?3
                      ELSE registered_at_ms
                    END
                WHERE session_id = ?4 AND outcome = 'active'
                "#,
                params![
                    active.listened_seconds,
                    i64::from(active.registered_play),
                    state_sync::now_ms(),
                    active.session_id,
                ],
            )
            .map_err(|error| format!("Could not update Aurora's listening progress: {error}"))?;
        if updated != 1 {
            return Err("Aurora's active listening session changed unexpectedly.".to_owned());
        }
        transaction
            .execute(
                "UPDATE history_meta SET content_revision = content_revision + 1 WHERE singleton = 1",
                [],
            )
            .map_err(|error| format!("Could not checkpoint Aurora's listening progress: {error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("Could not commit Aurora's listening progress: {error}"))?;
        if became_played {
            let _ = self.publish_if_due(false);
        }
        Ok(())
    }

    pub(crate) fn reset_position(&self, active: &mut ActiveHistorySession, position_seconds: f64) {
        active.last_position_seconds = position_seconds.max(0.0);
    }

    pub(crate) fn refresh_active_threshold(
        &self,
        active: &mut ActiveHistorySession,
        configured: u32,
    ) -> Result<(), String> {
        if active.configured_threshold_seconds == configured {
            return Ok(());
        }
        let effective_threshold_seconds = effective_threshold_seconds(
            configured,
            active
                .duration_seconds
                .map(|duration| duration.round() as i64),
        );
        let registered_play = active.registered_play
            || active.listened_seconds + f64::EPSILON >= effective_threshold_seconds;
        let mut connection = self.open()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not start the played-threshold update: {error}"))?;
        let updated = transaction
            .execute(
                r#"
                UPDATE listening_sessions
                SET threshold_seconds = ?1,
                    registered_play = ?2,
                    registered_at_ms = CASE
                      WHEN registered_at_ms IS NULL AND ?2 = 1 THEN ?3
                      ELSE registered_at_ms
                    END
                WHERE session_id = ?4 AND outcome = 'active'
                "#,
                params![
                    i64::from(configured),
                    i64::from(registered_play),
                    state_sync::now_ms(),
                    active.session_id,
                ],
            )
            .map_err(|error| format!("Could not apply Aurora's played threshold: {error}"))?;
        if updated != 1 {
            return Err("Aurora's active listening session changed unexpectedly.".to_owned());
        }
        transaction
            .execute(
                "UPDATE history_meta SET content_revision = content_revision + 1 WHERE singleton = 1",
                [],
            )
            .map_err(|error| format!("Could not checkpoint Aurora's played threshold: {error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("Could not commit Aurora's played threshold: {error}"))?;
        active.configured_threshold_seconds = configured;
        active.effective_threshold_seconds = effective_threshold_seconds;
        active.registered_play = registered_play;
        Ok(())
    }

    pub(crate) fn finish_session(
        &self,
        active: &ActiveHistorySession,
        outcome: &'static str,
    ) -> Result<(), String> {
        if !matches!(outcome, "completed" | "skipped" | "interrupted") {
            return Err("Aurora's listening outcome is invalid.".to_owned());
        }
        let mut connection = self.open()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not start the listening outcome update: {error}"))?;
        let updated = transaction
            .execute(
                r#"
                UPDATE listening_sessions
                SET ended_at_ms = ?1, listened_seconds = ?2,
                    registered_play = ?3,
                    registered_at_ms = CASE
                      WHEN registered_at_ms IS NULL AND ?3 = 1 THEN ?1
                      ELSE registered_at_ms
                    END,
                    outcome = ?4
                WHERE session_id = ?5 AND outcome = 'active'
                "#,
                params![
                    state_sync::now_ms(),
                    active.listened_seconds,
                    i64::from(active.registered_play),
                    outcome,
                    active.session_id,
                ],
            )
            .map_err(|error| format!("Could not finish Aurora's listening history: {error}"))?;
        if updated == 1 {
            transaction
                .execute(
                    "UPDATE history_meta SET content_revision = content_revision + 1 WHERE singleton = 1",
                    [],
                )
                .map_err(|error| format!("Could not checkpoint Aurora's listening outcome: {error}"))?;
        }
        transaction
            .commit()
            .map_err(|error| format!("Could not commit Aurora's listening outcome: {error}"))?;
        if updated == 1 {
            let _ = self.publish_if_due(false);
        }
        Ok(())
    }

    pub(crate) fn record_error(&self, error: String) {
        if let Ok(mut shared) = self.shared.lock() {
            shared.last_error = Some(error);
        }
    }

    pub(crate) fn page(
        &self,
        request: HistoryPageRequest,
        state_store: &StateStore,
    ) -> Result<HistoryPage, String> {
        validate_request(&request)?;
        let sync_message = match self.publish_if_due(false) {
            Ok(message) => message,
            Err(error) => {
                self.record_error(error.clone());
                error
            }
        };
        let sources = self.available_sources();
        let cursor = request.cursor.as_deref().map(parse_cursor).transpose()?;
        let mut rows = Vec::new();
        let mut devices = Vec::new();
        let mut summary = HistorySummary::default();
        let mut unique_tracks = HashSet::new();
        let mut top_by_key: HashMap<String, HistoryTopRow> = HashMap::new();
        let mut peer_warning = None;
        let per_source_limit = request.page_size.saturating_add(1);
        for source in sources {
            let source_result = open_valid_history_source(&source);
            let Some((metadata, connection)) = (match source_result {
                Ok(source) => source,
                Err(error) if source != self.path => {
                    peer_warning.get_or_insert(format!(
                        "A peer history snapshot was skipped without affecting local history: {error}"
                    ));
                    continue;
                }
                Err(error) => return Err(error),
            }) else {
                continue;
            };
            if source != self.path && metadata.device_id == self.device_id {
                continue;
            }
            let (source_sessions, source_last) = source_counts(&connection)?;
            devices.push(HistoryDevice {
                device_id: metadata.device_id.clone(),
                device_name: metadata.device_name.clone(),
                sessions: source_sessions,
                last_listened_at_ms: source_last,
                is_this_device: metadata.device_id == self.device_id,
            });
            accumulate_summary(&connection, &mut summary, &mut unique_tracks)?;
            for top in query_top_rows(&connection, 100)? {
                top_by_key
                    .entry(top.track_key.clone())
                    .and_modify(|combined| {
                        combined.plays = combined.plays.saturating_add(top.plays);
                        combined.listened_seconds += top.listened_seconds;
                        if top.last_played_at_ms > combined.last_played_at_ms {
                            combined.title = top.title.clone();
                            combined.artist = top.artist.clone();
                            combined.album = top.album.clone();
                            combined.directory = top.directory.clone();
                            combined.filename = top.filename.clone();
                            combined.last_played_at_ms = top.last_played_at_ms;
                        }
                    })
                    .or_insert(top);
            }
            if request
                .device_id
                .as_deref()
                .is_some_and(|device| device != metadata.device_id)
            {
                continue;
            }
            rows.extend(query_rows(
                &connection,
                &metadata,
                &request,
                cursor.as_ref(),
                per_source_limit,
            )?);
        }
        summary.unique_tracks = unique_tracks.len().min(i64::MAX as usize) as i64;
        rows.sort_by(|left, right| {
            right
                .started_at_ms
                .cmp(&left.started_at_ms)
                .then_with(|| right.session_id.cmp(&left.session_id))
        });
        let mut seen = HashSet::new();
        rows.retain(|row| seen.insert(row.session_id.clone()));
        let has_more = rows.len() > request.page_size;
        rows.truncate(request.page_size);
        let next_cursor = has_more
            .then(|| rows.last())
            .flatten()
            .map(|row| format!("{}:{}", row.started_at_ms, row.session_id));

        let mut top_rows = top_by_key.into_values().collect::<Vec<_>>();
        top_rows.sort_by(|left, right| {
            right
                .plays
                .cmp(&left.plays)
                .then_with(|| right.listened_seconds.total_cmp(&left.listened_seconds))
                .then_with(|| right.last_played_at_ms.cmp(&left.last_played_at_ms))
        });
        top_rows.truncate(8);
        let references = rows
            .iter()
            .map(|row| StoredQueueEntry {
                track_id: String::new(),
                track_key: Some(row.track_key.clone()),
                directory: Some(row.directory.clone()),
                filename: Some(row.filename.clone()),
            })
            .chain(top_rows.iter().map(|row| StoredQueueEntry {
                track_id: String::new(),
                track_key: Some(row.track_key.clone()),
                directory: Some(row.directory.clone()),
                filename: Some(row.filename.clone()),
            }))
            .collect::<Vec<_>>();
        let resolved = if references.is_empty() {
            Vec::new()
        } else {
            catalog::load_tracks_by_references(&references, state_store)
                .map(|(tracks, _, _)| tracks)
                .unwrap_or_default()
        };
        let resolved_by_key = resolved
            .into_iter()
            .map(|track| (track.track_key.clone(), track))
            .collect::<HashMap<_, _>>();
        let items = rows
            .into_iter()
            .map(|row| HistoryItem {
                track: resolved_by_key.get(&row.track_key).cloned(),
                session_id: row.session_id,
                track_key: row.track_key,
                title: row.title,
                artist: row.artist,
                album: row.album,
                genre: row.genre,
                duration_seconds: row.duration_seconds,
                device_id: row.device_id,
                device_name: row.device_name,
                started_at_ms: row.started_at_ms,
                ended_at_ms: row.ended_at_ms,
                listened_seconds: row.listened_seconds,
                registered_play: row.registered_play,
                registered_at_ms: row.registered_at_ms,
                outcome: row.outcome,
            })
            .collect();
        let top_tracks = top_rows
            .into_iter()
            .map(|row| HistoryTopTrack {
                track: resolved_by_key.get(&row.track_key).cloned(),
                track_key: row.track_key,
                title: row.title,
                artist: row.artist,
                album: row.album,
                plays: row.plays,
                listened_seconds: row.listened_seconds,
                last_played_at_ms: row.last_played_at_ms,
            })
            .collect();
        devices.sort_by(|left, right| {
            right
                .is_this_device
                .cmp(&left.is_this_device)
                .then_with(|| left.device_name.cmp(&right.device_name))
        });
        let warning = self
            .shared
            .lock()
            .ok()
            .and_then(|shared| shared.last_error.clone())
            .or(peer_warning);
        let sync_state = if warning.is_some() {
            "unavailable"
        } else {
            "synced"
        };
        Ok(HistoryPage {
            items,
            summary,
            top_tracks,
            devices,
            next_cursor,
            play_threshold_seconds: self.play_threshold_seconds()?,
            sync_state,
            sync_message: warning.unwrap_or(sync_message),
        })
    }

    pub(crate) fn track_insight(&self, track_key: &str) -> Result<TrackHistoryInsight, String> {
        if track_key.trim().is_empty() || track_key.len() > 2_048 {
            return Err("Listening-history track identity is invalid.".to_owned());
        }
        let mut insight = TrackHistoryInsight::default();
        for source in self.available_sources() {
            let (metadata, connection) = match open_valid_history_source(&source) {
                Ok(Some(value)) => value,
                Ok(None) => continue,
                Err(_) if source != self.path => continue,
                Err(error) => return Err(error),
            };
            if source != self.path && metadata.device_id == self.device_id {
                continue;
            }
            let (sessions, plays, skips, listened_seconds, last_listened_at_ms): (
                i64,
                i64,
                i64,
                f64,
                Option<i64>,
            ) = connection
                .query_row(
                    r#"
                    SELECT COUNT(*),
                           COALESCE(SUM(registered_play), 0),
                           COALESCE(SUM(CASE WHEN outcome = 'skipped' THEN 1 ELSE 0 END), 0),
                           COALESCE(SUM(listened_seconds), 0),
                           MAX(started_at_ms)
                    FROM listening_sessions
                    WHERE track_key = ?1
                    "#,
                    [track_key],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .map_err(|error| {
                    format!("Could not summarize this track's listening history: {error}")
                })?;
            insight.sessions = insight.sessions.saturating_add(sessions);
            insight.plays = insight.plays.saturating_add(plays);
            insight.skips = insight.skips.saturating_add(skips);
            insight.listened_seconds += listened_seconds.max(0.0);
            insight.last_listened_at_ms = match (insight.last_listened_at_ms, last_listened_at_ms) {
                (Some(current), Some(candidate)) => Some(current.max(candidate)),
                (None, candidate) => candidate,
                (current, None) => current,
            };
        }
        Ok(insight)
    }

    pub(crate) fn genre_insights(&self) -> Result<HashMap<String, GenreHistoryInsight>, String> {
        let mut insights: HashMap<String, GenreHistoryInsight> = HashMap::new();
        for source in self.available_sources() {
            let (metadata, connection) = match open_valid_history_source(&source) {
                Ok(Some(value)) => value,
                Ok(None) => continue,
                Err(_) if source != self.path => continue,
                Err(error) => return Err(error),
            };
            if source != self.path && metadata.device_id == self.device_id {
                continue;
            }
            let mut statement = connection
                .prepare(
                    r#"
                    SELECT TRIM(genre), COUNT(*), COALESCE(SUM(registered_play), 0),
                           COALESCE(SUM(listened_seconds), 0), MAX(started_at_ms)
                    FROM listening_sessions
                    WHERE NULLIF(TRIM(genre), '') IS NOT NULL
                    GROUP BY TRIM(genre)
                    "#,
                )
                .map_err(|error| format!("Could not prepare Aurora's genre history: {error}"))?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        GenreHistoryInsight {
                            sessions: row.get(1)?,
                            plays: row.get(2)?,
                            listened_seconds: row.get::<_, f64>(3)?.max(0.0),
                            last_listened_at_ms: row.get(4)?,
                        },
                    ))
                })
                .map_err(|error| format!("Could not read Aurora's genre history: {error}"))?;
            for row in rows {
                let (genre, candidate) = row
                    .map_err(|error| format!("Could not decode Aurora's genre history: {error}"))?;
                let combined = insights.entry(genre_identity(&genre)).or_default();
                combined.sessions = combined.sessions.saturating_add(candidate.sessions);
                combined.plays = combined.plays.saturating_add(candidate.plays);
                combined.listened_seconds += candidate.listened_seconds;
                combined.last_listened_at_ms =
                    match (combined.last_listened_at_ms, candidate.last_listened_at_ms) {
                        (Some(current), Some(next)) => Some(current.max(next)),
                        (None, next) => next,
                        (current, None) => current,
                    };
            }
        }
        Ok(insights)
    }

    pub(crate) fn played_track_keys_for_genre(
        &self,
        genre: &str,
    ) -> Result<HashSet<String>, String> {
        if genre.trim().is_empty() || genre.chars().count() > 256 {
            return Err("Genre selection is invalid.".to_owned());
        }
        let mut keys = HashSet::new();
        for source in self.available_sources() {
            let (metadata, connection) = match open_valid_history_source(&source) {
                Ok(Some(value)) => value,
                Ok(None) => continue,
                Err(_) if source != self.path => continue,
                Err(error) => return Err(error),
            };
            if source != self.path && metadata.device_id == self.device_id {
                continue;
            }
            let mut statement = connection
                .prepare(
                    "SELECT DISTINCT track_key FROM listening_sessions \
                     WHERE registered_play = 1 AND TRIM(genre) = TRIM(?1) COLLATE NOCASE",
                )
                .map_err(|error| {
                    format!("Could not prepare Aurora's played genre tracks: {error}")
                })?;
            let rows = statement
                .query_map([genre], |row| row.get::<_, String>(0))
                .map_err(|error| format!("Could not read Aurora's played genre tracks: {error}"))?;
            for row in rows {
                keys.insert(row.map_err(|error| {
                    format!("Could not decode Aurora's played genre tracks: {error}")
                })?);
            }
        }
        Ok(keys)
    }

    fn available_sources(&self) -> Vec<PathBuf> {
        let mut sources = vec![self.path.clone()];
        if !self.remote_directory.is_dir() {
            return sources;
        }
        let Ok(entries) = fs::read_dir(&self.remote_directory) else {
            return sources;
        };
        let mut remote = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.starts_with("aurora-history-") && name.ends_with(".sqlite3")
                    })
            })
            .take(MAX_HISTORY_SOURCES)
            .collect::<Vec<_>>();
        remote.sort();
        sources.extend(remote);
        sources
    }

    pub(crate) fn publish_if_due(&self, force: bool) -> Result<String, String> {
        {
            let shared = self
                .shared
                .lock()
                .map_err(|_| "Aurora's history mirror stopped unexpectedly.".to_owned())?;
            if shared.remote_write_blocked {
                return Err(shared.last_error.clone().unwrap_or_else(|| {
                    "Aurora will not overwrite this device's unreadable OneDrive history snapshot."
                        .to_owned()
                }));
            }
        }
        if !self.remote_directory.is_dir() {
            return Err(
                "The OneDrive _musicbackup folder is unavailable; local listening history remains safe."
                    .to_owned(),
            );
        }
        let metadata = read_history_metadata(&self.path)?;
        if metadata.content_revision == metadata.mirrored_revision && self.remote_path.is_file() {
            if let Ok(mut shared) = self.shared.lock() {
                shared.last_error = None;
            }
            return Ok("Listening history is mirrored to OneDrive.".to_owned());
        }
        let now = state_sync::now_ms();
        {
            let mut shared = self
                .shared
                .lock()
                .map_err(|_| "Aurora's history mirror stopped unexpectedly.".to_owned())?;
            if !force
                && shared
                    .last_publish_attempt_ms
                    .is_some_and(|last| now.saturating_sub(last) < HISTORY_SYNC_INTERVAL_MS)
            {
                return Ok(
                    "Listening history is waiting for its next OneDrive snapshot.".to_owned(),
                );
            }
            shared.last_publish_attempt_ms = Some(now);
        }
        let temporary = self.remote_directory.join(format!(
            ".aurora-history-{}-{now}.tmp.sqlite3",
            self.device_id
        ));
        if temporary.exists() {
            return Err("Aurora's history snapshot staging path already exists.".to_owned());
        }
        let publish_result = (|| {
            state_sync::consistent_copy(&self.path, &temporary)?;
            {
                let connection = Connection::open(&temporary).map_err(|error| {
                    format!("Could not open Aurora's staged history snapshot: {error}")
                })?;
                connection
                    .execute(
                        r#"
                        UPDATE history_meta
                        SET mirrored_revision = content_revision, last_synced_at_ms = ?1
                        WHERE singleton = 1
                        "#,
                        [now],
                    )
                    .map_err(|error| {
                        format!("Could not seal Aurora's history snapshot: {error}")
                    })?;
            }
            validate_history_database(&temporary, Some(&self.device_id))?;
            if self.remote_path.is_file() {
                state_sync::replace_file_atomic(&self.remote_path, &temporary)
            } else {
                fs::rename(&temporary, &self.remote_path).map_err(|error| {
                    format!("Could not publish Aurora's OneDrive history snapshot: {error}")
                })
            }
        })();
        if publish_result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        publish_result?;
        let connection = self.open()?;
        connection
            .execute(
                r#"
                UPDATE history_meta
                SET mirrored_revision = content_revision, last_synced_at_ms = ?1
                WHERE singleton = 1 AND content_revision = ?2
                "#,
                params![now, metadata.content_revision],
            )
            .map_err(|error| format!("Could not checkpoint Aurora's history mirror: {error}"))?;
        if let Ok(mut shared) = self.shared.lock() {
            shared.last_error = None;
        }
        Ok("Saved this device's listening history to OneDrive.".to_owned())
    }
}

pub(crate) fn genre_identity(genre: &str) -> String {
    genre.trim().to_lowercase()
}

fn query_top_rows(connection: &Connection, limit: usize) -> Result<Vec<HistoryTopRow>, String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT track_key, title, artist, album, directory, filename,
                   COUNT(*), COALESCE(SUM(listened_seconds), 0), MAX(started_at_ms)
            FROM listening_sessions
            WHERE registered_play = 1
            GROUP BY track_key
            ORDER BY COUNT(*) DESC, SUM(listened_seconds) DESC, MAX(started_at_ms) DESC
            LIMIT ?1
            "#,
        )
        .map_err(|error| format!("Could not prepare Aurora's top listening history: {error}"))?;
    statement
        .query_map([limit as i64], |row| {
            Ok(HistoryTopRow {
                track_key: row.get(0)?,
                title: row.get(1)?,
                artist: row.get(2)?,
                album: row.get(3)?,
                directory: row.get(4)?,
                filename: row.get(5)?,
                plays: row.get(6)?,
                listened_seconds: row.get::<_, f64>(7)?.max(0.0),
                last_played_at_ms: row.get(8)?,
            })
        })
        .map_err(|error| format!("Could not read Aurora's top listening history: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode Aurora's top listening history: {error}"))
}

fn validate_request(request: &HistoryPageRequest) -> Result<(), String> {
    if request.page_size == 0 || request.page_size > MAX_HISTORY_PAGE_SIZE {
        return Err(format!(
            "History pages must contain between 1 and {MAX_HISTORY_PAGE_SIZE} sessions."
        ));
    }
    if request
        .search
        .as_deref()
        .is_some_and(|value| value.len() > 200)
    {
        return Err("History search is too long.".to_owned());
    }
    if request
        .device_id
        .as_deref()
        .is_some_and(|value| !valid_device_id(value))
    {
        return Err("History device filter is invalid.".to_owned());
    }
    if !matches!(
        request.outcome.as_deref().unwrap_or("all"),
        "all" | "played" | "completed" | "skipped" | "interrupted"
    ) {
        return Err("History outcome filter is invalid.".to_owned());
    }
    if request.started_after_ms.is_some_and(|value| value < 0)
        || request.started_before_ms.is_some_and(|value| value < 0)
        || matches!(
            (request.started_after_ms, request.started_before_ms),
            (Some(after), Some(before)) if after > before
        )
    {
        return Err("History date range is invalid.".to_owned());
    }
    if let Some(cursor) = request.cursor.as_deref() {
        parse_cursor(cursor)?;
    }
    Ok(())
}

fn parse_cursor(cursor: &str) -> Result<(i64, String), String> {
    let (started, session_id) = cursor
        .split_once(':')
        .ok_or_else(|| "History cursor is invalid.".to_owned())?;
    let started = started
        .parse::<i64>()
        .ok()
        .filter(|value| *value >= 0)
        .ok_or_else(|| "History cursor is invalid.".to_owned())?;
    if session_id.is_empty()
        || session_id.len() > 180
        || !session_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err("History cursor is invalid.".to_owned());
    }
    Ok((started, session_id.to_owned()))
}

fn query_rows(
    connection: &Connection,
    metadata: &HistoryMetadata,
    request: &HistoryPageRequest,
    cursor: Option<&(i64, String)>,
    limit: usize,
) -> Result<Vec<HistoryRow>, String> {
    let search = request
        .search
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let outcome = request.outcome.as_deref().unwrap_or("all");
    let cursor_started = cursor.map(|value| value.0);
    let cursor_id = cursor.map(|value| value.1.as_str());
    let mut statement = connection
        .prepare(
            r#"
            SELECT session_id, track_key, title, artist, album, genre,
                   directory, filename, duration_seconds, started_at_ms,
                   ended_at_ms, listened_seconds, registered_play,
                   registered_at_ms, outcome
            FROM listening_sessions
            WHERE (?1 IS NULL OR
                   instr(lower(title || ' ' || artist || ' ' || album || ' ' || COALESCE(genre, '')), lower(?1)) > 0)
              AND (?2 = 'all'
                   OR (?2 = 'played' AND registered_play = 1)
                   OR outcome = ?2)
              AND (?3 IS NULL OR started_at_ms >= ?3)
              AND (?4 IS NULL OR started_at_ms <= ?4)
              AND (?5 IS NULL OR started_at_ms < ?5
                   OR (started_at_ms = ?5 AND session_id < ?6))
            ORDER BY started_at_ms DESC, session_id DESC
            LIMIT ?7
            "#,
        )
        .map_err(|error| format!("Could not prepare Aurora's history page: {error}"))?;
    statement
        .query_map(
            params![
                search,
                outcome,
                request.started_after_ms,
                request.started_before_ms,
                cursor_started,
                cursor_id,
                limit as i64,
            ],
            |row| {
                Ok(HistoryRow {
                    session_id: row.get(0)?,
                    track_key: row.get(1)?,
                    title: row.get(2)?,
                    artist: row.get(3)?,
                    album: row.get(4)?,
                    genre: row.get(5)?,
                    directory: row.get(6)?,
                    filename: row.get(7)?,
                    duration_seconds: row.get(8)?,
                    device_id: metadata.device_id.clone(),
                    device_name: metadata.device_name.clone(),
                    started_at_ms: row.get(9)?,
                    ended_at_ms: row.get(10)?,
                    listened_seconds: row.get::<_, f64>(11)?.max(0.0),
                    registered_play: row.get::<_, i64>(12)? == 1,
                    registered_at_ms: row.get(13)?,
                    outcome: row.get(14)?,
                })
            },
        )
        .map_err(|error| format!("Could not read Aurora's history page: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode Aurora's history page: {error}"))
}

fn source_counts(connection: &Connection) -> Result<(i64, Option<i64>), String> {
    connection
        .query_row(
            "SELECT COUNT(*), MAX(started_at_ms) FROM listening_sessions",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| format!("Could not summarize an Aurora history source: {error}"))
}

fn accumulate_summary(
    connection: &Connection,
    summary: &mut HistorySummary,
    unique_tracks: &mut HashSet<String>,
) -> Result<(), String> {
    let (sessions, plays, skips, listened): (i64, i64, i64, f64) = connection
        .query_row(
            r#"
            SELECT COUNT(*),
                   COALESCE(SUM(registered_play), 0),
                   COALESCE(SUM(CASE WHEN outcome = 'skipped' THEN 1 ELSE 0 END), 0),
                   COALESCE(SUM(listened_seconds), 0)
            FROM listening_sessions
            "#,
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|error| format!("Could not summarize Aurora's listening history: {error}"))?;
    summary.sessions = summary.sessions.saturating_add(sessions);
    summary.plays = summary.plays.saturating_add(plays);
    summary.skips = summary.skips.saturating_add(skips);
    summary.listened_seconds += listened.max(0.0);
    let mut statement = connection
        .prepare("SELECT DISTINCT track_key FROM listening_sessions WHERE registered_play = 1")
        .map_err(|error| format!("Could not prepare Aurora's unique-track history: {error}"))?;
    let keys = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| format!("Could not read Aurora's unique-track history: {error}"))?;
    for key in keys {
        unique_tracks.insert(
            key.map_err(|error| format!("Could not decode Aurora's track history: {error}"))?,
        );
    }
    Ok(())
}

fn open_valid_history_source(path: &Path) -> Result<Option<(HistoryMetadata, Connection)>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|error| format!("Could not open an Aurora history snapshot: {error}"))?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|error| format!("Could not configure an Aurora history snapshot: {error}"))?;
    connection
        .pragma_update(None, "query_only", true)
        .map_err(|error| format!("Could not protect an Aurora history snapshot: {error}"))?;
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|error| format!("Could not inspect an Aurora history snapshot: {error}"))?;
    if version != HISTORY_SCHEMA_VERSION {
        return Ok(None);
    }
    let metadata = read_metadata(&connection)?;
    Ok(Some((metadata, connection)))
}

fn read_history_metadata(path: &Path) -> Result<HistoryMetadata, String> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|error| format!("Could not open Aurora's history metadata: {error}"))?;
    read_metadata(&connection)
}

fn read_metadata(connection: &Connection) -> Result<HistoryMetadata, String> {
    connection
        .query_row(
            r#"
            SELECT device_id, device_name, content_revision, mirrored_revision
            FROM history_meta WHERE singleton = 1
            "#,
            [],
            |row| {
                Ok(HistoryMetadata {
                    device_id: row.get(0)?,
                    device_name: row.get(1)?,
                    content_revision: row.get(2)?,
                    mirrored_revision: row.get(3)?,
                })
            },
        )
        .map_err(|error| format!("Could not read Aurora's history metadata: {error}"))
}

fn validate_history_database(path: &Path, expected_device: Option<&str>) -> Result<(), String> {
    let Some((metadata, connection)) = open_valid_history_source(path)? else {
        return Err("The file is not a supported Aurora history database.".to_owned());
    };
    if expected_device.is_some_and(|expected| expected != metadata.device_id) {
        return Err("Aurora's history snapshot belongs to another device.".to_owned());
    }
    let quick_check: String = connection
        .pragma_query_value(None, "quick_check", |row| row.get(0))
        .map_err(|error| format!("Could not validate Aurora's history snapshot: {error}"))?;
    if quick_check != "ok" {
        return Err(format!(
            "Aurora's history snapshot failed SQLite validation: {quick_check}"
        ));
    }
    Ok(())
}

fn restore_local_history(remote: &Path, local: &Path, device_id: &str) -> Result<(), String> {
    validate_history_database(remote, Some(device_id))?;
    let parent = local
        .parent()
        .ok_or_else(|| "Aurora's local history path has no parent directory.".to_owned())?;
    let temporary = parent.join(format!(".aurora-history-restore-{device_id}.tmp.sqlite3"));
    if temporary.exists() {
        return Err("Aurora's history restore staging path already exists.".to_owned());
    }
    let result = (|| {
        fs::copy(remote, &temporary)
            .map_err(|error| format!("Could not restore Aurora history from OneDrive: {error}"))?;
        File::options()
            .read(true)
            .write(true)
            .open(&temporary)
            .and_then(|file| file.sync_all())
            .map_err(|error| format!("Could not flush Aurora's restored history: {error}"))?;
        validate_history_database(&temporary, Some(device_id))?;
        fs::rename(&temporary, local)
            .map_err(|error| format!("Could not install Aurora's restored history: {error}"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn effective_threshold_seconds(configured: u32, duration_seconds: Option<i64>) -> f64 {
    let configured = configured.clamp(MIN_PLAY_THRESHOLD_SECONDS, MAX_PLAY_THRESHOLD_SECONDS);
    duration_seconds
        .filter(|duration| *duration > 0)
        .map_or(configured as f64, |duration| {
            (configured as f64).min(duration as f64).max(1.0)
        })
}

fn valid_device_id(value: &str) -> bool {
    (8..=96).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn next_session_sequence() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    SEQUENCE.fetch_add(1, Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tag_model::LoveState;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "aurora-history-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ))
    }

    fn track(duration_seconds: i64) -> TrackSummary {
        TrackSummary {
            id: "42".to_owned(),
            track_key: r"d:\music\artist\track.mp3".to_owned(),
            album_id: Some("album-1".to_owned()),
            title: "Track".to_owned(),
            artist: "Artist".to_owned(),
            display_artist: None,
            album: "Album".to_owned(),
            release_year: Some(2024),
            original_year: Some(2024),
            publisher: None,
            rating: None,
            loved: false,
            love_state: LoveState::Neutral,
            tag_sync_state: None,
            can_undo_tag_edit: false,
            duration_seconds: Some(duration_seconds),
            genre: Some("Electronic".to_owned()),
            play_count: None,
            track_number: None,
            track_total: None,
            disc_number: None,
            disc_total: None,
            directory: r"D:\MUSIC\Artist".to_owned(),
            filename: "Track.mp3".to_owned(),
            catalog_import_run_id: 1,
        }
    }

    #[test]
    fn configurable_threshold_counts_forward_listening_and_not_seeks() {
        let root = temporary_root("threshold");
        let remote = root.join("remote");
        fs::create_dir_all(&remote).expect("remote directory");
        let store = HistoryStore::new(
            root.join("history.sqlite3"),
            remote,
            "device-test-1234".to_owned(),
            "Test computer".to_owned(),
        )
        .expect("history store");
        assert_eq!(
            store.play_threshold_seconds().expect("default threshold"),
            30
        );
        store
            .set_play_threshold_seconds(12)
            .expect("custom threshold");
        let mut active = store.begin_session(&track(240), 0.0).expect("session");
        store
            .observe_position(&mut active, 7.0)
            .expect("listen seven seconds");
        assert!(!active.registered_play);
        store.reset_position(&mut active, 220.0);
        store
            .observe_position(&mut active, 224.0)
            .expect("listen after seek");
        assert!(!active.registered_play);
        store
            .observe_position(&mut active, 225.0)
            .expect("cross threshold");
        assert!(active.registered_play);
        store
            .finish_session(&active, "skipped")
            .expect("finish session");

        let connection = store.open().expect("read session");
        let (listened, registered): (f64, i64) = connection
            .query_row(
                "SELECT listened_seconds, registered_play FROM listening_sessions",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("saved session");
        assert_eq!(listened, 12.0);
        assert_eq!(registered, 1);
        drop(connection);
        let insight = store
            .track_insight(&track(240).track_key)
            .expect("track insight");
        assert_eq!(insight.sessions, 1);
        assert_eq!(insight.plays, 1);
        assert_eq!(insight.skips, 1);
        assert_eq!(insight.listened_seconds, 12.0);
        assert!(insight.last_listened_at_ms.is_some());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn short_tracks_register_at_natural_duration_and_crashes_recover() {
        let root = temporary_root("short-track");
        let remote = root.join("remote");
        fs::create_dir_all(&remote).expect("remote directory");
        let path = root.join("history.sqlite3");
        let store = HistoryStore::new(
            path.clone(),
            remote.clone(),
            "device-test-5678".to_owned(),
            "Test computer".to_owned(),
        )
        .expect("history store");
        let mut short = store.begin_session(&track(8), 0.0).expect("short session");
        store
            .observe_position(&mut short, 8.0)
            .expect("finish short track");
        assert!(short.registered_play);
        let _interrupted = store
            .begin_session(&track(240), 0.0)
            .expect("active session");
        drop(store);

        let restored = HistoryStore::new(
            path,
            remote,
            "device-test-5678".to_owned(),
            "Test computer".to_owned(),
        )
        .expect("restored history");
        let active: i64 = restored
            .open()
            .expect("read restored")
            .query_row(
                "SELECT COUNT(*) FROM listening_sessions WHERE outcome = 'active'",
                [],
                |row| row.get(0),
            )
            .expect("active count");
        assert_eq!(active, 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn active_threshold_changes_apply_without_retracting_a_registered_play() {
        let root = temporary_root("active-threshold");
        let remote = root.join("remote");
        fs::create_dir_all(&remote).expect("remote directory");
        let store = HistoryStore::new(
            root.join("history.sqlite3"),
            remote,
            "device-test-9012".to_owned(),
            "Test computer".to_owned(),
        )
        .expect("history store");
        let mut active = store.begin_session(&track(240), 0.0).expect("session");
        store
            .observe_position(&mut active, 7.0)
            .expect("listen seven seconds");
        assert!(!active.registered_play);

        store
            .set_play_threshold_seconds(6)
            .expect("lower threshold");
        store
            .refresh_active_threshold(&mut active, 6)
            .expect("apply lower threshold");
        assert!(active.registered_play);

        store
            .set_play_threshold_seconds(60)
            .expect("raise threshold");
        store
            .refresh_active_threshold(&mut active, 60)
            .expect("apply higher threshold");
        assert!(active.registered_play);
        store
            .finish_session(&active, "interrupted")
            .expect("finish session");

        let (threshold, registered): (i64, i64) = store
            .open()
            .expect("read session")
            .query_row(
                "SELECT threshold_seconds, registered_play FROM listening_sessions",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("saved threshold");
        assert_eq!(threshold, 60);
        assert_eq!(registered, 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unreadable_remote_restore_never_blocks_local_history_or_gets_overwritten() {
        let root = temporary_root("unreadable-remote");
        let remote = root.join("remote");
        fs::create_dir_all(&remote).expect("remote directory");
        let device_id = "device-test-3456";
        let remote_path = remote.join(format!("aurora-history-{device_id}.sqlite3"));
        fs::write(&remote_path, b"not a sqlite database").expect("broken snapshot");

        let store = HistoryStore::new(
            root.join("history.sqlite3"),
            remote,
            device_id.to_owned(),
            "Test computer".to_owned(),
        )
        .expect("local history remains available");
        assert_eq!(store.play_threshold_seconds().expect("local threshold"), 30);
        assert!(store.publish_if_due(true).is_err());
        assert_eq!(
            fs::read(&remote_path).expect("preserved broken snapshot"),
            b"not a sqlite database"
        );
        let _ = fs::remove_dir_all(root);
    }
}
