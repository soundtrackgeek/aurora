//! Renames a library album folder and its MP3s after its tags, in place: Music Library moves
//! the files and rewrites its catalog paths, and Aurora then points its own path-keyed state
//! (queue, history, tag overlays, waveform cache) at the new files, so nothing is lost.

use crate::{
    album_naming::{album_folder_name, track_file_stem, track_number_width, track_position},
    artwork::CanonicalCover,
    catalog::{self, ResolvedTrack, normalize_track_key},
    connections, file_observations,
    history::HistoryStore,
    inbox::{self, InboxAlbum, InboxTagApplyRequest},
    library_bridge::{self, RelocateAlbumFile, RelocateAlbumPayload},
    library_sync::{CatalogSync, LibrarySyncCoordinator},
    state_store::StateStore,
    tag_model::EditableTagValues,
    tagging::{read_editable_tag_values, read_tag_for_write},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, path::Path, sync::Mutex};
use tauri::{AppHandle, Manager};

/// One track's old and new place, in the catalog's spelling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TrackRelocation {
    pub(crate) old_key: String,
    pub(crate) new_key: String,
    pub(crate) new_directory: String,
    pub(crate) new_filename: String,
}

impl TrackRelocation {
    pub(crate) fn from_paths(from_path: &str, to_path: &str) -> Result<Self, String> {
        let (old_directory, old_filename) = split_path(from_path)?;
        let (new_directory, new_filename) = split_path(to_path)?;
        Ok(Self {
            old_key: normalize_track_key(&old_directory, &old_filename),
            new_key: normalize_track_key(&new_directory, &new_filename),
            new_directory,
            new_filename,
        })
    }

    /// The new file as this device reaches it, like a resolved track's audio path.
    pub(crate) fn new_audio_path(&self) -> std::path::PathBuf {
        crate::device_mode::resolve_device_path(std::path::Path::new(&self.new_directory))
            .join(&self.new_filename)
    }

    pub(crate) fn apply(&self, track: &mut catalog::TrackSummary) {
        track.track_key.clone_from(&self.new_key);
        track.directory.clone_from(&self.new_directory);
        track.filename.clone_from(&self.new_filename);
    }
}

fn split_path(path: &str) -> Result<(String, String), String> {
    let index = path
        .rfind(['\\', '/'])
        .ok_or_else(|| format!("Music Library returned an invalid track path: {path}"))?;
    let (directory, filename) = (&path[..index], &path[index + 1..]);
    if directory.is_empty() || filename.is_empty() {
        return Err(format!(
            "Music Library returned an invalid track path: {path}"
        ));
    }
    Ok((directory.to_owned(), filename.to_owned()))
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LibraryAlbumRenameResult {
    pub(crate) album_id: String,
    pub(crate) source_path: String,
    pub(crate) destination_path: String,
    pub(crate) folder_renamed: bool,
    pub(crate) renamed_files: usize,
    pub(crate) warnings: Vec<String>,
}

/// The names one album should have: its folder and each track's new file name.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct AlbumRenamePlan {
    pub(crate) folder_name: String,
    pub(crate) files: Vec<(String, String)>,
}

pub(crate) struct NamedTrack<'a> {
    pub(crate) path: String,
    pub(crate) directory: &'a str,
    pub(crate) filename: &'a str,
    pub(crate) tags: EditableTagValues,
}

/// Builds the Inbox names from the tags in the files. The folder name needs one Album Artist,
/// Album and Year shared by every track, and every track needs a number, artist and title.
pub(crate) fn plan_album_names(tracks: &[NamedTrack<'_>]) -> Result<AlbumRenamePlan, String> {
    if tracks.is_empty() {
        return Err("This album has no MP3 files to rename.".to_owned());
    }
    let album_artist = shared_text(tracks.iter().map(|track| {
        track
            .tags
            .album_artist
            .as_deref()
            .or(track.tags.artist.as_deref())
    }))
    .ok_or("The tracks have different Album Artist tags. Make them match before renaming.")?;
    let album = shared_text(tracks.iter().map(|track| track.tags.album.as_deref()))
        .ok_or("The tracks have different Album tags. Make them match before renaming.")?;
    let years = tracks
        .iter()
        .map(|track| track.tags.year)
        .collect::<HashSet<_>>();
    if years.len() > 1 {
        return Err(
            "The tracks have different Year tags. Make them match before renaming.".to_owned(),
        );
    }
    let folder_name = album_folder_name(
        album_artist.as_deref(),
        album.as_deref(),
        tracks[0].tags.year,
    )?;
    let width = track_number_width(
        tracks
            .iter()
            .map(|track| track.tags.track_number.or(track.tags.track_total)),
    );
    let mut destinations = HashSet::new();
    let mut files = Vec::with_capacity(tracks.len());
    for track in tracks {
        let number = track.tags.track_number.ok_or_else(|| {
            format!(
                "Add a Track Number tag to {} before renaming.",
                track.filename
            )
        })?;
        let position = track_position(track.tags.disc_number, number, width);
        let stem = track_file_stem(
            &position,
            track.tags.artist.as_deref(),
            track.tags.title.as_deref(),
        )?;
        let filename = format!("{stem}.mp3");
        if !destinations.insert(normalize_track_key(track.directory, &filename)) {
            return Err(format!("More than one track would be named {filename}."));
        }
        files.push((track.path.clone(), filename));
    }
    Ok(AlbumRenamePlan { folder_name, files })
}

/// `Some(None)` when every value is empty, `None` when tracks disagree.
fn shared_text<'a>(values: impl Iterator<Item = Option<&'a str>>) -> Option<Option<String>> {
    let values = values
        .map(|value| value.map(str::trim).filter(|value| !value.is_empty()))
        .collect::<HashSet<_>>();
    match values.len() {
        0 => Some(None),
        1 => Some(values.into_iter().next().flatten().map(str::to_owned)),
        _ => None,
    }
}

fn catalog_path(directory: &str, filename: &str) -> String {
    let directory = directory.trim_end_matches(['\\', '/']);
    format!("{directory}\\{filename}")
}

fn album_directories(tracks: &[ResolvedTrack]) -> Vec<String> {
    let mut seen = HashSet::new();
    tracks
        .iter()
        .filter(|track| seen.insert(track.summary.directory.to_lowercase()))
        .map(|track| track.summary.directory.clone())
        .collect()
}

/// Every tag edit for the album must reach Music Library first: it renames only an album whose
/// catalog matches its files. Aurora tries the waiting folders now instead of waiting for the
/// background retry.
fn sync_album_first(app: &AppHandle, directories: &[String]) -> Result<(), String> {
    let store = app.state::<StateStore>();
    let coordinator = app.state::<LibrarySyncCoordinator>();
    let pending = store.pending_library_folder_sync_for_paths(directories)?;
    let mut detail = None;
    for folder in &pending {
        let report = coordinator.retry_folder_now(app, &folder.directory);
        if !report.completed(&folder.directory) {
            detail = report.catalog_sync.message.or(detail);
        }
    }
    if !store
        .pending_library_folder_sync_for_paths(directories)?
        .is_empty()
    {
        let reason = detail
            .map(|message| format!(" {message}"))
            .unwrap_or_default();
        return Err(format!(
            "Music Library has not taken this album's latest tag edits yet, so Aurora left the folder name as it is.{reason} Rename the folder again once the edits sync."
        ));
    }
    Ok(())
}

/// Renames one library album after its tags. The caller holds the tag-edit gate, so no tag
/// write can land between the sync check and the rename.
pub(crate) fn rename_album_locked(
    app: &AppHandle,
    album_id: &str,
) -> Result<LibraryAlbumRenameResult, String> {
    connections::require_music_writes()?;
    let store = app.state::<StateStore>();
    let tracks = catalog::resolve_album_tracks(album_id, &store)?;
    let directories = album_directories(&tracks);
    sync_album_first(app, &directories)?;
    // Re-read after the sync: it can update the catalog rows.
    let tracks = catalog::resolve_album_tracks(album_id, &store)?;
    let mut named = Vec::with_capacity(tracks.len());
    for track in &tracks {
        let (tag, _) = read_tag_for_write(&track.audio_path)?;
        named.push(NamedTrack {
            path: catalog_path(&track.summary.directory, &track.summary.filename),
            directory: &track.summary.directory,
            filename: &track.summary.filename,
            tags: read_editable_tag_values(&tag)?,
        });
    }
    let plan = plan_album_names(&named)?;
    let unchanged = plan
        .files
        .iter()
        .zip(&tracks)
        .all(|((_, filename), track)| filename == &track.summary.filename)
        && album_folder_matches(&tracks[0].summary.directory, &plan.folder_name);
    if unchanged {
        return Ok(LibraryAlbumRenameResult {
            album_id: album_id.to_owned(),
            source_path: tracks[0].summary.directory.clone(),
            destination_path: tracks[0].summary.directory.clone(),
            folder_renamed: false,
            renamed_files: 0,
            warnings: Vec::new(),
        });
    }
    let payload = RelocateAlbumPayload {
        album_id: album_id.to_owned(),
        folder_name: plan.folder_name,
        files: plan
            .files
            .into_iter()
            .map(|(from_path, to_filename)| RelocateAlbumFile {
                from_path,
                to_filename,
            })
            .collect(),
    };
    let response = library_bridge::relocate_album(app, &payload)?;
    let moves = response
        .files
        .iter()
        .map(|file| TrackRelocation::from_paths(&file.from_path, &file.to_path))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|item| item.old_key != item.new_key)
        .collect::<Vec<_>>();
    let mut warnings = response.warnings;
    warnings.extend(follow_relocation(
        app,
        &response.source_path,
        &response.destination_path,
        &moves,
    ));
    Ok(LibraryAlbumRenameResult {
        album_id: album_id.to_owned(),
        source_path: response.source_path,
        destination_path: response.destination_path,
        folder_renamed: response.folder_renamed,
        renamed_files: response.renamed_files,
        warnings,
    })
}

/// The tags already name this folder. Disc subfolders are named by their parent.
/// Catalog paths are Windows paths, so this splits them by hand on every platform.
fn album_folder_matches(track_directory: &str, folder_name: &str) -> bool {
    track_directory
        .split(['\\', '/'])
        .any(|component| component == folder_name)
}

/// Moves Aurora's own records to the new paths. Each store is updated on its own; a failure
/// becomes a warning because the files and Music Library already moved.
fn follow_relocation(
    app: &AppHandle,
    source_folder: &str,
    destination_folder: &str,
    moves: &[TrackRelocation],
) -> Vec<String> {
    let mut warnings = Vec::new();
    let store = app.state::<StateStore>();
    if let Err(error) = store.relocate_library_tracks(source_folder, destination_folder, moves) {
        warnings.push(format!(
            "Aurora could not move its tag edit records: {error}"
        ));
    }
    if let Err(error) = app.state::<HistoryStore>().relocate_tracks(moves) {
        warnings.push(format!(
            "Aurora could not move its listening history: {error}"
        ));
    }
    if let Err(error) = file_observations::forget_relocated(&store, moves) {
        warnings.push(format!("Aurora could not clear its file checks: {error}"));
    }
    if let Some(waveforms) = app.try_state::<Mutex<crate::waveform::WaveformStore>>()
        && let Ok(waveforms) = waveforms.lock()
        && let Err(error) = waveforms.relocate_tracks(moves)
    {
        warnings.push(format!("Aurora could not move its waveform cache: {error}"));
    }
    if let Some(playback) = app.try_state::<Mutex<crate::playback::PlaybackRuntime>>() {
        match playback.lock() {
            Ok(mut runtime) => runtime.relocate_tracks(moves),
            Err(_) => warnings.push("Aurora's player stopped unexpectedly.".to_owned()),
        }
    }
    warnings
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LibraryAutoTagRequest {
    album_id: String,
    #[serde(default)]
    rename_folder: bool,
    pub(crate) tags: InboxTagApplyRequest,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LibraryAutoTagResult {
    changed_tracks: usize,
    removed_tracks: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    recovery_path: Option<String>,
    pub(crate) catalog_sync: CatalogSync,
    #[serde(skip_serializing_if = "Option::is_none")]
    rename: Option<LibraryAlbumRenameResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rename_error: Option<String>,
}

fn same_file(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

const SINGLE_FOLDER_MESSAGE: &str = "Auto-tag works on library albums whose tracks are all in one folder. Move the disc folders' tracks into the album folder first.";

/// Reads a library album's files for the auto-tagger, in the same shape as an Inbox album.
pub(crate) fn auto_tag_album(app: &AppHandle, album_id: &str) -> Result<InboxAlbum, String> {
    let store = app.state::<StateStore>();
    let tracks = catalog::resolve_album_tracks(album_id.trim(), &store)?;
    if album_directories(&tracks).len() != 1 {
        return Err(SINGLE_FOLDER_MESSAGE.to_owned());
    }
    let folder = tracks[0]
        .audio_path
        .parent()
        .ok_or("The album folder could not be found.")?;
    inbox::scan_library_album(folder, |path| {
        tracks
            .iter()
            .any(|track| same_file(path, &track.audio_path))
    })
}

/// Applies a release the user reviewed in the auto-tagger to a library album: it writes the
/// tags, deletes the extra tracks (copies stay in the recovery folder), syncs the album to
/// Music Library as one reviewed update and, when asked, renames the folder and tracks.
/// The caller holds the tag-edit gate.
pub(crate) fn apply_auto_tags_locked(
    app: &AppHandle,
    request: LibraryAutoTagRequest,
    cover: Option<&CanonicalCover>,
    recovery_root: &Path,
) -> Result<LibraryAutoTagResult, String> {
    connections::require_music_writes()?;
    library_bridge::require_reviewed_album_sync(app)?;
    let store = app.state::<StateStore>();
    let album_id = request.album_id.trim().to_owned();
    let tracks = catalog::resolve_album_tracks(&album_id, &store)?;
    let directories = album_directories(&tracks);
    let [directory] = directories.as_slice() else {
        return Err(SINGLE_FOLDER_MESSAGE.to_owned());
    };
    let album_folder = tracks[0]
        .audio_path
        .parent()
        .ok_or("The album folder could not be found.")?;
    if !same_file(Path::new(request.tags.album_path()), album_folder) {
        return Err("The auto-tag edit is for another folder than this album. Reopen the album and try again.".to_owned());
    }
    let paths = request
        .tags
        .track_paths()
        .chain(
            request
                .tags
                .removed_track_paths()
                .iter()
                .map(String::as_str),
        )
        .collect::<Vec<_>>();
    for path in &paths {
        if !tracks
            .iter()
            .any(|track| same_file(Path::new(path), &track.audio_path))
        {
            return Err("Every auto-tagged track must still belong to the open album. Reopen the album and try again.".to_owned());
        }
    }
    let removed = tracks
        .iter()
        .filter(|track| {
            request
                .tags
                .removed_track_paths()
                .iter()
                .any(|path| same_file(Path::new(path), &track.audio_path))
        })
        .collect::<Vec<_>>();

    // Queue the whole album before any file changes, so a crash still syncs it later, and as a
    // reviewed sync, because renumbered or removed tracks need one.
    let album_files = tracks
        .iter()
        .map(|track| {
            (
                track.summary.directory.clone(),
                track.summary.filename.clone(),
            )
        })
        .collect::<Vec<_>>();
    store.queue_library_file_syncs(&album_files)?;
    store.mark_reviewed_album_sync(directory)?;
    let applied = inbox::apply_tags(
        request.tags.without_folder_rename(),
        cover,
        recovery_root,
        &[],
    )?;
    for track in removed {
        if !track.audio_path.exists() {
            let _ = file_observations::record_deleted(&store, &track.summary);
        }
    }

    let coordinator = app.state::<LibrarySyncCoordinator>();
    let report = coordinator.sync_reviewed_album(app, directory);
    let synced = report.completed(directory);
    let (rename, rename_error) = if !request.rename_folder {
        (None, None)
    } else if !synced {
        (
            None,
            Some("The folder was not renamed because Music Library has not taken the new tags yet. Rename it from the tag editor once they sync.".to_owned()),
        )
    } else {
        match rename_album_locked(app, &album_id) {
            Ok(result) => (Some(result), None),
            Err(error) => (None, Some(error)),
        }
    };
    Ok(LibraryAutoTagResult {
        changed_tracks: applied.changed_tracks(),
        removed_tracks: applied.removed_tracks(),
        recovery_path: applied.recovery_path().map(str::to_owned),
        catalog_sync: report.catalog_sync,
        rename,
        rename_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named<'a>(
        directory: &'a str,
        filename: &'a str,
        disc: Option<u32>,
        number: Option<u32>,
        year: Option<i32>,
    ) -> NamedTrack<'a> {
        NamedTrack {
            path: catalog_path(directory, filename),
            directory,
            filename,
            tags: EditableTagValues {
                album_artist: Some("Artist".to_owned()),
                artist: Some("Artist".to_owned()),
                album: Some("Album".to_owned()),
                title: Some(format!("Song {}", number.unwrap_or_default())),
                year,
                track_number: number,
                disc_number: disc,
                ..EditableTagValues::default()
            },
        }
    }

    #[test]
    fn plans_inbox_names_from_the_file_tags() {
        let tracks = [
            named(
                r"D:\Music\Artist - Album (1978)",
                "a.mp3",
                None,
                Some(1),
                Some(1976),
            ),
            named(
                r"D:\Music\Artist - Album (1978)",
                "b.mp3",
                None,
                Some(11),
                Some(1976),
            ),
        ];
        let plan = plan_album_names(&tracks).unwrap();
        assert_eq!(plan.folder_name, "Artist - Album (1976)");
        assert_eq!(
            plan.files,
            vec![
                (
                    r"D:\Music\Artist - Album (1978)\a.mp3".to_owned(),
                    "01 - Artist - Song 1.mp3".to_owned()
                ),
                (
                    r"D:\Music\Artist - Album (1978)\b.mp3".to_owned(),
                    "11 - Artist - Song 11.mp3".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn refuses_mixed_years_and_missing_numbers() {
        let mixed = [
            named(r"D:\Music\A", "a.mp3", None, Some(1), Some(1976)),
            named(r"D:\Music\A", "b.mp3", None, Some(2), Some(1978)),
        ];
        assert!(
            plan_album_names(&mixed)
                .unwrap_err()
                .contains("different Year")
        );
        let unnumbered = [named(r"D:\Music\A", "a.mp3", None, None, Some(1976))];
        assert!(
            plan_album_names(&unnumbered)
                .unwrap_err()
                .contains("Track Number")
        );
        let duplicate = [
            named(r"D:\Music\A", "a.mp3", Some(1), Some(1), Some(1976)),
            named(r"D:\Music\A", "b.mp3", Some(1), Some(1), Some(1976)),
        ];
        assert!(
            plan_album_names(&duplicate)
                .unwrap_err()
                .contains("More than one")
        );
        let discs = [
            named(r"D:\Music\A\CD1", "a.mp3", Some(1), Some(1), Some(1976)),
            named(r"D:\Music\A\CD2", "b.mp3", Some(2), Some(1), Some(1976)),
        ];
        assert_eq!(
            plan_album_names(&discs).unwrap().files[1].1,
            "2-01 - Artist - Song 1.mp3"
        );
    }

    #[test]
    fn state_store_follows_renamed_tracks_and_clears_reviewed_markers() {
        let root = tempfile::tempdir().unwrap();
        let store = StateStore::new(root.path().join("state.sqlite3")).unwrap();
        let old_folder = r"D:\Music\Artist - Album (1978)";
        let new_folder = r"D:\Music\Artist - Album (1976)";
        let moved = TrackRelocation::from_paths(
            &format!(r"{old_folder}\a.mp3"),
            &format!(r"{new_folder}\01 - Artist - Song.mp3"),
        )
        .unwrap();
        {
            let connection = store.open().unwrap();
            connection
                .execute(
                    "INSERT INTO playback_queue(position, track_id, track_key, directory, filename)
                     VALUES (0, '7', ?1, ?2, 'a.mp3')",
                    rusqlite::params![moved.old_key, old_folder],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO album_additions VALUES ('album', ?1, 1, 1)",
                    [old_folder],
                )
                .unwrap();
        }
        store
            .relocate_library_tracks(old_folder, new_folder, std::slice::from_ref(&moved))
            .unwrap();
        let connection = store.open().unwrap();
        let queued: (String, String, String) = connection
            .query_row(
                "SELECT track_key, directory, filename FROM playback_queue WHERE position = 0",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            queued,
            (
                moved.new_key.clone(),
                new_folder.to_owned(),
                "01 - Artist - Song.mp3".to_owned()
            )
        );
        let added: String = connection
            .query_row("SELECT destination_path FROM album_additions", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(added, new_folder);

        store
            .queue_library_file_syncs(&[(
                new_folder.to_owned(),
                "01 - Artist - Song.mp3".to_owned(),
            )])
            .unwrap();
        store.mark_reviewed_album_sync(new_folder).unwrap();
        assert!(
            store
                .is_reviewed_album_sync(&new_folder.to_uppercase())
                .unwrap()
        );
        let pending = store
            .pending_library_folder_sync_for_paths(&[new_folder.to_owned()])
            .unwrap();
        assert!(store.complete_library_folder_sync(&pending[0]).unwrap());
        assert!(!store.is_reviewed_album_sync(new_folder).unwrap());
    }

    #[test]
    fn relocations_use_aurora_track_keys() {
        let item = TrackRelocation::from_paths(
            r"D:\Music\Artist - Album (1978)\a.mp3",
            r"D:\Music\Artist - Album (1976)\01 - Artist - Song.mp3",
        )
        .unwrap();
        assert_eq!(item.old_key, r"d:\music\artist - album (1978)\a.mp3");
        assert_eq!(
            item.new_key,
            r"d:\music\artist - album (1976)\01 - artist - song.mp3"
        );
        assert_eq!(item.new_directory, r"D:\Music\Artist - Album (1976)");
        assert_eq!(item.new_filename, "01 - Artist - Song.mp3");
        assert!(TrackRelocation::from_paths("a.mp3", r"D:\b.mp3").is_err());
        assert!(album_folder_matches(
            r"D:\Music\Artist - Album (1976)\CD1",
            "Artist - Album (1976)"
        ));
        assert!(!album_folder_matches(
            r"D:\Music\Artist - Album (1978)",
            "Artist - Album (1976)"
        ));
    }
}
