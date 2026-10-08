use crate::state_store::{SCHEMA_VERSION, StateStore};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(windows)]
use std::ffi::c_void;
use std::{
    collections::HashSet,
    env,
    fs::{self, File},
    io::{Read, Write},
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

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SyncMetadata {
    lineage_id: String,
    snapshot_id: String,
    generation: i64,
    content_revision: i64,
    mirrored_revision: i64,
    last_synced_at_ms: Option<i64>,
}

/// Device-local evidence written before installing a remote snapshot. It must
/// never be included in the shared database or used to acknowledge a peer write.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PendingPublication {
    version: u8,
    remote_path: PathBuf,
    parent: SyncMetadata,
    snapshot: SyncMetadata,
    sha256: String,
    expected_remote_snapshot: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StateMirrorStatus {
    pub(crate) sync_state: &'static str,
    pub(crate) message: String,
    pub(crate) remote_path: String,
    pub(crate) last_synced_at_ms: Option<i64>,
    pub(crate) album_order_revision: u64,
}

pub(crate) fn default_remote_state_path() -> Result<PathBuf, String> {
    let folder = crate::connections::active().sync_folder;
    if !folder.is_empty() {
        return Ok(PathBuf::from(folder).join("aurora-state.sqlite3"));
    }
    if cfg!(target_os = "macos") || crate::connections::network_mode() {
        return Ok(PathBuf::new());
    }
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
    if let Err(message) = recover_pending_publication(local_path, remote_path) {
        return StartupSyncOutcome::Unavailable(message);
    }
    let snapshot = match crate::snapshot_io::LocalSnapshot::new(remote_path) {
        Ok(snapshot) => snapshot,
        Err(message) => return StartupSyncOutcome::Unavailable(message),
    };
    prepare_state_from_snapshot(local_path, &snapshot.path)
}

fn prepare_state_from_snapshot(local_path: &Path, remote_path: &Path) -> StartupSyncOutcome {
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
    album_order_revision: u64,
    last_album_merge_snapshot: Option<String>,
    last_diagnostic: Option<StateMirrorStatus>,
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
            album_order_revision: 0,
            last_album_merge_snapshot: None,
            last_diagnostic: None,
        })
    }

    pub(crate) fn sync_now(&mut self, bypass_throttle: bool) -> StateMirrorStatus {
        let status = if self.remote_path.as_os_str().is_empty() {
            self.status(
                "disabled",
                "Sync is off. Choose a sync folder in Settings → Connections and restart Aurora."
                    .to_owned(),
                None,
            )
        } else {
            match self.try_sync(bypass_throttle) {
                Ok(status) => status,
                Err(error) => self.status("unavailable", error, None),
            }
        };
        if self.last_diagnostic.as_ref() != Some(&status) {
            let _ = record_sync_diagnostic(self.store.path(), &status);
            self.last_diagnostic = Some(status.clone());
        }
        status
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

        recover_pending_publication(self.store.path(), &self.remote_path)?;
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
                let merged_additions = if dirty {
                    self.merge_remote_album_additions(&remote.snapshot_id)?
                } else {
                    0
                };
                return Ok(self.status(
                    if dirty { "conflict" } else { "remoteUpdate" },
                    if dirty {
                        merge_conflict_message(merged_additions)
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
                let merged_additions = self.merge_remote_album_additions(&remote.snapshot_id)?;
                return Ok(self.status(
                    "conflict",
                    if merged_additions == 0 {
                        "OneDrive does not contain the snapshot this device last published. Aurora is waiting rather than overwriting it."
                            .to_owned()
                    } else {
                        merge_conflict_message(merged_additions)
                    },
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
            album_order_revision: self.album_order_revision,
        }
    }

    fn merge_remote_album_additions(&mut self, snapshot_id: &str) -> Result<usize, String> {
        if self.last_album_merge_snapshot.as_deref() == Some(snapshot_id) {
            return Ok(0);
        }
        let merged = merge_remote_album_additions(&self.store, &self.remote_path)?;
        self.last_album_merge_snapshot = Some(snapshot_id.to_owned());
        if merged > 0 {
            self.album_order_revision = self.album_order_revision.saturating_add(1);
        }
        Ok(merged)
    }
}

fn merge_conflict_message(merged_additions: usize) -> String {
    if merged_additions == 0 {
        return "Aurora detected changes on both computers. Close Aurora on one machine and resolve which state to keep; neither file was overwritten."
            .to_owned();
    }
    format!(
        "Aurora left unrelated conflicting state untouched and safely imported {merged_additions} newer album-added {} so Added sorting stays current.",
        if merged_additions == 1 {
            "record"
        } else {
            "records"
        }
    )
}

fn merge_remote_album_additions(store: &StateStore, remote_path: &Path) -> Result<usize, String> {
    let remote_snapshot = crate::snapshot_io::LocalSnapshot::new(remote_path)?;
    let mut connection = store.open()?;
    connection
        .execute(
            "ATTACH DATABASE ?1 AS remote_state",
            [remote_snapshot.path.to_string_lossy().as_ref()],
        )
        .map_err(|error| format!("Could not attach Aurora's remote album-added state: {error}"))?;
    if table_columns(&connection, "remote_state", "album_additions")?.is_empty() {
        return Ok(0);
    }
    let transaction = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|error| format!("Could not prepare Aurora's album-added merge: {error}"))?;
    let merged = transaction
        .execute(
            r#"
            INSERT INTO album_additions(
              album_id, destination_path, import_run_id, added_at_ms
            )
            SELECT album_id, destination_path, import_run_id, added_at_ms
            FROM remote_state.album_additions
            WHERE true
            ON CONFLICT(album_id) DO UPDATE SET
              destination_path = excluded.destination_path,
              import_run_id = excluded.import_run_id,
              added_at_ms = excluded.added_at_ms
            WHERE excluded.added_at_ms > album_additions.added_at_ms
            "#,
            [],
        )
        .map_err(|error| format!("Could not merge Aurora's album-added state: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("Could not commit Aurora's album-added merge: {error}"))?;
    Ok(merged)
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
    let remote_snapshot = crate::snapshot_io::LocalSnapshot::new(remote_path)?;
    let remote_path = remote_snapshot.path.as_path();
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

    let journal_metadata_columns = [
        "before_file_tags_json",
        "after_file_tags_json",
        "edited_fields_json",
    ];
    let local_journal_columns = table_columns(&connection, "main", "tag_edit_operations")?;
    let remote_journal_columns = table_columns(&connection, "remote_state", "tag_edit_operations")?;
    let mut journal_comparison = "id, track_key, target_path, temp_path, backup_path, before_rating, before_love_state, before_release_year, after_rating, after_love_state, after_release_year, source_fingerprint, status, created_at_ms, updated_at_ms, error_message".to_owned();
    if journal_metadata_columns.iter().all(|column| {
        local_journal_columns.contains(*column) && remote_journal_columns.contains(*column)
    }) {
        journal_comparison
            .push_str(", before_file_tags_json, after_file_tags_json, edited_fields_json");
    }

    for (table, columns) in [
        ("saved_views", "id, name, view, filters_json"),
        ("playback_queue", "position, track_key, directory, filename"),
        (
            "playback_state",
            "singleton, current_index, volume, shuffle, repeat_mode",
        ),
        (
            "tag_overlays",
            "track_key, rating, love_state, release_year, last_operation_id",
        ),
        ("tag_edit_operations", journal_comparison.as_str()),
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
        (
            "album_additions",
            "album_id, destination_path, import_run_id, added_at_ms",
        ),
    ] {
        if matches!(table, "album_additions" | "saved_views")
            && (table_columns(&connection, "main", table)?.is_empty()
                || table_columns(&connection, "remote_state", table)?.is_empty())
        {
            return Ok(false);
        }
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

fn table_columns(
    connection: &Connection,
    schema: &str,
    table: &str,
) -> Result<HashSet<String>, String> {
    if !matches!(schema, "main" | "remote_state")
        || !table
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err("Aurora refused an unsafe state-schema inspection.".to_owned());
    }
    let mut statement = connection
        .prepare(&format!("PRAGMA {schema}.table_info({table})"))
        .map_err(|error| format!("Could not inspect Aurora's {table} schema: {error}"))?;
    statement
        .query_map([], |row| row.get(1))
        .map_err(|error| format!("Could not inspect Aurora's {table} columns: {error}"))?
        .collect::<Result<HashSet<_>, _>>()
        .map_err(|error| format!("Could not decode Aurora's {table} columns: {error}"))
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
    publish_snapshot_with_acknowledgement(
        store,
        remote_path,
        local_before,
        expected_remote_snapshot,
        acknowledge_publication,
    )
}

fn publish_snapshot_with_acknowledgement(
    store: &StateStore,
    remote_path: &Path,
    local_before: &SyncMetadata,
    expected_remote_snapshot: Option<&str>,
    acknowledge: impl FnOnce(&Path, &PendingPublication) -> Result<(), String>,
) -> Result<SyncMetadata, String> {
    let remote_parent = remote_path
        .parent()
        .ok_or_else(|| "Aurora's OneDrive state path has no parent directory.".to_owned())?;
    let stage = tempfile::tempdir().map_err(|e| e.to_string())?;
    let temporary = stage.path().join("state.sqlite3");
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

    let (sealed_metadata, sealed_sha256) = read_snapshot_fingerprint(&temporary)?;
    let uploaded = crate::snapshot_io::upload(&temporary, remote_parent)?;
    let temporary: &Path = uploaded.as_ref();
    let (uploaded_metadata, uploaded_sha256) = read_snapshot_fingerprint(temporary)?;
    if uploaded_metadata != sealed_metadata || uploaded_sha256 != sealed_sha256 {
        return Err("Aurora's uploaded snapshot differs from its verified local copy; the previous snapshot was left untouched.".to_owned());
    }
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
        let _ = fs::remove_file(temporary);
        return Err(
            "The OneDrive state changed while Aurora prepared its snapshot. Both versions were retained."
                .to_owned(),
        );
    }

    let receipt = PendingPublication {
        version: 1,
        remote_path: remote_path.to_owned(),
        parent: local_before.clone(),
        snapshot: sealed_metadata,
        sha256: sealed_sha256,
        expected_remote_snapshot: expected_remote_snapshot.map(str::to_owned),
    };
    persist_publication_receipt(store.path(), &receipt)?;

    if remote_path.is_file() {
        preserve_previous_remote(remote_path)?;
        replace_file_atomic(remote_path, temporary)?;
    } else {
        fs::rename(temporary, remote_path).map_err(|error| {
            format!("Could not publish Aurora's OneDrive state snapshot: {error}")
        })?;
    }
    acknowledge(store.path(), &receipt)?;
    remove_publication_receipt(store.path())?;
    Ok(receipt.snapshot)
}

fn publication_receipt_path(local_path: &Path) -> PathBuf {
    local_path.with_extension("publish.json")
}

fn persist_publication_receipt(
    local_path: &Path,
    receipt: &PendingPublication,
) -> Result<(), String> {
    let path = publication_receipt_path(local_path);
    let parent = path
        .parent()
        .ok_or_else(|| "Aurora's publication receipt has no parent folder.".to_owned())?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(receipt).map_err(|e| e.to_string())?;
    temporary.write_all(&bytes).map_err(|e| e.to_string())?;
    crate::snapshot_io::sync_file(temporary.as_file()).map_err(|e| e.to_string())?;
    let temporary = temporary.into_temp_path();
    if path.is_file() {
        replace_file_atomic(&path, &temporary)
    } else {
        fs::rename(&temporary, &path).map_err(|e| e.to_string())
    }
    .map_err(|e| format!("Could not save Aurora's publication recovery receipt: {e}"))
}

fn read_publication_receipt(local_path: &Path) -> Result<Option<PendingPublication>, String> {
    let path = publication_receipt_path(local_path);
    let file = match File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Could not read Aurora's publication receipt: {error}"
            ));
        }
    };
    let mut bytes = Vec::new();
    file.take(64 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 64 * 1024 {
        return Err("Aurora's publication receipt exceeds its size limit.".to_owned());
    }
    let receipt: PendingPublication = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Aurora's publication recovery receipt is unreadable: {e}"))?;
    if receipt.version != 1
        || receipt.parent.lineage_id != receipt.snapshot.lineage_id
        || receipt.parent.generation.checked_add(1) != Some(receipt.snapshot.generation)
        || receipt.snapshot.content_revision < receipt.parent.content_revision
        || receipt.snapshot.content_revision != receipt.snapshot.mirrored_revision
        || receipt.sha256.len() != 64
        || !receipt.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("Aurora's publication recovery receipt is invalid.".to_owned());
    }
    Ok(Some(receipt))
}

fn remove_publication_receipt(local_path: &Path) -> Result<(), String> {
    match fs::remove_file(publication_receipt_path(local_path)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "Could not finish Aurora's publication recovery: {error}"
        )),
    }
}

fn snapshot_sha256(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let length = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if length == 0 {
            break;
        }
        hash.update(&buffer[..length]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn read_snapshot_fingerprint(path: &Path) -> Result<(SyncMetadata, String), String> {
    // Preserve the existing bounded Mac SMB download path. Hashing a mounted
    // file directly could otherwise block the sync worker indefinitely.
    let snapshot = crate::snapshot_io::LocalSnapshot::new(path)?;
    Ok((
        read_required_metadata(&snapshot.path)?,
        snapshot_sha256(&snapshot.path)?,
    ))
}

fn publication_was_acknowledged(local: &SyncMetadata, receipt: &PendingPublication) -> bool {
    local.lineage_id == receipt.snapshot.lineage_id
        && local.generation >= receipt.snapshot.generation
        && local.mirrored_revision >= receipt.snapshot.mirrored_revision
        && (local.generation > receipt.snapshot.generation
            || local.snapshot_id == receipt.snapshot.snapshot_id)
}

fn recover_pending_publication(local_path: &Path, remote_path: &Path) -> Result<(), String> {
    let Some(receipt) = read_publication_receipt(local_path)? else {
        return Ok(());
    };
    // A receipt belongs to this installation and this exact sync destination.
    if !local_path.is_file() || receipt.remote_path != remote_path {
        return Ok(());
    }
    let local = read_required_metadata(local_path)?;
    if publication_was_acknowledged(&local, &receipt) {
        return remove_publication_receipt(local_path);
    }
    if local.lineage_id != receipt.parent.lineage_id
        || local.snapshot_id != receipt.parent.snapshot_id
        || local.generation != receipt.parent.generation
    {
        return Ok(());
    }
    let remote = if remote_path.is_file() {
        validate_database(remote_path)?
    } else {
        None
    };
    if remote.as_ref() == Some(&receipt.snapshot) {
        let (metadata, sha256) = read_snapshot_fingerprint(remote_path)?;
        if metadata != receipt.snapshot || sha256 != receipt.sha256 {
            return Err("Aurora's pending snapshot changed after publication; its recovery receipt was retained without acknowledging it.".to_owned());
        }
        acknowledge_publication(local_path, &receipt)?;
        let _ = record_sync_diagnostic(
            local_path,
            &StateMirrorStatus {
                sync_state: "recovered",
                message: format!(
                    "Recovered publication {} without replacing local content; only captured revision {} was acknowledged.",
                    receipt.snapshot.snapshot_id, receipt.snapshot.content_revision,
                ),
                remote_path: remote_path.to_string_lossy().into_owned(),
                last_synced_at_ms: receipt.snapshot.last_synced_at_ms,
                album_order_revision: 0,
            },
        );
        return remove_publication_receipt(local_path);
    }
    if remote.as_ref().map(|metadata| &metadata.snapshot_id)
        == receipt.expected_remote_snapshot.as_ref()
    {
        // The remote was never installed. Retrying will capture all current
        // local edits and write a fresh receipt before attempting replacement.
        remove_publication_receipt(local_path)?;
    }
    // An independently advanced remote keeps the normal conflict safeguards.
    Ok(())
}

fn acknowledge_publication(local_path: &Path, receipt: &PendingPublication) -> Result<(), String> {
    let (metadata, sha256) = read_snapshot_fingerprint(&receipt.remote_path)?;
    if metadata != receipt.snapshot || sha256 != receipt.sha256 {
        return Err("Aurora's published snapshot changed before acknowledgement; local edits and the recovery receipt were retained.".to_owned());
    }
    let connection = Connection::open_with_flags(
        local_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|e| e.to_string())?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let updated = connection
        .execute(
            r#"
            UPDATE state_sync_meta SET
              snapshot_id = ?1, generation = ?2, mirrored_revision = ?3,
              last_synced_at_ms = ?4
            WHERE singleton = 1 AND lineage_id = ?5
              AND snapshot_id = ?6 AND generation = ?7
              AND content_revision >= ?3 AND mirrored_revision = ?8
            "#,
            params![
                receipt.snapshot.snapshot_id,
                receipt.snapshot.generation,
                receipt.snapshot.mirrored_revision,
                receipt.snapshot.last_synced_at_ms,
                receipt.parent.lineage_id,
                receipt.parent.snapshot_id,
                receipt.parent.generation,
                receipt.parent.mirrored_revision,
            ],
        )
        .map_err(|error| format!("Could not checkpoint Aurora's published snapshot: {error}"))?;
    if updated != 1 {
        return Err(
            "Aurora published a valid snapshot, but its local acknowledgement is pending. The saved recovery receipt will retry without discarding newer local edits."
                .to_owned(),
        );
    }
    Ok(())
}

fn record_sync_diagnostic(local_path: &Path, status: &StateMirrorStatus) -> std::io::Result<()> {
    let path = local_path.with_extension("sync.jsonl");
    let previous = local_path.with_extension("sync.previous.jsonl");
    let event = serde_json::json!({
        "timestampMs": now_ms(),
        "version": env!("CARGO_PKG_VERSION"),
        "processId": std::process::id(),
        "status": status,
    });
    let mut bytes = serde_json::to_vec(&event)?;
    bytes.push(b'\n');
    if fs::metadata(&path).is_ok_and(|meta| meta.len() + bytes.len() as u64 > 1024 * 1024) {
        match fs::remove_file(&previous) {
            Ok(()) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error),
        }
        fs::rename(&path, previous)?;
    }
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(&bytes)
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
        .and_then(|file| crate::snapshot_io::sync_file(&file))
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
        .and_then(|file| crate::snapshot_io::sync_file(&file))
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
        .and_then(|file| crate::snapshot_io::sync_file(&file))
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
    let connection = crate::snapshot_io::open_read(path)
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
    let mut last_error = std::io::Error::other("File replacement did not run");
    for attempt in 0..20 {
        // SAFETY: Both paths are owned, NUL-terminated UTF-16 buffers that outlive the call.
        let result = unsafe {
            ReplaceFileW(
                target.as_ptr(),
                replacement.as_ptr(),
                std::ptr::null(),
                0, // REPLACEFILE_WRITE_THROUGH is unsupported; callers flush before replacing.
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if result != 0 {
            return Ok(());
        }
        last_error = std::io::Error::last_os_error();
        // Only retry sharing/lock violations: other errors may describe a partial replacement.
        if !matches!(last_error.raw_os_error(), Some(32 | 33)) || attempt == 19 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Err(format!(
        "Windows could not atomically replace Aurora's state snapshot: {last_error}"
    ))
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

    #[test]
    fn atomic_replacement_preserves_target_when_replacement_is_missing() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("settings.json");
        fs::write(&target, b"original").unwrap();
        assert!(replace_file_atomic(&target, &directory.path().join("missing")).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"original");
    }

    #[cfg(windows)]
    #[test]
    fn atomic_replacement_waits_for_a_temporary_windows_sharing_lock() {
        use std::os::windows::fs::OpenOptionsExt;
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("settings.json");
        let replacement = directory.path().join("replacement.json");
        fs::write(&target, b"original").unwrap();
        fs::write(&replacement, b"updated").unwrap();
        let locked = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&replacement)
            .unwrap();
        let release = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(120));
            drop(locked);
        });
        let result = replace_file_atomic(&target, &replacement);
        release.join().unwrap();
        result.unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"updated");
    }

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

    fn interrupt_after_remote_install(store: &StateStore, remote: &Path) -> PendingPublication {
        let parent = read_required_metadata(store.path()).unwrap();
        let remote_id = read_required_metadata(remote).unwrap().snapshot_id;
        let result = publish_snapshot_with_acknowledgement(
            store,
            remote,
            &parent,
            Some(&remote_id),
            |_, _| Err("simulated interruption after remote installation".to_owned()),
        );
        assert!(result.is_err());
        let receipt = read_publication_receipt(store.path()).unwrap().unwrap();
        assert_eq!(read_required_metadata(store.path()).unwrap(), parent);
        assert_eq!(read_required_metadata(remote).unwrap(), receipt.snapshot);
        receipt
    }

    #[test]
    fn first_publication_interruption_recovers_without_prior_remote_snapshot() {
        let root = tempfile::tempdir().unwrap();
        let store = StateStore::new(root.path().join("local.sqlite3")).unwrap();
        let remote = root.path().join("aurora-state.sqlite3");
        let parent = read_required_metadata(store.path()).unwrap();
        assert!(
            publish_snapshot_with_acknowledgement(&store, &remote, &parent, None, |_, _| Err(
                "simulated first-publication interruption".into()
            ),)
            .is_err()
        );
        store.save(&playback(0.82)).unwrap();
        assert_eq!(
            prepare_state_before_open(store.path(), &remote),
            StartupSyncOutcome::None
        );
        assert_eq!(store.load().unwrap().volume, 0.82);
        let recovered = read_required_metadata(store.path()).unwrap();
        assert_eq!(recovered.generation, parent.generation + 1);
        assert!(recovered.content_revision > recovered.mirrored_revision);
    }

    #[test]
    fn writes_after_snapshot_capture_remain_dirty_when_acknowledgement_succeeds() {
        let root = tempfile::tempdir().unwrap();
        let store = StateStore::new(root.path().join("local.sqlite3")).unwrap();
        let remote = root.path().join("aurora-state.sqlite3");
        let parent = read_required_metadata(store.path()).unwrap();
        let snapshot = publish_snapshot_with_acknowledgement(
            &store,
            &remote,
            &parent,
            None,
            |path, receipt| {
                store.save(&playback(0.82))?;
                acknowledge_publication(path, receipt)
            },
        )
        .unwrap();
        let local = read_required_metadata(store.path()).unwrap();
        assert_eq!(local.mirrored_revision, snapshot.content_revision);
        assert!(local.content_revision > local.mirrored_revision);
        assert_eq!(store.load().unwrap().volume, 0.82);
        assert!(!publication_receipt_path(store.path()).exists());
    }

    #[test]
    fn inability_to_store_a_receipt_never_installs_the_snapshot() {
        let root = tempfile::tempdir().unwrap();
        let store = StateStore::new(root.path().join("local.sqlite3")).unwrap();
        let remote = root.path().join("aurora-state.sqlite3");
        let mut sync =
            StateSyncService::new(store.clone(), remote.clone(), StartupSyncOutcome::None).unwrap();
        assert_eq!(sync.sync_now(true).sync_state, "synced");
        fs::create_dir(publication_receipt_path(store.path())).unwrap();
        store.save(&playback(0.82)).unwrap();
        let before = read_required_metadata(store.path()).unwrap();
        let remote_before = snapshot_sha256(&remote).unwrap();
        assert!(publish_snapshot(&store, &remote, &before, Some(&before.snapshot_id)).is_err());
        assert_eq!(read_required_metadata(store.path()).unwrap(), before);
        assert_eq!(snapshot_sha256(&remote).unwrap(), remote_before);
    }

    #[test]
    fn restart_recovers_an_interrupted_publication_without_replacing_later_local_edits() {
        let root = tempfile::tempdir().unwrap();
        let store = StateStore::new(root.path().join("local.sqlite3")).unwrap();
        let remote = root.path().join("aurora-state.sqlite3");
        let mut sync =
            StateSyncService::new(store.clone(), remote.clone(), StartupSyncOutcome::None).unwrap();
        assert_eq!(sync.sync_now(true).sync_state, "synced");
        store.save(&playback(0.42)).unwrap();
        let receipt = interrupt_after_remote_install(&store, &remote);
        store.save(&playback(0.77)).unwrap();
        store
            .upsert_overlay(
                "track-key",
                r"D:\MUSIC",
                "Track.mp3",
                &TagValues {
                    rating: None,
                    love_state: LoveState::Neutral,
                    release_year: None,
                },
                &TagValues {
                    rating: Some(5.0),
                    love_state: LoveState::Loved,
                    release_year: None,
                },
                1,
                None,
            )
            .unwrap();
        let later = read_required_metadata(store.path()).unwrap();
        assert_eq!(
            prepare_state_before_open(store.path(), &remote),
            StartupSyncOutcome::None
        );
        let recovered = read_required_metadata(store.path()).unwrap();
        assert_eq!(recovered.snapshot_id, receipt.snapshot.snapshot_id);
        assert_eq!(recovered.content_revision, later.content_revision);
        assert_eq!(
            recovered.mirrored_revision,
            receipt.snapshot.content_revision
        );
        assert!(recovered.content_revision > recovered.mirrored_revision);
        assert_eq!(store.load().unwrap().volume, 0.77);
        assert!(!publication_receipt_path(store.path()).exists());
        assert_eq!(sync.sync_now(true).sync_state, "synced");
        let snapshot =
            Connection::open_with_flags(&remote, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        assert_eq!(
            snapshot
                .query_row(
                    "SELECT rating FROM tag_overlays WHERE track_key='track-key'",
                    [],
                    |row| row.get::<_, f64>(0)
                )
                .unwrap(),
            5.0
        );
        assert_eq!(
            snapshot
                .query_row("SELECT volume FROM playback_state", [], |row| row
                    .get::<_, f32>(0))
                .unwrap(),
            0.77
        );
        assert_eq!(
            read_required_metadata(&remote).unwrap().generation,
            receipt.snapshot.generation + 1
        );
    }

    #[test]
    fn running_retry_recovers_the_receipt_and_publishes_newer_local_content() {
        let root = tempfile::tempdir().unwrap();
        let store = StateStore::new(root.path().join("local.sqlite3")).unwrap();
        let remote = root.path().join("aurora-state.sqlite3");
        let mut sync =
            StateSyncService::new(store.clone(), remote.clone(), StartupSyncOutcome::None).unwrap();
        assert_eq!(sync.sync_now(true).sync_state, "synced");
        store.save(&playback(0.42)).unwrap();
        let receipt = interrupt_after_remote_install(&store, &remote);
        store.save(&playback(0.81)).unwrap();
        assert_eq!(sync.sync_now(true).sync_state, "synced");
        let recovered = read_required_metadata(store.path()).unwrap();
        assert_eq!(recovered.generation, receipt.snapshot.generation + 1);
        assert_eq!(recovered.content_revision, recovered.mirrored_revision);
        assert_eq!(store.load().unwrap().volume, 0.81);
        assert!(!publication_receipt_path(store.path()).exists());
        assert_eq!(sync.sync_now(true).sync_state, "synced");
        assert_eq!(read_required_metadata(store.path()).unwrap(), recovered);
    }

    #[test]
    fn receipt_does_not_acknowledge_an_independently_advanced_peer() {
        let root = tempfile::tempdir().unwrap();
        let store = StateStore::new(root.path().join("local.sqlite3")).unwrap();
        let remote = root.path().join("aurora-state.sqlite3");
        let mut sync =
            StateSyncService::new(store.clone(), remote.clone(), StartupSyncOutcome::None).unwrap();
        assert_eq!(sync.sync_now(true).sync_state, "synced");
        store.save(&playback(0.42)).unwrap();
        let receipt = interrupt_after_remote_install(&store, &remote);
        let peer_path = root.path().join("peer.sqlite3");
        consistent_copy(&remote, &peer_path).unwrap();
        let peer = StateStore::new(peer_path).unwrap();
        peer.save(&playback(0.25)).unwrap();
        let mut peer_sync =
            StateSyncService::new(peer, remote.clone(), StartupSyncOutcome::None).unwrap();
        assert_eq!(peer_sync.sync_now(true).sync_state, "synced");
        store.save(&playback(0.81)).unwrap();
        let local_before = read_required_metadata(store.path()).unwrap();
        let remote_before = snapshot_sha256(&remote).unwrap();
        assert_eq!(sync.sync_now(true).sync_state, "conflict");
        assert_eq!(read_required_metadata(store.path()).unwrap(), local_before);
        assert_eq!(snapshot_sha256(&remote).unwrap(), remote_before);
        assert_eq!(
            read_publication_receipt(store.path())
                .unwrap()
                .unwrap()
                .snapshot,
            receipt.snapshot
        );
    }

    #[test]
    fn receipt_rejects_changed_snapshot_bytes_even_when_identity_matches() {
        let root = tempfile::tempdir().unwrap();
        let store = StateStore::new(root.path().join("local.sqlite3")).unwrap();
        let remote = root.path().join("aurora-state.sqlite3");
        let mut sync =
            StateSyncService::new(store.clone(), remote.clone(), StartupSyncOutcome::None).unwrap();
        assert_eq!(sync.sync_now(true).sync_state, "synced");
        store.save(&playback(0.42)).unwrap();
        let receipt = interrupt_after_remote_install(&store, &remote);
        let snapshot = Connection::open(&remote).unwrap();
        snapshot
            .execute("UPDATE playback_state SET volume=0.1", [])
            .unwrap();
        snapshot
            .execute(
                "UPDATE state_sync_meta SET content_revision=?1",
                [receipt.snapshot.content_revision],
            )
            .unwrap();
        drop(snapshot);
        assert_eq!(read_required_metadata(&remote).unwrap(), receipt.snapshot);
        let before = read_required_metadata(store.path()).unwrap();
        let status = sync.sync_now(true);
        assert_eq!(status.sync_state, "unavailable");
        assert!(status.message.contains("changed after publication"));
        assert_eq!(read_required_metadata(store.path()).unwrap(), before);
        assert!(publication_receipt_path(store.path()).exists());
    }

    #[test]
    fn receipt_before_remote_install_is_discarded_and_retried_from_current_content() {
        let root = tempfile::tempdir().unwrap();
        let store = StateStore::new(root.path().join("local.sqlite3")).unwrap();
        let remote = root.path().join("aurora-state.sqlite3");
        let mut sync =
            StateSyncService::new(store.clone(), remote.clone(), StartupSyncOutcome::None).unwrap();
        assert_eq!(sync.sync_now(true).sync_state, "synced");
        store.save(&playback(0.42)).unwrap();
        interrupt_after_remote_install(&store, &remote);
        fs::copy(root.path().join("aurora-state.previous.sqlite3"), &remote).unwrap();
        store.save(&playback(0.81)).unwrap();
        assert_eq!(sync.sync_now(true).sync_state, "synced");
        assert_eq!(
            read_required_metadata(store.path()).unwrap().snapshot_id,
            read_required_metadata(&remote).unwrap().snapshot_id
        );
        assert!(!publication_receipt_path(store.path()).exists());
        assert_eq!(store.load().unwrap().volume, 0.81);
    }

    #[test]
    fn completed_acknowledgement_with_leftover_receipt_is_idempotent() {
        let root = tempfile::tempdir().unwrap();
        let store = StateStore::new(root.path().join("local.sqlite3")).unwrap();
        let remote = root.path().join("aurora-state.sqlite3");
        let mut sync =
            StateSyncService::new(store.clone(), remote.clone(), StartupSyncOutcome::None).unwrap();
        assert_eq!(sync.sync_now(true).sync_state, "synced");
        store.save(&playback(0.42)).unwrap();
        let receipt = interrupt_after_remote_install(&store, &remote);
        acknowledge_publication(store.path(), &receipt).unwrap();
        let before = read_required_metadata(store.path()).unwrap();
        recover_pending_publication(store.path(), &remote).unwrap();
        recover_pending_publication(store.path(), &remote).unwrap();
        assert_eq!(read_required_metadata(store.path()).unwrap(), before);
        assert!(!publication_receipt_path(store.path()).exists());
    }

    #[test]
    fn receipt_cannot_acknowledge_an_identical_snapshot_at_a_different_destination() {
        let root = tempfile::tempdir().unwrap();
        let store = StateStore::new(root.path().join("local.sqlite3")).unwrap();
        let remote = root.path().join("aurora-state.sqlite3");
        let mut sync =
            StateSyncService::new(store.clone(), remote.clone(), StartupSyncOutcome::None).unwrap();
        assert_eq!(sync.sync_now(true).sync_state, "synced");
        store.save(&playback(0.42)).unwrap();
        interrupt_after_remote_install(&store, &remote);
        let other_destination = root.path().join("another-sync.sqlite3");
        fs::copy(&remote, &other_destination).unwrap();
        let before = read_required_metadata(store.path()).unwrap();
        let remote_before = snapshot_sha256(&other_destination).unwrap();
        recover_pending_publication(store.path(), &other_destination).unwrap();
        assert_eq!(read_required_metadata(store.path()).unwrap(), before);
        assert_eq!(snapshot_sha256(&other_destination).unwrap(), remote_before);
        assert!(publication_receipt_path(store.path()).exists());
    }

    #[test]
    fn corrupt_receipt_preserves_both_databases_and_logs_a_bounded_diagnostic() {
        let root = tempfile::tempdir().unwrap();
        let store = StateStore::new(root.path().join("local.sqlite3")).unwrap();
        let remote = root.path().join("aurora-state.sqlite3");
        let mut sync =
            StateSyncService::new(store.clone(), remote.clone(), StartupSyncOutcome::None).unwrap();
        assert_eq!(sync.sync_now(true).sync_state, "synced");
        assert_eq!(sync.sync_now(true).sync_state, "synced");
        assert_eq!(sync.sync_now(true).sync_state, "synced");
        let log = store.path().with_extension("sync.jsonl");
        assert_eq!(fs::read_to_string(&log).unwrap().lines().count(), 2);
        fs::write(&log, vec![b'x'; 1024 * 1024]).unwrap();
        fs::write(publication_receipt_path(store.path()), b"{broken").unwrap();
        let before = read_required_metadata(store.path()).unwrap();
        let remote_before = snapshot_sha256(&remote).unwrap();
        assert_eq!(sync.sync_now(true).sync_state, "unavailable");
        assert_eq!(read_required_metadata(store.path()).unwrap(), before);
        assert_eq!(snapshot_sha256(&remote).unwrap(), remote_before);
        assert_eq!(
            fs::read(publication_receipt_path(store.path())).unwrap(),
            b"{broken"
        );
        let diagnostic: serde_json::Value =
            serde_json::from_str(fs::read_to_string(&log).unwrap().trim()).unwrap();
        assert!(
            diagnostic["status"]["message"]
                .as_str()
                .unwrap()
                .contains("unreadable")
        );
        assert_eq!(
            fs::metadata(store.path().with_extension("sync.previous.jsonl"))
                .unwrap()
                .len(),
            1024 * 1024
        );
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
        desktop
            .record_album_additions(
                &[(
                    "album-from-desktop".to_owned(),
                    r"D:\MUSIC\Artist\Album".to_owned(),
                )],
                91,
                2_000,
            )
            .expect("desktop album addition");
        laptop.save(&playback(0.75)).expect("laptop change");
        assert_eq!(desktop_sync.sync_now(true).sync_state, "synced");
        let conflict = laptop_sync.sync_now(true);
        assert_eq!(conflict.sync_state, "conflict");
        assert_eq!(conflict.album_order_revision, 1);
        assert!(conflict.message.contains("1 newer album-added record"));
        let laptop_addition: (String, i64, i64) = laptop
            .open()
            .expect("laptop state connection")
            .query_row(
                "SELECT destination_path, import_run_id, added_at_ms FROM album_additions WHERE album_id = 'album-from-desktop'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("merged laptop album addition");
        assert_eq!(
            laptop_addition,
            (r"D:\MUSIC\Artist\Album".to_owned(), 91, 2_000)
        );
        assert_eq!(laptop_sync.sync_now(true).album_order_revision, 1);
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
    fn album_addition_merge_keeps_the_newest_record_from_either_device() {
        let root = temporary_root("album-addition-merge");
        fs::create_dir_all(&root).expect("temporary root");
        let local_path = root.join("local.sqlite3");
        let remote_path = root.join("remote.sqlite3");
        let local = StateStore::new(local_path).expect("local state");
        let remote = StateStore::new(remote_path.clone()).expect("remote state");
        local
            .record_album_additions(
                &[("shared-album".to_owned(), "local-newer".to_owned())],
                20,
                2_000,
            )
            .expect("local addition");
        remote
            .record_album_additions(
                &[
                    ("shared-album".to_owned(), "remote-older".to_owned()),
                    ("remote-only".to_owned(), "remote-only-path".to_owned()),
                ],
                10,
                1_000,
            )
            .expect("remote additions");

        assert_eq!(
            merge_remote_album_additions(&local, &remote_path).expect("album addition merge"),
            1
        );
        let connection = local.open().expect("local connection");
        let shared: (String, i64, i64) = connection
            .query_row(
                "SELECT destination_path, import_run_id, added_at_ms FROM album_additions WHERE album_id = 'shared-album'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("shared addition");
        assert_eq!(shared, ("local-newer".to_owned(), 20, 2_000));
        assert_eq!(
            connection
                .query_row(
                    "SELECT destination_path FROM album_additions WHERE album_id = 'remote-only'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .expect("remote-only addition"),
            "remote-only-path"
        );

        drop(connection);
        drop(remote);
        drop(local);
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
