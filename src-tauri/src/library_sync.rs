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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CatalogSyncStatus {
    Synced,
    Pending,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogSync {
    pub(crate) status: CatalogSyncStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) message: Option<String>,
    pub(crate) pending_folder_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) projection_token: Option<u64>,
}

impl CatalogSync {
    fn synced(pending_folder_count: usize) -> Self {
        Self {
            status: CatalogSyncStatus::Synced,
            message: Some("Music Library updated.".to_owned()),
            pending_folder_count,
            projection_token: None,
        }
    }

    fn pending(pending_folder_count: usize, detail: Option<String>) -> Self {
        let mut message =
            "Music Library update pending; Aurora will retry automatically.".to_owned();
        if let Some(detail) = detail.filter(|detail| !detail.trim().is_empty()) {
            message.push(' ');
            message.push_str(detail.trim());
        }
        Self {
            status: CatalogSyncStatus::Pending,
            message: Some(message),
            pending_folder_count,
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

    /// Gives reconciliation the same ordering and delivery token as an authoritative edit.
    pub(crate) fn serialize_reconciliation<T>(&self, operation: impl FnOnce() -> T) -> (T, u64) {
        self.serialize_projection_epoch(operation)
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
                    catalog_sync: CatalogSync::pending(priorities.len().max(1), Some(error)),
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
        let pending_folder_count = match store.pending_library_folder_sync_count() {
            Ok(count) => count,
            Err(error) => {
                return LibrarySyncReport {
                    catalog_sync: CatalogSync::pending(1, Some(error)),
                    completed_directories,
                };
            }
        };
        let catalog_sync = if targets.is_empty() {
            CatalogSync::synced(pending_folder_count)
        } else {
            CatalogSync::pending(
                pending_folder_count.max(targets.len()),
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

    fn run(&self, app: &AppHandle, priority_directories: &[String]) -> LibrarySyncReport {
        self.run_serialized(|| self.run_locked(app, priority_directories))
    }

    fn run_serialized<T>(&self, operation: impl FnOnce() -> T) -> T {
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
                        catalog_sync: CatalogSync::pending(1, Some(error)),
                        completed_directories: HashSet::new(),
                    };
                }
            }
        } else {
            match store.pending_library_folder_sync_for_paths(&priorities) {
                Ok(items) => items,
                Err(error) => {
                    return LibrarySyncReport {
                        catalog_sync: CatalogSync::pending(priorities.len(), Some(error)),
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
                sync_existing_library_folders(
                    app,
                    vec![target.directory.clone()],
                    target_changed_file_paths(target),
                )
                .map(|_| ())
            },
            |target| store.complete_library_folder_sync(target),
            |target, error| store.defer_library_folder_sync(target, error),
        );
        completed_directories.extend(processed.completed_directories);
        let mut first_error = processed.first_error;

        let pending_folder_count = match store.pending_library_folder_sync_count() {
            Ok(count) => count,
            Err(error) => {
                first_error.get_or_insert(error);
                1
            }
        };
        let priorities_completed = priorities
            .iter()
            .all(|directory| completed_directories.contains(&normalized_directory(directory)));
        let current_work_synced = if priorities.is_empty() {
            first_error.is_none()
                && (!completed_directories.is_empty() || pending_folder_count == 0)
        } else {
            priorities_completed && first_error.is_none()
        };
        let catalog_sync = if current_work_synced {
            CatalogSync::synced(pending_folder_count)
        } else {
            CatalogSync::pending(pending_folder_count.max(1), first_error)
        };

        LibrarySyncReport {
            catalog_sync,
            completed_directories,
        }
    }
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
        let sync = CatalogSync::pending(2, Some("Music Library is busy.".to_owned()));
        assert_eq!(sync.status, CatalogSyncStatus::Pending);
        assert_eq!(sync.pending_folder_count, 2);
        assert!(sync.message.unwrap().contains("retry automatically"));
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
                coordinator.run_serialized(|| {
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
    fn reconciliation_and_edit_share_one_projection_epoch() {
        let coordinator = Arc::new(LibrarySyncCoordinator::default());
        let reconciliation_inside = Arc::new(Barrier::new(2));
        let release_reconciliation = Arc::new(Barrier::new(2));
        let edit_started = Arc::new(Barrier::new(2));
        let native_projection = Arc::new(AtomicUsize::new(0));
        let steps = Arc::new(Mutex::new(Vec::new()));

        let reconciliation = {
            let coordinator = Arc::clone(&coordinator);
            let reconciliation_inside = Arc::clone(&reconciliation_inside);
            let release_reconciliation = Arc::clone(&release_reconciliation);
            let native_projection = Arc::clone(&native_projection);
            let steps = Arc::clone(&steps);
            thread::spawn(move || {
                let ((), token) = coordinator.serialize_reconciliation(|| {
                    steps.lock().expect("steps").push("reconciliation-bridge");
                    reconciliation_inside.wait();
                    release_reconciliation.wait();
                    steps.lock().expect("steps").push("reconciliation-read");
                    native_projection.store(1, Ordering::SeqCst);
                    steps.lock().expect("steps").push("reconciliation-project");
                });
                token
            })
        };

        reconciliation_inside.wait();
        let edit = {
            let coordinator = Arc::clone(&coordinator);
            let edit_started = Arc::clone(&edit_started);
            let native_projection = Arc::clone(&native_projection);
            let steps = Arc::clone(&steps);
            thread::spawn(move || {
                edit_started.wait();
                let ((), token) = coordinator.serialize_tag_edit(|| {
                    steps.lock().expect("steps").push("edit-write");
                    steps.lock().expect("steps").push("edit-bridge");
                    native_projection.store(2, Ordering::SeqCst);
                    steps.lock().expect("steps").push("edit-project");
                });
                token
            })
        };
        edit_started.wait();

        assert!(coordinator.edit_gate.try_lock().is_err());
        release_reconciliation.wait();

        let reconciliation_token = reconciliation.join().expect("reconciliation");
        let edit_token = edit.join().expect("edit");
        assert!(reconciliation_token < edit_token);
        assert_eq!(native_projection.load(Ordering::SeqCst), 2);
        assert_eq!(
            *steps.lock().expect("steps"),
            vec![
                "reconciliation-bridge",
                "reconciliation-read",
                "reconciliation-project",
                "edit-write",
                "edit-bridge",
                "edit-project",
            ]
        );
    }
}
