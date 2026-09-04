use crate::{
    library_bridge::sync_existing_library_folders,
    state_store::{PendingLibraryFolderSync, StateStore},
};
use serde::Serialize;
use std::{
    collections::HashSet,
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use tauri::{AppHandle, Manager};

const BACKGROUND_RETRY_FOLDERS: usize = 1;
const MAX_EXACT_OVERLAY_RETRIES: usize = 100;
const TRANSIENT_RETRY_DELAY_MS: i64 = 30_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CatalogSyncStatus {
    Synced,
    Pending,
    Blocked,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogSync {
    pub(crate) status: CatalogSyncStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) message: Option<String>,
    pub(crate) pending_folder_count: usize,
    pub(crate) blocked_folder_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) projection_token: Option<u64>,
}

impl CatalogSync {
    fn synced() -> Self {
        Self {
            status: CatalogSyncStatus::Synced,
            message: Some("Music Library updated.".to_owned()),
            pending_folder_count: 0,
            blocked_folder_count: 0,
            projection_token: None,
        }
    }

    fn pending(
        retryable_folder_count: usize,
        blocked_folder_count: usize,
        detail: Option<String>,
    ) -> Self {
        let mut message =
            "Music Library update pending; Aurora will retry automatically.".to_owned();
        if let Some(detail) = detail.filter(|detail| !detail.trim().is_empty()) {
            message.push(' ');
            message.push_str(detail.trim());
        }
        if blocked_folder_count > 0 {
            message.push_str(&format!(
                " {blocked_folder_count} other folder{} will not retry until its MP3s change again.",
                if blocked_folder_count == 1 { "" } else { "s" }
            ));
        }
        Self {
            status: CatalogSyncStatus::Pending,
            message: Some(message),
            pending_folder_count: retryable_folder_count + blocked_folder_count,
            blocked_folder_count,
            projection_token: None,
        }
    }

    fn blocked(blocked_folder_count: usize, detail: Option<String>) -> Self {
        let mut message = format!(
            "Music Library update needs attention for {blocked_folder_count} folder{}; automatic retries are paused until those MP3s change again.",
            if blocked_folder_count == 1 { "" } else { "s" }
        );
        if let Some(detail) = detail.filter(|detail| !detail.trim().is_empty()) {
            message.push(' ');
            message.push_str(detail.trim());
        }
        Self {
            status: CatalogSyncStatus::Blocked,
            message: Some(message),
            pending_folder_count: blocked_folder_count,
            blocked_folder_count,
            projection_token: None,
        }
    }
}

#[derive(Debug)]
pub(crate) struct LibrarySyncReport {
    pub(crate) catalog_sync: CatalogSync,
    completed_directories: HashSet<String>,
}

impl LibrarySyncReport {
    pub(crate) fn completed(&self, directory: &str) -> bool {
        self.completed_directories
            .contains(&normalized_directory(directory))
    }
}

pub(crate) struct LibrarySyncCoordinator {
    edit_gate: Mutex<()>,
    bridge_gate: Mutex<()>,
    next_projection_token: AtomicU64,
}

impl Default for LibrarySyncCoordinator {
    fn default() -> Self {
        Self {
            edit_gate: Mutex::new(()),
            bridge_gate: Mutex::new(()),
            next_projection_token: AtomicU64::new(1),
        }
    }
}

impl LibrarySyncCoordinator {
    /// Keeps the authoritative MP3 write, durable queue receipt, and native playback projection in
    /// one order. The returned token lets the frontend reject an older response delivered late.
    pub(crate) fn serialize_tag_edit<T>(&self, operation: impl FnOnce() -> T) -> (T, u64) {
        self.serialize_projection_epoch(operation)
    }

    /// Reserves a delivery token before background reconciliation starts. Foreground edits that
    /// begin later receive newer tokens without waiting for the background worker to finish.
    pub(crate) fn reserve_background_projection_token(&self) -> u64 {
        self.next_projection_token.fetch_add(1, Ordering::SeqCst)
    }

    fn serialize_projection_epoch<T>(&self, operation: impl FnOnce() -> T) -> (T, u64) {
        let _guard = self
            .edit_gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let result = operation();
        let token = self.next_projection_token.fetch_add(1, Ordering::SeqCst);
        (result, token)
    }

    pub(crate) fn queue_after_edit(
        &self,
        app: &AppHandle,
        directories: &[String],
    ) -> LibrarySyncReport {
        let store = app.state::<StateStore>();
        let priorities = unique_directories(directories);
        let targets = match store.pending_library_folder_sync_for_paths(&priorities) {
            Ok(targets) => targets,
            Err(error) => {
                return LibrarySyncReport {
                    catalog_sync: CatalogSync::pending(priorities.len().max(1), 0, Some(error)),
                    completed_directories: HashSet::new(),
                };
            }
        };
        let target_keys = targets
            .iter()
            .map(|target| normalized_directory(&target.directory))
            .collect::<HashSet<_>>();
        let completed_directories = priorities
            .iter()
            .map(|directory| normalized_directory(directory))
            .filter(|directory| !target_keys.contains(directory))
            .collect::<HashSet<_>>();
        let (retryable_folder_count, blocked_folder_count) =
            match store.library_folder_sync_counts() {
                Ok(counts) => counts,
                Err(error) => {
                    return LibrarySyncReport {
                        catalog_sync: CatalogSync::pending(1, 0, Some(error)),
                        completed_directories,
                    };
                }
            };
        let catalog_sync = if targets.is_empty() {
            if blocked_folder_count > 0 {
                CatalogSync::blocked(blocked_folder_count, None)
            } else {
                CatalogSync::synced()
            }
        } else {
            CatalogSync::pending(
                retryable_folder_count.max(targets.len()),
                blocked_folder_count,
                Some("The verified MP3 edit is queued for Music Library.".to_owned()),
            )
        };
        LibrarySyncReport {
            catalog_sync,
            completed_directories,
        }
    }

    pub(crate) fn retry_one(&self, app: &AppHandle) -> LibrarySyncReport {
        self.run(app, &[])
    }

    pub(crate) fn sync_directories(
        &self,
        app: &AppHandle,
        directories: &[String],
    ) -> LibrarySyncReport {
        self.run(app, directories)
    }

    fn run(&self, app: &AppHandle, priority_directories: &[String]) -> LibrarySyncReport {
        self.serialize_bridge_work(|| self.run_locked(app, priority_directories))
    }

    pub(crate) fn serialize_bridge_work<T>(&self, operation: impl FnOnce() -> T) -> T {
        let _guard = self
            .bridge_gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        operation()
    }

    fn run_locked(&self, app: &AppHandle, priority_directories: &[String]) -> LibrarySyncReport {
        let store = app.state::<StateStore>();
        let priorities = unique_directories(priority_directories);
        let targets = if priorities.is_empty() {
            match store.pending_library_folder_sync(BACKGROUND_RETRY_FOLDERS) {
                Ok(items) => items,
                Err(error) => {
                    return LibrarySyncReport {
                        catalog_sync: CatalogSync::pending(1, 0, Some(error)),
                        completed_directories: HashSet::new(),
                    };
                }
            }
        } else {
            match store.pending_library_folder_sync_for_paths(&priorities) {
                Ok(items) => items,
                Err(error) => {
                    return LibrarySyncReport {
                        catalog_sync: CatalogSync::pending(priorities.len(), 0, Some(error)),
                        completed_directories: HashSet::new(),
                    };
                }
            }
        };

        let target_keys = targets
            .iter()
            .map(|target| normalized_directory(&target.directory))
            .collect::<HashSet<_>>();
        let mut completed_directories = priorities
            .iter()
            .map(|directory| normalized_directory(directory))
            .filter(|directory| !target_keys.contains(directory))
            .collect::<HashSet<_>>();
        let processed = process_targets(
            targets,
            |target| {
                let overlay_filenames = if target.filename.is_none() {
                    store.pending_overlay_filenames_for_directory(
                        &target.directory,
                        MAX_EXACT_OVERLAY_RETRIES,
                    )?
                } else {
                    Vec::new()
                };
                sync_target_with_overlay_fallback(target, &overlay_filenames, |changed_files| {
                    sync_existing_library_folders(
                        app,
                        vec![target.directory.clone()],
                        changed_files,
                    )
                    .map(|_| ())
                })
            },
            |target| store.complete_library_folder_sync(target),
            |target, error| {
                if transient_library_sync_error(error) {
                    store.defer_transient_library_folder_sync(
                        target,
                        error,
                        TRANSIENT_RETRY_DELAY_MS,
                    )
                } else {
                    store.defer_library_folder_sync(target, error)
                }
            },
        );
        completed_directories.extend(processed.completed_directories);
        let mut first_error = processed.first_error;

        let (retryable_folder_count, blocked_folder_count) =
            match store.library_folder_sync_counts() {
                Ok(counts) => counts,
                Err(error) => {
                    first_error.get_or_insert(error);
                    (1, 0)
                }
            };
        let catalog_sync = if retryable_folder_count > 0 {
            CatalogSync::pending(retryable_folder_count, blocked_folder_count, first_error)
        } else if blocked_folder_count > 0 {
            CatalogSync::blocked(blocked_folder_count, first_error)
        } else if let Some(error) = first_error {
            CatalogSync::pending(1, 0, Some(error))
        } else {
            CatalogSync::synced()
        };

        LibrarySyncReport {
            catalog_sync,
            completed_directories,
        }
    }
}

fn transient_library_sync_error(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    [
        "another aurora album intake is already running",
        "being used by another process",
        "could not start the music library bridge",
        "music library did not return a response",
        "music library took too long",
        "could not monitor the music library bridge",
        "database is locked",
        "music library must be updated (or installed if missing)",
    ]
    .iter()
    .any(|fragment| error.contains(fragment))
}

fn target_changed_file_paths(target: &PendingLibraryFolderSync) -> Vec<String> {
    target
        .filename
        .as_deref()
        .map(|filename| Path::new(&target.directory).join(filename))
        .filter(|path| path.is_file())
        .map(|path| path.to_string_lossy().into_owned())
        .into_iter()
        .collect()
}

fn sync_target_with_overlay_fallback(
    target: &PendingLibraryFolderSync,
    overlay_filenames: &[String],
    mut synchronize: impl FnMut(Vec<String>) -> Result<(), String>,
) -> Result<(), String> {
    let primary = synchronize(target_changed_file_paths(target));
    let Err(primary_error) = primary else {
        return Ok(());
    };
    if target.filename.is_some() || transient_library_sync_error(&primary_error) {
        return Err(primary_error);
    }

    let mut fallback_error = None;
    for filename in overlay_filenames {
        let exact_target = PendingLibraryFolderSync {
            directory: target.directory.clone(),
            filename: Some(filename.clone()),
            token: target.token,
        };
        let changed_files = target_changed_file_paths(&exact_target);
        if changed_files.is_empty() {
            continue;
        }
        if let Err(error) = synchronize(changed_files) {
            if transient_library_sync_error(&error) {
                return Err(error);
            }
            fallback_error.get_or_insert(error);
        }
    }
    Err(fallback_error.unwrap_or(primary_error))
}

#[derive(Debug, Default)]
struct ProcessedTargets {
    completed_directories: HashSet<String>,
    first_error: Option<String>,
}

fn process_targets(
    targets: Vec<PendingLibraryFolderSync>,
    mut synchronize: impl FnMut(&PendingLibraryFolderSync) -> Result<(), String>,
    mut complete: impl FnMut(&PendingLibraryFolderSync) -> Result<bool, String>,
    mut defer: impl FnMut(&PendingLibraryFolderSync, &str) -> Result<bool, String>,
) -> ProcessedTargets {
    let mut processed = ProcessedTargets::default();
    for target in targets {
        match synchronize(&target) {
            Ok(()) => match complete(&target) {
                Ok(true) => {
                    processed
                        .completed_directories
                        .insert(normalized_directory(&target.directory));
                }
                Ok(false) => {}
                Err(error) => {
                    processed.first_error.get_or_insert(error);
                }
            },
            Err(error) => match defer(&target, &error) {
                Ok(_) => {
                    processed.first_error.get_or_insert(error);
                }
                Err(state_error) => {
                    processed.first_error.get_or_insert(format!(
                        "{error} Aurora also could not update the retry metadata: {state_error}"
                    ));
                }
            },
        }
    }
    processed
}

fn unique_directories(directories: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    directories
        .iter()
        .filter_map(|directory| {
            let directory = directory.trim();
            (!directory.is_empty() && seen.insert(normalized_directory(directory)))
                .then(|| directory.to_owned())
        })
        .collect()
}

fn normalized_directory(directory: &str) -> String {
    directory
        .trim()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::HashMap,
        sync::{
            Arc, Barrier,
            atomic::{AtomicUsize, Ordering},
        },
        thread,
        time::Duration,
    };
    use tempfile::TempDir;

    fn pending(directory: &str, token: i64) -> PendingLibraryFolderSync {
        PendingLibraryFolderSync {
            directory: directory.to_owned(),
            filename: None,
            token,
        }
    }

    #[test]
    fn exact_pending_filename_becomes_one_changed_file_path() {
        let directory = TempDir::new().expect("temporary album");
        let path = directory.path().join("Track.mp3");
        std::fs::write(&path, b"fixture").expect("exact MP3");
        let target = PendingLibraryFolderSync {
            directory: directory.path().to_string_lossy().into_owned(),
            filename: Some("Track.mp3".to_owned()),
            token: 41,
        };
        assert_eq!(
            target_changed_file_paths(&target),
            vec![path.to_string_lossy()]
        );
        assert!(target_changed_file_paths(&pending(r"D:\Music\Artist\Album", 42)).is_empty());
    }

    #[test]
    fn unavailable_exact_file_falls_back_to_the_folder() {
        let directory = TempDir::new().expect("temporary album");
        let target = PendingLibraryFolderSync {
            directory: directory.path().to_string_lossy().into_owned(),
            filename: Some("Missing.mp3".to_owned()),
            token: 42,
        };

        assert!(target_changed_file_paths(&target).is_empty());
    }

    #[test]
    fn transient_bridge_failures_never_consume_the_terminal_retry_budget() {
        assert!(transient_library_sync_error(
            "Another Aurora album intake is already running"
        ));
        assert!(transient_library_sync_error(
            "Could not configure SQLite pragmas: database is locked"
        ));
        assert!(transient_library_sync_error(
            "Music Library must be updated (or installed if missing)"
        ));
        assert!(!transient_library_sync_error(
            "The selected folder contains more than one album"
        ));
    }

    #[test]
    fn permanent_folder_failure_still_attempts_each_exact_overlay_file() {
        let directory = TempDir::new().expect("temporary album");
        std::fs::write(directory.path().join("One.mp3"), b"one").expect("first MP3");
        std::fs::write(directory.path().join("Two.mp3"), b"two").expect("second MP3");
        let target = PendingLibraryFolderSync {
            directory: directory.path().to_string_lossy().into_owned(),
            filename: None,
            token: 41,
        };
        let attempts = std::cell::RefCell::new(Vec::new());

        let result = sync_target_with_overlay_fallback(
            &target,
            &["One.mp3".to_owned(), "Two.mp3".to_owned()],
            |changed_files| {
                attempts.borrow_mut().push(changed_files.clone());
                if changed_files.is_empty() {
                    Err("The folder contains an unrelated sidecar".to_owned())
                } else {
                    Ok(())
                }
            },
        );

        assert_eq!(
            attempts.into_inner(),
            vec![
                Vec::<String>::new(),
                vec![
                    directory
                        .path()
                        .join("One.mp3")
                        .to_string_lossy()
                        .into_owned()
                ],
                vec![
                    directory
                        .path()
                        .join("Two.mp3")
                        .to_string_lossy()
                        .into_owned()
                ],
            ]
        );
        assert_eq!(
            result.unwrap_err(),
            "The folder contains an unrelated sidecar"
        );
    }

    #[test]
    fn transient_folder_failure_does_not_compete_with_exact_fallbacks() {
        let target = pending(r"D:\Music\Busy Album", 41);
        let calls = std::cell::Cell::new(0);
        let error = sync_target_with_overlay_fallback(&target, &["Track.mp3".to_owned()], |_| {
            calls.set(calls.get() + 1);
            Err("database is locked".to_owned())
        })
        .unwrap_err();

        assert_eq!(calls.get(), 1);
        assert_eq!(error, "database is locked");
    }

    #[test]
    fn directory_priorities_are_case_insensitive_and_stable() {
        assert_eq!(
            unique_directories(&[
                r"D:\Music\Artist\Album".to_owned(),
                r"d:/music/artist/album/".to_owned(),
                r"G:\Scores\Album".to_owned(),
            ]),
            vec![
                r"D:\Music\Artist\Album".to_owned(),
                r"G:\Scores\Album".to_owned(),
            ]
        );
    }

    #[test]
    fn pending_copy_is_explicit_about_automatic_retry() {
        let sync = CatalogSync::pending(2, 0, Some("Music Library is busy.".to_owned()));
        assert_eq!(sync.status, CatalogSyncStatus::Pending);
        assert_eq!(sync.pending_folder_count, 2);
        assert!(sync.message.unwrap().contains("retry automatically"));
    }

    #[test]
    fn blocked_copy_is_explicit_that_automatic_retry_stopped() {
        let sync = CatalogSync::blocked(2, Some("The folder is invalid.".to_owned()));
        assert_eq!(sync.status, CatalogSyncStatus::Blocked);
        assert_eq!(sync.pending_folder_count, 2);
        assert_eq!(sync.blocked_folder_count, 2);
        let message = sync.message.unwrap();
        assert!(message.contains("automatic retries are paused"));
        assert!(message.contains("The folder is invalid."));
    }

    #[test]
    fn one_failed_folder_does_not_block_a_later_folder() {
        let targets = vec![
            pending(r"D:\Music\Poisoned", 1),
            pending(r"D:\Music\Healthy", 2),
        ];
        let attempts = std::cell::RefCell::new(Vec::new());
        let deferred = std::cell::RefCell::new(Vec::new());
        let completions = HashMap::from([
            (normalized_directory(r"D:\Music\Poisoned"), false),
            (normalized_directory(r"D:\Music\Healthy"), true),
        ]);

        let processed = process_targets(
            targets,
            |target| {
                attempts.borrow_mut().push(target.directory.clone());
                if target.directory.ends_with("Poisoned") {
                    Err("scope rejected".to_owned())
                } else {
                    Ok(())
                }
            },
            |target| {
                Ok(*completions
                    .get(&normalized_directory(&target.directory))
                    .expect("completion fixture"))
            },
            |target, error| {
                deferred
                    .borrow_mut()
                    .push((target.directory.clone(), error.to_owned()));
                Ok(true)
            },
        );

        assert_eq!(
            attempts.into_inner(),
            vec![r"D:\Music\Poisoned", r"D:\Music\Healthy"]
        );
        assert_eq!(
            deferred.into_inner(),
            vec![(r"D:\Music\Poisoned".to_owned(), "scope rejected".to_owned())]
        );
        assert!(
            processed
                .completed_directories
                .contains(&normalized_directory(r"D:\Music\Healthy"))
        );
        assert_eq!(processed.first_error.as_deref(), Some("scope rejected"));
    }

    #[test]
    fn stale_completion_is_not_reported_as_synchronized() {
        let processed = process_targets(
            vec![pending(r"D:\Music\Album", 41)],
            |_| Ok(()),
            |_| Ok(false),
            |_, _| panic!("a successful bridge call must not be deferred"),
        );

        assert!(processed.completed_directories.is_empty());
        assert!(processed.first_error.is_none());
    }

    #[test]
    fn retry_metadata_failure_still_allows_the_next_folder_to_complete() {
        let attempts = std::cell::RefCell::new(Vec::new());
        let processed = process_targets(
            vec![
                pending(r"D:\Music\First", 1),
                pending(r"D:\Music\Second", 2),
            ],
            |target| {
                attempts.borrow_mut().push(target.directory.clone());
                if target.directory.ends_with("First") {
                    Err("bridge busy".to_owned())
                } else {
                    Ok(())
                }
            },
            |_| Ok(true),
            |_, _| Err("state unavailable".to_owned()),
        );

        assert_eq!(
            attempts.into_inner(),
            vec![r"D:\Music\First", r"D:\Music\Second"]
        );
        assert!(
            processed
                .completed_directories
                .contains(&normalized_directory(r"D:\Music\Second"))
        );
        assert_eq!(
            processed.first_error.as_deref(),
            Some("bridge busy Aurora also could not update the retry metadata: state unavailable")
        );
    }

    #[test]
    fn coordinator_gate_serializes_concurrent_workers() {
        let coordinator = Arc::new(LibrarySyncCoordinator::default());
        let start = Arc::new(Barrier::new(5));
        let active = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        let mut workers = Vec::new();

        for _ in 0..4 {
            let coordinator = Arc::clone(&coordinator);
            let start = Arc::clone(&start);
            let active = Arc::clone(&active);
            let maximum = Arc::clone(&maximum);
            workers.push(thread::spawn(move || {
                start.wait();
                coordinator.serialize_bridge_work(|| {
                    let now_active = active.fetch_add(1, Ordering::SeqCst) + 1;
                    maximum.fetch_max(now_active, Ordering::SeqCst);
                    thread::sleep(Duration::from_millis(5));
                    active.fetch_sub(1, Ordering::SeqCst);
                });
            }));
        }

        start.wait();
        for worker in workers {
            worker.join().expect("serialized worker");
        }
        assert_eq!(maximum.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn tag_edit_gate_orders_write_bridge_projection_and_tokens() {
        let coordinator = Arc::new(LibrarySyncCoordinator::default());
        let first_inside = Arc::new(Barrier::new(2));
        let release_first = Arc::new(Barrier::new(2));
        let second_started = Arc::new(Barrier::new(2));
        let projection = Arc::new(AtomicUsize::new(0));
        let steps = Arc::new(Mutex::new(Vec::new()));

        let first = {
            let coordinator = Arc::clone(&coordinator);
            let first_inside = Arc::clone(&first_inside);
            let release_first = Arc::clone(&release_first);
            let projection = Arc::clone(&projection);
            let steps = Arc::clone(&steps);
            thread::spawn(move || {
                let ((), token) = coordinator.serialize_tag_edit(|| {
                    steps.lock().expect("steps").push("a-write");
                    first_inside.wait();
                    release_first.wait();
                    steps.lock().expect("steps").push("a-bridge");
                    projection.store(1, Ordering::SeqCst);
                    steps.lock().expect("steps").push("a-project");
                });
                token
            })
        };

        first_inside.wait();
        let second = {
            let coordinator = Arc::clone(&coordinator);
            let second_started = Arc::clone(&second_started);
            let projection = Arc::clone(&projection);
            let steps = Arc::clone(&steps);
            thread::spawn(move || {
                second_started.wait();
                let ((), token) = coordinator.serialize_tag_edit(|| {
                    steps.lock().expect("steps").push("b-write");
                    steps.lock().expect("steps").push("b-bridge");
                    projection.store(2, Ordering::SeqCst);
                    steps.lock().expect("steps").push("b-project");
                });
                token
            })
        };
        second_started.wait();

        assert!(coordinator.edit_gate.try_lock().is_err());
        assert_eq!(projection.load(Ordering::SeqCst), 0);
        release_first.wait();

        let first_token = first.join().expect("first edit");
        let second_token = second.join().expect("second edit");
        assert!(first_token < second_token);
        assert_eq!(projection.load(Ordering::SeqCst), 2);
        assert_eq!(
            *steps.lock().expect("steps"),
            vec![
                "a-write",
                "a-bridge",
                "a-project",
                "b-write",
                "b-bridge",
                "b-project",
            ]
        );
    }

    #[test]
    fn background_reconciliation_never_holds_the_foreground_edit_gate() {
        let coordinator = Arc::new(LibrarySyncCoordinator::default());
        let background_inside = Arc::new(Barrier::new(2));
        let release_background = Arc::new(Barrier::new(2));

        let background = {
            let coordinator = Arc::clone(&coordinator);
            let background_inside = Arc::clone(&background_inside);
            let release_background = Arc::clone(&release_background);
            thread::spawn(move || {
                let token = coordinator.reserve_background_projection_token();
                coordinator.serialize_bridge_work(|| {
                    background_inside.wait();
                    release_background.wait();
                });
                token
            })
        };

        background_inside.wait();
        let ((), edit_token) = coordinator.serialize_tag_edit(|| ());
        assert!(coordinator.bridge_gate.try_lock().is_err());
        assert!(coordinator.edit_gate.try_lock().is_ok());
        release_background.wait();

        let background_token = background.join().expect("background reconciliation");
        assert!(background_token < edit_token);
    }
}
