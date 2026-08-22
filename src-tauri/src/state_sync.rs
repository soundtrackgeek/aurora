use crate::state_store::{SCHEMA_VERSION, StateStore};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde::Serialize;
use std::{
    env,
    ffi::c_void,
    fs::{self, File},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const SYNC_INTERVAL_MS: i64 = 60_000;
static TOKEN_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum StartupSyncOutcome {
    None,
    Restored,
    Updated,
    Conflict(String),
    Unavailable(String),
}

#[derive(Clone, Debug, PartialEq)]
struct SyncMetadata {
    lineage_id: String,
    snapshot_id: String,
    generation: i64,
    content_revision: i64,
    mirrored_revision: i64,
    last_synced_at_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StateMirrorStatus {
    pub(crate) sync_state: &'static str,
    pub(crate) message: String,
    pub(crate) remote_path: String,
    pub(crate) last_synced_at_ms: Option<i64>,
}

pub(crate) fn default_remote_state_path() -> Result<PathBuf, String> {
    let profile = env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .ok_or_else(|| "Windows USERPROFILE is unavailable.".to_owned())?;
    Ok(profile
        .join("OneDrive")
        .join("_musicbackup")
        .join("aurora-state.sqlite3"))
}

pub(crate) fn prepare_state_before_open(
    local_path: &Path,
    remote_path: &Path,
) -> StartupSyncOutcome {
    if !remote_path.is_file() {
        return StartupSyncOutcome::None;
    }
    let remote = match validate_database(remote_path) {
        Ok(metadata) => metadata,
        Err(error) => return StartupSyncOutcome::Unavailable(error),
    };
    if !local_path.is_file() {
        return match copy_snapshot_to_new_local(remote_path, local_path) {
            Ok(()) => StartupSyncOutcome::Restored,
            Err(error) => StartupSyncOutcome::Unavailable(error),
        };
    }
    let local = match validate_database(local_path) {
        Ok(metadata) => metadata,
        Err(error) => return StartupSyncOutcome::Unavailable(error),
    };
    let (Some(local), Some(remote)) = (local, remote) else {
        return StartupSyncOutcome::None;
    };
    if local.lineage_id != remote.lineage_id {
        return StartupSyncOutcome::Conflict(
            "The local and OneDrive state files have unrelated histories. Aurora left both untouched."
                .to_owned(),
        );
    }
    if remote.generation > local.generation {
        if local.content_revision != local.mirrored_revision {
            return StartupSyncOutcome::Conflict(
                "Both this device and OneDrive contain newer Aurora changes. Aurora left both untouched."
                    .to_owned(),
            );
        }
        return match replace_closed_local(remote_path, local_path) {
            Ok(()) => StartupSyncOutcome::Updated,
            Err(error) => StartupSyncOutcome::Unavailable(error),
        };
    }
    if remote.generation == local.generation && remote.snapshot_id == local.snapshot_id {
        StartupSyncOutcome::None
    } else {
        StartupSyncOutcome::Conflict(
            "The local and OneDrive state files do not share the same latest snapshot. Aurora left both untouched."
                .to_owned(),
        )
    }
}

pub(crate) struct StateSyncService {
    store: StateStore,
    remote_path: PathBuf,
    startup_outcome: StartupSyncOutcome,
    last_publish_attempt_ms: Option<i64>,
    allow_legacy_replace: bool,
}

impl StateSyncService {
    pub(crate) fn new(
        store: StateStore,
        remote_path: PathBuf,
        startup_outcome: StartupSyncOutcome,
    ) -> Result<Self, String> {
        ensure_sync_identity(&store)?;
        let allow_legacy_replace = matches!(startup_outcome, StartupSyncOutcome::Restored);
        Ok(Self {
            store,
            remote_path,
            startup_outcome,
            last_publish_attempt_ms: None,
            allow_legacy_replace,
        })
    }

    pub(crate) fn sync_now(&mut self, bypass_throttle: bool) -> StateMirrorStatus {
        match self.try_sync(bypass_throttle) {
            Ok(status) => status,
            Err(error) => self.status("unavailable", error, None),
        }
    }

    fn try_sync(&mut self, bypass_throttle: bool) -> Result<StateMirrorStatus, String> {
        let startup_conflict = match &self.startup_outcome {
            StartupSyncOutcome::Conflict(message) => Some(message.clone()),
            _ => None,
        };
        if let StartupSyncOutcome::Unavailable(message) = &self.startup_outcome {
            let message = message.clone();
            self.startup_outcome = StartupSyncOutcome::None;
            return Ok(self.status("unavailable", message, None));
        }
        let remote_parent = self
            .remote_path
            .parent()
            .ok_or_else(|| "Aurora's OneDrive state path has no parent directory.".to_owned())?;
        if !remote_parent.is_dir() {
            return Ok(self.status(
                "unavailable",
                "The OneDrive _musicbackup folder is unavailable. Aurora will retry without blocking the library."
                    .to_owned(),
                None,
            ));
        }

        let local = read_required_metadata(self.store.path())?;
        let remote_exists = self.remote_path.is_file();
        let remote = if remote_exists {
            validate_database(&self.remote_path)?
        } else {
            None
        };

        if !remote_exists && let Some(message) = startup_conflict {
            return Ok(self.status("conflict", message, local.last_synced_at_ms));
        }

        if remote_exists && remote.is_none() && !self.allow_legacy_replace {
            return Ok(self.status(
                "conflict",
                "The OneDrive state file predates safe snapshot lineage. Aurora left it untouched."
                    .to_owned(),
                local.last_synced_at_ms,
            ));
        }
        if let Some(remote) = &remote {
            if remote.lineage_id != local.lineage_id {
                return Ok(self.status(
                    "conflict",
                    "This device and OneDrive have unrelated Aurora state histories. Both files were left untouched."
                        .to_owned(),
                    local.last_synced_at_ms,
                ));
            }
            let snapshot_matches =
                remote.generation == local.generation && remote.snapshot_id == local.snapshot_id;
            if !snapshot_matches && semantic_state_matches(self.store.path(), &self.remote_path)? {
                let reconciled = adopt_remote_snapshot_identity(&self.store, &local, remote)?;
                self.startup_outcome = StartupSyncOutcome::None;
                self.allow_legacy_replace = false;
                return Ok(self.status(
                    "synced",
                    "Aurora reconciled equivalent state from both computers; only device-local catalog bookkeeping differed."
                        .to_owned(),
                    reconciled.last_synced_at_ms,
                ));
            }
            if remote.generation > local.generation {
                let dirty = local.content_revision != local.mirrored_revision;
                return Ok(self.status(
                    if dirty { "conflict" } else { "remoteUpdate" },
                    if dirty {
                        "Aurora detected changes on both computers. Close Aurora on one machine and resolve which state to keep; neither file was overwritten."
                            .to_owned()
                    } else {
                        "OneDrive has a newer Aurora state snapshot. Restart Aurora to apply it safely."
                            .to_owned()
                    },
                    remote.last_synced_at_ms,
                ));
            }
            if remote.generation < local.generation
                || (remote.generation == local.generation
                    && remote.snapshot_id != local.snapshot_id)
            {
                return Ok(self.status(
                    "conflict",
                    "OneDrive does not contain the snapshot this device last published. Aurora is waiting rather than overwriting it."
                        .to_owned(),
                    local.last_synced_at_ms,
                ));
            }
        }

        let dirty = local.content_revision != local.mirrored_revision;
        if remote_exists && !dirty {
            let message = match self.startup_outcome {
                StartupSyncOutcome::Restored => {
                    "Restored Aurora state from OneDrive and verified the local copy."
                }
                StartupSyncOutcome::Updated => {
                    "Applied the newer OneDrive state snapshot during startup."
                }
                _ => "Aurora state matches the verified OneDrive snapshot.",
            };
            self.startup_outcome = StartupSyncOutcome::None;
            return Ok(self.status("synced", message.to_owned(), local.last_synced_at_ms));
        }

        let now = now_ms();
        if !bypass_throttle
            && dirty
            && self
                .last_publish_attempt_ms
                .is_some_and(|last| now.saturating_sub(last) < SYNC_INTERVAL_MS)
        {
            return Ok(self.status(
                "pending",
                "Local Aurora changes are waiting for the next consistent OneDrive snapshot."
                    .to_owned(),
                local.last_synced_at_ms,
            ));
        }
        self.last_publish_attempt_ms = Some(now);
        let expected_remote_snapshot = remote
            .as_ref()
            .map(|metadata| metadata.snapshot_id.as_str());
        let result = publish_snapshot(
            &self.store,
            &self.remote_path,
            &local,
            expected_remote_snapshot,
        )?;
        self.allow_legacy_replace = false;
        self.startup_outcome = StartupSyncOutcome::None;
        Ok(self.status(
            "synced",
            "Saved a verified Aurora state snapshot to OneDrive.".to_owned(),
            result.last_synced_at_ms,
        ))
    }

    fn status(
        &self,
        sync_state: &'static str,
        message: String,
        last_synced_at_ms: Option<i64>,
    ) -> StateMirrorStatus {
        StateMirrorStatus {
            sync_state,
            message,
            remote_path: self.remote_path.to_string_lossy().into_owned(),
            last_synced_at_ms,
        }
    }
}

fn ensure_sync_identity(store: &StateStore) -> Result<(), String> {
    let connection = store.open()?;
    let lineage: String = connection
        .query_row(
            "SELECT lineage_id FROM state_sync_meta WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not read Aurora's state lineage: {error}"))?;
    if lineage.is_empty() {
        connection
            .execute(
                "UPDATE state_sync_meta SET lineage_id = ?1 WHERE singleton = 1",
                [new_token("lineage")],
            )
            .map_err(|error| format!("Could not initialize Aurora's state lineage: {error}"))?;
    }
    Ok(())
}

fn semantic_state_matches(local_path: &Path, remote_path: &Path) -> Result<bool, String> {
    let connection = Connection::open_with_flags(
        local_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|error| format!("Could not inspect Aurora's local sync state: {error}"))?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|error| format!("Could not configure Aurora's state comparison: {error}"))?;
    connection
        .pragma_update(None, "query_only", true)
        .map_err(|error| format!("Could not protect Aurora's state comparison: {error}"))?;
    connection
        .execute(
            "ATTACH DATABASE ?1 AS remote_state",
            [remote_path.to_string_lossy().as_ref()],
        )
        .map_err(|error| {
            format!("Could not attach Aurora's OneDrive snapshot read-only: {error}")
        })?;

    for (table, columns) in [
        ("playback_queue", "position, track_key, directory, filename"),
        (
            "playback_state",
            "singleton, current_index, volume, shuffle, repeat_mode",
        ),
        (
            "tag_overlays",
            "track_key, rating, love_state, release_year, last_operation_id",
        ),
        (
            "tag_edit_operations",
            "id, track_key, target_path, temp_path, backup_path, before_rating, before_love_state, before_release_year, after_rating, after_love_state, after_release_year, source_fingerprint, status, created_at_ms, updated_at_ms, error_message",
        ),
        (
            "musicbrainz_artist_decisions",
            "local_artist_key, display_artist, decision, artist_mbid, canonical_name, created_at_ms, updated_at_ms",
        ),
        (
            "musicbrainz_release_decisions",
            "local_artist_key, display_artist, artist_mbid, release_mbid, decision, local_album_id, created_at_ms, updated_at_ms",
        ),
        (
            "musicbrainz_curation_events",
            "id, entity_kind, local_artist_key, artist_mbid, release_mbid, before_json, after_json, created_at_ms",
        ),
    ] {
        let differs: bool = connection
            .query_row(
                &format!(
                    r#"
                    SELECT EXISTS(
                      SELECT {columns} FROM main.{table}
                      EXCEPT
                      SELECT {columns} FROM remote_state.{table}
                    ) OR EXISTS(
                      SELECT {columns} FROM remote_state.{table}
                      EXCEPT
                      SELECT {columns} FROM main.{table}
                    )
                    "#
                ),
                [],
                |row| row.get(0),
            )
            .map_err(|error| {
                format!("Could not compare Aurora's {table} state across computers: {error}")
            })?;
        if differs {
            return Ok(false);
        }
    }
    Ok(true)
}

fn adopt_remote_snapshot_identity(
    store: &StateStore,
    local: &SyncMetadata,
    remote: &SyncMetadata,
) -> Result<SyncMetadata, String> {
    let connection = store.open()?;
    let updated = connection
        .execute(
            r#"
            UPDATE state_sync_meta SET
              snapshot_id = ?1, generation = ?2,
              content_revision = ?3, mirrored_revision = ?3,
              last_synced_at_ms = ?4
            WHERE singleton = 1 AND lineage_id = ?5
              AND snapshot_id = ?6 AND generation = ?7
              AND content_revision = ?8 AND mirrored_revision = ?9
            "#,
            params![
                remote.snapshot_id,
                remote.generation,
                remote.content_revision,
                remote.last_synced_at_ms,
                local.lineage_id,
                local.snapshot_id,
                local.generation,
                local.content_revision,
                local.mirrored_revision,
            ],
        )
        .map_err(|error| format!("Could not reconcile Aurora's equivalent state: {error}"))?;
    if updated != 1 {
        return Err(
            "Aurora state changed while equivalent snapshots were reconciled. Aurora will retry without overwriting either file."
                .to_owned(),
        );
    }
    read_required_metadata(store.path())
}

fn publish_snapshot(
    store: &StateStore,
    remote_path: &Path,
    local_before: &SyncMetadata,
    expected_remote_snapshot: Option<&str>,
) -> Result<SyncMetadata, String> {
    let remote_parent = remote_path
        .parent()
        .ok_or_else(|| "Aurora's OneDrive state path has no parent directory.".to_owned())?;
    let temporary = remote_parent.join(format!(".aurora-state-{}.tmp.sqlite3", new_token("sync")));
    if temporary.exists() {
        return Err("Aurora's OneDrive snapshot staging path already exists.".to_owned());
    }
    consistent_copy(store.path(), &temporary)?;
    read_required_metadata(&temporary)?;
    let snapshot_id = new_token("snapshot");
    let synced_at = now_ms();
    let next_generation = local_before.generation.saturating_add(1);
    {
        let connection = Connection::open(&temporary)
            .map_err(|error| format!("Could not open Aurora's staged state snapshot: {error}"))?;
        connection
            .execute(
                r#"
                UPDATE state_sync_meta SET
                  lineage_id = ?1, snapshot_id = ?2, generation = ?3,
                  mirrored_revision = content_revision, last_synced_at_ms = ?4
                WHERE singleton = 1
                "#,
                params![
                    local_before.lineage_id,
                    snapshot_id,
                    next_generation,
                    synced_at
                ],
            )
            .map_err(|error| format!("Could not seal Aurora's state snapshot: {error}"))?;
    }
    validate_database(&temporary)?
        .ok_or_else(|| "Aurora's staged snapshot is missing sync metadata.".to_owned())?;

    let remote_now = if remote_path.is_file() {
        validate_database(remote_path)?
    } else {
        None
    };
    let remote_matches = match (expected_remote_snapshot, remote_now.as_ref()) {
        (None, None) => true,
        (Some(expected), Some(current)) => current.snapshot_id == expected,
        _ => false,
    };
    if !remote_matches {
        let _ = fs::remove_file(&temporary);
        return Err(
            "The OneDrive state changed while Aurora prepared its snapshot. Both versions were retained."
                .to_owned(),
        );
    }

    if remote_path.is_file() {
        preserve_previous_remote(remote_path)?;
        replace_file_atomic(remote_path, &temporary)?;
    } else {
        fs::rename(&temporary, remote_path).map_err(|error| {
            format!("Could not publish Aurora's OneDrive state snapshot: {error}")
        })?;
    }
    let snapshot = read_required_metadata(remote_path)?;
    let connection = store.open()?;
    let updated = connection
        .execute(
            r#"
            UPDATE state_sync_meta SET
              snapshot_id = ?1, generation = ?2, mirrored_revision = ?3,
              last_synced_at_ms = ?4
            WHERE singleton = 1 AND lineage_id = ?5
              AND snapshot_id = ?6 AND generation = ?7
            "#,
            params![
                snapshot.snapshot_id,
                snapshot.generation,
                snapshot.mirrored_revision,
                snapshot.last_synced_at_ms,
                local_before.lineage_id,
                local_before.snapshot_id,
                local_before.generation,
            ],
        )
        .map_err(|error| format!("Could not checkpoint Aurora's published snapshot: {error}"))?;
    if updated != 1 {
        return Err(
            "Aurora published a valid OneDrive snapshot, but local sync metadata changed concurrently. Restart Aurora to reconcile it."
                .to_owned(),
        );
    }
    Ok(snapshot)
}

fn preserve_previous_remote(remote_path: &Path) -> Result<(), String> {
    let previous = remote_path.with_file_name("aurora-state.previous.sqlite3");
    let temporary = remote_path.with_file_name(format!(
        ".aurora-state-previous-{}.tmp.sqlite3",
        new_token("backup")
    ));
    fs::copy(remote_path, &temporary).map_err(|error| {
        format!("Could not preserve the previous OneDrive state snapshot: {error}")
    })?;
    File::options()
        .read(true)
        .write(true)
        .open(&temporary)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("Could not flush the previous OneDrive snapshot: {error}"))?;
    validate_database(&temporary)?;
    if previous.is_file() {
        replace_file_atomic(&previous, &temporary)
    } else {
        fs::rename(&temporary, &previous)
            .map_err(|error| format!("Could not retain the previous OneDrive snapshot: {error}"))
    }
}

fn replace_closed_local(remote_path: &Path, local_path: &Path) -> Result<(), String> {
    let parent = local_path
        .parent()
        .ok_or_else(|| "Aurora's local state path has no parent directory.".to_owned())?;
    let safety = parent.join("aurora-state.pre-onedrive.sqlite3");
    let safety_temp = parent.join(format!(
        ".aurora-state-pre-onedrive-{}.tmp.sqlite3",
        new_token("local-backup")
    ));
    consistent_copy(local_path, &safety_temp)?;
    if safety.is_file() {
        replace_file_atomic(&safety, &safety_temp)?;
    } else {
        fs::rename(&safety_temp, &safety).map_err(|error| {
            format!("Could not retain Aurora's pre-OneDrive state backup: {error}")
        })?;
    }

    let replacement = parent.join(format!(
        ".aurora-state-restore-{}.tmp.sqlite3",
        new_token("restore")
    ));
    fs::copy(remote_path, &replacement)
        .map_err(|error| format!("Could not stage Aurora's newer OneDrive state: {error}"))?;
    File::options()
        .read(true)
        .write(true)
        .open(&replacement)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("Could not flush Aurora's newer OneDrive state: {error}"))?;
    validate_database(&replacement)?;

    checkpoint_and_remove_sidecars(local_path)?;
    replace_file_atomic(local_path, &replacement)
        .map_err(|error| format!("Could not install Aurora's newer OneDrive state: {error}"))
}

fn copy_snapshot_to_new_local(remote_path: &Path, local_path: &Path) -> Result<(), String> {
    let parent = local_path
        .parent()
        .ok_or_else(|| "Aurora's local state path has no parent directory.".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create Aurora's local state folder: {error}"))?;
    let temporary = parent.join(format!(
        ".aurora-state-first-run-{}.tmp.sqlite3",
        new_token("first-run")
    ));
    fs::copy(remote_path, &temporary)
        .map_err(|error| format!("Could not restore Aurora state from OneDrive: {error}"))?;
    File::options()
        .read(true)
        .write(true)
        .open(&temporary)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("Could not flush Aurora's restored state: {error}"))?;
    validate_database(&temporary)?;
    fs::rename(&temporary, local_path)
        .map_err(|error| format!("Could not install Aurora's restored state: {error}"))
}

fn checkpoint_and_remove_sidecars(path: &Path) -> Result<(), String> {
    {
        let connection = Connection::open(path).map_err(|error| {
            format!("Could not open Aurora's local state for checkpoint: {error}")
        })?;
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(|error| format!("Could not checkpoint Aurora's local state: {error}"))?;
    }
    for suffix in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{suffix}", path.to_string_lossy()));
        if sidecar.exists() {
            fs::remove_file(&sidecar).map_err(|error| {
                format!(
                    "Could not remove Aurora's checkpointed {} sidecar: {error}",
                    suffix
                )
            })?;
        }
    }
    Ok(())
}

pub(crate) fn consistent_copy(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        return Err("Aurora's state snapshot destination already exists.".to_owned());
    }
    let connection = Connection::open(source)
        .map_err(|error| format!("Could not open Aurora's local state for backup: {error}"))?;
    connection
        .busy_timeout(std::time::Duration::from_secs(10))
        .map_err(|error| format!("Could not configure Aurora's state backup: {error}"))?;
    connection
        .execute("VACUUM INTO ?1", [destination.to_string_lossy().as_ref()])
        .map_err(|error| format!("Could not create a consistent Aurora state snapshot: {error}"))?;
    Ok(())
}

fn validate_database(path: &Path) -> Result<Option<SyncMetadata>, String> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|error| format!("Could not open the Aurora state snapshot: {error}"))?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|error| format!("Could not configure Aurora's snapshot validation: {error}"))?;
    connection
        .pragma_update(None, "query_only", true)
        .map_err(|error| format!("Could not protect Aurora's snapshot validation: {error}"))?;
    let quick_check: String = connection
        .pragma_query_value(None, "quick_check", |row| row.get(0))
        .map_err(|error| format!("Could not validate the Aurora state snapshot: {error}"))?;
    if quick_check != "ok" {
        return Err(format!(
            "The Aurora state snapshot failed SQLite validation: {quick_check}"
        ));
    }
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|error| format!("Could not read the Aurora snapshot schema: {error}"))?;
    if version > SCHEMA_VERSION {
        return Err(format!(
            "The OneDrive Aurora state uses unsupported schema version {version}."
        ));
    }
    let has_playback: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='playback_state')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not inspect the Aurora state snapshot: {error}"))?;
    if !has_playback {
        return Err("The OneDrive file is not an Aurora state database.".to_owned());
    }
    let has_meta: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='state_sync_meta')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not inspect Aurora's snapshot lineage: {error}"))?;
    if !has_meta {
        return Ok(None);
    }
    read_metadata(&connection).map(Some)
}

fn read_required_metadata(path: &Path) -> Result<SyncMetadata, String> {
    validate_database(path)?
        .ok_or_else(|| "Aurora's local state is missing safe sync metadata.".to_owned())
}

fn read_metadata(connection: &Connection) -> Result<SyncMetadata, String> {
    connection
        .query_row(
            r#"
            SELECT lineage_id, snapshot_id, generation, content_revision,
                   mirrored_revision, last_synced_at_ms
            FROM state_sync_meta WHERE singleton = 1
            "#,
            [],
            |row| {
                Ok(SyncMetadata {
                    lineage_id: row.get(0)?,
                    snapshot_id: row.get(1)?,
                    generation: row.get(2)?,
                    content_revision: row.get(3)?,
                    mirrored_revision: row.get(4)?,
                    last_synced_at_ms: row.get(5)?,
                })
            },
        )
        .optional()
        .map_err(|error| format!("Could not read Aurora's snapshot lineage: {error}"))?
        .ok_or_else(|| "Aurora's state-sync metadata row is missing.".to_owned())
}

fn new_token(label: &str) -> String {
    let machine = env::var("COMPUTERNAME").unwrap_or_else(|_| "device".to_owned());
    let sequence = TOKEN_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!(
        "{label}-{}-{}-{}-{}",
        machine.to_ascii_lowercase(),
        std::process::id(),
        now_ms(),
        sequence
    )
}

pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

#[cfg(windows)]
pub(crate) fn replace_file_atomic(target: &Path, replacement: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn ReplaceFileW(
            replaced_file_name: *const u16,
            replacement_file_name: *const u16,
            backup_file_name: *const u16,
            replace_flags: u32,
            exclude: *mut c_void,
            reserved: *mut c_void,
        ) -> i32;
    }

    fn wide(path: &Path) -> Vec<u16> {
        use std::iter::once;
        path.as_os_str().encode_wide().chain(once(0)).collect()
    }

    let target = wide(target);
    let replacement = wide(replacement);
    // SAFETY: Both paths are owned, NUL-terminated UTF-16 buffers that outlive the call.
    let result = unsafe {
        ReplaceFileW(
            target.as_ptr(),
            replacement.as_ptr(),
            std::ptr::null(),
            0x0000_0001,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if result == 0 {
        Err(format!(
            "Windows could not atomically replace Aurora's state snapshot: {}",
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
pub(crate) fn replace_file_atomic(target: &Path, replacement: &Path) -> Result<(), String> {
    fs::rename(replacement, target)
        .map_err(|error| format!("Could not replace Aurora's state snapshot: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        state_store::{StoredPlaybackState, StoredQueueEntry},
        tag_model::{LoveState, TagValues},
    };

    fn temporary_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "aurora-state-sync-{label}-{}-{}",
            std::process::id(),
            now_ms()
        ))
    }

    fn playback(volume: f32) -> StoredPlaybackState {
        StoredPlaybackState {
            volume,
            ..StoredPlaybackState::default()
        }
    }

    #[test]
    fn publishes_consistent_snapshots_and_preserves_previous_remote() {
        let root = temporary_root("publish");
        fs::create_dir_all(&root).expect("temporary root");
        let local = root.join("local.sqlite3");
        let remote = root.join("aurora-state.sqlite3");
        let store = StateStore::new(local).expect("state store");
        let mut sync =
            StateSyncService::new(store.clone(), remote.clone(), StartupSyncOutcome::None)
                .expect("sync service");

        assert_eq!(sync.sync_now(true).sync_state, "synced");
        assert!(remote.is_file());
        store.save(&playback(0.42)).expect("local change");
        assert_eq!(sync.sync_now(true).sync_state, "synced");
        assert!(root.join("aurora-state.previous.sqlite3").is_file());

        let mirrored = StateStore::new(remote).expect("mirrored state");
        assert_eq!(mirrored.load().expect("mirrored playback").volume, 0.42);
        drop(mirrored);
        drop(sync);
        drop(store);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn restores_a_missing_local_database_from_onedrive() {
        let root = temporary_root("restore");
        fs::create_dir_all(&root).expect("temporary root");
        let source = root.join("source.sqlite3");
        let remote = root.join("aurora-state.sqlite3");
        let restored = root.join("laptop.sqlite3");
        let source_store = StateStore::new(source).expect("source store");
        source_store.save(&playback(0.63)).expect("source change");
        let mut source_sync = StateSyncService::new(
            source_store.clone(),
            remote.clone(),
            StartupSyncOutcome::None,
        )
        .expect("source sync");
        assert_eq!(source_sync.sync_now(true).sync_state, "synced");

        assert_eq!(
            prepare_state_before_open(&restored, &remote),
            StartupSyncOutcome::Restored
        );
        let laptop_store = StateStore::new(restored).expect("restored store");
        assert_eq!(laptop_store.load().expect("restored playback").volume, 0.63);

        drop(laptop_store);
        drop(source_sync);
        drop(source_store);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn divergent_devices_never_silently_overwrite_each_other() {
        let root = temporary_root("conflict");
        fs::create_dir_all(&root).expect("temporary root");
        let desktop_path = root.join("desktop.sqlite3");
        let laptop_path = root.join("laptop.sqlite3");
        let remote = root.join("aurora-state.sqlite3");
        let desktop = StateStore::new(desktop_path).expect("desktop state");
        let mut desktop_sync =
            StateSyncService::new(desktop.clone(), remote.clone(), StartupSyncOutcome::None)
                .expect("desktop sync");
        assert_eq!(desktop_sync.sync_now(true).sync_state, "synced");
        assert_eq!(
            prepare_state_before_open(&laptop_path, &remote),
            StartupSyncOutcome::Restored
        );
        let laptop = StateStore::new(laptop_path).expect("laptop state");
        let mut laptop_sync =
            StateSyncService::new(laptop.clone(), remote.clone(), StartupSyncOutcome::Restored)
                .expect("laptop sync");

        desktop.save(&playback(0.25)).expect("desktop change");
        laptop.save(&playback(0.75)).expect("laptop change");
        assert_eq!(desktop_sync.sync_now(true).sync_state, "synced");
        assert_eq!(laptop_sync.sync_now(true).sync_state, "conflict");
        let remote_store = StateStore::new(remote).expect("remote state");
        assert_eq!(remote_store.load().expect("remote playback").volume, 0.25);

        drop(remote_store);
        drop(laptop_sync);
        drop(desktop_sync);
        drop(laptop);
        drop(desktop);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn equivalent_onedrive_branches_reconcile_device_local_bookkeeping() {
        let root = temporary_root("equivalent-branches");
        fs::create_dir_all(&root).expect("temporary root");
        let desktop_path = root.join("desktop.sqlite3");
        let laptop_path = root.join("laptop.sqlite3");
        let remote = root.join("aurora-state.sqlite3");
        let desktop = StateStore::new(desktop_path).expect("desktop state");
        desktop
            .save(&StoredPlaybackState {
                queue: vec![StoredQueueEntry {
                    track_id: "desktop-import-id".to_owned(),
                    track_key: Some(r"d:\music\artist\track.mp3".to_owned()),
                    directory: Some(r"D:\MUSIC\Artist".to_owned()),
                    filename: Some("Track.mp3".to_owned()),
                }],
                current_index: Some(0),
                position_seconds: 6.0,
                ..StoredPlaybackState::default()
            })
            .expect("desktop playback");
        desktop
            .upsert_overlay(
                r"d:\music\artist\track.mp3",
                r"D:\MUSIC\Artist",
                "Track.mp3",
                &TagValues {
                    rating: None,
                    love_state: LoveState::Neutral,
                    release_year: None,
                },
                &TagValues {
                    rating: Some(4.0),
                    love_state: LoveState::Neutral,
                    release_year: None,
                },
                51,
                None,
            )
            .expect("desktop overlay");
        let mut desktop_sync =
            StateSyncService::new(desktop.clone(), remote.clone(), StartupSyncOutcome::None)
                .expect("desktop sync");
        assert_eq!(desktop_sync.sync_now(true).sync_state, "synced");
        assert_eq!(
            prepare_state_before_open(&laptop_path, &remote),
            StartupSyncOutcome::Restored
        );
        let laptop = StateStore::new(laptop_path.clone()).expect("laptop state");

        {
            let connection = laptop.open().expect("edit laptop bookkeeping");
            connection
                .execute_batch(
                    r#"
                    UPDATE playback_queue SET track_id = 'laptop-import-id';
                    UPDATE playback_state SET position_seconds = 18;
                    UPDATE tag_overlays
                    SET catalog_import_run_id = 52, updated_at_ms = updated_at_ms + 1000;
                    UPDATE state_sync_meta
                    SET snapshot_id = 'snapshot-keiya', generation = 8,
                        content_revision = 71, mirrored_revision = 71;
                    "#,
                )
                .expect("simulate laptop branch");
        }
        {
            let connection = Connection::open(&remote).expect("edit remote branch metadata");
            connection
                .execute_batch(
                    r#"
                    UPDATE state_sync_meta
                    SET snapshot_id = 'snapshot-desktop', generation = 8,
                        content_revision = 70, mirrored_revision = 70;
                    "#,
                )
                .expect("simulate desktop branch");
        }
        assert!(
            semantic_state_matches(&laptop_path, &remote).expect("compare equivalent branches")
        );

        let mut laptop_sync = StateSyncService::new(
            laptop.clone(),
            remote.clone(),
            StartupSyncOutcome::Conflict("simulated OneDrive conflict".to_owned()),
        )
        .expect("laptop sync");
        let status = laptop_sync.sync_now(true);
        assert_eq!(status.sync_state, "synced");
        assert!(status.message.contains("reconciled equivalent state"));
        let reconciled = read_required_metadata(&laptop_path).expect("reconciled metadata");
        assert_eq!(reconciled.snapshot_id, "snapshot-desktop");
        assert_eq!(reconciled.generation, 8);
        assert_eq!(reconciled.content_revision, 70);
        assert_eq!(
            laptop.load().expect("laptop bookkeeping retained").queue[0].track_id,
            "laptop-import-id"
        );

        drop(laptop_sync);
        drop(desktop_sync);
        drop(laptop);
        drop(desktop);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn clean_device_applies_a_newer_remote_snapshot_only_during_startup() {
        let root = temporary_root("startup-update");
        fs::create_dir_all(&root).expect("temporary root");
        let desktop_path = root.join("desktop.sqlite3");
        let laptop_path = root.join("laptop.sqlite3");
        let remote = root.join("aurora-state.sqlite3");
        let desktop = StateStore::new(desktop_path).expect("desktop state");
        let mut desktop_sync =
            StateSyncService::new(desktop.clone(), remote.clone(), StartupSyncOutcome::None)
                .expect("desktop sync");
        assert_eq!(desktop_sync.sync_now(true).sync_state, "synced");
        assert_eq!(
            prepare_state_before_open(&laptop_path, &remote),
            StartupSyncOutcome::Restored
        );
        let laptop = StateStore::new(laptop_path.clone()).expect("laptop state");

        desktop.save(&playback(0.31)).expect("new desktop state");
        assert_eq!(desktop_sync.sync_now(true).sync_state, "synced");
        drop(laptop);

        assert_eq!(
            prepare_state_before_open(&laptop_path, &remote),
            StartupSyncOutcome::Updated
        );
        assert!(root.join("aurora-state.pre-onedrive.sqlite3").is_file());
        let updated_laptop = StateStore::new(laptop_path).expect("updated laptop state");
        assert_eq!(
            updated_laptop.load().expect("updated playback").volume,
            0.31
        );

        drop(updated_laptop);
        drop(desktop_sync);
        drop(desktop);
        let _ = fs::remove_dir_all(root);
    }
}
