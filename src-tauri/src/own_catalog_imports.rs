//! Music Library import runs that only carried Aurora's own tag edits. The
//! catalog watcher uses this to skip a full library reload for them, because the
//! UI already shows those edits.
use std::{collections::VecDeque, sync::Mutex};

const REMEMBERED_RUNS: usize = 512;

static OWN_RUNS: Mutex<VecDeque<i64>> = Mutex::new(VecDeque::new());

pub(crate) fn record(run_ids: &[i64]) {
    let mut runs = OWN_RUNS.lock().unwrap_or_else(|error| error.into_inner());
    for id in run_ids {
        if !runs.contains(id) {
            runs.push_back(*id);
        }
    }
    while runs.len() > REMEMBERED_RUNS {
        runs.pop_front();
    }
}

/// True only for a non-empty list in which every run is Aurora's own.
pub(crate) fn contains_all(run_ids: &[i64]) -> bool {
    let runs = OWN_RUNS.lock().unwrap_or_else(|error| error.into_inner());
    !run_ids.is_empty() && run_ids.iter().all(|id| runs.contains(id))
}

/// Whether a folder-sync receipt only reflects Aurora's edits: no deletions, and
/// no more changed tracks than the files Aurora edited in those folders.
pub(crate) fn sync_only_carries_own_edits(
    changed_tracks: i64,
    deleted_files: usize,
    own_edited_files: usize,
) -> bool {
    deleted_files == 0 && changed_tracks >= 0 && changed_tracks as u64 <= own_edited_files as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_fully_recorded_run_lists_count_as_own() {
        record(&[900_001, 900_002]);
        assert!(contains_all(&[900_001]));
        assert!(contains_all(&[900_001, 900_002]));
        assert!(!contains_all(&[900_001, 900_003]));
        assert!(!contains_all(&[]));
    }

    #[test]
    fn receipts_with_deletions_or_extra_tracks_are_foreign() {
        assert!(sync_only_carries_own_edits(1, 0, 1));
        assert!(sync_only_carries_own_edits(0, 0, 0));
        assert!(!sync_only_carries_own_edits(2, 0, 1));
        assert!(!sync_only_carries_own_edits(1, 1, 3));
        assert!(!sync_only_carries_own_edits(-1, 0, 1));
    }
}
