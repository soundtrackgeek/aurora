use crate::{
    artwork::{
        CanonicalCover, canonical_front_cover_fingerprint, front_cover_fingerprint,
        front_cover_matches, validate_cover_bytes,
    },
    catalog::{self, CoverArchiveEntry, ResolvedTrack, TrackSummary},
    library_sync::CatalogSync,
    state_store::{StateStore, TagOperation, TagOverlay},
    tag_model::{
        EditableTagField, EditableTagValues, LoveState, TagEditRequest, TagEditorSnapshot,
        TagEditorTarget, TagEditorTrackState, TagEditorUpdateRequest, TagEditorUpdateResult,
        TagSyncState, TagValues, TrackTagState,
    },
};
use id3::{
    Tag, TagLike, Version,
    frame::{Content, ExtendedText, Frame, Unknown},
    no_tag_ok,
};
use image::ImageFormat;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    ffi::c_void,
    fs::{self, File, OpenOptions},
    io::{Cursor, Read, Seek, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MUSICBEE_POPM_OWNER: &str = "MusicBee";
const LEGACY_DEFAULT_POPM_OWNER: &str = "Default";
const LOVE_RATING_DESCRIPTION: &str = "LOVE RATING";
const RELEASE_TIME_DESCRIPTION: &str = "TDRL";
const DISPLAY_ARTIST_DESCRIPTION: &str = "DISPLAY ARTIST";
const MAX_PENDING_RECONCILIATION_BATCH: usize = 200;
const MAX_TAG_EDITOR_ALBUM_TRACKS: usize = 500;

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
const LEGACY_DEFAULT_RATINGS: [(f64, u8); 5] =
    [(1.0, 51), (2.0, 102), (3.0, 153), (4.0, 204), (5.0, 255)];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrackTagSnapshot {
    pub(crate) track: TrackSummary,
    pub(crate) tag_state: TrackTagState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) catalog_sync: Option<CatalogSync>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) catalog_sync: Option<CatalogSync>,
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
            catalog_sync: None,
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

#[derive(Clone)]
pub(crate) struct TagService {
    store: StateStore,
}

struct PreparedEditorWrite {
    resolved: ResolvedTrack,
    fingerprint: FileFingerprint,
    tag: Tag,
    version: Version,
    payload_hash: [u8; 32],
    before: EditableTagValues,
    after: EditableTagValues,
    before_legacy: TagValues,
    after_legacy: TagValues,
    preserved_frames: Vec<Frame>,
    before_artwork_fingerprint: Option<[u8; 32]>,
    artwork_changed: bool,
}

struct PreparedEditorWriteFailure {
    message: String,
    installed: bool,
}

struct PreparedCoverArchiveWrite {
    target: PathBuf,
    temporary: PathBuf,
    backup: PathBuf,
    mime_type: String,
    before_digest: [u8; 32],
    after_digest: [u8; 32],
}

impl PreparedCoverArchiveWrite {
    fn prepare(entry: CoverArchiveEntry, cover: &CanonicalCover) -> Result<Self, String> {
        let before_digest = archive_image_digest(&entry.path, &entry.mime_type)?;
        let bytes = cover_bytes_for_archive(cover, &entry.mime_type)?;
        let (after_digest, after_mime) = validate_cover_bytes(&bytes)?;
        if !same_cover_mime(after_mime, &entry.mime_type) {
            return Err(
                "Aurora could not preserve the archive cover's indexed image format.".to_owned(),
            );
        }
        let parent = entry
            .path
            .parent()
            .ok_or_else(|| "The album-cover archive entry has no parent folder.".to_owned())?;
        let filename = entry
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| "The album-cover archive filename is invalid.".to_owned())?;
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temporary = parent.join(format!(
            ".{filename}.aurora-{}-{unique}.cover-working.tmp",
            std::process::id()
        ));
        let backup = parent.join(format!(
            ".{filename}.aurora-{}-{unique}.cover-original.backup",
            std::process::id()
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| format!("Could not stage the archived album cover: {error}"))?;
        if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
            cleanup_owned_working_file(&temporary);
            return Err(format!(
                "Could not flush the staged archived album cover: {error}"
            ));
        }
        drop(file);
        match archive_image_digest(&temporary, &entry.mime_type) {
            Ok(digest) if digest == after_digest => {}
            Ok(_) => {
                cleanup_owned_working_file(&temporary);
                return Err(
                    "The staged archived album cover did not verify before the MP3 batch."
                        .to_owned(),
                );
            }
            Err(error) => {
                cleanup_owned_working_file(&temporary);
                return Err(format!(
                    "Aurora could not verify the staged archived album cover: {error}"
                ));
            }
        }
        Ok(Self {
            target: entry.path,
            temporary,
            backup,
            mime_type: entry.mime_type,
            before_digest,
            after_digest,
        })
    }

    fn cleanup_staged(&self) {
        cleanup_owned_working_file(&self.temporary);
    }

    fn install(self) -> Result<(), String> {
        let install_result = (|| -> Result<(), String> {
            let _write_exclusion = open_archive_write_exclusion(&self.target)?;
            if archive_image_digest(&self.target, &self.mime_type)? != self.before_digest {
                return Err(
                    "The archived album cover changed while Aurora prepared the MP3 batch."
                        .to_owned(),
                );
            }
            replace_file_atomic(&self.target, &self.temporary, Some(&self.backup))
                .map_err(|error| format!("Could not replace the archived album cover: {error}"))?;
            if archive_image_digest(&self.target, &self.mime_type)? != self.after_digest {
                return Err("The replaced archived album cover did not verify.".to_owned());
            }
            fs::remove_file(&self.backup).map_err(|error| {
                format!("Could not remove the archived cover's completed safety backup: {error}")
            })?;
            Ok(())
        })();
        if let Err(error) = install_result {
            self.cleanup_staged();
            return match restore_cover_archive_backup(&self) {
                Ok(()) => Err(format!(
                    "Aurora could not finish the archived cover replacement and restored the old archive image: {error}"
                )),
                Err(restore_error) => Err(format!(
                    "Aurora could not finish the archived cover replacement. Every available archive file was retained for manual recovery: {error}. Restore error: {restore_error}"
                )),
            };
        }
        Ok(())
    }
}

fn canonical_cover_mime(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => Some("image/jpeg"),
        "image/png" => Some("image/png"),
        "image/gif" => Some("image/gif"),
        "image/bmp" | "image/x-ms-bmp" => Some("image/bmp"),
        "image/webp" => Some("image/webp"),
        _ => None,
    }
}

fn same_cover_mime(left: &str, right: &str) -> bool {
    canonical_cover_mime(left).is_some_and(|left| Some(left) == canonical_cover_mime(right))
}

fn cover_bytes_for_archive(cover: &CanonicalCover, archive_mime: &str) -> Result<Vec<u8>, String> {
    let (_, selected_mime) = validate_cover_bytes(&cover.picture.data)?;
    if same_cover_mime(selected_mime, archive_mime) {
        return Ok(cover.picture.data.clone());
    }
    let format = match canonical_cover_mime(archive_mime) {
        Some("image/jpeg") => ImageFormat::Jpeg,
        Some("image/png") => ImageFormat::Png,
        Some("image/gif") => ImageFormat::Gif,
        Some("image/bmp") => ImageFormat::Bmp,
        Some("image/webp") => ImageFormat::WebP,
        _ => {
            return Err(
                "The indexed album-cover archive format is not supported for replacement."
                    .to_owned(),
            );
        }
    };
    let image = image::load_from_memory(&cover.picture.data)
        .map_err(|_| "Aurora could not decode the selected cover for the archive.".to_owned())?;
    let mut output = Cursor::new(Vec::new());
    image
        .write_to(&mut output, format)
        .map_err(|_| "Aurora could not encode the selected cover for the archive.".to_owned())?;
    Ok(output.into_inner())
}

fn archive_image_digest(path: &Path, expected_mime: &str) -> Result<[u8; 32], String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("Could not read the archived album cover: {error}"))?;
    let (digest, actual_mime) = validate_cover_bytes(&bytes)?;
    if !same_cover_mime(actual_mime, expected_mime) {
        return Err("The archived album cover does not match its indexed image format.".to_owned());
    }
    Ok(digest)
}

fn restore_cover_archive_backup(item: &PreparedCoverArchiveWrite) -> Result<(), String> {
    if item.target.is_file()
        && archive_image_digest(&item.target, &item.mime_type).ok() == Some(item.before_digest)
    {
        cleanup_owned_working_file(&item.backup);
        return Ok(());
    }
    if !item.backup.is_file()
        || archive_image_digest(&item.backup, &item.mime_type).ok() != Some(item.before_digest)
    {
        return Err(
            "The original archived cover safety copy is unavailable or invalid.".to_owned(),
        );
    }
    if item.target.is_file() {
        replace_file_atomic(&item.target, &item.backup, None)
            .map_err(|error| format!("Could not restore the old archived cover: {error}"))?;
    } else {
        move_file_without_replacing(&item.backup, &item.target)
            .map_err(|error| format!("Could not restore the missing archived cover: {error}"))?;
    }
    if archive_image_digest(&item.target, &item.mime_type)? != item.before_digest {
        return Err("The restored archived cover did not verify.".to_owned());
    }
    Ok(())
}

#[cfg(windows)]
fn open_archive_write_exclusion(path: &Path) -> Result<File, String> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_DELETE: u32 = 0x0000_0004;
    OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
        .open(path)
        .map_err(|error| {
            format!("The archived album cover is open for writing in another application: {error}")
        })
}

#[cfg(not(windows))]
fn open_archive_write_exclusion(path: &Path) -> Result<File, String> {
    OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|error| format!("Could not lock the archived album cover: {error}"))
}

impl From<String> for PreparedEditorWriteFailure {
    fn from(message: String) -> Self {
        Self {
            message,
            installed: false,
        }
    }
}

#[derive(Deserialize)]
struct OperationEditorMetadata {
    before: EditableTagValues,
    after: EditableTagValues,
    fields: Vec<EditableTagField>,
    before_artwork_fingerprint: Option<[u8; 32]>,
    after_artwork_fingerprint: Option<[u8; 32]>,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum OperationEditorFieldsJournal {
    Legacy(Vec<EditableTagField>),
    Current {
        fields: Vec<EditableTagField>,
        before_artwork_fingerprint: Option<[u8; 32]>,
        after_artwork_fingerprint: Option<[u8; 32]>,
    },
}

impl TagService {
    pub(crate) fn new(store: StateStore) -> Result<Self, String> {
        let service = Self { store };
        service.recover_interrupted_operations()?;
        let _ = service.store.cleanup_completed_tag_backups();
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

    pub(crate) fn inspect_editor(
        &self,
        target: TagEditorTarget,
    ) -> Result<TagEditorSnapshot, String> {
        let resolved = self.resolve_editor_target(&target)?;
        let mut tracks = Vec::with_capacity(resolved.len());
        for track in resolved {
            let (state, _) = editor_state_for_resolved(track)?;
            tracks.push(state);
        }
        Ok(TagEditorSnapshot { tracks })
    }

    pub(crate) fn editor_album_directory(
        &self,
        target: &TagEditorTarget,
    ) -> Result<PathBuf, String> {
        if !matches!(target, TagEditorTarget::Album { .. }) {
            return Err("Choose one complete album before replacing its cover.".to_owned());
        }
        let resolved = self.resolve_editor_target(target)?;
        let directory = resolved
            .first()
            .and_then(|track| track.audio_path.parent())
            .ok_or_else(|| "The selected album folder is unavailable.".to_owned())?
            .to_path_buf();
        if resolved
            .iter()
            .any(|track| track.audio_path.parent() != Some(directory.as_path()))
        {
            return Err("The selected album spans more than one folder, so Aurora cannot root the cover picker safely.".to_owned());
        }
        Ok(directory)
    }

    pub(crate) fn update_editor(
        &self,
        request: TagEditorUpdateRequest,
        artwork: Option<CanonicalCover>,
    ) -> Result<TagEditorUpdateResult, String> {
        request.validate()?;
        if request.artwork_token.is_some() != artwork.is_some() {
            return Err(
                "The selected album cover expired. Choose it again before saving.".to_owned(),
            );
        }
        if artwork.is_some() && !matches!(&request.target, TagEditorTarget::Album { .. }) {
            return Err(
                "Album artwork can only be replaced for one complete album selection.".to_owned(),
            );
        }
        let archive_entry = match (&request.target, artwork.as_ref()) {
            (TagEditorTarget::Album { album_id, .. }, Some(_)) => {
                Some(catalog::resolve_cover_archive_entry(album_id)?)
            }
            _ => None,
        };
        let desired_patch = request.values.clone().normalize();
        desired_patch.validate()?;
        let resolved = self.resolve_editor_update_target(&request.target, &request.expected)?;
        if request.expected.tracks.len() != resolved.len() {
            return Err(
                "The tag selection changed after the editor opened. Reload before saving."
                    .to_owned(),
            );
        }

        let mut expected_by_key = HashMap::with_capacity(request.expected.tracks.len());
        let mut expected_ids = HashSet::with_capacity(request.expected.tracks.len());
        for expected in &request.expected.tracks {
            if !expected_ids.insert(expected.track_id.as_str())
                || expected_by_key
                    .insert(expected.track_key.as_str(), expected)
                    .is_some()
            {
                return Err(
                    "The saved tag selection contains duplicate tracks. Reload before saving."
                        .to_owned(),
                );
            }
        }

        // Every selected file is opened, decoded and revision-checked before the first write.
        let mut prepared = Vec::with_capacity(resolved.len());
        for resolved in resolved {
            let expected = expected_by_key
                .remove(resolved.summary.track_key.as_str())
                .ok_or_else(|| {
                    "The tag selection changed after the editor opened. Reload before saving."
                        .to_owned()
                })?;
            let fingerprint = FileFingerprint::read(&resolved.audio_path)?;
            let (tag, version) = read_tag_for_write(&resolved.audio_path)?;
            let before = read_editable_tag_values(&tag)?;
            let before_legacy = read_tag_values(&tag)?;
            let payload_hash = audio_payload_hash(&resolved.audio_path)?;
            if FileFingerprint::read(&resolved.audio_path)? != fingerprint {
                return Err(
                    "An MP3 changed while Aurora checked the batch. No tags were written."
                        .to_owned(),
                );
            }
            verify_editor_expected(expected, &fingerprint, &before)?;
            let after = merge_editor_patch(&before, &request.fields, &desired_patch)?;
            let mut after_legacy = before_legacy.clone();
            after_legacy.rating = after.rating;
            after_legacy.release_year = after.release_year;
            after_legacy.validate()?;
            let artwork_changed = artwork
                .as_ref()
                .is_some_and(|cover| !front_cover_matches(&tag, &cover.digest));
            let before_artwork_fingerprint = artwork_changed.then(|| front_cover_fingerprint(&tag));
            let preserved_frames = editor_non_target_frames(&tag, &request.fields, artwork_changed);
            prepared.push(PreparedEditorWrite {
                resolved,
                fingerprint,
                tag,
                version,
                payload_hash,
                before,
                after,
                before_legacy,
                after_legacy,
                preserved_frames,
                before_artwork_fingerprint,
                artwork_changed,
            });
        }
        if !expected_by_key.is_empty() {
            return Err(
                "The tag selection changed after the editor opened. No tags were written."
                    .to_owned(),
            );
        }
        let archive_write = match (archive_entry, artwork.as_ref()) {
            (Some(entry), Some(cover)) => Some(PreparedCoverArchiveWrite::prepare(entry, cover)?),
            _ => None,
        };

        let mut completed = Vec::new();
        for item in prepared {
            if item.before == item.after && !item.artwork_changed {
                continue;
            }
            let track_id = item.resolved.summary.id.clone();
            let track_key = item.resolved.summary.track_key.clone();
            match self.write_prepared_editor(item, &request.fields, artwork.as_ref()) {
                Ok(()) => completed.push((track_id, track_key)),
                Err(write_error) => {
                    if let Some(archive_write) = &archive_write {
                        archive_write.cleanup_staged();
                    }
                    let mut rollback_errors = Vec::new();
                    if write_error.installed {
                        let recovered = self.recover_interrupted_operations();
                        match recovered.and_then(|()| self.undo(&track_id, &track_key).map(|_| ()))
                        {
                            Ok(()) => {}
                            Err(error) => rollback_errors.push(format!("{track_key}: {error}")),
                        }
                    }
                    for (track_id, track_key) in completed.iter().rev() {
                        if let Err(error) = self.undo(track_id, track_key) {
                            rollback_errors.push(format!("{track_key}: {error}"));
                        }
                    }
                    if rollback_errors.is_empty() {
                        let _ = self.store.cleanup_completed_tag_backups();
                        return Err(format!(
                            "Aurora could not finish the batch and restored every MP3 it had changed. {}",
                            write_error.message
                        ));
                    }
                    return Err(format!(
                        "Aurora could not finish the batch. Some MP3s may still contain the edit; retained backups require recovery. Write error: {}. Rollback errors: {}",
                        write_error.message,
                        rollback_errors.join("; ")
                    ));
                }
            }
        }
        if let Some(archive_write) = archive_write
            && let Err(archive_error) = archive_write.install()
        {
            let mut rollback_errors = Vec::new();
            for (track_id, track_key) in completed.iter().rev() {
                if let Err(error) = self.undo(track_id, track_key) {
                    rollback_errors.push(format!("{track_key}: {error}"));
                }
            }
            if rollback_errors.is_empty() {
                let _ = self.store.cleanup_completed_tag_backups();
                return Err(format!(
                    "Aurora could not finish the cover replacement and restored every MP3 it had changed. {archive_error}"
                ));
            }
            return Err(format!(
                "Aurora could not finish the cover replacement. Some MP3s may still contain the edit; retained backups require recovery. Archive error: {archive_error}. Rollback errors: {}",
                rollback_errors.join("; ")
            ));
        }
        let changed_keys = completed
            .iter()
            .map(|(_, track_key)| track_key.clone())
            .collect::<HashSet<_>>();
        self.store.cleanup_completed_tag_backups().map_err(|error| {
            format!(
                "Aurora installed and verified every MP3 edit, but could not remove its completed safety backups: {error}"
            )
        })?;
        self.editor_result(&request.target, &request.expected, &changed_keys)
    }

    fn resolve_editor_target(
        &self,
        target: &TagEditorTarget,
    ) -> Result<Vec<ResolvedTrack>, String> {
        match target {
            TagEditorTarget::Track {
                track_id,
                track_key,
                ..
            } => Ok(vec![catalog::resolve_track(
                track_id,
                track_key,
                &self.store,
            )?]),
            TagEditorTarget::Album { album_id, .. } => {
                catalog::resolve_album_tracks(album_id, &self.store)
            }
            TagEditorTarget::Tracks { tracks, .. } => {
                if tracks.is_empty() || tracks.len() > MAX_TAG_EDITOR_ALBUM_TRACKS {
                    return Err(format!(
                        "Select between 1 and {MAX_TAG_EDITOR_ALBUM_TRACKS} tracks for tag editing."
                    ));
                }
                let mut seen = HashSet::with_capacity(tracks.len());
                tracks
                    .iter()
                    .map(|track| {
                        if !seen.insert(track.track_key.as_str()) {
                            return Err("The tag selection contains duplicate tracks.".to_owned());
                        }
                        catalog::resolve_track(&track.track_id, &track.track_key, &self.store)
                    })
                    .collect()
            }
            TagEditorTarget::Albums { album_ids, .. } => {
                if album_ids.is_empty() || album_ids.len() > 100 {
                    return Err("Select between 1 and 100 albums for tag editing.".to_owned());
                }
                let mut seen_albums = HashSet::with_capacity(album_ids.len());
                let mut seen_tracks = HashSet::new();
                let mut resolved = Vec::new();
                for album_id in album_ids {
                    if !seen_albums.insert(album_id.as_str()) {
                        return Err("The tag selection contains duplicate albums.".to_owned());
                    }
                    for track in catalog::resolve_album_tracks(album_id, &self.store)? {
                        if seen_tracks.insert(track.summary.track_key.clone()) {
                            resolved.push(track);
                        }
                        if resolved.len() > MAX_TAG_EDITOR_ALBUM_TRACKS {
                            return Err(format!(
                                "The selected albums contain more than {MAX_TAG_EDITOR_ALBUM_TRACKS} tracks. Narrow the selection before editing tags."
                            ));
                        }
                    }
                }
                if resolved.is_empty() {
                    return Err("The selected albums no longer contain any tracks.".to_owned());
                }
                Ok(resolved)
            }
        }
    }

    fn resolve_editor_update_target(
        &self,
        target: &TagEditorTarget,
        expected: &TagEditorSnapshot,
    ) -> Result<Vec<ResolvedTrack>, String> {
        let primary = self.resolve_editor_target(target);
        if matches!(
            target,
            TagEditorTarget::Tracks { .. } | TagEditorTarget::Albums { .. }
        ) {
            return primary.and_then(|resolved| {
                editor_selection_matches_expected(&resolved, expected)
                    .then_some(resolved)
                    .ok_or_else(|| {
                        "The tag selection changed after the editor opened. Reload before saving."
                            .to_owned()
                    })
            });
        }
        let TagEditorTarget::Album { album_id, .. } = target else {
            return primary;
        };
        if album_id.trim().is_empty() || album_id.chars().count() > 512 {
            return Err("Album identity is invalid.".to_owned());
        }
        if let Ok(resolved) = &primary
            && editor_selection_matches_expected(resolved, expected)
        {
            return primary;
        }

        self.resolve_expected_album_tracks(expected).map_err(|fallback_error| {
            let primary_error = primary
                .err()
                .unwrap_or_else(|| "the catalog album now has a different track set".to_owned());
            format!(
                "The album changed identity after Music Library refreshed it, and Aurora could not safely rebind every selected file. Original lookup: {primary_error}. Stable-file lookup: {fallback_error}"
            )
        })
    }

    fn resolve_expected_album_tracks(
        &self,
        expected: &TagEditorSnapshot,
    ) -> Result<Vec<ResolvedTrack>, String> {
        if expected.tracks.is_empty() || expected.tracks.len() > MAX_TAG_EDITOR_ALBUM_TRACKS {
            return Err(format!(
                "The saved album selection must contain between 1 and {MAX_TAG_EDITOR_ALBUM_TRACKS} tracks."
            ));
        }
        let mut seen_keys = HashSet::with_capacity(expected.tracks.len());
        let mut seen_ids = HashSet::with_capacity(expected.tracks.len());
        let mut resolved = Vec::with_capacity(expected.tracks.len());
        for track in &expected.tracks {
            if !seen_keys.insert(track.track_key.as_str())
                || !seen_ids.insert(track.track_id.as_str())
            {
                return Err("The saved album selection contains duplicate tracks.".to_owned());
            }
            resolved.push(catalog::resolve_track(
                &track.track_id,
                &track.track_key,
                &self.store,
            )?);
        }
        let album_ids = resolved
            .iter()
            .filter_map(|track| track.summary.album_id.as_deref())
            .collect::<HashSet<_>>();
        if album_ids.len() != 1
            || resolved
                .iter()
                .any(|track| track.summary.album_id.is_none())
        {
            return Err(
                "The selected files no longer resolve to one complete catalog album.".to_owned(),
            );
        }
        let current_album_id =
            (*album_ids.iter().next().expect("one album id was validated")).to_owned();
        let complete_album = catalog::resolve_album_tracks(&current_album_id, &self.store)?;
        if !editor_selection_matches_expected(&complete_album, expected) {
            return Err(
                "The catalog album membership changed after the editor opened. Reload before saving."
                    .to_owned(),
            );
        }
        Ok(complete_album)
    }

    fn resolve_expected_tracks(
        &self,
        expected: &TagEditorSnapshot,
    ) -> Result<Vec<ResolvedTrack>, String> {
        if expected.tracks.is_empty() || expected.tracks.len() > MAX_TAG_EDITOR_ALBUM_TRACKS {
            return Err(format!(
                "The saved selection must contain between 1 and {MAX_TAG_EDITOR_ALBUM_TRACKS} tracks."
            ));
        }
        let mut seen_keys = HashSet::with_capacity(expected.tracks.len());
        let mut seen_ids = HashSet::with_capacity(expected.tracks.len());
        expected
            .tracks
            .iter()
            .map(|track| {
                if !seen_keys.insert(track.track_key.as_str())
                    || !seen_ids.insert(track.track_id.as_str())
                {
                    return Err("The saved selection contains duplicate tracks.".to_owned());
                }
                catalog::resolve_track(&track.track_id, &track.track_key, &self.store)
            })
            .collect()
    }

    fn editor_result(
        &self,
        target: &TagEditorTarget,
        expected: &TagEditorSnapshot,
        changed_keys: &HashSet<String>,
    ) -> Result<TagEditorUpdateResult, String> {
        let resolved = match self.resolve_editor_update_target(target, expected) {
            Ok(resolved) => resolved,
            Err(primary_error) if matches!(target, TagEditorTarget::Albums { .. }) => {
                self.resolve_expected_tracks(expected).map_err(|fallback_error| format!(
                    "Aurora saved the selected files but could not safely rebind their refreshed album identities. Original lookup: {primary_error}. Stable-file lookup: {fallback_error}"
                ))?
            }
            Err(error) => return Err(error),
        };
        let mut states = Vec::with_capacity(resolved.len());
        let mut tracks = Vec::with_capacity(resolved.len());
        for resolved in resolved {
            let (state, mut resolved) = editor_state_for_resolved(resolved)?;
            apply_editable_values_to_summary(&mut resolved.summary, &state.values);
            if changed_keys.contains(&resolved.summary.track_key) {
                resolved.summary.tag_sync_state = Some(TagSyncState::PendingImport);
            }
            resolved.summary.can_undo_tag_edit =
                self.store.can_undo(&resolved.summary.track_key)?;
            states.push(state);
            tracks.push(resolved.summary);
        }
        Ok(TagEditorUpdateResult {
            state: TagEditorSnapshot { tracks: states },
            tracks,
            catalog_sync: None,
        })
    }

    fn write_prepared_editor(
        &self,
        mut item: PreparedEditorWrite,
        fields: &[EditableTagField],
        artwork: Option<&CanonicalCover>,
    ) -> Result<(), PreparedEditorWriteFailure> {
        let target_path_text = item.resolved.audio_path.to_string_lossy().into_owned();
        let before_json = serde_json::to_string(&item.before)
            .map_err(|error| format!("Could not journal the original MP3 tags: {error}"))?;
        let after_json = serde_json::to_string(&item.after)
            .map_err(|error| format!("Could not journal the edited MP3 tags: {error}"))?;
        let after_artwork_fingerprint = item.artwork_changed.then(|| {
            canonical_front_cover_fingerprint(
                artwork.expect("an artwork change was prepared with a selected cover"),
            )
        });
        let fields_json = serde_json::to_string(&OperationEditorFieldsJournal::Current {
            fields: fields.to_vec(),
            before_artwork_fingerprint: item.before_artwork_fingerprint,
            after_artwork_fingerprint,
        })
        .map_err(|error| format!("Could not journal the selected tag fields: {error}"))?;
        let operation_id = self.store.begin_tag_operation_with_metadata(
            &item.resolved.summary.track_key,
            &target_path_text,
            &item.before_legacy,
            &item.after_legacy,
            &item.fingerprint.to_string(),
            Some(&before_json),
            Some(&after_json),
            Some(&fields_json),
        )?;
        let (temp_path, backup_path) = operation_paths(&item.resolved.audio_path, operation_id)?;
        self.store.set_operation_paths(
            operation_id,
            &temp_path.to_string_lossy(),
            &backup_path.to_string_lossy(),
        )?;

        let mut installed = false;
        let write_result = (|| -> Result<(), String> {
            if temp_path.exists() || backup_path.exists() {
                return Err(
                    "Aurora's safe-write paths already exist; no file was changed.".to_owned(),
                );
            }
            fs::copy(&item.resolved.audio_path, &temp_path).map_err(|error| {
                format!("Could not create the same-folder MP3 working copy: {error}")
            })?;
            apply_editor_tag_changes(&mut item.tag, item.version, fields, &item.after)?;
            if item.artwork_changed {
                item.tag
                    .remove_picture_by_type(id3::frame::PictureType::CoverFront);
                item.tag.add_frame(
                    artwork
                        .expect("an artwork change was prepared with a selected cover")
                        .picture
                        .clone(),
                );
            }
            item.tag
                .write_to_path(&temp_path, item.version)
                .map_err(|error| format!("Could not write the MP3 working copy: {error}"))?;
            File::options()
                .read(true)
                .write(true)
                .open(&temp_path)
                .and_then(|file| file.sync_all())
                .map_err(|error| format!("Could not flush the MP3 working copy: {error}"))?;
            verify_editor_written_file(
                &temp_path,
                &item.after,
                fields,
                item.artwork_changed,
                after_artwork_fingerprint.as_ref(),
                &item.preserved_frames,
                &item.payload_hash,
            )?;
            let _write_exclusion = open_write_exclusion(&item.resolved.audio_path)?;
            if FileFingerprint::read(&item.resolved.audio_path)? != item.fingerprint {
                return Err(
                    "The MP3 changed while Aurora prepared the edit. Its original was left untouched."
                        .to_owned(),
                );
            }
            if let Err(error) =
                replace_file_atomic(&item.resolved.audio_path, &temp_path, Some(&backup_path))
            {
                installed = replacement_requires_recovery(
                    &item.resolved.audio_path,
                    &backup_path,
                    &item.fingerprint,
                );
                return Err(if installed {
                    format!(
                        "Windows could not complete Aurora's atomic save. Every file was retained for startup recovery: {error}"
                    )
                } else {
                    format!(
                        "Windows could not start Aurora's atomic save. The original MP3 was left untouched: {error}"
                    )
                });
            }
            installed = true;
            if let Err(error) = self.store.mark_operation(operation_id, "replaced", None) {
                return Err(format!(
                    "Aurora installed the edit but could not checkpoint its journal; startup recovery will verify it: {error}"
                ));
            }
            if FileFingerprint::read(&backup_path)?.to_string() != item.fingerprint.to_string() {
                return Err(
                    "Another application replaced the MP3 during Aurora's save. Aurora retained both files for recovery."
                        .to_owned(),
                );
            }
            verify_editor_written_file(
                &item.resolved.audio_path,
                &item.after,
                fields,
                item.artwork_changed,
                after_artwork_fingerprint.as_ref(),
                &item.preserved_frames,
                &item.payload_hash,
            )
            .map_err(|error| {
                format!(
                    "Aurora could not verify the installed edit and retained both files: {error}"
                )
            })?;
            Ok(())
        })();
        if let Err(error) = write_result {
            if !installed {
                cleanup_owned_working_file(&temp_path);
                let _ = self
                    .store
                    .mark_operation(operation_id, "failed", Some(&error));
            }
            return Err(PreparedEditorWriteFailure {
                message: error,
                installed,
            });
        }
        self.store.finish_tag_operation(
            operation_id,
            &item.resolved.summary.track_key,
            &item.resolved.summary.directory,
            &item.resolved.summary.filename,
            &item.resolved.catalog_values,
            &item.after_legacy,
            item.resolved.summary.catalog_import_run_id,
        )
        .map_err(|error| PreparedEditorWriteFailure {
            message: format!(
                "Aurora installed the MP3 edit, but could not finish its journal; startup recovery will reconcile it: {error}"
            ),
            installed: true,
        })
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
                    &overlay,
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
                        &overlay,
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
                        &overlay,
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
                        self.record_pending_failure(&mut report, &overlay, failure)?;
                        continue;
                    }
                };
            match self.reconcile_pending_overlay(
                &overlay,
                &catalog_values.0,
                catalog_values.1,
                &audio_path,
            ) {
                Ok(Some(outcome)) => {
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
                Ok(None) => {}
                Err(failure) => {
                    if matches!(failure.kind, PendingOverlayFailureKind::State) {
                        return Err(failure.message);
                    }
                    self.record_pending_failure(&mut report, &overlay, failure)?;
                }
            }
        }
        Ok(report)
    }

    fn record_pending_failure(
        &self,
        report: &mut TagReconciliationReport,
        overlay: &TagOverlay,
        failure: PendingOverlayFailure,
    ) -> Result<(), String> {
        if self
            .store
            .defer_overlay_reconciliation_if_current(overlay)?
        {
            report.record_failure(&overlay.track_key, failure);
        }
        Ok(())
    }

    fn reconcile_pending_overlay(
        &self,
        overlay: &TagOverlay,
        catalog_values: &TagValues,
        catalog_import_run_id: i64,
        audio_path: &Path,
    ) -> Result<Option<PendingOverlayOutcome>, PendingOverlayFailure> {
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
        let current = self
            .store
            .reconcile_pending_overlay_if_current(
                overlay,
                catalog_values,
                &values,
                catalog_import_run_id,
            )
            .map_err(|message| PendingOverlayFailure {
                kind: PendingOverlayFailureKind::State,
                message,
            })?;
        if !current {
            return Ok(None);
        }
        Ok(Some(PendingOverlayOutcome {
            values,
            external_change,
            catalog_caught_up,
        }))
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
                recovery_required = replacement_requires_recovery(
                    &resolved.audio_path,
                    &backup_path,
                    &original_fingerprint,
                );
                return Err(if recovery_required {
                    format!(
                        "Windows could not complete Aurora's atomic save. Every file was retained for startup recovery: {error}"
                    )
                } else {
                    format!(
                        "Windows could not start Aurora's atomic save. The original MP3 was left untouched: {error}"
                    )
                });
            }
            recovery_required = true;
            if let Err(error) = self.store.mark_operation(operation_id, "replaced", None) {
                return Err(format!(
                    "Aurora installed and retained the edit, but could not checkpoint its journal. It will verify the retained files at startup: {error}"
                ));
            }
            if FileFingerprint::read(&backup_path)?.to_string() != original_fingerprint.to_string()
            {
                return Err(
                    "Another application atomically replaced the MP3 during Aurora's save. Aurora retained both files without overwriting either; reload the track before editing again."
                        .to_owned(),
                );
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
        self.store.cleanup_completed_tag_backups().map_err(|error| {
            format!(
                "Aurora installed and verified the MP3 edit, but could not remove its completed safety backup: {error}"
            )
        })?;
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
        let editor_metadata = operation_editor_metadata(&operation)?;
        let current_matches = if let Some(metadata) = &editor_metadata {
            read_editable_tag_values(&current_tag)? == metadata.after
                && metadata
                    .after_artwork_fingerprint
                    .is_none_or(|expected| front_cover_fingerprint(&current_tag) == expected)
        } else {
            current == operation.after
        };
        if !current_matches {
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
        let non_target_frames_match = if let Some(metadata) = &editor_metadata {
            let artwork_changed = metadata.after_artwork_fingerprint.is_some();
            same_frames(
                &editor_non_target_frames(&current_tag, &metadata.fields, artwork_changed),
                &editor_non_target_frames(&backup_tag, &metadata.fields, artwork_changed),
            )
        } else {
            same_frames(
                &non_target_frames(&current_tag),
                &non_target_frames(&backup_tag),
            )
        };
        if !non_target_frames_match {
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
            return Err(
                "Another application atomically replaced the MP3 during undo. Aurora retained every file and left the undo journal pending for startup recovery."
                    .to_owned(),
            );
        }
        let restored = read_tag_values_from_path(&resolved.audio_path)?;
        let restored_tag = read_tag_for_write(&resolved.audio_path)?.0;
        let restored_editor_values = editor_metadata
            .as_ref()
            .map(|_| read_editable_tag_values(&restored_tag))
            .transpose()?;
        let restored_values_match = if let Some(metadata) = &editor_metadata {
            restored_editor_values.as_ref() == Some(&metadata.before)
                && metadata
                    .before_artwork_fingerprint
                    .is_none_or(|expected| front_cover_fingerprint(&restored_tag) == expected)
        } else {
            restored == operation.before
        };
        let restored_frames_match = if let Some(metadata) = &editor_metadata {
            let artwork_changed = metadata.after_artwork_fingerprint.is_some();
            same_frames(
                &editor_non_target_frames(&restored_tag, &metadata.fields, artwork_changed),
                &editor_non_target_frames(&backup_tag, &metadata.fields, artwork_changed),
            )
        } else {
            same_frames(
                &non_target_frames(&restored_tag),
                &non_target_frames(&backup_tag),
            )
        };
        let restored_is_verified = restored_values_match
            && audio_payload_hash(&resolved.audio_path)? == audio_payload_hash(backup_path)?
            && restored_frames_match;
        if !restored_is_verified {
            return Err(
                "Aurora could not verify the installed undo. It retained every file and left the undo journal pending for startup recovery."
                    .to_owned(),
            );
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
        let mut snapshot = self.snapshot_with_values(resolved, restored, Some(operation.id))?;
        if let Some(values) = restored_editor_values {
            apply_full_editor_undo_projection(&mut snapshot, &values);
        }
        Ok(snapshot)
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
            catalog_sync: None,
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
        let Some(current_backup) = operation.temp_path.as_ref() else {
            self.store.mark_operation(
                operation.id,
                "failed",
                Some(
                    "Aurora cannot recover this interrupted undo because its journal has no safety path. The target was left untouched.",
                ),
            )?;
            return Ok(());
        };
        let undo_replacement =
            sibling_operation_path(&operation.target_path, operation.id, "undo-original.tmp")?;
        let Some(original_backup) = operation.backup_path.as_ref().filter(|path| path.is_file())
        else {
            self.store.mark_operation(
                operation.id,
                "failed",
                Some(
                    "Aurora cannot recover this interrupted undo because its original rollback copy is missing. Every remaining file was retained.",
                ),
            )?;
            return Ok(());
        };
        if !current_backup.is_file() {
            if operation.target_path.is_file()
                && operation_file_matches(&operation.target_path, original_backup, operation, false)
            {
                self.finish_recovered_undo(operation)?;
                cleanup_owned_working_file(&undo_replacement);
            } else if operation.target_path.is_file()
                && operation_file_matches(&operation.target_path, original_backup, operation, true)
            {
                cleanup_owned_working_file(&undo_replacement);
                self.store.mark_operation(
                    operation.id,
                    "verified",
                    Some("Aurora closed before replacing the MP3 during undo."),
                )?;
            } else {
                let message = if operation.target_path.is_file() {
                    "Undo recovery found an ambiguous or externally changed target and retained every remaining file."
                } else {
                    "Undo recovery found a missing target without its safety backup and retained every remaining file."
                };
                self.store
                    .mark_operation(operation.id, "failed", Some(message))?;
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
        if !operation.target_path.is_file() {
            if undo_replacement.is_file()
                && operation_file_matches(&undo_replacement, original_backup, operation, false)
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
        if operation_file_matches(&operation.target_path, original_backup, operation, false) {
            self.finish_recovered_undo(operation)?;
            cleanup_owned_working_file(current_backup);
            cleanup_owned_working_file(&undo_replacement);
        } else if operation_file_matches(&operation.target_path, original_backup, operation, true)
            && operation_file_matches(current_backup, original_backup, operation, true)
        {
            self.store.mark_operation(
                operation.id,
                "verified",
                Some("Aurora recovered an undo that stopped before installing its replacement."),
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

    fn finish_recovered_undo(
        &self,
        operation: &crate::state_store::TagOperation,
    ) -> Result<(), String> {
        let catalog_target =
            crate::device_mode::catalog_path_for_device_path(&operation.target_path);
        let directory = catalog_target
            .parent()
            .ok_or_else(|| "The recovered undo path has no parent directory.".to_owned())?
            .to_string_lossy()
            .into_owned();
        let filename = catalog_target
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
                prior_overlay.map(|overlay| (overlay.catalog_values, overlay.catalog_import_run_id))
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
        )
    }

    fn recover_replaced_operation(
        &self,
        operation: &crate::state_store::TagOperation,
    ) -> Result<(), String> {
        let Some(backup_path) = operation.backup_path.as_ref().filter(|path| path.is_file()) else {
            self.store.mark_operation(
                operation.id,
                "failed",
                Some(
                    "Aurora cannot recover this interrupted tag write because its rollback copy is missing. The target and every remaining file were retained.",
                ),
            )?;
            return Ok(());
        };
        let backup_matches_source = FileFingerprint::read(backup_path)
            .is_ok_and(|fingerprint| fingerprint.to_string() == operation.source_fingerprint);
        if !operation.target_path.is_file() {
            let replacement = operation.temp_path.as_ref().filter(|path| path.is_file());
            if backup_matches_source
                && replacement
                    .is_some_and(|path| operation_file_matches(path, backup_path, operation, true))
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
        if operation_file_matches(&operation.target_path, backup_path, operation, true)
            && backup_matches_source
        {
            if let Some(temp_path) = &operation.temp_path {
                cleanup_owned_working_file(temp_path);
            }
            let catalog_target =
                crate::device_mode::catalog_path_for_device_path(&operation.target_path);
            let directory = catalog_target
                .parent()
                .ok_or_else(|| "The recovered MP3 path has no parent directory.".to_owned())?
                .to_string_lossy()
                .into_owned();
            let filename = catalog_target
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
        } else if backup_matches_source
            && operation_file_matches(&operation.target_path, backup_path, operation, false)
        {
            self.store.mark_operation(
                operation.id,
                "failed",
                Some(
                    "Aurora recovered an atomic save that stopped before installing the edit; the original MP3 was untouched.",
                ),
            )?;
            if let Some(temp_path) = &operation.temp_path {
                cleanup_owned_working_file(temp_path);
            }
            cleanup_owned_working_file(backup_path);
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

fn editor_selection_matches_expected(
    resolved: &[ResolvedTrack],
    expected: &TagEditorSnapshot,
) -> bool {
    if resolved.len() != expected.tracks.len() {
        return false;
    }
    let expected_keys = expected
        .tracks
        .iter()
        .map(|track| track.track_key.as_str())
        .collect::<HashSet<_>>();
    expected_keys.len() == expected.tracks.len()
        && resolved
            .iter()
            .all(|track| expected_keys.contains(track.summary.track_key.as_str()))
}

fn editor_state_for_resolved(
    resolved: ResolvedTrack,
) -> Result<(TagEditorTrackState, ResolvedTrack), String> {
    let before = FileFingerprint::read(&resolved.audio_path)?;
    let (tag, _) = read_tag_for_write(&resolved.audio_path)?;
    let values = read_editable_tag_values(&tag)?;
    let after = FileFingerprint::read(&resolved.audio_path)?;
    if before != after {
        return Err("An MP3 changed while Aurora read its tags. Reload the editor.".to_owned());
    }
    Ok((
        TagEditorTrackState {
            track_id: resolved.summary.id.clone(),
            track_key: resolved.summary.track_key.clone(),
            revision: before.to_string(),
            values,
        },
        resolved,
    ))
}

fn verify_editor_expected(
    expected: &TagEditorTrackState,
    fingerprint: &FileFingerprint,
    values: &EditableTagValues,
) -> Result<(), String> {
    if expected.revision != fingerprint.to_string() || expected.values != *values {
        return Err(
            "An MP3 changed outside Aurora after the editor opened. No tags were written; reload before saving."
                .to_owned(),
        );
    }
    Ok(())
}

pub(crate) fn read_editable_tag_values(tag: &Tag) -> Result<EditableTagValues, String> {
    let legacy = read_tag_values(tag)?;
    let display_artist =
        unique_extended_text_value(tag, DISPLAY_ARTIST_DESCRIPTION, "DISPLAY ARTIST")?;
    Ok(EditableTagValues {
        album_artist: joined_text_frame_values(tag, "TPE2"),
        artist: display_artist.or_else(|| joined_text_frame_values(tag, "TPE1")),
        album: normalized_tag_text(tag.album()),
        title: normalized_tag_text(tag.title()),
        genre: normalized_tag_text(tag.genre()),
        publisher: normalized_tag_text(tag.get("TPUB").and_then(|frame| frame.content().text())),
        rating: legacy.rating,
        year: tag
            .date_recorded()
            .map(|timestamp| timestamp.year)
            .or_else(|| tag.year())
            .or_else(|| {
                tag.get("TDRC")
                    .and_then(|frame| frame.content().text())
                    .and_then(parse_release_year)
            }),
        release_year: legacy.release_year,
        track_number: tag.track(),
        track_total: tag.total_tracks(),
        disc_number: tag.disc(),
        disc_total: tag.total_discs(),
    })
}

fn normalized_tag_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn unique_extended_text_value(
    tag: &Tag,
    description: &str,
    label: &str,
) -> Result<Option<String>, String> {
    let values = tag
        .extended_texts()
        .filter(|text| text.description.eq_ignore_ascii_case(description))
        .map(|text| {
            text.value
                .trim_start()
                .trim_end_matches(|character: char| character == '\0' || character.is_whitespace())
                .to_owned()
        })
        .collect::<Vec<_>>();
    if values.len() > 1 {
        return Err(format!(
            "This MP3 has duplicate {label} values; Aurora left it untouched."
        ));
    }
    Ok(values.into_iter().next().filter(|value| !value.is_empty()))
}

fn remove_extended_text_case_insensitive(tag: &mut Tag, description: &str) -> bool {
    let descriptions = tag
        .extended_texts()
        .filter(|text| text.description.eq_ignore_ascii_case(description))
        .map(|text| text.description.clone())
        .collect::<HashSet<_>>();
    let had_match = !descriptions.is_empty();
    for description in descriptions {
        tag.remove_extended_text(Some(&description), None);
    }
    had_match
}

fn joined_text_frame_values(tag: &Tag, id: &str) -> Option<String> {
    let values = tag
        .get(id)?
        .content()
        .text_values()?
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    (!values.is_empty()).then(|| values.join("; "))
}

fn set_optional_credit_text(tag: &mut Tag, id: &str, value: Option<&str>) -> Result<(), String> {
    tag.remove(id);
    if let Some(value) = value {
        let credits = value.split(';').map(str::trim).collect::<Vec<_>>();
        if credits.is_empty()
            || credits.len() > 64
            || credits
                .iter()
                .any(|credit| credit.is_empty() || credit.chars().any(char::is_control))
        {
            return Err("Album Artist contains an invalid or empty credit.".to_owned());
        }
        tag.set_text_values(id, credits);
    }
    Ok(())
}

pub(crate) fn merge_editor_patch(
    before: &EditableTagValues,
    fields: &[EditableTagField],
    patch: &EditableTagValues,
) -> Result<EditableTagValues, String> {
    let mut after = before.clone();
    for field in fields {
        match field {
            EditableTagField::AlbumArtist => after.album_artist = patch.album_artist.clone(),
            EditableTagField::Artist => after.artist = patch.artist.clone(),
            EditableTagField::Album => after.album = patch.album.clone(),
            EditableTagField::Title => after.title = patch.title.clone(),
            EditableTagField::Genre => after.genre = patch.genre.clone(),
            EditableTagField::Publisher => after.publisher = patch.publisher.clone(),
            EditableTagField::Rating => after.rating = patch.rating,
            EditableTagField::Year => after.year = patch.year,
            EditableTagField::ReleaseYear => after.release_year = patch.release_year,
            EditableTagField::TrackNumber => after.track_number = patch.track_number,
            EditableTagField::TrackTotal => after.track_total = patch.track_total,
            EditableTagField::DiscNumber => after.disc_number = patch.disc_number,
            EditableTagField::DiscTotal => after.disc_total = patch.disc_total,
        }
    }
    normalize_number_pair(
        &mut after.track_number,
        &mut after.track_total,
        fields.contains(&EditableTagField::TrackNumber),
        fields.contains(&EditableTagField::TrackTotal),
        patch.track_number,
        patch.track_total,
        "track",
    )?;
    normalize_number_pair(
        &mut after.disc_number,
        &mut after.disc_total,
        fields.contains(&EditableTagField::DiscNumber),
        fields.contains(&EditableTagField::DiscTotal),
        patch.disc_number,
        patch.disc_total,
        "disc",
    )?;
    after.validate()?;
    Ok(after)
}

#[allow(clippy::too_many_arguments)]
fn normalize_number_pair(
    number: &mut Option<u32>,
    total: &mut Option<u32>,
    number_selected: bool,
    total_selected: bool,
    requested_number: Option<u32>,
    requested_total: Option<u32>,
    label: &str,
) -> Result<(), String> {
    if number_selected && requested_number.is_none() {
        if total_selected && requested_total.is_some() {
            return Err(format!(
                "A {label} total cannot be saved without a {label} number."
            ));
        }
        *total = None;
    }
    if number.is_none() && total.is_some() {
        return Err(format!(
            "A {label} total cannot be saved on a file without a {label} number."
        ));
    }
    if matches!((*number, *total), (Some(number), Some(total)) if number > total) {
        return Err(format!(
            "The {label} number cannot be greater than the {label} total."
        ));
    }
    Ok(())
}

pub(crate) fn apply_editor_tag_changes(
    tag: &mut Tag,
    version: Version,
    fields: &[EditableTagField],
    after: &EditableTagValues,
) -> Result<(), String> {
    for field in fields {
        match field {
            EditableTagField::AlbumArtist => {
                set_optional_credit_text(tag, "TPE2", after.album_artist.as_deref())?
            }
            EditableTagField::Artist => {
                remove_extended_text_case_insensitive(tag, DISPLAY_ARTIST_DESCRIPTION);
                if let Some(artist) = &after.artist {
                    tag.add_frame(ExtendedText {
                        description: DISPLAY_ARTIST_DESCRIPTION.to_owned(),
                        value: artist.clone(),
                    });
                }
            }
            EditableTagField::Album => set_optional_text(tag, "TALB", after.album.as_deref()),
            EditableTagField::Title => set_optional_text(tag, "TIT2", after.title.as_deref()),
            EditableTagField::Genre => set_optional_text(tag, "TCON", after.genre.as_deref()),
            EditableTagField::Publisher => {
                set_optional_text(tag, "TPUB", after.publisher.as_deref())
            }
            EditableTagField::Rating => set_musicbee_rating(tag, version, after.rating)?,
            EditableTagField::Year => {
                tag.remove("TDRC");
                tag.remove("TYER");
                if let Some(year) = after.year {
                    if version == Version::Id3v24 {
                        tag.set_text("TDRC", year.to_string());
                    } else {
                        tag.set_text("TYER", year.to_string());
                    }
                }
            }
            EditableTagField::ReleaseYear => {
                let had_legacy =
                    remove_extended_text_case_insensitive(tag, RELEASE_TIME_DESCRIPTION);
                tag.remove("TDRL");
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
            EditableTagField::TrackNumber | EditableTagField::TrackTotal => {
                set_number_pair(tag, "TRCK", after.track_number, after.track_total)
            }
            EditableTagField::DiscNumber | EditableTagField::DiscTotal => {
                set_number_pair(tag, "TPOS", after.disc_number, after.disc_total)
            }
        }
    }
    Ok(())
}

fn set_optional_text(tag: &mut Tag, id: &str, value: Option<&str>) {
    tag.remove(id);
    if let Some(value) = value {
        tag.set_text(id, value.to_owned());
    }
}

fn set_number_pair(tag: &mut Tag, id: &str, number: Option<u32>, total: Option<u32>) {
    tag.remove(id);
    if let Some(number) = number {
        let value = total.map_or_else(|| number.to_string(), |total| format!("{number}/{total}"));
        tag.set_text(id, value);
    }
}

fn set_musicbee_rating(tag: &mut Tag, version: Version, rating: Option<f64>) -> Result<(), String> {
    let preserved = tag
        .remove("POPM")
        .into_iter()
        .filter(|frame| {
            frame
                .content()
                .popularimeter()
                .is_none_or(|value| !is_musicbee_rating_owner(&value.user))
        })
        .collect::<Vec<_>>();
    for frame in preserved {
        tag.add_frame(frame);
    }
    if let Some(rating) = rating {
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
    Ok(())
}

fn apply_editable_values_to_summary(summary: &mut TrackSummary, values: &EditableTagValues) {
    summary.title = values
        .title
        .clone()
        .unwrap_or_else(|| "Untitled".to_owned());
    summary.artist = values
        .album_artist
        .clone()
        .or_else(|| values.artist.clone())
        .unwrap_or_else(|| "Unknown Artist".to_owned());
    summary.display_artist = values.artist.clone();
    summary.album = values
        .album
        .clone()
        .unwrap_or_else(|| "Unknown Album".to_owned());
    summary.genre = values.genre.clone();
    summary.publisher = values.publisher.clone();
    summary.rating = values.rating;
    summary.original_year = values.year.map(i64::from);
    summary.release_year = values.release_year.map(i64::from);
    summary.track_number = values.track_number;
    summary.track_total = values.track_total;
    summary.disc_number = values.disc_number;
    summary.disc_total = values.disc_total;
}

fn apply_full_editor_undo_projection(snapshot: &mut TrackTagSnapshot, values: &EditableTagValues) {
    apply_editable_values_to_summary(&mut snapshot.track, values);
    snapshot.track.tag_sync_state = Some(TagSyncState::PendingImport);
    snapshot.tag_state.sync_state = Some(TagSyncState::PendingImport);
}

fn is_editor_target_frame(
    frame: &Frame,
    fields: &[EditableTagField],
    artwork_changed: bool,
) -> bool {
    if artwork_changed
        && frame
            .content()
            .picture()
            .is_some_and(|picture| picture.picture_type == id3::frame::PictureType::CoverFront)
    {
        return true;
    }
    match frame.id() {
        "TPE2" => fields.contains(&EditableTagField::AlbumArtist),
        "TALB" => fields.contains(&EditableTagField::Album),
        "TIT2" => fields.contains(&EditableTagField::Title),
        "TCON" => fields.contains(&EditableTagField::Genre),
        "TPUB" => fields.contains(&EditableTagField::Publisher),
        "TDRC" | "TYER" => fields.contains(&EditableTagField::Year),
        "TDRL" => fields.contains(&EditableTagField::ReleaseYear),
        "TRCK" => {
            fields.contains(&EditableTagField::TrackNumber)
                || fields.contains(&EditableTagField::TrackTotal)
        }
        "TPOS" => {
            fields.contains(&EditableTagField::DiscNumber)
                || fields.contains(&EditableTagField::DiscTotal)
        }
        "POPM" => {
            fields.contains(&EditableTagField::Rating)
                && frame
                    .content()
                    .popularimeter()
                    .is_some_and(|value| is_musicbee_rating_owner(&value.user))
        }
        "TXXX" => frame.content().extended_text().is_some_and(|value| {
            (fields.contains(&EditableTagField::ReleaseYear)
                && value
                    .description
                    .eq_ignore_ascii_case(RELEASE_TIME_DESCRIPTION))
                || (fields.contains(&EditableTagField::Artist)
                    && value
                        .description
                        .eq_ignore_ascii_case(DISPLAY_ARTIST_DESCRIPTION))
        }),
        _ => false,
    }
}

pub(crate) fn editor_non_target_frames(
    tag: &Tag,
    fields: &[EditableTagField],
    artwork_changed: bool,
) -> Vec<Frame> {
    tag.frames()
        .filter(|frame| !is_editor_target_frame(frame, fields, artwork_changed))
        .cloned()
        .collect()
}

pub(crate) fn verify_editor_written_file(
    path: &Path,
    expected: &EditableTagValues,
    fields: &[EditableTagField],
    artwork_changed: bool,
    expected_artwork_fingerprint: Option<&[u8; 32]>,
    preserved_frames: &[Frame],
    expected_payload_hash: &[u8; 32],
) -> Result<(), String> {
    let (tag, _) = read_tag_for_write(path)?;
    if read_editable_tag_values(&tag)? != *expected {
        return Err("the edited ID3 values did not round-trip".to_owned());
    }
    if artwork_changed
        && expected_artwork_fingerprint
            .is_none_or(|expected| &front_cover_fingerprint(&tag) != expected)
    {
        return Err("the embedded front cover did not round-trip".to_owned());
    }
    if !same_frames(
        &editor_non_target_frames(&tag, fields, artwork_changed),
        preserved_frames,
    ) {
        return Err("an unselected ID3 frame changed".to_owned());
    }
    if &audio_payload_hash(path)? != expected_payload_hash {
        return Err("the audio payload changed".to_owned());
    }
    Ok(())
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
    let audio_path = crate::device_mode::resolve_device_path(directory_path).join(filename_path);
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

pub(crate) fn read_tag_for_write(path: &Path) -> Result<(Tag, Version), String> {
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
    let rating = effective_musicbee_rating(tag)?;

    let love_value = unique_extended_text_value(tag, LOVE_RATING_DESCRIPTION, "MusicBee Love")?;
    let love_state = match love_value.as_deref() {
        None => LoveState::Neutral,
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
    let legacy_release_value =
        unique_extended_text_value(tag, RELEASE_TIME_DESCRIPTION, "MusicBee Release Time")?;
    let legacy_release = legacy_release_value.as_deref().and_then(parse_release_year);

    Ok(TagValues {
        rating,
        love_state,
        release_year: native_release.or(legacy_release),
    })
}

fn is_musicbee_rating_owner(owner: &str) -> bool {
    owner == MUSICBEE_POPM_OWNER || owner == LEGACY_DEFAULT_POPM_OWNER
}

fn effective_musicbee_rating(tag: &Tag) -> Result<Option<f64>, String> {
    let ratings_for = |owner: &str| {
        tag.frames()
            .filter(|frame| frame.id() == "POPM")
            .filter_map(|frame| frame.content().popularimeter())
            .filter(|popularimeter| popularimeter.user == owner)
            .map(|popularimeter| popularimeter.rating)
            .collect::<Vec<_>>()
    };
    let musicbee = ratings_for(MUSICBEE_POPM_OWNER);
    let legacy_default = ratings_for(LEGACY_DEFAULT_POPM_OWNER);
    if musicbee.len() > 1 || legacy_default.len() > 1 {
        return Err(
            "This MP3 has duplicate MusicBee-compatible rating frames; Aurora left it untouched."
                .to_owned(),
        );
    }

    if let Some(byte) = musicbee.first().copied() {
        return (byte != 0).then(|| rating_from_byte(byte)).transpose();
    }
    let Some(byte) = legacy_default.first().copied() else {
        return Ok(None);
    };
    if byte == 0 {
        return Ok(None);
    }
    LEGACY_DEFAULT_RATINGS
        .iter()
        .find(|(_, candidate)| *candidate == byte)
        .map(|(rating, _)| Some(*rating))
        .ok_or_else(|| {
            format!(
                "This MP3 uses unsupported legacy Default rating byte {byte}; Aurora left it untouched."
            )
        })
}

fn apply_tag_changes(
    tag: &mut Tag,
    version: Version,
    before: &TagValues,
    after: &TagValues,
) -> Result<(), String> {
    if before.rating != after.rating {
        set_musicbee_rating(tag, version, after.rating)?;
    }

    if before.love_state != after.love_state {
        remove_extended_text_case_insensitive(tag, LOVE_RATING_DESCRIPTION);
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
        let had_legacy = remove_extended_text_case_insensitive(tag, RELEASE_TIME_DESCRIPTION);
        tag.remove("TDRL");
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
            .is_some_and(|value| is_musicbee_rating_owner(&value.user)),
        "TXXX" => frame.content().extended_text().is_some_and(|value| {
            value
                .description
                .eq_ignore_ascii_case(LOVE_RATING_DESCRIPTION)
                || value
                    .description
                    .eq_ignore_ascii_case(RELEASE_TIME_DESCRIPTION)
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

fn operation_editor_metadata(
    operation: &TagOperation,
) -> Result<Option<OperationEditorMetadata>, String> {
    match (
        operation.before_file_tags_json.as_deref(),
        operation.after_file_tags_json.as_deref(),
        operation.edited_fields_json.as_deref(),
    ) {
        (None, None, None) => Ok(None),
        (Some(before), Some(after), Some(fields)) => {
            let fields = serde_json::from_str::<OperationEditorFieldsJournal>(fields)
                .map_err(|error| format!("Aurora's edited-field journal is invalid: {error}"))?;
            let (fields, before_artwork_fingerprint, after_artwork_fingerprint) = match fields {
                OperationEditorFieldsJournal::Legacy(fields) => (fields, None, None),
                OperationEditorFieldsJournal::Current {
                    fields,
                    before_artwork_fingerprint,
                    after_artwork_fingerprint,
                } => (
                    fields,
                    before_artwork_fingerprint,
                    after_artwork_fingerprint,
                ),
            };
            Ok(Some(OperationEditorMetadata {
                before: serde_json::from_str(before).map_err(|error| {
                    format!("Aurora's original full-tag journal is invalid: {error}")
                })?,
                after: serde_json::from_str(after).map_err(|error| {
                    format!("Aurora's edited full-tag journal is invalid: {error}")
                })?,
                fields,
                before_artwork_fingerprint,
                after_artwork_fingerprint,
            }))
        }
        _ => Err("Aurora's full-tag journal is incomplete; no file was overwritten.".to_owned()),
    }
}

fn operation_file_matches(
    candidate: &Path,
    reference: &Path,
    operation: &TagOperation,
    expected_after: bool,
) -> bool {
    let Ok(metadata) = operation_editor_metadata(operation) else {
        return false;
    };
    if let Some(metadata) = metadata {
        let expected = if expected_after {
            &metadata.after
        } else {
            &metadata.before
        };
        let tags_match = match (read_tag_for_write(candidate), read_tag_for_write(reference)) {
            (Ok((candidate_tag, _)), Ok((reference_tag, _))) => {
                let artwork_changed = metadata.after_artwork_fingerprint.is_some();
                let candidate_artwork_matches = if expected_after {
                    metadata
                        .after_artwork_fingerprint
                        .is_none_or(|fingerprint| {
                            front_cover_fingerprint(&candidate_tag) == fingerprint
                        })
                } else {
                    metadata
                        .before_artwork_fingerprint
                        .is_none_or(|fingerprint| {
                            front_cover_fingerprint(&candidate_tag) == fingerprint
                        })
                };
                let reference_artwork_matches =
                    metadata
                        .before_artwork_fingerprint
                        .is_none_or(|fingerprint| {
                            front_cover_fingerprint(&reference_tag) == fingerprint
                        });
                read_editable_tag_values(&candidate_tag).is_ok_and(|values| values == *expected)
                    && candidate_artwork_matches
                    && reference_artwork_matches
                    && same_frames(
                        &editor_non_target_frames(
                            &candidate_tag,
                            &metadata.fields,
                            artwork_changed,
                        ),
                        &editor_non_target_frames(
                            &reference_tag,
                            &metadata.fields,
                            artwork_changed,
                        ),
                    )
            }
            _ => false,
        };
        let audio_matches = match (audio_payload_hash(candidate), audio_payload_hash(reference)) {
            (Ok(candidate), Ok(reference)) => candidate == reference,
            _ => false,
        };
        tags_match && audio_matches
    } else {
        known_file_matches(
            candidate,
            reference,
            if expected_after {
                &operation.after
            } else {
                &operation.before
            },
        )
    }
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

pub(crate) fn audio_payload_hash(path: &Path) -> Result<[u8; 32], String> {
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

fn replacement_requires_recovery(target: &Path, backup: &Path, original: &FileFingerprint) -> bool {
    if backup.is_file() || !target.is_file() {
        return true;
    }
    FileFingerprint::read(target)
        .ok()
        .is_none_or(|current| current != *original)
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
            "Windows could not atomically replace the file: {}",
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
        return Err(format!("Could not replace the file: {error}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use id3::frame::{Picture, PictureType, Popularimeter};
    use std::io::{Cursor, Write};

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

    fn valid_cover_bytes_with_format(red: u8, format: image::ImageFormat) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([red, 0, 0, 255]));
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, format)
            .expect("encode cover");
        bytes.into_inner()
    }

    fn valid_cover_bytes(red: u8) -> Vec<u8> {
        valid_cover_bytes_with_format(red, image::ImageFormat::Png)
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

    fn resolved_track_fixture(track_id: &str, track_key: &str, album_id: &str) -> ResolvedTrack {
        let values = TagValues {
            rating: None,
            love_state: LoveState::Neutral,
            release_year: None,
        };
        ResolvedTrack {
            summary: TrackSummary {
                id: track_id.to_owned(),
                track_key: track_key.to_owned(),
                album_id: Some(album_id.to_owned()),
                title: "Fixture title".to_owned(),
                artist: "Fixture album artist".to_owned(),
                display_artist: Some("Fixture artist".to_owned()),
                album: "Fixture album".to_owned(),
                release_year: None,
                original_year: None,
                publisher: None,
                rating: None,
                loved: false,
                love_state: LoveState::Neutral,
                tag_sync_state: None,
                can_undo_tag_edit: false,
                duration_seconds: None,
                genre: None,
                play_count: None,
                track_number: None,
                track_total: None,
                disc_number: None,
                disc_total: None,
                directory: r"D:\Music\Fixture".to_owned(),
                filename: "01.mp3".to_owned(),
                catalog_import_run_id: 1,
            },
            audio_path: PathBuf::from(r"D:\Music\Fixture\01.mp3"),
            catalog_values: values,
        }
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
            .expect("reconcile external rating")
            .expect("current overlay");

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
            .expect("reconcile catalog catch-up")
            .expect("current overlay");

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
            .expect("reconcile unchanged file")
            .expect("current overlay");

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
    fn mixed_track_total_patch_preserves_each_track_number() {
        let fields = [EditableTagField::TrackTotal];
        let patch = EditableTagValues {
            track_total: Some(12),
            ..Default::default()
        };
        let first = EditableTagValues {
            track_number: Some(1),
            track_total: Some(10),
            ..Default::default()
        };
        let second = EditableTagValues {
            track_number: Some(7),
            track_total: Some(10),
            ..Default::default()
        };

        let first = merge_editor_patch(&first, &fields, &patch).expect("first track patch");
        let second = merge_editor_patch(&second, &fields, &patch).expect("second track patch");

        assert_eq!((first.track_number, first.track_total), (Some(1), Some(12)));
        assert_eq!(
            (second.track_number, second.track_total),
            (Some(7), Some(12))
        );
    }

    #[test]
    fn editor_rejects_track_or_disc_numbers_above_their_totals() {
        let before = EditableTagValues {
            track_number: Some(3),
            track_total: Some(10),
            disc_number: Some(1),
            disc_total: Some(2),
            ..Default::default()
        };
        let track_error = merge_editor_patch(
            &before,
            &[EditableTagField::TrackNumber],
            &EditableTagValues {
                track_number: Some(11),
                ..Default::default()
            },
        )
        .expect_err("track number above total");
        assert!(track_error.contains("track number cannot be greater"));

        let disc_error = merge_editor_patch(
            &before,
            &[EditableTagField::DiscNumber],
            &EditableTagValues {
                disc_number: Some(3),
                ..Default::default()
            },
        )
        .expect_err("disc number above total");
        assert!(disc_error.contains("disc number cannot be greater"));
    }

    #[test]
    fn full_editor_preserves_audio_and_unselected_frames() {
        let path = fixture_path("full-editor-roundtrip");
        write_fixture(&path, Version::Id3v24);
        let (mut tag, version) = read_tag_for_write(&path).expect("read fixture");
        tag.set_artist("Old Artist");
        tag.set_album_artist("Old Album Artist");
        tag.set_album("Old Album");
        tag.set_text("TRCK", "3/10");
        tag.add_frame(ExtendedText {
            description: DISPLAY_ARTIST_DESCRIPTION.to_owned(),
            value: "Old Artist".to_owned(),
        });
        tag.write_to_path(&path, version).expect("seed editor tags");

        let payload_hash = audio_payload_hash(&path).expect("payload before");
        let (mut tag, version) = read_tag_for_write(&path).expect("read seeded fixture");
        let before = read_editable_tag_values(&tag).expect("editable values before");
        let fields = [
            EditableTagField::Artist,
            EditableTagField::Album,
            EditableTagField::Rating,
            EditableTagField::TrackTotal,
        ];
        let patch = EditableTagValues {
            artist: Some("New Artist".to_owned()),
            album: Some("New Album".to_owned()),
            rating: Some(4.5),
            track_total: Some(12),
            ..Default::default()
        };
        let after = merge_editor_patch(&before, &fields, &patch).expect("merge edit");
        let preserved = editor_non_target_frames(&tag, &fields, false);
        apply_editor_tag_changes(&mut tag, version, &fields, &after).expect("apply edit");
        tag.write_to_path(&path, version).expect("write edit");

        verify_editor_written_file(
            &path,
            &after,
            &fields,
            false,
            None,
            &preserved,
            &payload_hash,
        )
        .expect("verify full edit");
        let written = Tag::read_from_path(&path).expect("read written tag");
        assert!(written.extended_texts().any(|text| {
            text.description == DISPLAY_ARTIST_DESCRIPTION && text.value == "New Artist"
        }));
        assert!(written.extended_texts().any(|text| {
            text.description == "MUSICBRAINZ_TRACKID" && text.value == "fixture-mbid"
        }));
        assert!(written.frames().any(|frame| {
            frame.content().popularimeter().is_some_and(|popm| {
                popm.user == "other-player" && popm.rating == 77 && popm.counter == 42
            })
        }));
        assert_eq!(
            audio_payload_hash(&path).expect("payload after"),
            payload_hash
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn full_editor_replaces_only_front_artwork_and_verifies_the_selected_image() {
        let path = fixture_path("full-editor-artwork-roundtrip");
        write_fixture(&path, Version::Id3v24);
        let payload_hash = audio_payload_hash(&path).expect("payload before");
        let (mut tag, version) = read_tag_for_write(&path).expect("read fixture");
        tag.add_frame(Picture {
            mime_type: "image/png".to_owned(),
            picture_type: PictureType::Other,
            description: "booklet".to_owned(),
            data: valid_cover_bytes(3),
        });
        tag.write_to_path(&path, version)
            .expect("seed secondary artwork");

        let (mut tag, version) = read_tag_for_write(&path).expect("read seeded fixture");
        let values = read_editable_tag_values(&tag).expect("editable values");
        let preserved = editor_non_target_frames(&tag, &[], true);
        let cover = crate::artwork::canonical_cover_from_picture(&Picture {
            mime_type: "image/png".to_owned(),
            picture_type: PictureType::CoverFront,
            description: String::new(),
            data: valid_cover_bytes(9),
        })
        .expect("replacement cover");
        let fingerprint = canonical_front_cover_fingerprint(&cover);
        tag.remove_picture_by_type(PictureType::CoverFront);
        tag.add_frame(cover.picture);
        tag.write_to_path(&path, version)
            .expect("write replacement cover");

        verify_editor_written_file(
            &path,
            &values,
            &[],
            true,
            Some(&fingerprint),
            &preserved,
            &payload_hash,
        )
        .expect("verify artwork edit");
        let written = Tag::read_from_path(&path).expect("read written artwork");
        assert_eq!(
            written
                .pictures()
                .filter(|picture| picture.picture_type == PictureType::CoverFront)
                .count(),
            1
        );
        assert!(
            written
                .pictures()
                .any(|picture| picture.picture_type == PictureType::Other)
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn library_cover_save_replaces_the_indexed_archive_image_atomically() {
        let root = fixture_path("cover-archive-root").with_extension("dir");
        fs::create_dir_all(&root).expect("create archive fixture folder");
        let target = root.join("album-cover.bmp");
        let before = valid_cover_bytes_with_format(3, image::ImageFormat::Bmp);
        fs::write(&target, &before).expect("write original archive cover");
        let target = fs::canonicalize(&target).expect("canonical archive cover");
        let replacement = valid_cover_bytes(9);
        let cover = crate::artwork::canonical_cover_from_picture(&Picture {
            mime_type: "image/png".to_owned(),
            picture_type: PictureType::CoverFront,
            description: String::new(),
            data: replacement,
        })
        .expect("replacement cover");

        let prepared = PreparedCoverArchiveWrite::prepare(
            CoverArchiveEntry {
                path: target.clone(),
                mime_type: "image/bmp".to_owned(),
            },
            &cover,
        )
        .expect("stage archive cover");
        assert_eq!(fs::read(&target).expect("original archive cover"), before);
        prepared.install().expect("install archive cover");

        let written = fs::read(&target).expect("read replaced archive cover");
        assert_eq!(
            validate_cover_bytes(&written)
                .expect("validate replaced archive cover")
                .1,
            "image/bmp"
        );
        assert_eq!(
            image::load_from_memory(&written)
                .expect("decode replaced archive cover")
                .to_rgba8()
                .get_pixel(0, 0)
                .0[0],
            9
        );
        assert_eq!(
            fs::read_dir(&root)
                .expect("list archive fixture folder")
                .filter_map(Result::ok)
                .count(),
            1
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failed_library_cover_save_can_restore_the_original_archive_image() {
        let root = fixture_path("cover-archive-rollback").with_extension("dir");
        fs::create_dir_all(&root).expect("create archive rollback folder");
        let target = root.join("album-cover.png");
        let before = valid_cover_bytes(3);
        fs::write(&target, &before).expect("write original archive cover");
        let target = fs::canonicalize(&target).expect("canonical archive cover");
        let cover = crate::artwork::canonical_cover_from_picture(&Picture {
            mime_type: "image/png".to_owned(),
            picture_type: PictureType::CoverFront,
            description: String::new(),
            data: valid_cover_bytes(9),
        })
        .expect("replacement cover");
        let prepared = PreparedCoverArchiveWrite::prepare(
            CoverArchiveEntry {
                path: target.clone(),
                mime_type: "image/png".to_owned(),
            },
            &cover,
        )
        .expect("stage archive cover");

        replace_file_atomic(
            &prepared.target,
            &prepared.temporary,
            Some(&prepared.backup),
        )
        .expect("simulate archive replacement");
        assert_ne!(fs::read(&target).expect("new archive cover"), before);
        restore_cover_archive_backup(&prepared).expect("restore original archive cover");
        assert_eq!(fs::read(&target).expect("restored archive cover"), before);
        assert!(!prepared.backup.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn editor_reads_display_artist_and_round_trips_multi_value_credits() {
        let mut tag = Tag::with_version(Version::Id3v24);
        tag.set_text("TPE1", "A.L.I.S.O.N\0Krosia\0VIQ");
        tag.set_text("TPE2", "Various Artists\0Score Collective");
        tag.add_frame(ExtendedText {
            description: DISPLAY_ARTIST_DESCRIPTION.to_owned(),
            value: "A.L.I.S.O.N; Krosia; VIQ".to_owned(),
        });

        let before = read_editable_tag_values(&tag).expect("read multi-value credits");
        assert_eq!(before.artist.as_deref(), Some("A.L.I.S.O.N; Krosia; VIQ"));
        assert_eq!(
            before.album_artist.as_deref(),
            Some("Various Artists; Score Collective")
        );

        let original_artist_credits = tag
            .artists()
            .expect("artist credits")
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let fields = [EditableTagField::Artist, EditableTagField::AlbumArtist];
        let after = merge_editor_patch(
            &before,
            &fields,
            &EditableTagValues {
                artist: Some("New Artist featuring Guest Artist".to_owned()),
                album_artist: Some("New Album Artist; Score Collective".to_owned()),
                ..Default::default()
            },
        )
        .expect("merge artist credits");
        apply_editor_tag_changes(&mut tag, Version::Id3v24, &fields, &after)
            .expect("write artist credits");

        assert_eq!(
            tag.artists()
                .map(|credits| credits.into_iter().map(str::to_owned).collect::<Vec<_>>()),
            Some(original_artist_credits)
        );
        assert_eq!(
            joined_text_frame_values(&tag, "TPE2").as_deref(),
            Some("New Album Artist; Score Collective")
        );
        assert!(tag.extended_texts().any(|text| {
            text.description == DISPLAY_ARTIST_DESCRIPTION
                && text.value == "New Artist featuring Guest Artist"
        }));
        assert_eq!(
            read_editable_tag_values(&tag).expect("read edited credits"),
            after
        );
    }

    #[test]
    fn editor_maps_year_and_release_year_for_v23_and_v24() {
        let desired = EditableTagValues {
            year: Some(1999),
            release_year: Some(2001),
            ..Default::default()
        };
        let fields = [EditableTagField::Year, EditableTagField::ReleaseYear];

        let mut v23 = Tag::with_version(Version::Id3v23);
        apply_editor_tag_changes(&mut v23, Version::Id3v23, &fields, &desired)
            .expect("write v2.3 dates");
        assert_eq!(
            v23.get("TYER").and_then(|frame| frame.content().text()),
            Some("1999")
        );
        assert!(v23.get("TDRC").is_none());
        assert!(v23.get("TDRL").is_none());
        assert!(
            v23.extended_texts().any(|text| {
                text.description == RELEASE_TIME_DESCRIPTION && text.value == "2001"
            })
        );
        assert_eq!(read_editable_tag_values(&v23).expect("read v2.3"), desired);

        let mut v24 = Tag::with_version(Version::Id3v24);
        apply_editor_tag_changes(&mut v24, Version::Id3v24, &fields, &desired)
            .expect("write v2.4 dates");
        assert_eq!(
            v24.get("TDRC").and_then(|frame| frame.content().text()),
            Some("1999")
        );
        assert_eq!(
            v24.get("TDRL").and_then(|frame| frame.content().text()),
            Some("2001")
        );
        assert!(v24.get("TYER").is_none());
        assert_eq!(read_editable_tag_values(&v24).expect("read v2.4"), desired);
    }

    #[test]
    fn musicbee_popm_zero_is_unrated() {
        let mut tag = Tag::with_version(Version::Id3v24);
        tag.add_frame(Popularimeter {
            user: MUSICBEE_POPM_OWNER.to_owned(),
            rating: 0,
            counter: 0,
        });
        assert_eq!(
            read_tag_values(&tag).expect("read unrated POPM").rating,
            None
        );
        assert_eq!(
            read_editable_tag_values(&tag)
                .expect("read editable unrated POPM")
                .rating,
            None
        );
    }

    #[test]
    fn legacy_default_popm_is_read_then_replaced_or_cleared_as_one_rating() {
        let path = fixture_path("legacy-default-rating");
        write_fixture(&path, Version::Id3v23);
        let (mut tag, version) = read_tag_for_write(&path).expect("read fixture");
        tag.add_frame(Popularimeter {
            user: LEGACY_DEFAULT_POPM_OWNER.to_owned(),
            rating: 204,
            counter: 0,
        });
        tag.write_to_path(&path, version)
            .expect("seed legacy rating");

        let (mut tag, version) = read_tag_for_write(&path).expect("read legacy rating");
        let before = read_tag_values(&tag).expect("read legacy Default rating");
        assert_eq!(before.rating, Some(4.0));

        let rated = TagValues {
            rating: Some(4.5),
            ..before.clone()
        };
        apply_tag_changes(&mut tag, Version::Id3v23, &before, &rated)
            .expect("replace legacy rating");
        tag.write_to_path(&path, version)
            .expect("write replacement");
        let (mut tag, version) = read_tag_for_write(&path).expect("read replacement");
        let popm = tag
            .frames()
            .filter_map(|frame| frame.content().popularimeter())
            .map(|value| (value.user.as_str(), value.rating, value.counter))
            .collect::<Vec<_>>();
        assert!(popm.contains(&(MUSICBEE_POPM_OWNER, 242, 0)));
        assert!(popm.contains(&("other-player", 77, 42)));
        assert!(
            !popm
                .iter()
                .any(|value| value.0 == LEGACY_DEFAULT_POPM_OWNER)
        );

        let cleared = TagValues {
            rating: None,
            ..rated.clone()
        };
        apply_tag_changes(&mut tag, version, &rated, &cleared).expect("clear compatible ratings");
        tag.write_to_path(&path, version)
            .expect("write cleared rating");
        let (tag, _) = read_tag_for_write(&path).expect("read cleared rating");
        assert_eq!(
            read_tag_values(&tag).expect("read cleared rating").rating,
            None
        );
        assert!(
            tag.frames()
                .filter_map(|frame| frame.content().popularimeter())
                .all(|value| !is_musicbee_rating_owner(&value.user))
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn explicit_musicbee_popm_takes_precedence_over_legacy_default() {
        let mut tag = Tag::with_version(Version::Id3v24);
        tag.add_frame(Popularimeter {
            user: LEGACY_DEFAULT_POPM_OWNER.to_owned(),
            rating: 255,
            counter: 0,
        });
        tag.add_frame(Popularimeter {
            user: MUSICBEE_POPM_OWNER.to_owned(),
            rating: 128,
            counter: 0,
        });

        assert_eq!(
            read_tag_values(&tag).expect("read preferred rating").rating,
            Some(3.0)
        );
    }

    #[test]
    fn musicbee_extended_text_descriptions_are_case_insensitive_and_unique() {
        let mut tag = Tag::with_version(Version::Id3v23);
        tag.add_frame(ExtendedText {
            description: "love rating".to_owned(),
            value: "L".to_owned(),
        });
        tag.add_frame(ExtendedText {
            description: "tdrl".to_owned(),
            value: "2004-10-08".to_owned(),
        });

        let before = read_tag_values(&tag).expect("read case-insensitive MusicBee fields");
        assert_eq!(before.love_state, LoveState::Loved);
        assert_eq!(before.release_year, Some(2004));

        let after = TagValues {
            rating: None,
            love_state: LoveState::Neutral,
            release_year: Some(2010),
        };
        apply_tag_changes(&mut tag, Version::Id3v23, &before, &after)
            .expect("rewrite case-insensitive MusicBee fields");
        assert!(!tag.extended_texts().any(|text| {
            text.description
                .eq_ignore_ascii_case(LOVE_RATING_DESCRIPTION)
        }));
        let release_values = tag
            .extended_texts()
            .filter(|text| {
                text.description
                    .eq_ignore_ascii_case(RELEASE_TIME_DESCRIPTION)
            })
            .collect::<Vec<_>>();
        assert_eq!(release_values.len(), 1);
        assert_eq!(release_values[0].description, RELEASE_TIME_DESCRIPTION);
        assert_eq!(release_values[0].value, "2010");

        tag.add_frame(ExtendedText {
            description: "Love Rating".to_owned(),
            value: "B".to_owned(),
        });
        tag.add_frame(ExtendedText {
            description: "LOVE RATING".to_owned(),
            value: "L".to_owned(),
        });
        assert!(
            read_tag_values(&tag)
                .expect_err("case-insensitive duplicate Love fields")
                .contains("duplicate MusicBee Love")
        );
    }

    #[test]
    fn musicbee_extended_text_values_ignore_trailing_null_terminators() {
        let mut tag = Tag::with_version(Version::Id3v24);
        tag.add_frame(ExtendedText {
            description: LOVE_RATING_DESCRIPTION.to_owned(),
            value: "L\0".to_owned(),
        });
        tag.add_frame(ExtendedText {
            description: RELEASE_TIME_DESCRIPTION.to_owned(),
            value: "1988\0".to_owned(),
        });

        let values = read_tag_values(&tag).expect("read null-terminated MusicBee fields");
        assert_eq!(values.love_state, LoveState::Loved);
        assert_eq!(values.release_year, Some(1988));
    }

    #[test]
    fn album_selection_rebind_uses_stable_keys_when_track_ids_change() {
        let expected = TagEditorSnapshot {
            tracks: vec![TagEditorTrackState {
                track_id: "old-track-id".to_owned(),
                track_key: r"d:\music\fixture\01.mp3".to_owned(),
                revision: "revision".to_owned(),
                values: EditableTagValues::default(),
            }],
        };
        let rebound = vec![resolved_track_fixture(
            "new-track-id",
            r"d:\music\fixture\01.mp3",
            "new-album-id",
        )];
        assert!(editor_selection_matches_expected(&rebound, &expected));

        let different = vec![resolved_track_fixture(
            "new-track-id",
            r"d:\music\fixture\02.mp3",
            "new-album-id",
        )];
        assert!(!editor_selection_matches_expected(&different, &expected));

        let expanded_album = vec![
            resolved_track_fixture("new-track-id", r"d:\music\fixture\01.mp3", "new-album-id"),
            resolved_track_fixture("added-track-id", r"d:\music\fixture\02.mp3", "new-album-id"),
        ];
        assert!(!editor_selection_matches_expected(
            &expanded_album,
            &expected
        ));
    }

    #[test]
    fn full_editor_undo_projects_every_restored_field_and_marks_catalog_pending() {
        let resolved = resolved_track_fixture("track-id", r"d:\music\fixture\01.mp3", "album-id");
        let mut snapshot = TrackTagSnapshot {
            track: resolved.summary,
            tag_state: TrackTagState {
                values: TagValues {
                    rating: Some(4.5),
                    love_state: LoveState::Neutral,
                    release_year: Some(2026),
                },
                sync_state: None,
                can_undo: false,
            },
            catalog_sync: None,
        };
        let restored = EditableTagValues {
            album_artist: Some("Restored Album Artist".to_owned()),
            artist: Some("Restored Display Artist".to_owned()),
            album: Some("Restored Album".to_owned()),
            title: Some("Restored Title".to_owned()),
            genre: Some("Soundtrack".to_owned()),
            publisher: Some("Restored Label".to_owned()),
            rating: Some(4.5),
            year: Some(1999),
            release_year: Some(2026),
            track_number: Some(3),
            track_total: Some(12),
            disc_number: Some(1),
            disc_total: Some(2),
        };

        apply_full_editor_undo_projection(&mut snapshot, &restored);

        assert_eq!(snapshot.track.title, "Restored Title");
        assert_eq!(snapshot.track.artist, "Restored Album Artist");
        assert_eq!(
            snapshot.track.display_artist.as_deref(),
            Some("Restored Display Artist")
        );
        assert_eq!(snapshot.track.album, "Restored Album");
        assert_eq!(snapshot.track.original_year, Some(1999));
        assert_eq!(snapshot.track.track_number, Some(3));
        assert_eq!(snapshot.track.track_total, Some(12));
        assert_eq!(
            snapshot.track.tag_sync_state,
            Some(TagSyncState::PendingImport)
        );
        assert_eq!(
            snapshot.tag_state.sync_state,
            Some(TagSyncState::PendingImport)
        );
    }

    #[test]
    fn stale_editor_revision_fails_before_file_mutation() {
        let path = fixture_path("stale-editor-revision");
        write_fixture(&path, Version::Id3v24);
        let bytes_before = fs::read(&path).expect("fixture bytes before");
        let fingerprint = FileFingerprint::read(&path).expect("fixture fingerprint");
        let (tag, _) = read_tag_for_write(&path).expect("read fixture");
        let values = read_editable_tag_values(&tag).expect("read editable values");
        let expected = TagEditorTrackState {
            track_id: "1".to_owned(),
            track_key: "fixture".to_owned(),
            revision: "stale-revision".to_owned(),
            values,
        };

        assert!(verify_editor_expected(&expected, &fingerprint, &expected.values).is_err());
        assert_eq!(fs::read(&path).expect("fixture bytes after"), bytes_before);
        let _ = fs::remove_file(path);
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
        assert!(!backup.exists());
        assert!(
            store
                .latest_undo_operation("fixture-track-key")
                .expect("completed operation lookup")
                .is_none()
        );
        let _ = fs::remove_file(target);
        let _ = fs::remove_file(&state_path);
        let _ = fs::remove_file(PathBuf::from(format!("{}-wal", state_path.display())));
        let _ = fs::remove_file(PathBuf::from(format!("{}-shm", state_path.display())));
    }

    #[test]
    fn undo_recovery_recognizes_a_completed_restore_without_its_current_backup() {
        let target = fixture_path("undo-finished-missing-current-target");
        write_fixture(&target, Version::Id3v23);
        let state_path = fixture_path("undo-finished-missing-current-state.sqlite3");
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
                "undo-finished-missing-current-key",
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
        replace_file_atomic(&target, &working, Some(&backup)).expect("install edit");
        store
            .mark_operation(operation_id, "replaced", None)
            .expect("checkpoint replacement");
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
                "undo-finished-missing-current-key",
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
        fs::remove_file(&undo_current).expect("simulate missing current backup after replacement");

        TagService::new(store.clone()).expect("recover completed undo");

        assert_eq!(
            read_tag_values_from_path(&target).expect("read recovered target"),
            before
        );
        assert!(store.interrupted_operations().expect("journal").is_empty());
        assert!(
            store
                .latest_undo_operation("undo-finished-missing-current-key")
                .expect("undo lookup")
                .is_none()
        );

        let _ = fs::remove_file(target);
        let _ = fs::remove_file(backup);
        let _ = fs::remove_file(undo_replacement);
        remove_state_fixture(&state_path);
    }

    #[test]
    fn missing_post_replace_backup_is_terminal_without_blocking_startup() {
        let target = fixture_path("missing-post-replace-backup-target");
        write_fixture(&target, Version::Id3v23);
        let state_path = fixture_path("missing-post-replace-backup-state.sqlite3");
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
                "missing-post-replace-backup-key",
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
        replace_file_atomic(&target, &working, Some(&backup)).expect("install edit");
        store
            .mark_operation(operation_id, "replaced", None)
            .expect("checkpoint replacement");
        fs::remove_file(&backup).expect("simulate missing rollback copy");

        TagService::new(store.clone()).expect("start despite missing rollback copy");

        assert_eq!(
            read_tag_values_from_path(&target).expect("read retained target"),
            after
        );
        assert!(store.interrupted_operations().expect("journal").is_empty());

        let _ = fs::remove_file(target);
        remove_state_fixture(&state_path);
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
        assert!(!backup.exists());
        assert!(store.interrupted_operations().expect("journal").is_empty());
        assert!(
            store
                .latest_undo_operation("partial-replace-track-key")
                .expect("completed operation lookup")
                .is_none()
        );

        let _ = fs::remove_file(target);
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
