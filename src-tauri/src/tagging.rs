use crate::{
    catalog::{self, ResolvedTrack, TrackSummary},
    state_store::{StateStore, TagOverlay},
    tag_model::{LoveState, TagEditRequest, TagSyncState, TagValues, TrackTagState},
};
use id3::{
    Tag, TagLike, Version,
    frame::{Content, ExtendedText, Frame, Unknown},
    no_tag_ok,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    ffi::c_void,
    fs::{self, File, OpenOptions},
    io::{Read, Seek},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MUSICBEE_POPM_OWNER: &str = "MusicBee";
const LOVE_RATING_DESCRIPTION: &str = "LOVE RATING";
const RELEASE_TIME_DESCRIPTION: &str = "TDRL";
const RETAINED_BACKUPS: usize = 20;
const MAX_PENDING_RECONCILIATION_BATCH: usize = 200;

const MUSICBEE_RATINGS: [(f64, u8); 10] = [
    (0.5, 13),
    (1.0, 1),
    (1.5, 54),
    (2.0, 64),
    (2.5, 118),
    (3.0, 128),
    (3.5, 186),
    (4.0, 196),
    (4.5, 242),
    (5.0, 255),
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrackTagSnapshot {
    pub(crate) track: TrackSummary,
    pub(crate) tag_state: TrackTagState,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TagReconciliationChange {
    pub(crate) track_key: String,
    pub(crate) values: TagValues,
    pub(crate) sync_state: Option<TagSyncState>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TagReconciliationIssue {
    pub(crate) track_key: String,
    pub(crate) message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TagReconciliationReport {
    pub(crate) processed: usize,
    pub(crate) reconciled: usize,
    pub(crate) external_changes: usize,
    pub(crate) catalog_caught_up: usize,
    pub(crate) unchanged: usize,
    pub(crate) unavailable: usize,
    pub(crate) invalid: usize,
    pub(crate) conflicted: usize,
    pub(crate) has_more: bool,
    pub(crate) changes: Vec<TagReconciliationChange>,
    pub(crate) issues: Vec<TagReconciliationIssue>,
}

impl TagReconciliationReport {
    fn new(has_more: bool) -> Self {
        Self {
            processed: 0,
            reconciled: 0,
            external_changes: 0,
            catalog_caught_up: 0,
            unchanged: 0,
            unavailable: 0,
            invalid: 0,
            conflicted: 0,
            has_more,
            changes: Vec::new(),
            issues: Vec::new(),
        }
    }

    fn record_failure(&mut self, track_key: &str, failure: PendingOverlayFailure) {
        match failure.kind {
            PendingOverlayFailureKind::Unavailable => self.unavailable += 1,
            PendingOverlayFailureKind::Invalid => self.invalid += 1,
            PendingOverlayFailureKind::Conflicted => self.conflicted += 1,
            PendingOverlayFailureKind::State => {}
        }
        self.issues.push(TagReconciliationIssue {
            track_key: track_key.to_owned(),
            message: failure.message,
        });
    }
}

struct PendingOverlayOutcome {
    values: TagValues,
    external_change: bool,
    catalog_caught_up: bool,
}

#[derive(Clone, Copy, Debug)]
enum PendingOverlayFailureKind {
    Unavailable,
    Invalid,
    Conflicted,
    State,
}

#[derive(Debug)]
struct PendingOverlayFailure {
    kind: PendingOverlayFailureKind,
    message: String,
}

pub(crate) struct TagService {
    store: StateStore,
}

impl TagService {
    pub(crate) fn new(store: StateStore) -> Result<Self, String> {
        let service = Self { store };
        service.recover_interrupted_operations()?;
        Ok(service)
    }

    pub(crate) fn inspect(
        &self,
        track_id: &str,
        track_key: &str,
    ) -> Result<TrackTagSnapshot, String> {
        let resolved = catalog::resolve_track(track_id, track_key, &self.store)?;
        let values = read_tag_values_from_path(&resolved.audio_path)?;
        self.snapshot_with_values(resolved, values, None)
    }

    pub(crate) fn reconcile_pending_overlays(
        &self,
        requested_limit: usize,
    ) -> Result<TagReconciliationReport, String> {
        let limit = requested_limit.clamp(1, MAX_PENDING_RECONCILIATION_BATCH);
        let mut overlays = self.store.pending_overlays(limit + 1)?;
        let has_more = overlays.len() > limit;
        overlays.truncate(limit);
        let mut report = TagReconciliationReport::new(has_more);

        for overlay in overlays {
            report.processed += 1;
            if overlay.track_key
                != catalog::normalize_track_key(&overlay.directory, &overlay.filename)
            {
                self.record_pending_failure(
                    &mut report,
                    &overlay.track_key,
                    PendingOverlayFailure {
                        kind: PendingOverlayFailureKind::Unavailable,
                        message:
                            "This pending track no longer matches its stable catalog identity."
                                .to_owned(),
                    },
                )?;
                continue;
            }
            let catalog_values = match catalog::catalog_tag_values_by_path(
                &overlay.directory,
                &overlay.filename,
            ) {
                Ok(Some(values)) => values,
                Ok(None) => {
                    self.record_pending_failure(
                        &mut report,
                        &overlay.track_key,
                        PendingOverlayFailure {
                            kind: PendingOverlayFailureKind::Unavailable,
                            message: "This pending track is no longer present in Music Library's catalog."
                                .to_owned(),
                        },
                    )?;
                    continue;
                }
                Err(error) => {
                    self.record_pending_failure(
                        &mut report,
                        &overlay.track_key,
                        PendingOverlayFailure {
                            kind: PendingOverlayFailureKind::Unavailable,
                            message: error,
                        },
                    )?;
                    continue;
                }
            };
            let audio_path =
                match validated_pending_overlay_path(&overlay.directory, &overlay.filename) {
                    Ok(path) => path,
                    Err(failure) => {
                        self.record_pending_failure(&mut report, &overlay.track_key, failure)?;
                        continue;
                    }
                };
            match self.reconcile_pending_overlay(
                &overlay,
                &catalog_values.0,
                catalog_values.1,
                &audio_path,
            ) {
                Ok(outcome) => {
                    report.reconciled += 1;
                    if outcome.external_change {
                        report.external_changes += 1;
                    }
                    if outcome.catalog_caught_up {
                        report.catalog_caught_up += 1;
                    }
                    if !outcome.external_change && !outcome.catalog_caught_up {
                        report.unchanged += 1;
                    } else {
                        report.changes.push(TagReconciliationChange {
                            track_key: overlay.track_key,
                            sync_state: (!outcome.catalog_caught_up)
                                .then_some(TagSyncState::PendingImport),
                            values: outcome.values,
                        });
                    }
                }
                Err(failure) => {
                    if matches!(failure.kind, PendingOverlayFailureKind::State) {
                        return Err(failure.message);
                    }
                    self.record_pending_failure(&mut report, &overlay.track_key, failure)?;
                }
            }
        }
        Ok(report)
    }

    fn record_pending_failure(
        &self,
        report: &mut TagReconciliationReport,
        track_key: &str,
        failure: PendingOverlayFailure,
    ) -> Result<(), String> {
        self.store.defer_overlay_reconciliation(track_key)?;
        report.record_failure(track_key, failure);
        Ok(())
    }

    fn reconcile_pending_overlay(
        &self,
        overlay: &TagOverlay,
        catalog_values: &TagValues,
        catalog_import_run_id: i64,
        audio_path: &Path,
    ) -> Result<PendingOverlayOutcome, PendingOverlayFailure> {
        let before_fingerprint =
            FileFingerprint::read(audio_path).map_err(|message| PendingOverlayFailure {
                kind: PendingOverlayFailureKind::Unavailable,
                message,
            })?;
        let values =
            read_tag_values_from_path(audio_path).map_err(|message| PendingOverlayFailure {
                kind: PendingOverlayFailureKind::Invalid,
                message,
            })?;
        let after_fingerprint =
            FileFingerprint::read(audio_path).map_err(|message| PendingOverlayFailure {
                kind: PendingOverlayFailureKind::Unavailable,
                message,
            })?;
        if before_fingerprint != after_fingerprint {
            return Err(PendingOverlayFailure {
                kind: PendingOverlayFailureKind::Conflicted,
                message: "The MP3 changed while Aurora reconciled it; its pending state was left untouched."
                    .to_owned(),
            });
        }

        let external_change = values != overlay.values;
        let catalog_caught_up = values == *catalog_values;
        self.store
            .upsert_overlay(
                &overlay.track_key,
                &overlay.directory,
                &overlay.filename,
                catalog_values,
                &values,
                catalog_import_run_id,
                overlay.last_operation_id,
            )
            .map_err(|message| PendingOverlayFailure {
                kind: PendingOverlayFailureKind::State,
                message,
            })?;
        Ok(PendingOverlayOutcome {
            values,
            external_change,
            catalog_caught_up,
        })
    }

    pub(crate) fn update(&self, request: TagEditRequest) -> Result<TrackTagSnapshot, String> {
        request.expected.validate()?;
        request.desired.validate()?;
        let resolved = catalog::resolve_track(&request.track_id, &request.track_key, &self.store)?;
        let original_fingerprint = FileFingerprint::read(&resolved.audio_path)?;
        let (mut tag, version) = read_tag_for_write(&resolved.audio_path)?;
        let original_payload_hash = audio_payload_hash(&resolved.audio_path)?;
        if FileFingerprint::read(&resolved.audio_path)? != original_fingerprint {
            return Err(
                "The MP3 changed while Aurora opened its tags. Reload before saving.".to_owned(),
            );
        }
        let current = read_tag_values(&tag)?;
        if current != request.expected {
            return Err(
                "This MP3 changed outside Aurora after the editor opened. Reload its tags before saving."
                    .to_owned(),
            );
        }
        if current == request.desired {
            return self.snapshot_with_values(resolved, current, None);
        }

        let preserved_frames = non_target_frames(&tag);
        let target_path_text = resolved.audio_path.to_string_lossy().into_owned();
        let operation_id = self.store.begin_tag_operation(
            &resolved.summary.track_key,
            &target_path_text,
            &current,
            &request.desired,
            &original_fingerprint.to_string(),
        )?;
        let (temp_path, backup_path) = operation_paths(&resolved.audio_path, operation_id)?;
        self.store.set_operation_paths(
            operation_id,
            &temp_path.to_string_lossy(),
            &backup_path.to_string_lossy(),
        )?;

        let mut recovery_required = false;
        let write_result = (|| -> Result<(), String> {
            if temp_path.exists() || backup_path.exists() {
                return Err(
                    "Aurora's safe-write paths already exist; no file was changed.".to_owned(),
                );
            }
            fs::copy(&resolved.audio_path, &temp_path).map_err(|error| {
                format!("Could not create the same-folder MP3 working copy: {error}")
            })?;
            apply_tag_changes(&mut tag, version, &current, &request.desired)?;
            tag.write_to_path(&temp_path, version)
                .map_err(|error| format!("Could not write the MP3 working copy: {error}"))?;
            File::options()
                .read(true)
                .write(true)
                .open(&temp_path)
                .and_then(|file| file.sync_all())
                .map_err(|error| format!("Could not flush the MP3 working copy: {error}"))?;
            verify_written_file(
                &temp_path,
                &request.desired,
                &preserved_frames,
                &original_payload_hash,
            )?;
            let _write_exclusion = open_write_exclusion(&resolved.audio_path)?;
            if FileFingerprint::read(&resolved.audio_path)? != original_fingerprint {
                return Err(
                    "The MP3 changed while Aurora prepared the edit. The original was left untouched."
                        .to_owned(),
                );
            }
            if let Err(error) =
                replace_file_atomic(&resolved.audio_path, &temp_path, Some(&backup_path))
            {
                recovery_required = true;
                return Err(format!(
                    "Windows could not complete Aurora's atomic save. Every file was retained for startup recovery: {error}"
                ));
            }
            if FileFingerprint::read(&backup_path)?.to_string() != original_fingerprint.to_string()
            {
                return Err(
                    "Another application atomically replaced the MP3 during Aurora's save. Aurora retained both files without overwriting either; reload the track before editing again."
                        .to_owned(),
                );
            }
            if let Err(error) = self.store.mark_operation(operation_id, "replaced", None) {
                recovery_required = true;
                return Err(format!(
                    "Aurora installed and retained the edit, but could not checkpoint its journal. It will verify the retained files at startup: {error}"
                ));
            }
            if let Err(error) = verify_written_file(
                &resolved.audio_path,
                &request.desired,
                &preserved_frames,
                &original_payload_hash,
            ) {
                return Err(format!(
                    "Aurora could not verify the installed edit and retained both files without overwriting either: {error}"
                ));
            }
            Ok(())
        })();

        if let Err(error) = write_result {
            if !recovery_required {
                cleanup_owned_working_file(&temp_path);
                let _ = self
                    .store
                    .mark_operation(operation_id, "failed", Some(&error));
            }
            return Err(error);
        }

        if let Err(error) = self.store.finish_tag_operation(
            operation_id,
            &resolved.summary.track_key,
            &resolved.summary.directory,
            &resolved.summary.filename,
            &resolved.catalog_values,
            &request.desired,
            resolved.summary.catalog_import_run_id,
        ) {
            return Err(format!(
                "Aurora installed and verified the MP3 edit, but could not finish its journal. The retained files will be reconciled at startup: {error}"
            ));
        }
        let _ = self.store.prune_old_backups(RETAINED_BACKUPS);
        self.snapshot_with_values(resolved, request.desired, Some(operation_id))
    }

    pub(crate) fn undo(&self, track_id: &str, track_key: &str) -> Result<TrackTagSnapshot, String> {
        let resolved = catalog::resolve_track(track_id, track_key, &self.store)?;
        let operation = self
            .store
            .latest_undo_operation(&resolved.summary.track_key)?
            .ok_or_else(|| {
                "There is no retained Aurora tag edit to undo for this track.".to_owned()
            })?;
        if !same_path(&operation.target_path, &resolved.audio_path) {
            return Err("The saved undo target no longer matches this track.".to_owned());
        }
        let _write_exclusion = open_write_exclusion(&resolved.audio_path)?;
        let undo_source_fingerprint = FileFingerprint::read(&resolved.audio_path)?;
        let (current_tag, _) = read_tag_for_write(&resolved.audio_path)?;
        let current = read_tag_values(&current_tag)?;
        if current != operation.after {
            return Err(
                "The MP3 tags changed after Aurora's edit, so undo will not overwrite them."
                    .to_owned(),
            );
        }
        let backup_path = operation
            .backup_path
            .as_ref()
            .ok_or_else(|| "Aurora's rollback copy is no longer available.".to_owned())?;
        let (backup_tag, _) = read_tag_for_write(backup_path)?;
        if !same_frames(
            &non_target_frames(&current_tag),
            &non_target_frames(&backup_tag),
        ) {
            return Err(
                "The MP3 has other metadata changes after Aurora's edit, so undo will not erase them."
                    .to_owned(),
            );
        }
        if audio_payload_hash(&resolved.audio_path)? != audio_payload_hash(backup_path)? {
            return Err("Aurora refused an undo whose audio payload does not match.".to_owned());
        }
        if FileFingerprint::read(&resolved.audio_path)? != undo_source_fingerprint {
            return Err("The MP3 changed while Aurora prepared the undo. Try again.".to_owned());
        }

        let undo_backup =
            sibling_operation_path(&resolved.audio_path, operation.id, "undo-current.backup")?;
        let undo_replacement =
            sibling_operation_path(&resolved.audio_path, operation.id, "undo-original.tmp")?;
        if undo_backup.exists() || undo_replacement.exists() {
            return Err("Aurora's undo safety path already exists.".to_owned());
        }
        fs::copy(backup_path, &undo_replacement)
            .map_err(|error| format!("Could not prepare Aurora's undo copy: {error}"))?;
        File::options()
            .read(true)
            .write(true)
            .open(&undo_replacement)
            .and_then(|file| file.sync_all())
            .map_err(|error| format!("Could not flush Aurora's undo copy: {error}"))?;
        self.store.begin_undo(
            operation.id,
            &undo_backup.to_string_lossy(),
            &undo_source_fingerprint.to_string(),
        )?;
        if let Err(error) =
            replace_file_atomic(&resolved.audio_path, &undo_replacement, Some(&undo_backup))
        {
            return Err(format!(
                "Windows could not complete Aurora's atomic undo. Every file was retained for startup recovery: {error}"
            ));
        }
        if FileFingerprint::read(&undo_backup)?.to_string() != undo_source_fingerprint.to_string() {
            let message = "Another application atomically replaced the MP3 during undo. Aurora retained every file without overwriting either version.";
            self.store
                .mark_operation(operation.id, "failed", Some(message))?;
            return Err(message.to_owned());
        }
        let restored = read_tag_values_from_path(&resolved.audio_path)?;
        let restored_is_verified = restored == operation.before
            && audio_payload_hash(&resolved.audio_path)? == audio_payload_hash(backup_path)?
            && same_frames(
                &non_target_frames(&read_tag_for_write(&resolved.audio_path)?.0),
                &non_target_frames(&backup_tag),
            );
        if !restored_is_verified {
            let message = "Aurora could not verify the installed undo and retained every file without overwriting a possibly newer external edit.";
            self.store
                .mark_operation(operation.id, "failed", Some(message))?;
            return Err(message.to_owned());
        }
        if let Err(error) = self.store.finish_undo_operation(
            operation.id,
            &resolved.summary.track_key,
            &resolved.summary.directory,
            &resolved.summary.filename,
            &resolved.catalog_values,
            &restored,
            resolved.summary.catalog_import_run_id,
        ) {
            return Err(format!(
                "Aurora installed and verified the undo, but could not finish its journal. The retained files will be reconciled at startup: {error}"
            ));
        }
        cleanup_owned_working_file(&undo_backup);
        self.snapshot_with_values(resolved, restored, Some(operation.id))
    }

    fn snapshot_with_values(
        &self,
        mut resolved: ResolvedTrack,
        values: TagValues,
        operation_id: Option<i64>,
    ) -> Result<TrackTagSnapshot, String> {
        let pending_import = values != resolved.catalog_values;
        if operation_id.is_none() {
            self.store.upsert_overlay(
                &resolved.summary.track_key,
                &resolved.summary.directory,
                &resolved.summary.filename,
                &resolved.catalog_values,
                &values,
                resolved.summary.catalog_import_run_id,
                operation_id,
            )?;
        }
        resolved.summary.apply_tag_values(&values, pending_import);
        let can_undo = self.store.can_undo(&resolved.summary.track_key)?;
        resolved.summary.can_undo_tag_edit = can_undo;
        Ok(TrackTagSnapshot {
            track: resolved.summary,
            tag_state: TrackTagState {
                values,
                sync_state: pending_import.then_some(TagSyncState::PendingImport),
                can_undo,
            },
        })
    }

    fn recover_interrupted_operations(&self) -> Result<(), String> {
        for operation in self.store.interrupted_operations()? {
            match operation.status.as_str() {
                "prepared" => {
                    if operation
                        .backup_path
                        .as_ref()
                        .is_some_and(|path| path.is_file())
                    {
                        self.recover_replaced_operation(&operation)?;
                    } else if operation.target_path.is_file() {
                        if let Some(temp_path) = &operation.temp_path {
                            cleanup_owned_working_file(temp_path);
                        }
                        self.store.mark_operation(
                            operation.id,
                            "failed",
                            Some(
                                "Aurora closed before replacing the MP3; the original was untouched.",
                            ),
                        )?;
                    } else {
                        self.store.mark_operation(
                            operation.id,
                            "failed",
                            Some(
                                "Tag-write recovery found a missing target without a verified backup and retained every remaining file.",
                            ),
                        )?;
                    }
                }
                "replaced" => {
                    self.recover_replaced_operation(&operation)?;
                }
                "undoing" => {
                    self.recover_interrupted_undo(&operation)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn recover_interrupted_undo(
        &self,
        operation: &crate::state_store::TagOperation,
    ) -> Result<(), String> {
        let current_backup = operation.temp_path.as_ref().ok_or_else(|| {
            "Aurora cannot recover an interrupted undo because its safety path is missing."
                .to_owned()
        })?;
        let undo_replacement =
            sibling_operation_path(&operation.target_path, operation.id, "undo-original.tmp")?;
        if !current_backup.is_file() {
            if operation.target_path.is_file() {
                cleanup_owned_working_file(&undo_replacement);
                self.store.mark_operation(
                    operation.id,
                    "verified",
                    Some("Aurora closed before replacing the MP3 during undo."),
                )?;
            } else {
                self.store.mark_operation(
                    operation.id,
                    "failed",
                    Some(
                        "Undo recovery found a missing target without its safety backup and retained every remaining file.",
                    ),
                )?;
            }
            return Ok(());
        }
        if !FileFingerprint::read(current_backup)
            .is_ok_and(|fingerprint| fingerprint.to_string() == operation.source_fingerprint)
        {
            self.store.mark_operation(
                operation.id,
                "failed",
                Some(
                    "Undo recovery found a competing file replacement and retained every file without overwriting it.",
                ),
            )?;
            return Ok(());
        }
        let original_backup = operation
            .backup_path
            .as_ref()
            .filter(|path| path.is_file())
            .ok_or_else(|| "Aurora's original undo backup is missing.".to_owned())?;
        if !operation.target_path.is_file() {
            if undo_replacement.is_file()
                && known_file_matches(&undo_replacement, original_backup, &operation.before)
            {
                if let Err(error) =
                    move_file_without_replacing(&undo_replacement, &operation.target_path)
                    && !operation.target_path.is_file()
                {
                    self.store.mark_operation(
                        operation.id,
                        "failed",
                        Some(&format!(
                            "Undo recovery could not restore the missing canonical MP3 and retained every file: {error}"
                        )),
                    )?;
                    return Ok(());
                }
            } else {
                self.store.mark_operation(
                    operation.id,
                    "failed",
                    Some(
                        "Undo recovery could not verify the replacement for a missing target and retained every file.",
                    ),
                )?;
                return Ok(());
            }
        }
        let target_values_match = read_tag_values_from_path(&operation.target_path)
            .is_ok_and(|values| values == operation.before);
        let target_frames_match = match (
            read_tag_for_write(&operation.target_path),
            read_tag_for_write(original_backup),
        ) {
            (Ok((target, _)), Ok((backup, _))) => {
                same_frames(&non_target_frames(&target), &non_target_frames(&backup))
            }
            _ => false,
        };
        let audio_matches = match (
            audio_payload_hash(&operation.target_path),
            audio_payload_hash(original_backup),
        ) {
            (Ok(target), Ok(backup)) => target == backup,
            _ => false,
        };
        if target_values_match && target_frames_match && audio_matches {
            let directory = operation
                .target_path
                .parent()
                .ok_or_else(|| "The recovered undo path has no parent directory.".to_owned())?
                .to_string_lossy()
                .into_owned();
            let filename = operation
                .target_path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| "The recovered undo filename is invalid.".to_owned())?;
            let prior_overlay = self
                .store
                .overlays_for_keys(std::slice::from_ref(&operation.track_key))?
                .into_iter()
                .next();
            let current_catalog = catalog::catalog_tag_values_by_path(&directory, filename)
                .ok()
                .flatten();
            let (catalog_values, import_run_id) = current_catalog
                .or_else(|| {
                    prior_overlay
                        .map(|overlay| (overlay.catalog_values, overlay.catalog_import_run_id))
                })
                .unwrap_or_else(|| (operation.before.clone(), 0));
            self.store.finish_undo_operation(
                operation.id,
                &operation.track_key,
                &directory,
                filename,
                &catalog_values,
                &operation.before,
                import_run_id,
            )?;
            cleanup_owned_working_file(current_backup);
            cleanup_owned_working_file(&undo_replacement);
        } else {
            self.store.mark_operation(
                operation.id,
                "failed",
                Some(
                    "Undo recovery found an ambiguous or externally changed target and retained every file without overwriting it.",
                ),
            )?;
        }
        Ok(())
    }

    fn recover_replaced_operation(
        &self,
        operation: &crate::state_store::TagOperation,
    ) -> Result<(), String> {
        let backup_path = operation
            .backup_path
            .as_ref()
            .filter(|path| path.is_file())
            .ok_or_else(|| {
                "Aurora cannot recover an interrupted tag write because its rollback copy is missing."
                    .to_owned()
            })?;
        let backup_matches_source = FileFingerprint::read(backup_path)
            .is_ok_and(|fingerprint| fingerprint.to_string() == operation.source_fingerprint);
        if !operation.target_path.is_file() {
            let replacement = operation.temp_path.as_ref().filter(|path| path.is_file());
            if backup_matches_source
                && replacement
                    .is_some_and(|path| known_file_matches(path, backup_path, &operation.after))
            {
                let replacement = replacement.expect("verified replacement path");
                if let Err(error) = move_file_without_replacing(replacement, &operation.target_path)
                    && !operation.target_path.is_file()
                {
                    self.store.mark_operation(
                        operation.id,
                        "failed",
                        Some(&format!(
                            "Tag-write recovery could not restore the missing canonical MP3 and retained every file: {error}"
                        )),
                    )?;
                    return Ok(());
                }
            } else {
                self.store.mark_operation(
                    operation.id,
                    "failed",
                    Some(
                        "Tag-write recovery could not verify the replacement for a missing target and retained every file.",
                    ),
                )?;
                return Ok(());
            }
        }
        let target_values_match = read_tag_values_from_path(&operation.target_path)
            .is_ok_and(|values| values == operation.after);
        let audio_matches = match (
            audio_payload_hash(&operation.target_path),
            audio_payload_hash(backup_path),
        ) {
            (Ok(target), Ok(backup)) => target == backup,
            _ => false,
        };
        let non_target_frames_match = match (
            read_tag_for_write(&operation.target_path),
            read_tag_for_write(backup_path),
        ) {
            (Ok((target, _)), Ok((backup, _))) => {
                same_frames(&non_target_frames(&target), &non_target_frames(&backup))
            }
            _ => false,
        };
        if target_values_match && audio_matches && non_target_frames_match && backup_matches_source
        {
            if let Some(temp_path) = &operation.temp_path {
                cleanup_owned_working_file(temp_path);
            }
            let directory = operation
                .target_path
                .parent()
                .ok_or_else(|| "The recovered MP3 path has no parent directory.".to_owned())?
                .to_string_lossy()
                .into_owned();
            let filename = operation
                .target_path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| "The recovered MP3 filename is invalid.".to_owned())?;
            let prior_overlay = self
                .store
                .overlays_for_keys(std::slice::from_ref(&operation.track_key))?
                .into_iter()
                .next();
            let current_catalog = catalog::catalog_tag_values_by_path(&directory, filename)
                .ok()
                .flatten();
            let (catalog_values, import_run_id) = current_catalog
                .or_else(|| {
                    prior_overlay
                        .map(|overlay| (overlay.catalog_values, overlay.catalog_import_run_id))
                })
                .unwrap_or_else(|| (operation.before.clone(), 0));
            self.store.finish_tag_operation(
                operation.id,
                &operation.track_key,
                &directory,
                filename,
                &catalog_values,
                &operation.after,
                import_run_id,
            )?;
        } else {
            self.store.mark_operation(
                operation.id,
                "failed",
                Some(
                    "Tag-write recovery found an ambiguous or externally changed target and retained every file without overwriting it.",
                ),
            )?;
        }
        Ok(())
    }
}

fn validated_pending_overlay_path(
    directory: &str,
    filename: &str,
) -> Result<PathBuf, PendingOverlayFailure> {
    let directory_path = Path::new(directory);
    let filename_path = Path::new(filename);
    if !directory_path.is_absolute()
        || filename_path.is_absolute()
        || filename_path.components().count() != 1
        || !matches!(
            filename_path.components().next(),
            Some(std::path::Component::Normal(_))
        )
    {
        return Err(PendingOverlayFailure {
            kind: PendingOverlayFailureKind::Unavailable,
            message: "The catalog contains an unsafe pending-track location.".to_owned(),
        });
    }
    let audio_path = directory_path.join(filename_path);
    let is_mp3 = audio_path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mp3"));
    if !is_mp3 || !audio_path.is_file() {
        return Err(PendingOverlayFailure {
            kind: PendingOverlayFailureKind::Unavailable,
            message: "The pending MP3 is unavailable at its catalog location.".to_owned(),
        });
    }
    Ok(audio_path)
}

fn read_tag_for_write(path: &Path) -> Result<(Tag, Version), String> {
    let tag = no_tag_ok(Tag::read_from_path(path))
        .map_err(|error| format!("Could not safely decode this MP3's ID3 tag: {error}"))?;
    match tag {
        Some(tag) => {
            let version = tag.version();
            Ok((tag, version))
        }
        None => Ok((Tag::with_version(Version::Id3v23), Version::Id3v23)),
    }
}

fn read_tag_values_from_path(path: &Path) -> Result<TagValues, String> {
    let (tag, _) = read_tag_for_write(path)?;
    read_tag_values(&tag)
}

fn read_tag_values(tag: &Tag) -> Result<TagValues, String> {
    let musicbee_ratings = tag
        .frames()
        .filter(|frame| frame.id() == "POPM")
        .filter_map(|frame| frame.content().popularimeter())
        .filter(|popularimeter| popularimeter.user == MUSICBEE_POPM_OWNER)
        .map(|popularimeter| popularimeter.rating)
        .collect::<Vec<_>>();
    if musicbee_ratings.len() > 1 {
        return Err(
            "This MP3 has duplicate MusicBee rating frames; Aurora left it untouched.".to_owned(),
        );
    }
    let rating = musicbee_ratings
        .first()
        .map(|byte| rating_from_byte(*byte))
        .transpose()?;

    let love_values = tag
        .extended_texts()
        .filter(|text| text.description == LOVE_RATING_DESCRIPTION)
        .map(|text| text.value.trim())
        .collect::<Vec<_>>();
    if love_values.len() > 1 {
        return Err(
            "This MP3 has duplicate MusicBee Love frames; Aurora left it untouched.".to_owned(),
        );
    }
    let love_state = match love_values.first().copied() {
        None | Some("") => LoveState::Neutral,
        Some("L") => LoveState::Loved,
        Some("B") => LoveState::Banned,
        Some(_) => {
            return Err(
                "This MP3 has an unsupported MusicBee Love value; Aurora left it untouched."
                    .to_owned(),
            );
        }
    };

    let native_release = tag
        .get("TDRL")
        .and_then(|frame| frame.content().text())
        .and_then(parse_release_year);
    let legacy_release = tag
        .extended_texts()
        .find(|text| text.description == RELEASE_TIME_DESCRIPTION)
        .and_then(|text| parse_release_year(&text.value));

    Ok(TagValues {
        rating,
        love_state,
        release_year: native_release.or(legacy_release),
    })
}

fn apply_tag_changes(
    tag: &mut Tag,
    version: Version,
    before: &TagValues,
    after: &TagValues,
) -> Result<(), String> {
    if before.rating != after.rating {
        let preserved = tag
            .remove("POPM")
            .into_iter()
            .filter(|frame| {
                frame
                    .content()
                    .popularimeter()
                    .is_none_or(|value| value.user != MUSICBEE_POPM_OWNER)
            })
            .collect::<Vec<_>>();
        for frame in preserved {
            tag.add_frame(frame);
        }
        if let Some(rating) = after.rating {
            let byte = rating_to_byte(rating)?;
            let mut data = Vec::with_capacity(MUSICBEE_POPM_OWNER.len() + 6);
            data.extend_from_slice(MUSICBEE_POPM_OWNER.as_bytes());
            data.push(0);
            data.push(byte);
            data.extend_from_slice(&[0, 0, 0, 0]);
            tag.add_frame(Frame::with_content(
                "POPM",
                Content::Unknown(Unknown { data, version }),
            ));
        }
    }

    if before.love_state != after.love_state {
        tag.remove_extended_text(Some(LOVE_RATING_DESCRIPTION), None);
        let value = match after.love_state {
            LoveState::Neutral => None,
            LoveState::Loved => Some("L"),
            LoveState::Banned => Some("B"),
        };
        if let Some(value) = value {
            tag.add_frame(ExtendedText {
                description: LOVE_RATING_DESCRIPTION.to_owned(),
                value: value.to_owned(),
            });
        }
    }

    if before.release_year != after.release_year {
        let had_legacy = tag
            .extended_texts()
            .any(|text| text.description == RELEASE_TIME_DESCRIPTION);
        tag.remove("TDRL");
        tag.remove_extended_text(Some(RELEASE_TIME_DESCRIPTION), None);
        if let Some(year) = after.release_year {
            let value = year.to_string();
            if version == Version::Id3v24 {
                tag.set_text("TDRL", value.clone());
                if had_legacy {
                    tag.add_frame(ExtendedText {
                        description: RELEASE_TIME_DESCRIPTION.to_owned(),
                        value,
                    });
                }
            } else {
                tag.add_frame(ExtendedText {
                    description: RELEASE_TIME_DESCRIPTION.to_owned(),
                    value,
                });
            }
        }
    }
    Ok(())
}

fn rating_to_byte(rating: f64) -> Result<u8, String> {
    MUSICBEE_RATINGS
        .iter()
        .find(|(value, _)| (*value - rating).abs() < f64::EPSILON)
        .map(|(_, byte)| *byte)
        .ok_or_else(|| "Rating is not on MusicBee's supported half-star scale.".to_owned())
}

fn rating_from_byte(byte: u8) -> Result<f64, String> {
    MUSICBEE_RATINGS
        .iter()
        .find(|(_, value)| *value == byte)
        .map(|(rating, _)| *rating)
        .ok_or_else(|| {
            format!(
                "This MP3 uses unsupported MusicBee rating byte {byte}; Aurora left it untouched."
            )
        })
}

fn parse_release_year(value: &str) -> Option<i32> {
    let year = value.trim().chars().take(4).collect::<String>();
    if year.len() != 4 || !year.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    year.parse::<i32>()
        .ok()
        .filter(|year| (1000..=2999).contains(year))
}

fn is_target_frame(frame: &Frame) -> bool {
    match frame.id() {
        "TDRL" => true,
        "POPM" => frame
            .content()
            .popularimeter()
            .is_some_and(|value| value.user == MUSICBEE_POPM_OWNER),
        "TXXX" => frame.content().extended_text().is_some_and(|value| {
            matches!(
                value.description.as_str(),
                LOVE_RATING_DESCRIPTION | RELEASE_TIME_DESCRIPTION
            )
        }),
        _ => false,
    }
}

fn non_target_frames(tag: &Tag) -> Vec<Frame> {
    tag.frames()
        .filter(|frame| !is_target_frame(frame))
        .cloned()
        .collect()
}

fn verify_written_file(
    path: &Path,
    expected: &TagValues,
    preserved_frames: &[Frame],
    expected_payload_hash: &[u8; 32],
) -> Result<(), String> {
    let (tag, _) = read_tag_for_write(path)?;
    let actual = read_tag_values(&tag)?;
    if &actual != expected {
        return Err("the written MusicBee tag values did not round-trip".to_owned());
    }
    if !same_frames(&non_target_frames(&tag), preserved_frames) {
        return Err("a non-Aurora ID3 frame changed".to_owned());
    }
    if &audio_payload_hash(path)? != expected_payload_hash {
        return Err("the audio payload changed".to_owned());
    }
    Ok(())
}

fn known_file_matches(candidate: &Path, reference: &Path, expected: &TagValues) -> bool {
    let values_match = read_tag_values_from_path(candidate).is_ok_and(|values| values == *expected);
    let frames_match = match (read_tag_for_write(candidate), read_tag_for_write(reference)) {
        (Ok((candidate, _)), Ok((reference, _))) => same_frames(
            &non_target_frames(&candidate),
            &non_target_frames(&reference),
        ),
        _ => false,
    };
    let audio_matches = match (audio_payload_hash(candidate), audio_payload_hash(reference)) {
        (Ok(candidate), Ok(reference)) => candidate == reference,
        _ => false,
    };
    values_match && frames_match && audio_matches
}

fn same_frames(actual: &[Frame], expected: &[Frame]) -> bool {
    if actual.len() != expected.len() {
        return false;
    }
    let mut unmatched = actual.to_vec();
    for frame in expected {
        let Some(index) = unmatched.iter().position(|candidate| candidate == frame) else {
            return false;
        };
        unmatched.remove(index);
    }
    unmatched.is_empty()
}

fn audio_payload_hash(path: &Path) -> Result<[u8; 32], String> {
    let mut file = File::open(path)
        .map_err(|error| format!("Could not open the MP3 for audio verification: {error}"))?;
    Tag::skip(&mut file)
        .map_err(|error| format!("Could not locate the MP3 audio payload: {error}"))?;
    let _ = file
        .stream_position()
        .map_err(|error| format!("Could not locate the MP3 audio position: {error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("Could not verify the MP3 audio payload: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().into())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileFingerprint {
    length: u64,
    modified_ns: u128,
    created_ns: u128,
    canonical_path: PathBuf,
    #[cfg(windows)]
    volume_serial: u32,
    #[cfg(windows)]
    file_index: u64,
}

impl FileFingerprint {
    fn read(path: &Path) -> Result<Self, String> {
        let metadata = fs::metadata(path)
            .map_err(|error| format!("Could not inspect the source MP3: {error}"))?;
        let modified_ns = system_time_ns(metadata.modified().ok());
        let created_ns = system_time_ns(metadata.created().ok());
        let canonical_path = fs::canonicalize(path)
            .map_err(|error| format!("Could not resolve the source MP3: {error}"))?;
        #[cfg(windows)]
        let (volume_serial, file_index) = windows_file_identity(path)?;
        Ok(Self {
            length: metadata.len(),
            modified_ns,
            created_ns,
            canonical_path,
            #[cfg(windows)]
            volume_serial,
            #[cfg(windows)]
            file_index,
        })
    }
}

impl std::fmt::Display for FileFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}:{}:{}",
            self.length, self.modified_ns, self.created_ns
        )?;
        #[cfg(windows)]
        write!(formatter, ":{}:{}", self.volume_serial, self.file_index)?;
        Ok(())
    }
}

#[cfg(windows)]
fn windows_file_identity(path: &Path) -> Result<(u32, u64), String> {
    use std::os::windows::io::AsRawHandle;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct FileTime {
        low_date_time: u32,
        high_date_time: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct ByHandleFileInformation {
        file_attributes: u32,
        creation_time: FileTime,
        last_access_time: FileTime,
        last_write_time: FileTime,
        volume_serial_number: u32,
        file_size_high: u32,
        file_size_low: u32,
        number_of_links: u32,
        file_index_high: u32,
        file_index_low: u32,
    }

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn GetFileInformationByHandle(
            file: *mut c_void,
            information: *mut ByHandleFileInformation,
        ) -> i32;
    }

    let file = File::open(path)
        .map_err(|error| format!("Could not open the MP3 for identity verification: {error}"))?;
    let mut information = ByHandleFileInformation::default();
    // SAFETY: The file handle is live and the output points to a valid writable structure.
    let result =
        unsafe { GetFileInformationByHandle(file.as_raw_handle().cast(), &mut information) };
    if result == 0 {
        return Err(format!(
            "Windows could not verify the MP3 file identity: {}",
            std::io::Error::last_os_error()
        ));
    }
    let file_index =
        (u64::from(information.file_index_high) << 32) | u64::from(information.file_index_low);
    Ok((information.volume_serial_number, file_index))
}

fn system_time_ns(value: Option<SystemTime>) -> u128 {
    value
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn operation_paths(target: &Path, operation_id: i64) -> Result<(PathBuf, PathBuf), String> {
    Ok((
        sibling_operation_path(target, operation_id, "working.tmp")?,
        sibling_operation_path(target, operation_id, "original.backup")?,
    ))
}

fn sibling_operation_path(
    target: &Path,
    operation_id: i64,
    suffix: &str,
) -> Result<PathBuf, String> {
    let parent = target
        .parent()
        .ok_or_else(|| "The MP3 has no parent directory for a safe write.".to_owned())?;
    let filename = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "The MP3 filename cannot be represented safely.".to_owned())?;
    Ok(parent.join(format!(".{filename}.aurora-{operation_id}.{suffix}")))
}

fn cleanup_owned_working_file(path: &Path) {
    let owned = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with('.') && name.contains(".aurora-"));
    if owned && path.is_file() {
        let _ = fs::remove_file(path);
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

#[cfg(windows)]
fn open_write_exclusion(path: &Path) -> Result<File, String> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_DELETE: u32 = 0x0000_0004;
    OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
        .open(path)
        .map_err(|error| {
            format!(
                "The MP3 is open for writing in another application. Close that edit and try again: {error}"
            )
        })
}

#[cfg(not(windows))]
fn open_write_exclusion(path: &Path) -> Result<File, String> {
    OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|error| format!("Could not lock the MP3 for replacement: {error}"))
}

#[cfg(windows)]
fn replace_file_atomic(
    target: &Path,
    replacement: &Path,
    backup: Option<&Path>,
) -> Result<(), String> {
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
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    let target = wide(target);
    let replacement = wide(replacement);
    let backup = backup.map(wide);
    let backup_pointer = backup
        .as_ref()
        .map_or(std::ptr::null(), |value| value.as_ptr());
    // SAFETY: All strings are owned, NUL-terminated UTF-16 buffers that outlive the call.
    let result = unsafe {
        ReplaceFileW(
            target.as_ptr(),
            replacement.as_ptr(),
            backup_pointer,
            0x0000_0001,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if result == 0 {
        Err(format!(
            "Windows could not atomically replace the MP3: {}",
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn move_file_without_replacing(source: &Path, target: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn MoveFileW(existing_file_name: *const u16, new_file_name: *const u16) -> i32;
    }

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    let source = wide(source);
    let target = wide(target);
    // SAFETY: Both strings are owned, NUL-terminated UTF-16 buffers that outlive the call.
    let result = unsafe { MoveFileW(source.as_ptr(), target.as_ptr()) };
    if result == 0 {
        Err(format!(
            "Windows could not restore Aurora's missing MP3 path without replacing another file: {}",
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn move_file_without_replacing(source: &Path, target: &Path) -> Result<(), String> {
    fs::hard_link(source, target)
        .map_err(|error| format!("Could not restore the missing MP3 path safely: {error}"))?;
    fs::remove_file(source)
        .map_err(|error| format!("Could not remove Aurora's recovered working copy: {error}"))
}

#[cfg(not(windows))]
fn replace_file_atomic(
    target: &Path,
    replacement: &Path,
    backup: Option<&Path>,
) -> Result<(), String> {
    if let Some(backup) = backup {
        fs::rename(target, backup)
            .map_err(|error| format!("Could not create the rollback file: {error}"))?;
    } else {
        fs::remove_file(target)
            .map_err(|error| format!("Could not prepare the target replacement: {error}"))?;
    }
    if let Err(error) = fs::rename(replacement, target) {
        if let Some(backup) = backup {
            let _ = fs::rename(backup, target);
        }
        return Err(format!("Could not replace the MP3: {error}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use id3::frame::{Picture, PictureType, Popularimeter};
    use std::io::Write;

    fn fixture_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "aurora-tag-{label}-{}-{unique}.mp3",
            std::process::id()
        ))
    }

    fn write_fixture(path: &Path, version: Version) -> Vec<u8> {
        let audio = b"FAKE-MPEG-AUDIO-PAYLOAD".repeat(512);
        File::create(path)
            .expect("create fixture")
            .write_all(&audio)
            .expect("write audio");
        let mut tag = Tag::with_version(version);
        tag.set_title("Sæglópur");
        tag.add_frame(ExtendedText {
            description: "MUSICBRAINZ_TRACKID".to_owned(),
            value: "fixture-mbid".to_owned(),
        });
        tag.add_frame(Popularimeter {
            user: "other-player".to_owned(),
            rating: 77,
            counter: 42,
        });
        tag.add_frame(Picture {
            mime_type: "image/jpeg".to_owned(),
            picture_type: PictureType::CoverFront,
            description: "cover".to_owned(),
            data: vec![1, 2, 3, 4],
        });
        tag.write_to_path(path, version).expect("write fixture tag");
        audio
    }

    fn replace_musicbee_values(path: &Path, values: &TagValues) {
        let (mut tag, version) = read_tag_for_write(path).expect("read fixture tags");
        let before = read_tag_values(&tag).expect("read fixture values");
        apply_tag_changes(&mut tag, version, &before, values).expect("apply MusicBee values");
        tag.write_to_path(path, version)
            .expect("write MusicBee values");
    }

    fn seed_pending_overlay(
        store: &StateStore,
        target: &Path,
        catalog_values: &TagValues,
        overlay_values: &TagValues,
    ) -> TagOverlay {
        let directory = target
            .parent()
            .expect("target directory")
            .to_string_lossy()
            .into_owned();
        let filename = target
            .file_name()
            .and_then(|name| name.to_str())
            .expect("target filename");
        let track_key = target.to_string_lossy().to_lowercase();
        store
            .upsert_overlay(
                &track_key,
                &directory,
                filename,
                catalog_values,
                overlay_values,
                52,
                Some(41),
            )
            .expect("seed overlay");
        store
            .pending_overlays(1)
            .expect("read pending overlay")
            .remove(0)
    }

    fn remove_state_fixture(path: &Path) {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(PathBuf::from(format!("{}-wal", path.display())));
        let _ = fs::remove_file(PathBuf::from(format!("{}-shm", path.display())));
    }

    #[test]
    fn pending_reconciliation_adopts_external_musicbee_rating() {
        let target = fixture_path("external-rating");
        write_fixture(&target, Version::Id3v23);
        let catalog_values = TagValues {
            rating: None,
            love_state: LoveState::Neutral,
            release_year: None,
        };
        let aurora_values = TagValues {
            rating: Some(2.0),
            ..catalog_values.clone()
        };
        replace_musicbee_values(&target, &aurora_values);
        let state_path = fixture_path("external-rating-state.sqlite3");
        let store = StateStore::new(state_path.clone()).expect("state store");
        let overlay = seed_pending_overlay(&store, &target, &catalog_values, &aurora_values);

        let musicbee_values = TagValues {
            rating: Some(4.0),
            ..catalog_values.clone()
        };
        replace_musicbee_values(&target, &musicbee_values);
        let service = TagService {
            store: store.clone(),
        };
        let outcome = service
            .reconcile_pending_overlay(&overlay, &catalog_values, 53, &target)
            .expect("reconcile external rating");

        assert!(outcome.external_change);
        assert!(!outcome.catalog_caught_up);
        let reconciled = store.pending_overlays(1).expect("reconciled overlay");
        assert_eq!(reconciled[0].values, musicbee_values);
        assert_eq!(reconciled[0].last_operation_id, Some(41));

        let _ = fs::remove_file(target);
        remove_state_fixture(&state_path);
    }

    #[test]
    fn pending_reconciliation_removes_overlay_after_catalog_catches_up() {
        let target = fixture_path("catalog-catch-up");
        write_fixture(&target, Version::Id3v23);
        let old_catalog = TagValues {
            rating: None,
            love_state: LoveState::Neutral,
            release_year: None,
        };
        let current = TagValues {
            rating: Some(2.0),
            ..old_catalog.clone()
        };
        replace_musicbee_values(&target, &current);
        let state_path = fixture_path("catalog-catch-up-state.sqlite3");
        let store = StateStore::new(state_path.clone()).expect("state store");
        let overlay = seed_pending_overlay(&store, &target, &old_catalog, &current);
        let service = TagService {
            store: store.clone(),
        };

        let outcome = service
            .reconcile_pending_overlay(&overlay, &current, 53, &target)
            .expect("reconcile catalog catch-up");

        assert!(!outcome.external_change);
        assert!(outcome.catalog_caught_up);
        assert!(store.pending_overlays(1).expect("overlays").is_empty());

        let _ = fs::remove_file(target);
        remove_state_fixture(&state_path);
    }

    #[test]
    fn pending_reconciliation_leaves_unchanged_mp3_pending() {
        let target = fixture_path("unchanged-pending");
        write_fixture(&target, Version::Id3v23);
        let catalog_values = TagValues {
            rating: None,
            love_state: LoveState::Neutral,
            release_year: None,
        };
        let current = TagValues {
            rating: Some(2.0),
            ..catalog_values.clone()
        };
        replace_musicbee_values(&target, &current);
        let state_path = fixture_path("unchanged-pending-state.sqlite3");
        let store = StateStore::new(state_path.clone()).expect("state store");
        let overlay = seed_pending_overlay(&store, &target, &catalog_values, &current);
        let service = TagService {
            store: store.clone(),
        };

        let outcome = service
            .reconcile_pending_overlay(&overlay, &catalog_values, 53, &target)
            .expect("reconcile unchanged file");

        assert!(!outcome.external_change);
        assert!(!outcome.catalog_caught_up);
        assert_eq!(
            store.pending_overlays(1).expect("overlays")[0].values,
            current
        );

        let _ = fs::remove_file(target);
        remove_state_fixture(&state_path);
    }

    #[test]
    fn pending_reconciliation_retains_overlay_when_mp3_is_missing() {
        let target = fixture_path("missing-pending");
        write_fixture(&target, Version::Id3v23);
        let catalog_values = TagValues {
            rating: None,
            love_state: LoveState::Neutral,
            release_year: None,
        };
        let current = TagValues {
            rating: Some(2.0),
            ..catalog_values.clone()
        };
        let state_path = fixture_path("missing-pending-state.sqlite3");
        let store = StateStore::new(state_path.clone()).expect("state store");
        let overlay = seed_pending_overlay(&store, &target, &catalog_values, &current);
        fs::remove_file(&target).expect("remove MP3");
        let service = TagService {
            store: store.clone(),
        };

        let result = service.reconcile_pending_overlay(&overlay, &catalog_values, 53, &target);
        let Err(failure) = result else {
            panic!("missing MP3 should not reconcile");
        };

        assert!(matches!(
            failure.kind,
            PendingOverlayFailureKind::Unavailable
        ));
        assert_eq!(
            store.pending_overlays(1).expect("overlays")[0].values,
            current
        );

        remove_state_fixture(&state_path);
    }

    #[test]
    fn writes_exact_musicbee_frames_and_preserves_everything_else() {
        let path = fixture_path("roundtrip");
        write_fixture(&path, Version::Id3v23);
        let before_hash = audio_payload_hash(&path).expect("hash before");
        let (mut tag, version) = read_tag_for_write(&path).expect("read fixture");
        let preserved = non_target_frames(&tag);
        let before = read_tag_values(&tag).expect("values before");
        let after = TagValues {
            rating: Some(4.5),
            love_state: LoveState::Loved,
            release_year: Some(2005),
        };
        apply_tag_changes(&mut tag, version, &before, &after).expect("mutate tag");
        tag.write_to_path(&path, version).expect("write tag");
        verify_written_file(&path, &after, &preserved, &before_hash).expect("verify write");

        let bytes = fs::read(&path).expect("read bytes");
        let exact_popm = [MUSICBEE_POPM_OWNER.as_bytes(), &[0, 242, 0, 0, 0, 0]].concat();
        assert!(
            bytes
                .windows(exact_popm.len())
                .any(|window| window == exact_popm)
        );
        let written = Tag::read_from_path(&path).expect("read written tag");
        assert_eq!(written.version(), Version::Id3v23);
        assert!(written.extended_texts().any(|text| {
            text.description == "MUSICBRAINZ_TRACKID" && text.value == "fixture-mbid"
        }));
        assert!(
            written
                .pictures()
                .any(|picture| picture.data == [1, 2, 3, 4])
        );
        assert_eq!(audio_payload_hash(&path).expect("hash after"), before_hash);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn v24_release_time_updates_native_and_existing_legacy_frame() {
        let mut tag = Tag::with_version(Version::Id3v24);
        tag.set_text("TDRL", "2011-02-03");
        tag.add_frame(ExtendedText {
            description: RELEASE_TIME_DESCRIPTION.to_owned(),
            value: "2011".to_owned(),
        });
        let before = read_tag_values(&tag).expect("before values");
        let after = TagValues {
            release_year: Some(2012),
            ..before.clone()
        };
        apply_tag_changes(&mut tag, Version::Id3v24, &before, &after).expect("update year");
        assert_eq!(
            tag.get("TDRL").and_then(|frame| frame.content().text()),
            Some("2012")
        );
        assert!(
            tag.extended_texts().any(|text| {
                text.description == RELEASE_TIME_DESCRIPTION && text.value == "2012"
            })
        );
    }

    #[test]
    fn validation_rejects_unknown_musicbee_rating_without_mutating() {
        let mut tag = Tag::with_version(Version::Id3v23);
        tag.add_frame(Popularimeter {
            user: MUSICBEE_POPM_OWNER.to_owned(),
            rating: 99,
            counter: 0,
        });
        assert!(read_tag_values(&tag).is_err());
    }

    #[test]
    fn undo_comparison_detects_unrelated_id3_changes() {
        let mut original = Tag::with_version(Version::Id3v23);
        original.set_title("Original title");
        original.add_frame(ExtendedText {
            description: "MUSICBRAINZ_TRACKID".to_owned(),
            value: "fixture-mbid".to_owned(),
        });
        let mut externally_changed = original.clone();
        externally_changed.set_title("Changed in MusicBee");

        assert!(!same_frames(
            &non_target_frames(&original),
            &non_target_frames(&externally_changed),
        ));
    }

    #[cfg(windows)]
    #[test]
    fn write_exclusion_still_allows_atomic_replacement() {
        let target = fixture_path("exclusive-target");
        let replacement = fixture_path("exclusive-replacement");
        let backup = fixture_path("exclusive-backup");
        fs::write(&target, b"original").expect("write target");
        fs::write(&replacement, b"replacement").expect("write replacement");
        let original_fingerprint = FileFingerprint::read(&target).expect("fingerprint target");

        let guard = open_write_exclusion(&target).expect("open exclusive guard");
        replace_file_atomic(&target, &replacement, Some(&backup)).expect("replace while guarded");
        drop(guard);

        assert_eq!(fs::read(&target).expect("read target"), b"replacement");
        assert_eq!(fs::read(&backup).expect("read backup"), b"original");
        assert_eq!(
            FileFingerprint::read(&backup)
                .expect("fingerprint backup")
                .to_string(),
            original_fingerprint.to_string()
        );
        let _ = fs::remove_file(target);
        let _ = fs::remove_file(backup);
    }

    #[test]
    fn prepared_journal_recovers_the_post_replace_crash_window() {
        let target = fixture_path("crash-window-target");
        write_fixture(&target, Version::Id3v23);
        let state_path = fixture_path("crash-window-state.sqlite3");
        let store = StateStore::new(state_path.clone()).expect("create state store");
        let (mut tag, version) = read_tag_for_write(&target).expect("read source tag");
        let before = read_tag_values(&tag).expect("read source values");
        let after = TagValues {
            rating: Some(4.5),
            love_state: LoveState::Loved,
            release_year: Some(2005),
        };
        let operation_id = store
            .begin_tag_operation(
                "fixture-track-key",
                &target.to_string_lossy(),
                &before,
                &after,
                &FileFingerprint::read(&target)
                    .expect("fingerprint")
                    .to_string(),
            )
            .expect("begin operation");
        let (working, backup) = operation_paths(&target, operation_id).expect("operation paths");
        store
            .set_operation_paths(
                operation_id,
                &working.to_string_lossy(),
                &backup.to_string_lossy(),
            )
            .expect("save operation paths");
        fs::copy(&target, &working).expect("copy source");
        apply_tag_changes(&mut tag, version, &before, &after).expect("mutate working tag");
        tag.write_to_path(&working, version)
            .expect("write working tag");
        replace_file_atomic(&target, &working, Some(&backup)).expect("simulate replacement");

        TagService::new(store.clone()).expect("recover interrupted replacement");

        assert_eq!(
            read_tag_values_from_path(&target).expect("read target"),
            after
        );
        assert!(store.interrupted_operations().expect("journal").is_empty());
        assert_eq!(
            store
                .all_overlays()
                .expect("recovered overlay")
                .first()
                .map(|overlay| overlay.values.clone()),
            Some(after.clone())
        );
        let recovered_operation = store
            .latest_undo_operation("fixture-track-key")
            .expect("undo lookup")
            .expect("undo operation");

        let undo_current = sibling_operation_path(&target, operation_id, "undo-current.backup")
            .expect("undo safety path");
        let undo_replacement = sibling_operation_path(&target, operation_id, "undo-original.tmp")
            .expect("undo replacement path");
        fs::copy(
            recovered_operation
                .backup_path
                .as_ref()
                .expect("original backup"),
            &undo_replacement,
        )
        .expect("copy undo replacement");
        store
            .begin_undo(
                operation_id,
                &undo_current.to_string_lossy(),
                &FileFingerprint::read(&target)
                    .expect("undo source fingerprint")
                    .to_string(),
            )
            .expect("begin interrupted undo");
        replace_file_atomic(&target, &undo_replacement, Some(&undo_current))
            .expect("simulate undo replacement");

        TagService::new(store.clone()).expect("recover interrupted undo");

        assert_eq!(
            read_tag_values_from_path(&target).expect("read undone target"),
            before
        );
        assert!(store.all_overlays().expect("undo overlay").is_empty());
        assert!(
            store
                .latest_undo_operation("fixture-track-key")
                .expect("completed undo lookup")
                .is_none()
        );
        let _ = fs::remove_file(target);
        let _ = fs::remove_file(backup);
        let _ = fs::remove_file(undo_current);
        let _ = fs::remove_file(&state_path);
        let _ = fs::remove_file(PathBuf::from(format!("{}-wal", state_path.display())));
        let _ = fs::remove_file(PathBuf::from(format!("{}-shm", state_path.display())));
    }

    #[test]
    fn partial_replace_failure_restores_the_missing_canonical_path() {
        let target = fixture_path("partial-replace-target");
        write_fixture(&target, Version::Id3v23);
        let state_path = fixture_path("partial-replace-state.sqlite3");
        let store = StateStore::new(state_path.clone()).expect("create state store");
        let (mut tag, version) = read_tag_for_write(&target).expect("read source tag");
        let before = read_tag_values(&tag).expect("read source values");
        let after = TagValues {
            rating: Some(4.5),
            love_state: LoveState::Loved,
            release_year: Some(2005),
        };
        let operation_id = store
            .begin_tag_operation(
                "partial-replace-track-key",
                &target.to_string_lossy(),
                &before,
                &after,
                &FileFingerprint::read(&target)
                    .expect("source fingerprint")
                    .to_string(),
            )
            .expect("begin operation");
        let (working, backup) = operation_paths(&target, operation_id).expect("operation paths");
        store
            .set_operation_paths(
                operation_id,
                &working.to_string_lossy(),
                &backup.to_string_lossy(),
            )
            .expect("save operation paths");
        fs::copy(&target, &working).expect("copy source");
        apply_tag_changes(&mut tag, version, &before, &after).expect("mutate working tag");
        tag.write_to_path(&working, version)
            .expect("write working tag");

        fs::rename(&target, &backup).expect("simulate partial save move");
        assert!(!target.exists());
        TagService::new(store.clone()).expect("recover partial save");
        assert_eq!(
            read_tag_values_from_path(&target).expect("read recovered target"),
            after
        );
        assert!(backup.is_file());

        let recovered_operation = store
            .latest_undo_operation("partial-replace-track-key")
            .expect("undo lookup")
            .expect("undo operation");
        let undo_current = sibling_operation_path(&target, operation_id, "undo-current.backup")
            .expect("undo safety path");
        let undo_replacement = sibling_operation_path(&target, operation_id, "undo-original.tmp")
            .expect("undo replacement path");
        fs::copy(
            recovered_operation
                .backup_path
                .as_ref()
                .expect("original backup"),
            &undo_replacement,
        )
        .expect("copy undo replacement");
        store
            .begin_undo(
                operation_id,
                &undo_current.to_string_lossy(),
                &FileFingerprint::read(&target)
                    .expect("undo source fingerprint")
                    .to_string(),
            )
            .expect("begin undo");
        fs::rename(&target, &undo_current).expect("simulate partial undo move");
        assert!(!target.exists());

        TagService::new(store.clone()).expect("recover partial undo");
        assert_eq!(
            read_tag_values_from_path(&target).expect("read recovered undo"),
            before
        );
        assert!(store.interrupted_operations().expect("journal").is_empty());

        let _ = fs::remove_file(target);
        let _ = fs::remove_file(backup);
        let _ = fs::remove_file(undo_current);
        let _ = fs::remove_file(&state_path);
        let _ = fs::remove_file(PathBuf::from(format!("{}-wal", state_path.display())));
        let _ = fs::remove_file(PathBuf::from(format!("{}-shm", state_path.display())));
    }

    #[test]
    fn recovery_never_overwrites_an_ambiguous_external_change() {
        let target = fixture_path("ambiguous-recovery-target");
        write_fixture(&target, Version::Id3v23);
        let state_path = fixture_path("ambiguous-recovery-state.sqlite3");
        let store = StateStore::new(state_path.clone()).expect("create state store");
        let (mut tag, version) = read_tag_for_write(&target).expect("read source tag");
        let before = read_tag_values(&tag).expect("read source values");
        let after = TagValues {
            rating: Some(4.5),
            love_state: LoveState::Loved,
            release_year: Some(2005),
        };
        let operation_id = store
            .begin_tag_operation(
                "ambiguous-track-key",
                &target.to_string_lossy(),
                &before,
                &after,
                &FileFingerprint::read(&target)
                    .expect("source fingerprint")
                    .to_string(),
            )
            .expect("begin operation");
        let (working, backup) = operation_paths(&target, operation_id).expect("operation paths");
        store
            .set_operation_paths(
                operation_id,
                &working.to_string_lossy(),
                &backup.to_string_lossy(),
            )
            .expect("save operation paths");
        fs::copy(&target, &working).expect("copy source");
        apply_tag_changes(&mut tag, version, &before, &after).expect("mutate working tag");
        tag.write_to_path(&working, version)
            .expect("write working tag");
        replace_file_atomic(&target, &working, Some(&backup)).expect("simulate replacement");

        let (mut external, external_version) =
            read_tag_for_write(&target).expect("read installed edit");
        external.set_title("Changed in MusicBee after Aurora stopped");
        external
            .write_to_path(&target, external_version)
            .expect("write external change");
        let external_bytes = fs::read(&target).expect("capture external file");
        let backup_bytes = fs::read(&backup).expect("capture retained backup");

        TagService::new(store.clone()).expect("recover ambiguous replacement");

        assert_eq!(
            fs::read(&target).expect("read preserved target"),
            external_bytes
        );
        assert_eq!(
            fs::read(&backup).expect("read preserved backup"),
            backup_bytes
        );
        assert!(store.interrupted_operations().expect("journal").is_empty());
        assert!(store.all_overlays().expect("overlays").is_empty());

        let _ = fs::remove_file(target);
        let _ = fs::remove_file(backup);
        let _ = fs::remove_file(&state_path);
        let _ = fs::remove_file(PathBuf::from(format!("{}-wal", state_path.display())));
        let _ = fs::remove_file(PathBuf::from(format!("{}-shm", state_path.display())));
    }

    #[test]
    fn undo_recovery_never_overwrites_an_ambiguous_external_change() {
        let target = fixture_path("ambiguous-undo-target");
        write_fixture(&target, Version::Id3v23);
        let state_path = fixture_path("ambiguous-undo-state.sqlite3");
        let store = StateStore::new(state_path.clone()).expect("create state store");
        let (mut tag, version) = read_tag_for_write(&target).expect("read source tag");
        let before = read_tag_values(&tag).expect("read source values");
        let after = TagValues {
            rating: Some(4.5),
            love_state: LoveState::Loved,
            release_year: Some(2005),
        };
        let operation_id = store
            .begin_tag_operation(
                "ambiguous-undo-track-key",
                &target.to_string_lossy(),
                &before,
                &after,
                &FileFingerprint::read(&target)
                    .expect("source fingerprint")
                    .to_string(),
            )
            .expect("begin operation");
        let (working, backup) = operation_paths(&target, operation_id).expect("operation paths");
        store
            .set_operation_paths(
                operation_id,
                &working.to_string_lossy(),
                &backup.to_string_lossy(),
            )
            .expect("save operation paths");
        fs::copy(&target, &working).expect("copy source");
        apply_tag_changes(&mut tag, version, &before, &after).expect("mutate working tag");
        tag.write_to_path(&working, version)
            .expect("write working tag");
        replace_file_atomic(&target, &working, Some(&backup)).expect("install Aurora edit");
        let directory = target
            .parent()
            .expect("target parent")
            .to_string_lossy()
            .into_owned();
        let filename = target
            .file_name()
            .and_then(|name| name.to_str())
            .expect("target filename");
        store
            .finish_tag_operation(
                operation_id,
                "ambiguous-undo-track-key",
                &directory,
                filename,
                &before,
                &after,
                0,
            )
            .expect("finish edit journal");

        let undo_current = sibling_operation_path(&target, operation_id, "undo-current.backup")
            .expect("undo safety path");
        let undo_replacement = sibling_operation_path(&target, operation_id, "undo-original.tmp")
            .expect("undo replacement path");
        fs::copy(&backup, &undo_replacement).expect("copy undo replacement");
        store
            .begin_undo(
                operation_id,
                &undo_current.to_string_lossy(),
                &FileFingerprint::read(&target)
                    .expect("undo source fingerprint")
                    .to_string(),
            )
            .expect("begin undo");
        replace_file_atomic(&target, &undo_replacement, Some(&undo_current)).expect("install undo");

        let (mut external, external_version) =
            read_tag_for_write(&target).expect("read installed undo");
        external.set_title("Changed in MusicBee after interrupted undo");
        external
            .write_to_path(&target, external_version)
            .expect("write external change");
        let external_bytes = fs::read(&target).expect("capture external file");
        let undo_backup_bytes = fs::read(&undo_current).expect("capture undo backup");

        TagService::new(store.clone()).expect("recover ambiguous undo");

        assert_eq!(
            fs::read(&target).expect("read preserved target"),
            external_bytes
        );
        assert_eq!(
            fs::read(&undo_current).expect("read preserved undo backup"),
            undo_backup_bytes
        );
        assert!(store.interrupted_operations().expect("journal").is_empty());

        let _ = fs::remove_file(target);
        let _ = fs::remove_file(backup);
        let _ = fs::remove_file(undo_current);
        let _ = fs::remove_file(&state_path);
        let _ = fs::remove_file(PathBuf::from(format!("{}-wal", state_path.display())));
        let _ = fs::remove_file(PathBuf::from(format!("{}-shm", state_path.display())));
    }
}
