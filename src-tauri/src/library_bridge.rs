use rusqlite::Connection;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

use crate::{
    catalog::{default_catalog_path, open_catalog},
    device_mode,
    library_sync::LibrarySyncCoordinator,
    state_store::StateStore,
};

const PROTOCOL_VERSION: u32 = 1;
const MUSIC_LIBRARY_EXE_ENV: &str = "AURORA_MUSIC_LIBRARY_EXE";
const MAX_RESPONSE_BYTES: u64 = 16 * 1024 * 1024;
const CAPABILITIES_TIMEOUT: Duration = Duration::from_secs(30);
const PREVIEW_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const APPLY_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const SYNC_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const CHILD_POLL_INTERVAL: Duration = Duration::from_millis(50);
const MAX_SYNC_FOLDERS: usize = 32;
const ABANDONED_INBOX_TEMP_AGE_MS: u64 = 5 * 60 * 1_000;
const MAX_INTAKE_CLEANUP_ENTRIES: usize = 100_000;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

static EXCHANGE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LibraryCategoryId {
    General,
    Scores,
    Synthwave,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryCategoryCapability {
    pub id: LibraryCategoryId,
    pub label: String,
    pub destination_root: String,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryBridgeSupports {
    pub single_album: bool,
    pub batch_folders: bool,
    pub cross_volume_copy: bool,
    pub preview_required: bool,
    #[serde(default)]
    pub sync_existing_folders: bool,
    #[serde(default)]
    pub default_popm_rating_fallback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryBridgeCapabilities {
    pub bridge_version: u32,
    pub categories: Vec<LibraryCategoryCapability>,
    pub supports: LibraryBridgeSupports,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryIntakePreviewRequest {
    pub source_path: String,
    pub category: LibraryCategoryId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryIntakeCategory {
    pub id: LibraryCategoryId,
    pub label: String,
    pub destination_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryIntakeDelta {
    pub added_tracks: u64,
    pub changed_tracks: u64,
    pub removed_tracks: u64,
    pub added_albums: u64,
    pub changed_albums: u64,
    pub removed_albums: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryIntakePreviewAlbum {
    pub source_path: String,
    pub destination_path: String,
    pub artist: String,
    pub album: String,
    pub year: String,
    pub track_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryIntakePreview {
    pub plan_id: String,
    pub session_id: i64,
    pub source_path: String,
    pub category: LibraryIntakeCategory,
    pub album_count: u64,
    pub track_count: u64,
    pub delta: LibraryIntakeDelta,
    pub albums: Vec<LibraryIntakePreviewAlbum>,
    pub can_apply: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryIntakeApplyRequest {
    pub plan_id: String,
    pub session_id: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LibraryIntakeApplyStatus {
    Completed,
    CompletedWithWarnings,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LibraryIntakeCleanupStatus {
    Removed,
    Retained,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryIntakeAppliedAlbum {
    pub source_path: String,
    pub destination_path: String,
    pub cleanup_status: LibraryIntakeCleanupStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryIntakeApplyResult {
    pub plan_id: String,
    pub session_id: i64,
    pub status: LibraryIntakeApplyStatus,
    pub album_count: u64,
    pub track_count: u64,
    pub moved_album_count: u64,
    pub import_run_id: i64,
    pub backup_path: Option<String>,
    pub albums: Vec<LibraryIntakeAppliedAlbum>,
    pub cleanup_warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProtocolRequest<T> {
    protocol_version: u32,
    operation: &'static str,
    payload: T,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProtocolResponse<T> {
    protocol_version: u32,
    ok: bool,
    result: Option<T>,
    error: Option<ProtocolError>,
}

#[derive(Debug, Deserialize)]
struct ProtocolError {
    code: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct EmptyPayload {}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewPayload<'a> {
    source_path: &'a str,
    category: LibraryCategoryId,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplyPayload<'a> {
    plan_id: &'a str,
    session_id: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncExistingFoldersPayload<'a> {
    folder_paths: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    changed_file_paths: Option<&'a [String]>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum LibraryFolderSyncStatus {
    Updated,
    Unchanged,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct LibraryFolderSyncReceipt {
    folder_path: String,
    status: LibraryFolderSyncStatus,
    changed_tracks: i64,
    changed_albums: i64,
    import_run_id: Option<i64>,
    backup_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LibraryExistingFoldersSyncResult {
    synced_folder_count: usize,
    updated_folder_count: usize,
    changed_tracks: i64,
    changed_albums: i64,
    import_run_ids: Vec<i64>,
    backup_paths: Vec<String>,
    folders: Vec<LibraryFolderSyncReceipt>,
}

struct ExchangeFiles {
    request_path: PathBuf,
    response_path: PathBuf,
}

impl ExchangeFiles {
    fn create(directory: &Path) -> Result<(Self, File), String> {
        fs::create_dir_all(directory).map_err(|error| {
            format!("Aurora could not prepare its Music Library bridge folder: {error}")
        })?;

        for _ in 0..16 {
            let sequence = EXCHANGE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let stem = format!("exchange-{}-{timestamp}-{sequence}", std::process::id());
            let request_path = directory.join(format!("{stem}.request.json"));
            let response_path = directory.join(format!("{stem}.response.json"));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&request_path)
            {
                Ok(file) => {
                    return Ok((
                        Self {
                            request_path,
                            response_path,
                        },
                        file,
                    ));
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(format!(
                        "Aurora could not create a Music Library bridge request: {error}"
                    ));
                }
            }
        }

        Err("Aurora could not allocate a unique Music Library bridge request.".to_owned())
    }
}

impl Drop for ExchangeFiles {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.request_path);
        let _ = fs::remove_file(&self.response_path);
    }
}

#[tauri::command]
pub async fn select_library_intake_folder(app: AppHandle) -> Result<Option<String>, String> {
    let selected = app
        .dialog()
        .file()
        .set_title("Choose an album or a folder of albums")
        .blocking_pick_folder();

    selected
        .map(|path| {
            path.into_path()
                .map_err(|error| format!("Aurora could not read the selected folder: {error}"))?
                .into_os_string()
                .into_string()
                .map_err(|_| "The selected folder path is not valid Unicode.".to_owned())
        })
        .transpose()
}

#[tauri::command]
pub async fn library_bridge_capabilities(
    app: AppHandle,
) -> Result<LibraryBridgeCapabilities, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = invoke_bridge::<_, LibraryBridgeCapabilities>(
            &app,
            "capabilities",
            EmptyPayload {},
            CAPABILITIES_TIMEOUT,
        )?;
        validate_capabilities(&result)?;
        Ok(result)
    })
    .await
    .map_err(|error| format!("The Music Library bridge worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
pub async fn preview_library_intake_batch(
    app: AppHandle,
    request: LibraryIntakePreviewRequest,
) -> Result<LibraryIntakePreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let coordinator = app.state::<LibrarySyncCoordinator>();
        coordinator.serialize_bridge_work(|| {
            let source_path = validate_source_path(&request.source_path)?;
            cleanup_abandoned_inbox_temporary_files(Path::new(&source_path), current_time_ms())?;
            let result = invoke_bridge::<_, LibraryIntakePreview>(
                &app,
                "previewBatch",
                PreviewPayload {
                    source_path: &source_path,
                    category: request.category,
                },
                PREVIEW_TIMEOUT,
            )?;
            validate_preview(&result, request.category)?;
            Ok(result)
        })
    })
    .await
    .map_err(|error| format!("The album preview worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
pub async fn apply_library_intake_batch(
    app: AppHandle,
    request: LibraryIntakeApplyRequest,
) -> Result<LibraryIntakeApplyResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let coordinator = app.state::<LibrarySyncCoordinator>();
        coordinator.serialize_bridge_work(|| {
            validate_apply_request(&request)?;
            let mut result = invoke_bridge::<_, LibraryIntakeApplyResult>(
                &app,
                "applyBatch",
                ApplyPayload {
                    plan_id: &request.plan_id,
                    session_id: request.session_id,
                },
                APPLY_TIMEOUT,
            )?;
            validate_apply_result(&result, &request)?;
            if let Err(error) = record_album_additions(&app, &result) {
                result.status = LibraryIntakeApplyStatus::CompletedWithWarnings;
                result.cleanup_warnings.push(format!(
                    "Music was added, but Aurora could not record its added date: {error}"
                ));
            }
            Ok(result)
        })
    })
    .await
    .map_err(|error| format!("The album import worker stopped unexpectedly: {error}"))?
}

pub(crate) fn sync_existing_library_folders(
    app: &AppHandle,
    folder_paths: Vec<String>,
    changed_file_paths: Vec<String>,
) -> Result<LibraryExistingFoldersSyncResult, String> {
    let folder_paths = validate_sync_folder_paths(folder_paths)?;
    let changed_file_paths = validate_sync_file_paths(changed_file_paths)?;
    let capabilities = invoke_bridge::<_, LibraryBridgeCapabilities>(
        app,
        "capabilities",
        EmptyPayload {},
        CAPABILITIES_TIMEOUT,
    )?;
    validate_capabilities(&capabilities)?;
    validate_tag_sync_capabilities(&capabilities)?;
    let result = invoke_bridge::<_, LibraryExistingFoldersSyncResult>(
        app,
        "syncExistingFolders",
        SyncExistingFoldersPayload {
            folder_paths: &folder_paths,
            changed_file_paths: (!changed_file_paths.is_empty())
                .then_some(changed_file_paths.as_slice()),
        },
        SYNC_TIMEOUT,
    )?;
    validate_sync_result(&result, &folder_paths)?;
    Ok(result)
}

fn validate_sync_file_paths(file_paths: Vec<String>) -> Result<Vec<String>, String> {
    let mut normalized = Vec::with_capacity(file_paths.len());
    let mut seen = HashSet::with_capacity(file_paths.len());
    for raw_path in file_paths {
        let raw_path = raw_path.trim();
        let file = Path::new(raw_path);
        if raw_path.is_empty()
            || !file.is_absolute()
            || !file
                .extension()
                .and_then(OsStr::to_str)
                .is_some_and(|extension| extension.eq_ignore_ascii_case("mp3"))
        {
            return Err(format!(
                "Aurora cannot synchronize a missing or invalid MP3: {}.",
                file.display()
            ));
        }
        if !file.is_file() {
            return Ok(Vec::new());
        }
        let key = raw_path.replace('/', "\\").to_lowercase();
        if seen.insert(key) {
            normalized.push(raw_path.to_owned());
        }
    }
    Ok(normalized)
}

fn validate_source_path(source_path: &str) -> Result<String, String> {
    let source_path = source_path.trim();
    if source_path.is_empty() {
        return Err("Choose an album or a folder of albums first.".to_owned());
    }

    let metadata = fs::metadata(source_path)
        .map_err(|_| "The selected album folder no longer exists or cannot be read.".to_owned())?;
    if !metadata.is_dir() {
        return Err("The selected album source must be a folder.".to_owned());
    }
    Ok(source_path.to_owned())
}

fn cleanup_abandoned_inbox_temporary_files(root: &Path, current_ms: u64) -> Result<(), String> {
    let mut pending = vec![root.to_path_buf()];
    let mut visited_entries = 0usize;
    let mut recent_temporary = None;
    while let Some(directory) = pending.pop() {
        let entries = fs::read_dir(&directory).map_err(|error| {
            format!("Aurora could not inspect the Inbox folder before previewing it: {error}")
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                format!("Aurora could not inspect an Inbox entry before previewing it: {error}")
            })?;
            visited_entries += 1;
            if visited_entries > MAX_INTAKE_CLEANUP_ENTRIES {
                return Err(
                    "Aurora stopped checking for abandoned Inbox staging files because the selected folder contains too many entries."
                        .to_owned(),
                );
            }
            let file_type = entry.file_type().map_err(|error| {
                format!("Aurora could not inspect an Inbox entry before previewing it: {error}")
            })?;
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                pending.push(entry.path());
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let Some(created_ms) = inbox_temporary_timestamp(&entry.file_name()) else {
                continue;
            };
            if current_ms.saturating_sub(created_ms) < ABANDONED_INBOX_TEMP_AGE_MS {
                recent_temporary.get_or_insert_with(|| entry.path());
                continue;
            }
            fs::remove_file(entry.path()).map_err(|error| {
                format!(
                    "Aurora found an abandoned Inbox staging file but could not remove it: {} ({error})",
                    entry.path().display()
                )
            })?;
        }
    }
    if let Some(path) = recent_temporary {
        return Err(format!(
            "Aurora is still finalizing an Inbox tag edit at {}. Wait a few minutes, then preview destinations again.",
            path.display()
        ));
    }
    Ok(())
}

fn inbox_temporary_timestamp(name: &OsStr) -> Option<u64> {
    let body = name
        .to_str()?
        .strip_prefix(".aurora-inbox-")?
        .strip_suffix(".tmp.mp3")?;
    let (timestamp, index) = body.rsplit_once('-')?;
    let timestamp = timestamp.parse().ok()?;
    index.parse::<usize>().ok()?;
    Some(timestamp)
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}

fn validate_apply_request(request: &LibraryIntakeApplyRequest) -> Result<(), String> {
    if request.plan_id.trim().is_empty() || request.plan_id.len() > 256 {
        return Err("Preview the album folder again before importing it.".to_owned());
    }
    if request.session_id <= 0 {
        return Err("The album import preview is invalid. Preview the folder again.".to_owned());
    }
    Ok(())
}

fn validate_sync_folder_paths(folder_paths: Vec<String>) -> Result<Vec<String>, String> {
    if folder_paths.is_empty() || folder_paths.len() > MAX_SYNC_FOLDERS {
        return Err(format!(
            "Aurora can synchronize between 1 and {MAX_SYNC_FOLDERS} album folders at a time."
        ));
    }

    let mut normalized = Vec::with_capacity(folder_paths.len());
    let mut seen = HashSet::with_capacity(folder_paths.len());
    for raw_path in folder_paths {
        let raw_path = raw_path.trim();
        if raw_path.is_empty() {
            return Err("Aurora cannot synchronize an empty album folder path.".to_owned());
        }
        let folder = Path::new(raw_path);
        if !folder.is_absolute() || !folder.is_dir() {
            return Err(format!(
                "Aurora cannot synchronize a missing or invalid album folder: {}.",
                folder.display()
            ));
        }
        let key = raw_path.replace('/', "\\").to_lowercase();
        if seen.insert(key) {
            normalized.push(raw_path.to_owned());
        }
    }
    if normalized.is_empty() {
        return Err("Aurora could not identify an album folder to synchronize.".to_owned());
    }
    Ok(normalized)
}

fn validate_sync_result(
    result: &LibraryExistingFoldersSyncResult,
    requested_folders: &[String],
) -> Result<(), String> {
    let updated_receipts = result
        .folders
        .iter()
        .filter(|folder| folder.status == LibraryFolderSyncStatus::Updated)
        .count();
    let changed_tracks = result
        .folders
        .iter()
        .map(|folder| folder.changed_tracks)
        .sum::<i64>();
    let changed_albums = result
        .folders
        .iter()
        .map(|folder| folder.changed_albums)
        .sum::<i64>();
    let receipt_import_run_ids = result
        .folders
        .iter()
        .filter_map(|folder| folder.import_run_id)
        .collect::<Vec<_>>();
    let receipt_backup_paths = result
        .folders
        .iter()
        .filter_map(|folder| folder.backup_path.clone())
        .collect::<Vec<_>>();

    let invalid_receipt = result.folders.iter().any(|folder| {
        folder.folder_path.trim().is_empty()
            || folder.changed_tracks < 0
            || folder.changed_albums < 0
            || folder.import_run_id.is_some_and(|id| id <= 0)
            || (folder.status == LibraryFolderSyncStatus::Unchanged
                && (folder.changed_tracks != 0
                    || folder.changed_albums != 0
                    || folder.import_run_id.is_some()
                    || folder.backup_path.is_some()))
            || (folder.status == LibraryFolderSyncStatus::Updated && folder.import_run_id.is_none())
    });
    let unique_run_ids = result
        .import_run_ids
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let unique_folders = result
        .folders
        .iter()
        .map(|folder| folder.folder_path.replace('/', "\\").to_lowercase())
        .collect::<HashSet<_>>();
    let requested_folder_keys = requested_folders
        .iter()
        .map(|folder| folder.replace('/', "\\").to_lowercase())
        .collect::<HashSet<_>>();

    if result.synced_folder_count != requested_folders.len()
        || result.synced_folder_count != result.folders.len()
        || result.updated_folder_count != updated_receipts
        || result.changed_tracks != changed_tracks
        || result.changed_albums != changed_albums
        || result.import_run_ids != receipt_import_run_ids
        || result.backup_paths != receipt_backup_paths
        || unique_run_ids.len() != result.import_run_ids.len()
        || unique_folders.len() != result.folders.len()
        || unique_folders != requested_folder_keys
        || result.import_run_ids.iter().any(|id| *id <= 0)
        || invalid_receipt
    {
        return Err(update_music_library_message(
            "Music Library returned an invalid tag-sync receipt.".to_owned(),
        ));
    }
    Ok(())
}

fn validate_capabilities(result: &LibraryBridgeCapabilities) -> Result<(), String> {
    if result.bridge_version != PROTOCOL_VERSION {
        return Err(update_music_library_message(format!(
            "Aurora needs bridge version {PROTOCOL_VERSION}, but Music Library reported version {}.",
            result.bridge_version
        )));
    }
    if !result.supports.single_album
        || !result.supports.batch_folders
        || !result.supports.cross_volume_copy
        || !result.supports.preview_required
    {
        return Err(update_music_library_message(
            "The installed bridge does not support Aurora's safe batch-import workflow.".to_owned(),
        ));
    }

    for expected in [
        LibraryCategoryId::General,
        LibraryCategoryId::Scores,
        LibraryCategoryId::Synthwave,
    ] {
        if result
            .categories
            .iter()
            .filter(|category| category.id == expected)
            .count()
            != 1
        {
            return Err(update_music_library_message(
                "The installed bridge does not provide all three Aurora library destinations."
                    .to_owned(),
            ));
        }
    }

    if result
        .categories
        .iter()
        .any(|category| category.available && category.destination_root.trim().is_empty())
    {
        return Err(update_music_library_message(
            "Music Library reported an invalid destination folder.".to_owned(),
        ));
    }
    Ok(())
}

fn validate_tag_sync_capabilities(result: &LibraryBridgeCapabilities) -> Result<(), String> {
    if !result.supports.sync_existing_folders || !result.supports.default_popm_rating_fallback {
        return Err(update_music_library_message(
            "Aurora needs Music Library 0.144.2 or newer for safe tag synchronization.".to_owned(),
        ));
    }
    Ok(())
}

fn validate_preview(
    result: &LibraryIntakePreview,
    requested_category: LibraryCategoryId,
) -> Result<(), String> {
    if result.plan_id.trim().is_empty() || result.session_id <= 0 {
        return Err(update_music_library_message(
            "Music Library returned an invalid preview plan.".to_owned(),
        ));
    }
    if result.category.id != requested_category {
        return Err(update_music_library_message(
            "Music Library returned a preview for the wrong destination.".to_owned(),
        ));
    }
    Ok(())
}

fn validate_apply_result(
    result: &LibraryIntakeApplyResult,
    request: &LibraryIntakeApplyRequest,
) -> Result<(), String> {
    let removed_sources = result
        .albums
        .iter()
        .filter(|album| album.cleanup_status == LibraryIntakeCleanupStatus::Removed)
        .count() as u64;
    if result.plan_id != request.plan_id
        || result.session_id != request.session_id
        || result.import_run_id <= 0
        || result.album_count == 0
        || result.track_count == 0
        || result.albums.len() as u64 != result.album_count
        || result.moved_album_count > result.album_count
        || result.moved_album_count != removed_sources
    {
        return Err(update_music_library_message(
            "Music Library returned an invalid import receipt.".to_owned(),
        ));
    }
    Ok(())
}

fn record_album_additions(
    app: &AppHandle,
    result: &LibraryIntakeApplyResult,
) -> Result<(), String> {
    let connection = open_catalog(&default_catalog_path()?)?;
    let additions = resolve_album_additions(&connection, &result.albums)?;
    let added_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    app.state::<StateStore>()
        .record_album_additions(&additions, result.import_run_id, added_at_ms)
}

fn resolve_album_additions(
    connection: &Connection,
    albums: &[LibraryIntakeAppliedAlbum],
) -> Result<Vec<(String, String)>, String> {
    let mut exact_statement = connection
        .prepare(
            r#"
            SELECT DISTINCT album_id
            FROM tracks
            WHERE file_path IN (?1, ?2, ?3)
            LIMIT 2
            "#,
        )
        .map_err(|error| format!("Could not prepare the imported-album lookup: {error}"))?;
    let mut descendant_statement = connection
        .prepare(
            r#"
            SELECT DISTINCT album_id
            FROM tracks
            WHERE file_path LIKE ?1 ESCAPE '^' COLLATE NOCASE
            LIMIT 2
            "#,
        )
        .map_err(|error| format!("Could not prepare the nested imported-album lookup: {error}"))?;
    let mut additions = Vec::with_capacity(albums.len());
    for album in albums {
        let catalog_destination =
            device_mode::catalog_path_for_device_path(Path::new(&album.destination_path));
        let catalog_destination = catalog_destination.to_string_lossy();
        let destination = catalog_destination.trim_end_matches(['\\', '/']);
        let escaped = destination
            .replace('^', "^^")
            .replace('%', "^%")
            .replace('_', "^_");
        let descendant_pattern = format!("{escaped}\\%");
        let windows_directory = format!("{destination}\\");
        let portable_directory = format!("{destination}/");
        let mut album_ids = exact_statement
            .query_map(
                [
                    destination,
                    windows_directory.as_str(),
                    portable_directory.as_str(),
                ],
                |row| row.get::<_, String>(0),
            )
            .map_err(|error| format!("Could not read the imported album identity: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Could not decode the imported album identity: {error}"))?;
        if album_ids.is_empty() {
            album_ids = descendant_statement
                .query_map([descendant_pattern.as_str()], |row| row.get::<_, String>(0))
                .map_err(|error| {
                    format!("Could not read the nested imported album identity: {error}")
                })?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| {
                    format!("Could not decode the nested imported album identity: {error}")
                })?;
        }
        let [album_id] = album_ids.as_slice() else {
            return Err(format!(
                "the catalog did not resolve exactly one album at {}",
                album.destination_path
            ));
        };
        additions.push((album_id.clone(), album.destination_path.clone()));
    }
    Ok(additions)
}

fn invoke_bridge<TRequest, TResponse>(
    app: &AppHandle,
    operation: &'static str,
    payload: TRequest,
    timeout: Duration,
) -> Result<TResponse, String>
where
    TRequest: Serialize,
    TResponse: DeserializeOwned,
{
    let executable = discover_music_library_executable()?;
    let bridge_directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Aurora could not locate its app-data folder: {error}"))?
        .join("music-library-bridge");
    invoke_bridge_at(&executable, &bridge_directory, operation, payload, timeout)
}

fn invoke_bridge_at<TRequest, TResponse>(
    executable: &Path,
    bridge_directory: &Path,
    operation: &'static str,
    payload: TRequest,
    timeout: Duration,
) -> Result<TResponse, String>
where
    TRequest: Serialize,
    TResponse: DeserializeOwned,
{
    let (exchange, request_file) = ExchangeFiles::create(bridge_directory)?;
    let request = ProtocolRequest {
        protocol_version: PROTOCOL_VERSION,
        operation,
        payload,
    };
    let mut writer = BufWriter::new(request_file);
    serde_json::to_writer(&mut writer, &request)
        .map_err(|error| format!("Aurora could not encode the Music Library request: {error}"))?;
    writer
        .flush()
        .map_err(|error| format!("Aurora could not save the Music Library request: {error}"))?;
    writer
        .get_ref()
        .sync_all()
        .map_err(|error| format!("Aurora could not finish the Music Library request: {error}"))?;
    drop(writer);

    let mut command = Command::new(executable);
    command
        .arg("--aurora-bridge")
        .arg(&exchange.request_path)
        .arg(&exchange.response_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = command.spawn().map_err(|error| {
        update_music_library_message(format!(
            "Aurora could not start the Music Library bridge: {error}"
        ))
    })?;
    let status = wait_for_child(&mut child, timeout, operation)?;
    let response = read_protocol_response::<TResponse>(&exchange.response_path, operation);

    if !status.success() {
        return match response {
            Err(error) => Err(error),
            Ok(_) => Err(format!(
                "Music Library stopped before it completed {operation} (exit code {}).",
                status
                    .code()
                    .map_or_else(|| "unknown".to_owned(), |code| code.to_string())
            )),
        };
    }
    response
}

fn wait_for_child(
    child: &mut Child,
    timeout: Duration,
    operation: &str,
) -> Result<ExitStatus, String> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if started.elapsed() < timeout => thread::sleep(CHILD_POLL_INTERVAL),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "Music Library took too long to complete {operation}. Nothing further was sent by Aurora; try the preview again."
                ));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "Aurora could not monitor the Music Library bridge: {error}"
                ));
            }
        }
    }
}

fn read_protocol_response<T: DeserializeOwned>(
    response_path: &Path,
    operation: &str,
) -> Result<T, String> {
    let file = File::open(response_path).map_err(|_| {
        update_music_library_message(format!(
            "Music Library did not return a response for {operation}."
        ))
    })?;
    let length = file
        .metadata()
        .map_err(|error| format!("Aurora could not inspect the Music Library response: {error}"))?
        .len();
    if length > MAX_RESPONSE_BYTES {
        return Err(update_music_library_message(
            "Music Library returned an unexpectedly large bridge response.".to_owned(),
        ));
    }

    let mut bytes = Vec::with_capacity(length as usize);
    BufReader::new(file)
        .take(MAX_RESPONSE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Aurora could not read the Music Library response: {error}"))?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(update_music_library_message(
            "Music Library returned an unexpectedly large bridge response.".to_owned(),
        ));
    }
    let response: ProtocolResponse<T> = serde_json::from_slice(&bytes).map_err(|error| {
        update_music_library_message(format!(
            "Music Library returned an incompatible response: {error}"
        ))
    })?;

    if response.protocol_version != PROTOCOL_VERSION {
        return Err(update_music_library_message(format!(
            "Aurora needs protocol version {PROTOCOL_VERSION}, but Music Library returned version {}.",
            response.protocol_version
        )));
    }
    if !response.ok {
        let error = response.error.ok_or_else(|| {
            update_music_library_message(
                "Music Library returned an incomplete bridge error.".to_owned(),
            )
        })?;
        return Err(protocol_error_message(operation, error));
    }
    response.result.ok_or_else(|| {
        update_music_library_message("Music Library returned no bridge result.".to_owned())
    })
}

fn protocol_error_message(operation: &str, error: ProtocolError) -> String {
    let normalized_code = error.code.to_ascii_lowercase();
    let needs_update = operation == "capabilities"
        || ["protocol", "version", "bridge", "operation", "unsupported"]
            .iter()
            .any(|fragment| normalized_code.contains(fragment));
    if needs_update {
        update_music_library_message(format!("{} ({})", error.message, error.code))
    } else {
        format!(
            "Music Library could not complete {operation}: {} ({})",
            error.message, error.code
        )
    }
}

fn update_music_library_message(detail: String) -> String {
    format!(
        "Music Library must be updated (or installed if missing) before Aurora can add albums or synchronize edited tags. {detail}"
    )
}

fn discover_music_library_executable() -> Result<PathBuf, String> {
    discover_music_library_executable_from(
        std::env::var_os(MUSIC_LIBRARY_EXE_ENV),
        std::env::var_os("LOCALAPPDATA"),
    )
}

fn discover_music_library_executable_from(
    override_path: Option<OsString>,
    local_app_data: Option<OsString>,
) -> Result<PathBuf, String> {
    let candidate = if let Some(path) = override_path {
        PathBuf::from(path)
    } else {
        let local_app_data = local_app_data.ok_or_else(|| {
            "Aurora could not locate Music Library because LOCALAPPDATA is unavailable.".to_owned()
        })?;
        PathBuf::from(local_app_data)
            .join("Music Library")
            .join("music-library.exe")
    };
    validate_executable(&candidate)
}

fn validate_executable(candidate: &Path) -> Result<PathBuf, String> {
    let plausible_name = candidate
        .file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name.eq_ignore_ascii_case("music-library.exe"));
    if !plausible_name {
        return Err(format!(
            "The {MUSIC_LIBRARY_EXE_ENV} path must point to music-library.exe."
        ));
    }
    if !candidate.is_file() {
        return Err(update_music_library_message(format!(
            "Aurora could not find music-library.exe at {}.",
            candidate.display()
        )));
    }
    candidate.canonicalize().map_err(|error| {
        update_music_library_message(format!(
            "Aurora could not validate music-library.exe: {error}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_response(path: &Path, value: serde_json::Value) {
        fs::write(path, serde_json::to_vec(&value).expect("response JSON"))
            .expect("write response");
    }

    #[test]
    fn resolves_completed_intake_paths_to_stable_album_ids() {
        let connection = Connection::open_in_memory().expect("catalog fixture");
        connection
            .execute_batch(
                r#"
                CREATE TABLE tracks(album_id TEXT NOT NULL, file_path TEXT NOT NULL);
                CREATE INDEX idx_tracks_file ON tracks(file_path);
                INSERT INTO tracks VALUES
                  ('album-root', 'D:\MUSIC\Artist\Album\'),
                  ('album-nested', 'D:\MUSIC\Artist\100% Score\Disc 1\');
                "#,
            )
            .expect("catalog fixture schema");
        let albums = vec![
            LibraryIntakeAppliedAlbum {
                source_path: r"C:\Inbox\Album".to_owned(),
                destination_path: r"Y:\Music\Artist\Album".to_owned(),
                cleanup_status: LibraryIntakeCleanupStatus::Removed,
            },
            LibraryIntakeAppliedAlbum {
                source_path: r"C:\Inbox\Score".to_owned(),
                destination_path: r"Y:\Music\Artist\100% Score".to_owned(),
                cleanup_status: LibraryIntakeCleanupStatus::Removed,
            },
        ];

        assert_eq!(
            resolve_album_additions(&connection, &albums).expect("resolve album additions"),
            vec![
                ("album-root".to_owned(), r"Y:\Music\Artist\Album".to_owned()),
                (
                    "album-nested".to_owned(),
                    r"Y:\Music\Artist\100% Score".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn protocol_request_uses_the_exact_v1_shape() {
        let request = ProtocolRequest {
            protocol_version: PROTOCOL_VERSION,
            operation: "previewBatch",
            payload: PreviewPayload {
                source_path: r"C:\Inbox\Scores",
                category: LibraryCategoryId::Scores,
            },
        };
        let value = serde_json::to_value(request).expect("request");
        assert_eq!(
            value,
            serde_json::json!({
                "protocolVersion": 1,
                "operation": "previewBatch",
                "payload": {
                    "sourcePath": r"C:\Inbox\Scores",
                    "category": "scores"
                }
            })
        );
    }

    #[test]
    fn preview_cleanup_removes_only_abandoned_inbox_temporary_mp3s() {
        let directory = TempDir::new().expect("temp directory");
        let nested = directory.path().join("Album");
        fs::create_dir_all(&nested).expect("nested album");
        let current_ms = 10 * ABANDONED_INBOX_TEMP_AGE_MS;
        let abandoned_ms = current_ms - ABANDONED_INBOX_TEMP_AGE_MS;
        let abandoned = nested.join(format!(".aurora-inbox-{abandoned_ms}-3.tmp.mp3"));
        let recovery = nested.join(format!(".aurora-inbox-{abandoned_ms}-3.backup.mp3"));
        let malformed = nested.join(".aurora-inbox-unknown-3.tmp.mp3");
        let track = nested.join("03 - Track.mp3");
        for path in [&abandoned, &recovery, &malformed, &track] {
            fs::write(path, b"fixture").expect("fixture file");
        }

        cleanup_abandoned_inbox_temporary_files(directory.path(), current_ms)
            .expect("cleanup succeeds");

        assert!(!abandoned.exists());
        assert!(recovery.exists());
        assert!(malformed.exists());
        assert!(track.exists());
    }

    #[test]
    fn preview_cleanup_refuses_to_race_a_recent_inbox_tag_edit() {
        let directory = TempDir::new().expect("temp directory");
        let current_ms = 10 * ABANDONED_INBOX_TEMP_AGE_MS;
        let recent = directory
            .path()
            .join(format!(".aurora-inbox-{}-0.tmp.mp3", current_ms - 1));
        fs::write(&recent, b"fixture").expect("fixture file");

        let error = cleanup_abandoned_inbox_temporary_files(directory.path(), current_ms)
            .expect_err("recent staging file blocks preview");

        assert!(recent.exists());
        assert!(error.contains("still finalizing an Inbox tag edit"));
    }

    #[test]
    fn protocol_response_deserializes_the_preview_contract() {
        let directory = TempDir::new().expect("temp directory");
        let response_path = directory.path().join("response.json");
        write_response(
            &response_path,
            serde_json::json!({
                "protocolVersion": 1,
                "ok": true,
                "result": {
                    "planId": "plan-1",
                    "sessionId": 42,
                    "sourcePath": r"C:\Inbox",
                    "category": {
                        "id": "scores",
                        "label": "Movie / TV / game music",
                        "destinationRoot": r"D:\Scores"
                    },
                    "albumCount": 1,
                    "trackCount": 12,
                    "delta": {
                        "addedTracks": 12,
                        "changedTracks": 0,
                        "removedTracks": 0,
                        "addedAlbums": 1,
                        "changedAlbums": 0,
                        "removedAlbums": 0
                    },
                    "albums": [{
                        "sourcePath": r"C:\Inbox\Album",
                        "destinationPath": r"D:\Scores\Album",
                        "artist": "Composer",
                        "album": "Score",
                        "year": "2026",
                        "trackCount": 12
                    }],
                    "canApply": true
                }
            }),
        );

        let preview: LibraryIntakePreview =
            read_protocol_response(&response_path, "previewBatch").expect("preview");
        assert_eq!(preview.session_id, 42);
        assert_eq!(preview.category.id, LibraryCategoryId::Scores);
        assert_eq!(preview.albums[0].track_count, 12);
    }

    #[test]
    fn protocol_response_deserializes_and_validates_capabilities() {
        let directory = TempDir::new().expect("temp directory");
        let response_path = directory.path().join("response.json");
        write_response(
            &response_path,
            serde_json::json!({
                "protocolVersion": 1,
                "ok": true,
                "result": {
                    "bridgeVersion": 1,
                    "categories": [
                        {"id": "general", "label": "General music", "destinationRoot": r"D:\Music", "available": true},
                        {"id": "scores", "label": "Movie / TV / game music", "destinationRoot": r"D:\Scores", "available": true},
                        {"id": "synthwave", "label": "Synthwave", "destinationRoot": r"D:\Synthwave", "available": true}
                    ],
                    "supports": {
                        "singleAlbum": true,
                        "batchFolders": true,
                        "crossVolumeCopy": true,
                        "previewRequired": true
                    }
                }
            }),
        );

        let capabilities: LibraryBridgeCapabilities =
            read_protocol_response(&response_path, "capabilities").expect("capabilities");
        validate_capabilities(&capabilities).expect("valid capabilities");
        assert_eq!(capabilities.categories.len(), 3);
        assert!(!capabilities.supports.sync_existing_folders);
        assert!(!capabilities.supports.default_popm_rating_fallback);
        assert!(validate_tag_sync_capabilities(&capabilities).is_err());
    }

    #[test]
    fn tag_sync_requires_the_default_popm_preservation_capability() {
        let capabilities = LibraryBridgeCapabilities {
            bridge_version: PROTOCOL_VERSION,
            categories: Vec::new(),
            supports: LibraryBridgeSupports {
                single_album: true,
                batch_folders: true,
                cross_volume_copy: true,
                preview_required: true,
                sync_existing_folders: true,
                default_popm_rating_fallback: true,
            },
        };

        validate_tag_sync_capabilities(&capabilities).expect("safe tag-sync capability");
    }

    #[test]
    fn protocol_request_uses_the_exact_existing_folder_sync_shape() {
        let folder_paths = vec![r"D:\Scores\Album".to_owned()];
        let changed_file_paths = vec![r"D:\Scores\Album\Track.mp3".to_owned()];
        let request = ProtocolRequest {
            protocol_version: PROTOCOL_VERSION,
            operation: "syncExistingFolders",
            payload: SyncExistingFoldersPayload {
                folder_paths: &folder_paths,
                changed_file_paths: Some(&changed_file_paths),
            },
        };
        let value = serde_json::to_value(request).expect("request");
        assert_eq!(
            value,
            serde_json::json!({
                "protocolVersion": 1,
                "operation": "syncExistingFolders",
                "payload": {
                    "folderPaths": [r"D:\Scores\Album"],
                    "changedFilePaths": [r"D:\Scores\Album\Track.mp3"]
                }
            })
        );
    }

    #[test]
    fn protocol_request_omits_exact_files_for_safe_full_folder_fallback() {
        let folder_paths = vec![r"D:\Scores\Album".to_owned()];
        let request = ProtocolRequest {
            protocol_version: PROTOCOL_VERSION,
            operation: "syncExistingFolders",
            payload: SyncExistingFoldersPayload {
                folder_paths: &folder_paths,
                changed_file_paths: None,
            },
        };

        assert_eq!(
            serde_json::to_value(request).expect("request"),
            serde_json::json!({
                "protocolVersion": 1,
                "operation": "syncExistingFolders",
                "payload": {"folderPaths": [r"D:\Scores\Album"]}
            })
        );
    }

    #[test]
    fn missing_exact_file_downgrades_to_safe_full_folder_sync() {
        let directory = TempDir::new().expect("temp directory");
        let missing = directory.path().join("Missing.mp3");

        assert!(
            validate_sync_file_paths(vec![missing.to_string_lossy().into_owned()])
                .expect("missing exact target falls back")
                .is_empty()
        );
    }

    #[test]
    fn protocol_response_deserializes_and_validates_existing_folder_sync() {
        let directory = TempDir::new().expect("temp directory");
        let response_path = directory.path().join("response.json");
        write_response(
            &response_path,
            serde_json::json!({
                "protocolVersion": 1,
                "ok": true,
                "result": {
                    "syncedFolderCount": 2,
                    "updatedFolderCount": 1,
                    "changedTracks": 12,
                    "changedAlbums": 1,
                    "importRunIds": [91],
                    "backupPaths": [r"C:\Backups\sync.sqlite3"],
                    "folders": [
                        {
                            "folderPath": r"D:\Scores\Album One",
                            "status": "updated",
                            "changedTracks": 12,
                            "changedAlbums": 1,
                            "importRunId": 91,
                            "backupPath": r"C:\Backups\sync.sqlite3"
                        },
                        {
                            "folderPath": r"D:\Scores\Album Two",
                            "status": "unchanged",
                            "changedTracks": 0,
                            "changedAlbums": 0,
                            "importRunId": null,
                            "backupPath": null
                        }
                    ]
                }
            }),
        );

        let receipt: LibraryExistingFoldersSyncResult =
            read_protocol_response(&response_path, "syncExistingFolders").expect("sync receipt");
        validate_sync_result(
            &receipt,
            &[
                r"D:\Scores\Album One".to_owned(),
                r"D:\Scores\Album Two".to_owned(),
            ],
        )
        .expect("valid sync receipt");
        assert_eq!(receipt.updated_folder_count, 1);
    }

    #[test]
    fn existing_folder_sync_receipt_must_name_the_requested_folders() {
        let receipt = LibraryExistingFoldersSyncResult {
            synced_folder_count: 1,
            updated_folder_count: 0,
            changed_tracks: 0,
            changed_albums: 0,
            import_run_ids: Vec::new(),
            backup_paths: Vec::new(),
            folders: vec![LibraryFolderSyncReceipt {
                folder_path: r"D:\Scores\Different Album".to_owned(),
                status: LibraryFolderSyncStatus::Unchanged,
                changed_tracks: 0,
                changed_albums: 0,
                import_run_id: None,
                backup_path: None,
            }],
        };

        assert!(
            validate_sync_result(&receipt, &[r"D:\Scores\Requested Album".to_owned()]).is_err()
        );
    }

    #[test]
    fn protocol_response_deserializes_the_apply_receipt_contract() {
        let directory = TempDir::new().expect("temp directory");
        let response_path = directory.path().join("response.json");
        write_response(
            &response_path,
            serde_json::json!({
                "protocolVersion": 1,
                "ok": true,
                "result": {
                    "planId": "plan-1",
                    "sessionId": 42,
                    "status": "completedWithWarnings",
                    "albumCount": 1,
                    "trackCount": 12,
                    "movedAlbumCount": 1,
                    "importRunId": 91,
                    "backupPath": null,
                    "albums": [{
                        "sourcePath": r"C:\Inbox\Album",
                        "destinationPath": r"D:\Scores\Album",
                        "cleanupStatus": "retained"
                    }],
                    "cleanupWarnings": ["The empty source folder was retained."]
                }
            }),
        );

        let receipt: LibraryIntakeApplyResult =
            read_protocol_response(&response_path, "applyBatch").expect("apply receipt");
        assert_eq!(
            receipt.status,
            LibraryIntakeApplyStatus::CompletedWithWarnings
        );
        assert_eq!(
            receipt.albums[0].cleanup_status,
            LibraryIntakeCleanupStatus::Retained
        );
        assert_eq!(receipt.backup_path, None);
    }

    #[test]
    fn protocol_failure_preserves_the_helpers_actionable_message() {
        let directory = TempDir::new().expect("temp directory");
        let response_path = directory.path().join("response.json");
        write_response(
            &response_path,
            serde_json::json!({
                "protocolVersion": 1,
                "ok": false,
                "error": {
                    "code": "destination_unavailable",
                    "message": "Configure the Scores destination in Music Library."
                }
            }),
        );

        let error = read_protocol_response::<LibraryIntakePreview>(&response_path, "previewBatch")
            .expect_err("bridge error");
        assert!(error.contains("Configure the Scores destination"));
        assert!(error.contains("destination_unavailable"));
        assert!(!error.contains("must be updated"));
    }

    #[test]
    fn incompatible_protocol_explicitly_requests_a_music_library_update() {
        let directory = TempDir::new().expect("temp directory");
        let response_path = directory.path().join("response.json");
        write_response(
            &response_path,
            serde_json::json!({
                "protocolVersion": 2,
                "ok": true,
                "result": {}
            }),
        );

        let error = read_protocol_response::<serde_json::Value>(&response_path, "capabilities")
            .expect_err("protocol mismatch");
        assert!(error.contains("Music Library must be updated"));
        assert!(error.contains("protocol version 1"));
    }

    #[test]
    fn executable_discovery_prefers_and_validates_the_override() {
        let directory = TempDir::new().expect("temp directory");
        let override_path = directory.path().join("music-library.exe");
        fs::write(&override_path, b"fake helper").expect("fake executable");

        let discovered = discover_music_library_executable_from(
            Some(override_path.clone().into_os_string()),
            Some(OsString::from(r"C:\Ignored")),
        )
        .expect("override");
        assert_eq!(discovered, override_path.canonicalize().expect("canonical"));
    }

    #[test]
    fn executable_discovery_uses_the_documented_local_app_data_location() {
        let directory = TempDir::new().expect("temp directory");
        let install_directory = directory.path().join("Music Library");
        fs::create_dir_all(&install_directory).expect("install directory");
        let executable = install_directory.join("music-library.exe");
        fs::write(&executable, b"fake helper").expect("fake executable");

        let discovered = discover_music_library_executable_from(
            None,
            Some(directory.path().as_os_str().to_owned()),
        )
        .expect("default executable");
        assert_eq!(discovered, executable.canonicalize().expect("canonical"));
    }

    #[test]
    fn exchange_files_are_removed_on_drop() {
        let directory = TempDir::new().expect("temp directory");
        let (exchange, request_file) = ExchangeFiles::create(directory.path()).expect("exchange");
        drop(request_file);
        fs::write(&exchange.response_path, b"{}").expect("response");
        let request_path = exchange.request_path.clone();
        let response_path = exchange.response_path.clone();
        drop(exchange);

        assert!(!request_path.exists());
        assert!(!response_path.exists());
    }

    #[test]
    fn apply_receipt_must_match_the_preview_identity() {
        let request = LibraryIntakeApplyRequest {
            plan_id: "plan-1".to_owned(),
            session_id: 7,
        };
        let result = LibraryIntakeApplyResult {
            plan_id: "different-plan".to_owned(),
            session_id: 7,
            status: LibraryIntakeApplyStatus::Completed,
            album_count: 1,
            track_count: 8,
            moved_album_count: 1,
            import_run_id: 2,
            backup_path: None,
            albums: Vec::new(),
            cleanup_warnings: Vec::new(),
        };

        let error = validate_apply_result(&result, &request).expect_err("mismatched receipt");
        assert!(error.contains("invalid import receipt"));
    }

    #[test]
    fn apply_receipt_moved_count_must_match_removed_sources() {
        let request = LibraryIntakeApplyRequest {
            plan_id: "plan-1".to_owned(),
            session_id: 7,
        };
        let result = LibraryIntakeApplyResult {
            plan_id: request.plan_id.clone(),
            session_id: request.session_id,
            status: LibraryIntakeApplyStatus::CompletedWithWarnings,
            album_count: 1,
            track_count: 8,
            moved_album_count: 1,
            import_run_id: 2,
            backup_path: None,
            albums: vec![LibraryIntakeAppliedAlbum {
                source_path: r"C:\Inbox\Album".to_owned(),
                destination_path: r"D:\MUSIC\Album".to_owned(),
                cleanup_status: LibraryIntakeCleanupStatus::Retained,
            }],
            cleanup_warnings: vec!["Source retained".to_owned()],
        };

        let error = validate_apply_result(&result, &request).expect_err("invalid moved count");
        assert!(error.contains("invalid import receipt"));
    }
}
