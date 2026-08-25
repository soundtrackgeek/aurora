use crate::{
    state_sync,
    tag_model::{EditableTagField, EditableTagValues},
    tagging::{
        apply_editor_tag_changes, audio_payload_hash, merge_editor_patch, read_editable_tag_values,
        read_tag_for_write,
    },
};
use keyring::Entry;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const SETTINGS_VERSION: u8 = 1;
const MAX_MONITORED_FOLDERS: usize = 10;
const MAX_SCANNED_DIRECTORIES: usize = 50_000;
const DISCOGS_SERVICE: &str = "Aurora";
const DISCOGS_TOKEN_USER: &str = "Discogs personal access token";
const DISCOGS_KEY_USER: &str = "Discogs consumer key";
const DISCOGS_SECRET_USER: &str = "Discogs consumer secret";
const USER_AGENT: &str = concat!(
    "Aurora/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/soundtrackgeek/aurora)"
);

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InboxSettings {
    monitored_folders: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InboxSettingsFile {
    version: u8,
    settings: InboxSettings,
}

pub(crate) struct InboxRuntime {
    path: PathBuf,
    settings: InboxSettings,
    warning: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InboxSettingsStatus {
    monitored_folders: Vec<String>,
    discogs_configured: bool,
    discogs_auth_mode: Option<DiscogsAuthMode>,
    discogs_incomplete_consumer_key: bool,
    warning: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DiscogsAuthMode {
    Token,
    Consumer,
}

enum DiscogsAuth {
    Token(String),
    Consumer { key: String, secret: String },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(
    tag = "mode",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum DiscogsCredentialsRequest {
    Token {
        token: String,
    },
    Consumer {
        consumer_key: String,
        consumer_secret: String,
    },
    Clear,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InboxAlbum {
    id: String,
    path: String,
    folder_name: String,
    artist: Option<String>,
    album: Option<String>,
    genre: Option<String>,
    publisher: Option<String>,
    year: Option<i32>,
    track_count: usize,
    artwork_present: bool,
    modified_at_ms: u64,
    readiness: InboxReadiness,
    tracks: Vec<InboxTrack>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InboxReadiness {
    ready: bool,
    issues: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InboxTrack {
    path: String,
    file_name: String,
    title: Option<String>,
    artist: Option<String>,
    track_number: Option<u32>,
    track_total: Option<u32>,
    disc_number: Option<u32>,
    disc_total: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InboxSnapshot {
    settings: InboxSettingsStatus,
    albums: Vec<InboxAlbum>,
    scanned_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum MetadataSource {
    Musicbrainz,
    Discogs,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReleaseCandidate {
    source: MetadataSource,
    id: String,
    score: u32,
    title: String,
    artist: String,
    year: Option<i32>,
    country: Option<String>,
    format: Option<String>,
    publisher: Option<String>,
    track_count: Option<u32>,
    cover_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReleaseSearchRequest {
    artist: String,
    album: String,
    track_count: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReleaseSearchResult {
    candidates: Vec<ReleaseCandidate>,
    discogs_configured: bool,
    warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReleaseCandidateDetail {
    candidate: ReleaseCandidate,
    album_artist: Option<String>,
    album: Option<String>,
    genre: Option<String>,
    publisher: Option<String>,
    year: Option<i32>,
    disc_total: Option<u32>,
    tracks: Vec<ReleaseTrack>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReleaseTrack {
    title: String,
    artist: Option<String>,
    track_number: Option<u32>,
    track_total: Option<u32>,
    disc_number: Option<u32>,
    disc_total: Option<u32>,
    duration_ms: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReleaseDetailRequest {
    source: MetadataSource,
    id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct InboxTrackPatch {
    path: String,
    values: EditableTagValues,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct InboxTagApplyRequest {
    album_path: String,
    fields: Vec<EditableTagField>,
    tracks: Vec<InboxTrackPatch>,
    #[serde(default)]
    rename_after_apply: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InboxTagApplyResult {
    changed_tracks: usize,
    renamed_tracks: usize,
    album_path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct InboxRenameRequest {
    album_path: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InboxRenameResult {
    album_path: String,
    renamed_tracks: usize,
    folder_renamed: bool,
}

struct PreparedWrite {
    target: PathBuf,
    temporary: PathBuf,
    backup: PathBuf,
}

struct PreparedRename {
    source: PathBuf,
    temporary: PathBuf,
    destination: PathBuf,
}

impl InboxRuntime {
    pub(crate) fn load(path: PathBuf) -> Self {
        let mut warning = None;
        let settings = if path.is_file() {
            match fs::read_to_string(&path)
                .map_err(|error| error.to_string())
                .and_then(|json| {
                    serde_json::from_str::<InboxSettingsFile>(&json)
                        .map_err(|error| error.to_string())
                }) {
                Ok(file) if file.version == SETTINGS_VERSION => file.settings,
                Ok(_) => {
                    warning = Some("Aurora found Inbox settings from an unsupported version and used safe defaults.".to_owned());
                    InboxSettings::default()
                }
                Err(error) => {
                    warning = Some(format!(
                        "Aurora could not read Inbox settings and used safe defaults: {error}"
                    ));
                    InboxSettings::default()
                }
            }
        } else {
            InboxSettings::default()
        };
        Self {
            path,
            settings,
            warning,
        }
    }

    pub(crate) fn status(&self) -> InboxSettingsStatus {
        let auth = discogs_auth();
        InboxSettingsStatus {
            monitored_folders: self.settings.monitored_folders.clone(),
            discogs_configured: auth.is_some(),
            discogs_auth_mode: auth.as_ref().map(|value| match value {
                DiscogsAuth::Token(_) => DiscogsAuthMode::Token,
                DiscogsAuth::Consumer { .. } => DiscogsAuthMode::Consumer,
            }),
            discogs_incomplete_consumer_key: auth.is_none() && consumer_key().is_some(),
            warning: self.warning.clone(),
        }
    }

    pub(crate) fn resolve_cover_track(&self, value: &str) -> Result<PathBuf, String> {
        if value.trim().is_empty() || value.chars().count() > 32_768 {
            return Err("The Inbox cover path is invalid.".to_owned());
        }
        let track = fs::canonicalize(value)
            .map_err(|_| "The Inbox cover track is unavailable.".to_owned())?;
        if !track.is_file() || !is_mp3(&track) {
            return Err("The Inbox cover source is not an MP3.".to_owned());
        }
        let allowed = self.settings.monitored_folders.iter().any(|root| {
            fs::canonicalize(root).is_ok_and(|canonical_root| track.starts_with(canonical_root))
        });
        allowed
            .then_some(track)
            .ok_or_else(|| "The Inbox cover source is outside the monitored folders.".to_owned())
    }

    pub(crate) fn add_folder(&mut self, folder: String) -> Result<InboxSettingsStatus, String> {
        if self.settings.monitored_folders.len() >= MAX_MONITORED_FOLDERS {
            return Err("Inbox can monitor at most 10 folders.".to_owned());
        }
        let canonical = canonical_directory(&folder)?;
        let value = path_text(&canonical)?;
        if self
            .settings
            .monitored_folders
            .iter()
            .any(|current| paths_equal(current, &value))
        {
            return Ok(self.status());
        }
        self.settings.monitored_folders.push(value);
        self.persist()?;
        Ok(self.status())
    }

    pub(crate) fn remove_folder(&mut self, folder: &str) -> Result<InboxSettingsStatus, String> {
        self.settings
            .monitored_folders
            .retain(|current| !paths_equal(current, folder));
        self.persist()?;
        Ok(self.status())
    }

    pub(crate) fn scan(&self) -> Result<InboxSnapshot, String> {
        let mut album_directories = HashSet::new();
        let mut visited = 0usize;
        for root in &self.settings.monitored_folders {
            collect_album_directories(Path::new(root), &mut album_directories, &mut visited)?;
        }
        let mut albums = album_directories
            .into_iter()
            .filter_map(|path| match scan_album(&path) {
                Ok(album) => Some(album),
                Err(error) => {
                    eprintln!("Inbox skipped {}: {error}", path.display());
                    None
                }
            })
            .collect::<Vec<_>>();
        albums.sort_by(|left, right| {
            right
                .modified_at_ms
                .cmp(&left.modified_at_ms)
                .then_with(|| left.path.cmp(&right.path))
        });
        Ok(InboxSnapshot {
            settings: self.status(),
            albums,
            scanned_at_ms: now_ms(),
        })
    }

    fn persist(&mut self) -> Result<(), String> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "Aurora's Inbox settings have no parent folder.".to_owned())?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create Aurora's Inbox settings folder: {error}"))?;
        let bytes = serde_json::to_vec_pretty(&InboxSettingsFile {
            version: SETTINGS_VERSION,
            settings: self.settings.clone(),
        })
        .map_err(|error| format!("Could not encode Inbox settings: {error}"))?;
        let temporary = parent.join(format!(
            ".aurora-inbox-settings-{}-{}.tmp",
            std::process::id(),
            now_ms()
        ));
        let mut file = File::options()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|error| format!("Could not save Inbox settings: {error}"))?;
        if let Err(error) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
            let _ = fs::remove_file(&temporary);
            return Err(format!("Could not flush Inbox settings: {error}"));
        }
        drop(file);
        let result = if self.path.is_file() {
            state_sync::replace_file_atomic(&self.path, &temporary)
        } else {
            fs::rename(&temporary, &self.path)
                .map_err(|error| format!("Could not install Inbox settings: {error}"))
        };
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result?;
        self.warning = None;
        Ok(())
    }
}

pub(crate) fn save_discogs_credentials(request: DiscogsCredentialsRequest) -> Result<(), String> {
    match request {
        DiscogsCredentialsRequest::Token { token } => {
            let token = validate_credential(token, "personal access token")?;
            credential_entry(DISCOGS_TOKEN_USER)?
                .set_password(&token)
                .map_err(credential_error)?;
            delete_credential(DISCOGS_KEY_USER)?;
            delete_credential(DISCOGS_SECRET_USER)?;
        }
        DiscogsCredentialsRequest::Consumer {
            consumer_key,
            consumer_secret,
        } => {
            let key = validate_credential(consumer_key, "consumer key")?;
            let secret = validate_credential(consumer_secret, "consumer secret")?;
            credential_entry(DISCOGS_KEY_USER)?
                .set_password(&key)
                .map_err(credential_error)?;
            if let Err(error) = credential_entry(DISCOGS_SECRET_USER)?.set_password(&secret) {
                let _ = delete_credential(DISCOGS_KEY_USER);
                return Err(credential_error(error));
            }
            delete_credential(DISCOGS_TOKEN_USER)?;
        }
        DiscogsCredentialsRequest::Clear => {
            delete_credential(DISCOGS_TOKEN_USER)?;
            delete_credential(DISCOGS_KEY_USER)?;
            delete_credential(DISCOGS_SECRET_USER)?;
        }
    }
    Ok(())
}

pub(crate) fn search_releases(
    request: ReleaseSearchRequest,
) -> Result<ReleaseSearchResult, String> {
    validate_query(&request.artist, "artist")?;
    validate_query(&request.album, "album")?;
    let mut warnings = Vec::new();
    let mut candidates = match search_musicbrainz(&request) {
        Ok(rows) => rows,
        Err(error) => {
            warnings.push(error);
            Vec::new()
        }
    };
    let discogs_configured = discogs_auth().is_some();
    if discogs_configured {
        match search_discogs(&request) {
            Ok(rows) => candidates.extend(rows),
            Err(error) => warnings.push(error),
        }
    }
    candidates.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.year.cmp(&right.year))
    });
    candidates.truncate(20);
    Ok(ReleaseSearchResult {
        candidates,
        discogs_configured,
        warnings,
    })
}

pub(crate) fn release_detail(
    request: ReleaseDetailRequest,
) -> Result<ReleaseCandidateDetail, String> {
    validate_identifier(&request.id)?;
    match request.source {
        MetadataSource::Musicbrainz => musicbrainz_detail(&request.id),
        MetadataSource::Discogs => discogs_detail(&request.id),
    }
}

pub(crate) fn apply_tags(request: InboxTagApplyRequest) -> Result<InboxTagApplyResult, String> {
    if request.fields.is_empty() || request.tracks.is_empty() {
        return Err("Choose at least one field and one track before applying tags.".to_owned());
    }
    let unique_fields = request.fields.iter().copied().collect::<HashSet<_>>();
    if unique_fields.len() != request.fields.len() {
        return Err("The Inbox tag edit contains duplicate fields.".to_owned());
    }
    let album = canonical_directory(&request.album_path)?;
    let mut prepared = Vec::new();
    let sequence = now_ms();
    for (index, patch) in request.tracks.iter().enumerate() {
        let target = fs::canonicalize(&patch.path)
            .map_err(|error| format!("Could not open an Inbox track: {error}"))?;
        if target.parent() != Some(album.as_path()) || !is_mp3(&target) {
            cleanup_prepared(&prepared);
            return Err("An Inbox track is outside the selected album folder.".to_owned());
        }
        let (mut tag, version) = read_tag_for_write(&target)?;
        let before = read_editable_tag_values(&tag)?;
        let after =
            merge_editor_patch(&before, &request.fields, &patch.values.clone().normalize())?;
        if before == after {
            continue;
        }
        let original_hash = audio_payload_hash(&target)?;
        let temporary = album.join(format!(".aurora-inbox-{sequence}-{index}.tmp.mp3"));
        let backup = album.join(format!(".aurora-inbox-{sequence}-{index}.backup.mp3"));
        fs::copy(&target, &temporary)
            .map_err(|error| format!("Could not prepare an Inbox tag edit: {error}"))?;
        apply_editor_tag_changes(&mut tag, version, &request.fields, &after)?;
        if let Err(error) = tag.write_to_path(&temporary, version) {
            let _ = fs::remove_file(&temporary);
            cleanup_prepared(&prepared);
            return Err(format!("Could not write staged Inbox tags: {error}"));
        }
        let (written, _) = read_tag_for_write(&temporary)?;
        if read_editable_tag_values(&written)? != after
            || audio_payload_hash(&temporary)? != original_hash
        {
            let _ = fs::remove_file(&temporary);
            cleanup_prepared(&prepared);
            return Err(
                "Aurora could not verify a staged Inbox tag edit. No originals were changed."
                    .to_owned(),
            );
        }
        prepared.push(PreparedWrite {
            target,
            temporary,
            backup,
        });
    }
    for item in &prepared {
        if let Err(error) = fs::copy(&item.target, &item.backup) {
            cleanup_prepared(&prepared);
            return Err(format!("Could not create an Inbox safety backup: {error}"));
        }
    }
    let mut installed = 0usize;
    for item in &prepared {
        if let Err(error) = state_sync::replace_file_atomic(&item.target, &item.temporary) {
            for previous in prepared[..installed].iter().rev() {
                let _ = state_sync::replace_file_atomic(&previous.target, &previous.backup);
            }
            cleanup_prepared(&prepared);
            return Err(format!(
                "Aurora could not finish the Inbox tag batch and restored earlier tracks: {error}"
            ));
        }
        installed += 1;
    }
    let rename_result = if request.rename_after_apply {
        match rename_album_path(&album) {
            Ok(result) => Some(result),
            Err(error) => {
                for item in prepared.iter().rev() {
                    let _ = state_sync::replace_file_atomic(&item.target, &item.backup);
                }
                cleanup_prepared(&prepared);
                return Err(format!(
                    "Aurora could not rename the tagged album and restored its original tags: {error}"
                ));
            }
        }
    } else {
        None
    };
    let final_album = rename_result
        .as_ref()
        .map(|result| PathBuf::from(&result.album_path))
        .unwrap_or_else(|| album.clone());
    for item in &prepared {
        if let Some(name) = item.backup.file_name() {
            let _ = fs::remove_file(final_album.join(name));
        }
    }
    Ok(InboxTagApplyResult {
        changed_tracks: installed,
        renamed_tracks: rename_result
            .as_ref()
            .map_or(0, |result| result.renamed_tracks),
        album_path: path_text(&final_album)?,
    })
}

pub(crate) fn rename_album(request: InboxRenameRequest) -> Result<InboxRenameResult, String> {
    let album = canonical_directory(&request.album_path)?;
    rename_album_path(&album)
}

fn rename_album_path(album: &Path) -> Result<InboxRenameResult, String> {
    let scanned = scan_album(album)?;
    let album_artist = required_component(scanned.artist.as_deref(), "Album Artist")?;
    let album_title = required_component(scanned.album.as_deref(), "Album")?;
    let year = scanned
        .year
        .filter(|value| *value > 0)
        .ok_or_else(|| "Add a Year tag before renaming this album.".to_owned())?;
    let parent = album
        .parent()
        .ok_or_else(|| "The Inbox album has no parent folder.".to_owned())?;
    let folder_name = sanitize_component(&format!("{album_artist} - {album_title} ({year})"))?;
    let destination_folder = parent.join(folder_name);
    let folder_renamed = !path_eq(album, &destination_folder);
    if folder_renamed && destination_folder.exists() {
        return Err(format!(
            "The destination album folder already exists: {}",
            destination_folder.display()
        ));
    }

    let sequence = now_ms();
    let track_width = scanned
        .tracks
        .iter()
        .filter_map(|track| track.track_number.or(track.track_total))
        .max()
        .map_or(2, decimal_width)
        .max(2);
    let mut destinations = HashSet::new();
    let sources = scanned
        .tracks
        .iter()
        .map(|track| path_key(Path::new(&track.path)))
        .collect::<HashSet<_>>();
    let mut prepared = Vec::new();
    for (index, track) in scanned.tracks.iter().enumerate() {
        let track_number = track.track_number.ok_or_else(|| {
            format!(
                "Add a Track Number tag to {} before renaming.",
                track.file_name
            )
        })?;
        let artist = required_component(track.artist.as_deref(), "Artist")?;
        let title = required_component(track.title.as_deref(), "Title")?;
        let position = match track.disc_number {
            Some(disc) => format!("{disc}-{track_number:0track_width$}"),
            None => format!("{track_number:0track_width$}"),
        };
        let file_name = sanitize_component(&format!("{position} - {artist} - {title}"))?;
        let source = PathBuf::from(&track.path);
        let destination = album.join(format!("{file_name}.mp3"));
        if !destinations.insert(path_key(&destination)) {
            return Err(format!(
                "More than one track would be named {file_name}.mp3."
            ));
        }
        if destination.exists() && !sources.contains(&path_key(&destination)) {
            return Err(format!(
                "The destination track already exists: {}",
                destination.display()
            ));
        }
        if path_eq(&source, &destination) {
            continue;
        }
        prepared.push(PreparedRename {
            source,
            temporary: album.join(format!(".aurora-rename-{sequence}-{index}.tmp")),
            destination,
        });
    }

    stage_renames(&prepared)?;
    if folder_renamed && let Err(error) = fs::rename(album, &destination_folder) {
        rollback_renames(&prepared, prepared.len());
        return Err(format!("Could not rename the album folder: {error}"));
    }
    Ok(InboxRenameResult {
        album_path: path_text(if folder_renamed {
            &destination_folder
        } else {
            album
        })?,
        renamed_tracks: prepared.len(),
        folder_renamed,
    })
}

fn stage_renames(items: &[PreparedRename]) -> Result<(), String> {
    for (index, item) in items.iter().enumerate() {
        if let Err(error) = fs::rename(&item.source, &item.temporary) {
            for previous in items[..index].iter().rev() {
                let _ = fs::rename(&previous.temporary, &previous.source);
            }
            return Err(format!("Could not prepare track renames: {error}"));
        }
    }
    for (index, item) in items.iter().enumerate() {
        if let Err(error) = fs::rename(&item.temporary, &item.destination) {
            rollback_renames(items, index);
            return Err(format!("Could not finish track renames: {error}"));
        }
    }
    Ok(())
}

fn rollback_renames(items: &[PreparedRename], placed: usize) {
    for item in items[..placed].iter().rev() {
        let _ = fs::rename(&item.destination, &item.temporary);
    }
    for item in items.iter().rev() {
        if item.temporary.exists() {
            let _ = fs::rename(&item.temporary, &item.source);
        }
    }
}

fn cleanup_prepared(items: &[PreparedWrite]) {
    for item in items {
        let _ = fs::remove_file(&item.temporary);
        let _ = fs::remove_file(&item.backup);
    }
}

fn collect_album_directories(
    root: &Path,
    albums: &mut HashSet<PathBuf>,
    visited: &mut usize,
) -> Result<(), String> {
    if !root.is_dir() {
        return Ok(());
    }
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        *visited += 1;
        if *visited > MAX_SCANNED_DIRECTORIES {
            return Err("Inbox stopped scanning because the monitored folders contain too many directories.".to_owned());
        }
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        let mut has_mp3 = false;
        let mut children = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                children.push(path);
            } else if kind.is_file() && is_mp3(&path) {
                has_mp3 = true;
            }
        }
        if has_mp3 {
            albums.insert(directory);
        }
        pending.extend(children);
    }
    Ok(())
}

fn scan_album(directory: &Path) -> Result<InboxAlbum, String> {
    let canonical = fs::canonicalize(directory).map_err(|error| error.to_string())?;
    let mut tracks = fs::read_dir(&canonical)
        .map_err(|error| error.to_string())?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_mp3(path))
        .map(|path| scan_track(&path))
        .collect::<Result<Vec<_>, _>>()?;
    tracks.sort_by(|left, right| {
        left.disc_number
            .cmp(&right.disc_number)
            .then_with(|| left.track_number.cmp(&right.track_number))
            .then_with(|| left.file_name.cmp(&right.file_name))
    });
    let mut tags = Vec::new();
    let mut artwork_present = false;
    let mut modified_at_ms = 0;
    for (index, track) in tracks.iter().enumerate() {
        let path = Path::new(&track.path);
        let (tag, _) = read_tag_for_write(path)?;
        if index == 0 {
            artwork_present = tag.pictures().next().is_some();
        }
        modified_at_ms = modified_at_ms.max(
            fs::metadata(path)
                .ok()
                .and_then(|value| value.modified().ok())
                .and_then(system_time_ms)
                .unwrap_or(0),
        );
        tags.push(read_editable_tag_values(&tag)?);
    }
    let artist = common_text(
        tags.iter()
            .map(|tag| tag.album_artist.as_deref().or(tag.artist.as_deref())),
    );
    let album = common_text(tags.iter().map(|tag| tag.album.as_deref()));
    let genre = common_text(tags.iter().map(|tag| tag.genre.as_deref()));
    let publisher = common_text(tags.iter().map(|tag| tag.publisher.as_deref()));
    let year = common_value(tags.iter().map(|tag| tag.year));
    let mut issues = Vec::new();
    if artist.is_none() {
        issues.push("Album artist is missing or inconsistent".to_owned());
    }
    if album.is_none() {
        issues.push("Album title is missing or inconsistent".to_owned());
    }
    if tracks.iter().any(|track| track.title.is_none()) {
        issues.push("One or more track titles are missing".to_owned());
    }
    if tracks.iter().any(|track| track.track_number.is_none()) {
        issues.push("Track numbers are incomplete".to_owned());
    }
    if tracks.len() > 1 && tracks.iter().any(|track| track.track_total.is_none()) {
        issues.push("Track totals are incomplete".to_owned());
    }
    let has_disc_numbers = tracks.iter().any(|track| track.disc_number.is_some());
    if has_disc_numbers && tracks.iter().any(|track| track.disc_number.is_none()) {
        issues.push("Disc numbers are incomplete".to_owned());
    }
    if genre.is_none() {
        issues.push("Genre is missing or inconsistent".to_owned());
    }
    if publisher.is_none() {
        issues.push("Publisher is missing or inconsistent".to_owned());
    }
    let path = path_text(&canonical)?;
    let id = hex_hash(path.as_bytes());
    Ok(InboxAlbum {
        id,
        path,
        folder_name: canonical
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Album")
            .to_owned(),
        artist,
        album,
        genre,
        publisher,
        year,
        track_count: tracks.len(),
        artwork_present,
        modified_at_ms,
        readiness: InboxReadiness {
            ready: issues.is_empty(),
            issues,
        },
        tracks,
    })
}

fn scan_track(path: &Path) -> Result<InboxTrack, String> {
    let canonical = fs::canonicalize(path).map_err(|error| error.to_string())?;
    let (tag, _) = read_tag_for_write(&canonical)?;
    let values = read_editable_tag_values(&tag)?;
    Ok(InboxTrack {
        path: path_text(&canonical)?,
        file_name: canonical
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("track.mp3")
            .to_owned(),
        title: values.title,
        artist: values.artist,
        track_number: values.track_number,
        track_total: values.track_total,
        disc_number: values.disc_number,
        disc_total: values.disc_total,
    })
}

fn search_musicbrainz(request: &ReleaseSearchRequest) -> Result<Vec<ReleaseCandidate>, String> {
    wait_for_musicbrainz();
    let query = format!(
        "artist:\"{}\" AND release:\"{}\"{}",
        escape_lucene(&request.artist),
        escape_lucene(&request.album),
        request
            .track_count
            .map_or_else(String::new, |count| format!(" AND tracks:{count}"))
    );
    let value: Value = http_client()?
        .get("https://musicbrainz.org/ws/2/release/")
        .query(&[("query", query.as_str()), ("limit", "10"), ("fmt", "json")])
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("MusicBrainz search failed: {error}"))?
        .json()
        .map_err(|error| format!("MusicBrainz returned invalid data: {error}"))?;
    Ok(value
        .get("releases")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|row| {
            let id = text(row, "id")?;
            let title = text(row, "title")?;
            let artist = artist_credit(row).unwrap_or_else(|| request.artist.clone());
            let media = row.get("media").and_then(Value::as_array);
            Some(ReleaseCandidate {
                source: MetadataSource::Musicbrainz,
                id,
                score: number(row, "score").unwrap_or(0) as u32,
                title,
                artist,
                year: text(row, "date").and_then(|value| parse_year(&value)),
                country: text(row, "country"),
                format: media
                    .and_then(|rows| rows.first())
                    .and_then(|value| text(value, "format")),
                publisher: first_label(row),
                track_count: media.map(|rows| {
                    rows.iter()
                        .filter_map(|value| number(value, "track-count"))
                        .sum::<u64>() as u32
                }),
                cover_url: None,
            })
        })
        .collect())
}

fn search_discogs(request: &ReleaseSearchRequest) -> Result<Vec<ReleaseCandidate>, String> {
    let auth = discogs_auth().ok_or_else(|| "Add a Discogs personal token or consumer key and secret in Settings before searching Discogs.".to_owned())?;
    let response = http_client()?
        .get("https://api.discogs.com/database/search")
        .header(reqwest::header::AUTHORIZATION, discogs_authorization(&auth))
        .query(&[
            ("artist", request.artist.as_str()),
            ("release_title", request.album.as_str()),
            ("type", "release"),
            ("per_page", "10"),
        ])
        .send()
        .map_err(|_| "Aurora could not connect to Discogs.".to_owned())?;
    let status = response.status();
    let value: Value = response
        .error_for_status()
        .map_err(|_| {
            format!("Discogs search failed with HTTP {status}. Check the saved credentials.")
        })?
        .json()
        .map_err(|error| format!("Discogs returned invalid data: {error}"))?;
    Ok(value
        .get("results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|row| {
            let id = number(row, "id")?.to_string();
            let raw_title = text(row, "title")?;
            let (artist, title) = raw_title.split_once(" - ").map_or_else(
                || (request.artist.clone(), raw_title.clone()),
                |(artist, title)| (artist.to_owned(), title.to_owned()),
            );
            let formats = strings(row, "format");
            let labels = strings(row, "label");
            let track_match = request.track_count.map_or(0, |_| 5);
            Some(ReleaseCandidate {
                source: MetadataSource::Discogs,
                id,
                score: (85 + track_match) as u32,
                title,
                artist,
                year: number(row, "year").map(|value| value as i32),
                country: text(row, "country"),
                format: formats.first().cloned(),
                publisher: labels.first().cloned(),
                track_count: None,
                cover_url: text(row, "cover_image"),
            })
        })
        .collect())
}

fn musicbrainz_detail(id: &str) -> Result<ReleaseCandidateDetail, String> {
    wait_for_musicbrainz();
    let url = format!("https://musicbrainz.org/ws/2/release/{id}");
    let value: Value = http_client()?
        .get(url)
        .query(&[
            (
                "inc",
                "recordings+artist-credits+labels+release-groups+genres",
            ),
            ("fmt", "json"),
        ])
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("MusicBrainz release lookup failed: {error}"))?
        .json()
        .map_err(|error| format!("MusicBrainz returned invalid release data: {error}"))?;
    let media = value
        .get("media")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let disc_total = (!media.is_empty()).then_some(media.len() as u32);
    let mut tracks = Vec::new();
    for (disc_index, medium) in media.iter().enumerate() {
        let rows = medium
            .get("tracks")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let total = rows.len() as u32;
        for (index, row) in rows.iter().enumerate() {
            let recording = row.get("recording").unwrap_or(row);
            tracks.push(ReleaseTrack {
                title: text(row, "title")
                    .or_else(|| text(recording, "title"))
                    .unwrap_or_else(|| format!("Track {}", index + 1)),
                artist: artist_credit(row).or_else(|| artist_credit(recording)),
                track_number: Some((index + 1) as u32),
                track_total: Some(total),
                disc_number: Some((disc_index + 1) as u32),
                disc_total,
                duration_ms: number(row, "length"),
            });
        }
    }
    let genres = value
        .get("release-group")
        .and_then(|group| group.get("genres"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|row| text(row, "name"))
        .collect::<Vec<_>>();
    let candidate = ReleaseCandidate {
        source: MetadataSource::Musicbrainz,
        id: id.to_owned(),
        score: 100,
        title: text(&value, "title").unwrap_or_default(),
        artist: artist_credit(&value).unwrap_or_default(),
        year: text(&value, "date").and_then(|value| parse_year(&value)),
        country: text(&value, "country"),
        format: media.first().and_then(|value| text(value, "format")),
        publisher: first_label(&value),
        track_count: Some(tracks.len() as u32),
        cover_url: Some(format!(
            "https://coverartarchive.org/release/{id}/front-250"
        )),
    };
    Ok(ReleaseCandidateDetail {
        album_artist: Some(candidate.artist.clone()).filter(|value| !value.is_empty()),
        album: Some(candidate.title.clone()).filter(|value| !value.is_empty()),
        genre: (!genres.is_empty()).then(|| genres.join("; ")),
        publisher: candidate.publisher.clone(),
        year: candidate.year,
        disc_total,
        tracks,
        candidate,
    })
}

fn discogs_detail(id: &str) -> Result<ReleaseCandidateDetail, String> {
    let auth = discogs_auth().ok_or_else(|| {
        "Add Discogs credentials in Settings before opening a Discogs release.".to_owned()
    })?;
    let url = format!("https://api.discogs.com/releases/{id}");
    let response = http_client()?
        .get(url)
        .header(reqwest::header::AUTHORIZATION, discogs_authorization(&auth))
        .send()
        .map_err(|_| "Aurora could not connect to Discogs.".to_owned())?;
    let status = response.status();
    let value: Value = response
        .error_for_status()
        .map_err(|_| {
            format!(
                "Discogs release lookup failed with HTTP {status}. Check the saved credentials."
            )
        })?
        .json()
        .map_err(|error| format!("Discogs returned invalid release data: {error}"))?;
    let artist = value
        .get("artists_sort")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
            value
                .get("artists")
                .and_then(Value::as_array)
                .and_then(|rows| rows.first())
                .and_then(|row| text(row, "name"))
        })
        .unwrap_or_default();
    let rows = value
        .get("tracklist")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let disc_total = rows
        .iter()
        .filter_map(|row| text(row, "position").and_then(|value| parse_disc_track(&value).0))
        .max();
    let playable = rows
        .iter()
        .filter(|row| text(row, "type_").as_deref() != Some("heading"))
        .collect::<Vec<_>>();
    let total = playable.len() as u32;
    let tracks = playable
        .into_iter()
        .enumerate()
        .map(|(index, row)| {
            let (disc, position) = text(row, "position")
                .map(|value| parse_disc_track(&value))
                .unwrap_or((None, None));
            ReleaseTrack {
                title: text(row, "title").unwrap_or_else(|| format!("Track {}", index + 1)),
                artist: None,
                track_number: position.or(Some((index + 1) as u32)),
                track_total: Some(total),
                disc_number: disc,
                disc_total,
                duration_ms: text(row, "duration").and_then(|value| parse_duration_ms(&value)),
            }
        })
        .collect::<Vec<_>>();
    let labels = value
        .get("labels")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|row| text(row, "name"))
        .collect::<Vec<_>>();
    let genres = [strings(&value, "genres"), strings(&value, "styles")].concat();
    let formats = value
        .get("formats")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|row| text(row, "name"))
        .collect::<Vec<_>>();
    let candidate = ReleaseCandidate {
        source: MetadataSource::Discogs,
        id: id.to_owned(),
        score: 100,
        title: text(&value, "title").unwrap_or_default(),
        artist,
        year: number(&value, "year").map(|value| value as i32),
        country: text(&value, "country"),
        format: formats.first().cloned(),
        publisher: labels.first().cloned(),
        track_count: Some(tracks.len() as u32),
        cover_url: text(&value, "thumb").or_else(|| text(&value, "cover_image")),
    };
    Ok(ReleaseCandidateDetail {
        album_artist: Some(candidate.artist.clone()).filter(|value| !value.is_empty()),
        album: Some(candidate.title.clone()).filter(|value| !value.is_empty()),
        genre: (!genres.is_empty()).then(|| genres.join("; ")),
        publisher: candidate.publisher.clone(),
        year: candidate.year,
        disc_total,
        tracks,
        candidate,
    })
}

fn http_client() -> Result<Client, String> {
    Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| format!("Could not start metadata networking: {error}"))
}

fn wait_for_musicbrainz() {
    static LAST_REQUEST: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    let lock = LAST_REQUEST.get_or_init(|| Mutex::new(None));
    if let Ok(mut last) = lock.lock() {
        if let Some(previous) = *last {
            let elapsed = previous.elapsed();
            if elapsed < Duration::from_secs(1) {
                thread::sleep(Duration::from_secs(1) - elapsed);
            }
        }
        *last = Some(Instant::now());
    }
}

fn credential_entry(user: &str) -> Result<Entry, String> {
    Entry::new(DISCOGS_SERVICE, user)
        .map_err(|error| format!("Windows Credential Manager is unavailable: {error}"))
}

fn saved_credential(user: &str) -> Option<String> {
    credential_entry(user)
        .ok()
        .and_then(|entry| entry.get_password().ok())
        .filter(|value| !value.trim().is_empty())
}

fn consumer_key() -> Option<String> {
    saved_credential(DISCOGS_KEY_USER).or_else(|| {
        std::env::var("DISCOGS")
            .ok()
            .filter(|value| !value.trim().is_empty())
    })
}

fn discogs_auth() -> Option<DiscogsAuth> {
    if let Some(token) = saved_credential(DISCOGS_TOKEN_USER).or_else(|| {
        std::env::var("DISCOGS_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty())
    }) {
        return Some(DiscogsAuth::Token(token));
    }
    let key = consumer_key()?;
    let secret = saved_credential(DISCOGS_SECRET_USER).or_else(|| {
        std::env::var("DISCOGS_SECRET")
            .ok()
            .filter(|value| !value.trim().is_empty())
    })?;
    Some(DiscogsAuth::Consumer { key, secret })
}

fn discogs_authorization(auth: &DiscogsAuth) -> String {
    match auth {
        DiscogsAuth::Token(token) => format!("Discogs token={token}"),
        DiscogsAuth::Consumer { key, secret } => format!("Discogs key={key}, secret={secret}"),
    }
}

fn validate_credential(value: String, label: &str) -> Result<String, String> {
    let value = value.trim().to_owned();
    if value.is_empty() || value.chars().count() > 512 || value.chars().any(char::is_whitespace) {
        return Err(format!("The Discogs {label} is invalid."));
    }
    Ok(value)
}

fn credential_error(error: keyring::Error) -> String {
    format!("Windows Credential Manager could not update Discogs credentials: {error}")
}

fn delete_credential(user: &str) -> Result<(), String> {
    match credential_entry(user)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(credential_error(error)),
    }
}

fn canonical_directory(value: &str) -> Result<PathBuf, String> {
    if value.trim().is_empty() || value.chars().count() > 32_768 {
        return Err("The Inbox folder path is invalid.".to_owned());
    }
    let path = fs::canonicalize(value)
        .map_err(|error| format!("Aurora could not open that folder: {error}"))?;
    if !path.is_dir() {
        return Err("The selected Inbox path is not a folder.".to_owned());
    }
    Ok(path)
}

fn validate_query(value: &str, label: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 512 || trimmed.chars().any(char::is_control)
    {
        return Err(format!("The {label} search value is invalid."));
    }
    Ok(())
}

fn validate_identifier(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err("The metadata release identifier is invalid.".to_owned());
    }
    Ok(())
}

fn path_text(path: &Path) -> Result<String, String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| "Inbox requires Unicode folder paths.".to_owned())
}
fn path_key(path: &Path) -> String {
    let value = path.to_string_lossy();
    if cfg!(windows) {
        value.to_lowercase()
    } else {
        value.into_owned()
    }
}
fn path_eq(left: &Path, right: &Path) -> bool {
    path_key(left) == path_key(right)
}
fn paths_equal(left: &str, right: &str) -> bool {
    if cfg!(windows) {
        left.eq_ignore_ascii_case(right)
    } else {
        left == right
    }
}
fn required_component(value: Option<&str>, label: &str) -> Result<String, String> {
    let value = value.map(str::trim).filter(|value| !value.is_empty());
    value
        .map(str::to_owned)
        .ok_or_else(|| format!("Add a {label} tag before renaming this album."))
}
fn sanitize_component(value: &str) -> Result<String, String> {
    let mut sanitized = value
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    sanitized = sanitized.trim().trim_end_matches(['.', ' ']).to_owned();
    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        return Err("The tags do not produce a valid Windows file name.".to_owned());
    }
    let stem = sanitized.split('.').next().unwrap_or_default();
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if reserved.iter().any(|name| stem.eq_ignore_ascii_case(name)) {
        sanitized.insert(0, '_');
    }
    Ok(sanitized)
}
fn decimal_width(value: u32) -> usize {
    value.max(1).ilog10() as usize + 1
}
fn is_mp3(path: &Path) -> bool {
    !path
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.starts_with(".aurora-"))
        && path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("mp3"))
}
fn system_time_ms(value: SystemTime) -> Option<u64> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as u64)
}
fn now_ms() -> u64 {
    system_time_ms(SystemTime::now()).unwrap_or(0)
}
fn hex_hash(value: &[u8]) -> String {
    Sha256::digest(value)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn common_text<'a>(values: impl Iterator<Item = Option<&'a str>>) -> Option<String> {
    common_value(values.map(|value| value.map(str::to_owned)))
}
fn common_value<T: Clone + PartialEq>(mut values: impl Iterator<Item = Option<T>>) -> Option<T> {
    let first = values.next().flatten()?;
    values
        .all(|value| value.as_ref() == Some(&first))
        .then_some(first)
}
fn escape_lucene(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
fn parse_year(value: &str) -> Option<i32> {
    value.get(..4)?.parse().ok()
}
fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)?
        .as_str()
        .map(str::to_owned)
        .filter(|value| !value.trim().is_empty())
}
fn number(value: &Value, key: &str) -> Option<u64> {
    value
        .get(key)?
        .as_u64()
        .or_else(|| value.get(key)?.as_str()?.parse().ok())
}
fn strings(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(str::to_owned))
        .collect()
}
fn artist_credit(value: &Value) -> Option<String> {
    value
        .get("artist-credit")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    row.get("name")
                        .and_then(Value::as_str)
                        .or_else(|| {
                            row.get("artist")
                                .and_then(|artist| artist.get("name"))
                                .and_then(Value::as_str)
                        })
                        .map(str::to_owned)
                })
                .collect::<Vec<_>>()
                .join("; ")
        })
        .filter(|value| !value.is_empty())
}
fn first_label(value: &Value) -> Option<String> {
    value
        .get("label-info")
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .and_then(|row| row.get("label"))
        .and_then(|label| text(label, "name"))
}
fn parse_disc_track(value: &str) -> (Option<u32>, Option<u32>) {
    let normalized = value.trim();
    if let Some((disc, track)) = normalized.split_once('-') {
        let parsed = (disc.parse().ok(), track.parse().ok());
        if parsed.0.is_some() && parsed.1.is_some() {
            return parsed;
        }
    }
    if normalized
        .chars()
        .all(|character| character.is_ascii_digit())
    {
        return (None, normalized.parse().ok());
    }
    // Vinyl positions such as A1, A2, B1 describe sides, not separate discs.
    // Returning no numeric position lets the release-order index become 01, 02, 03….
    (None, None)
}
fn parse_duration_ms(value: &str) -> Option<u64> {
    let mut parts = value.split(':').map(|part| part.parse::<u64>().ok());
    let minutes = parts.next()??;
    let seconds = parts.next()??;
    Some((minutes * 60 + seconds) * 1_000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use id3::frame::{Picture, PictureType};
    use id3::{Tag, TagLike, Version};

    #[test]
    fn discogs_positions_support_multidisc_and_plain_tracks() {
        assert_eq!(parse_disc_track("2-03"), (Some(2), Some(3)));
        assert_eq!(parse_disc_track("3"), (None, Some(3)));
        assert_eq!(parse_disc_track("A1"), (None, None));
        assert_eq!(parse_disc_track("B1"), (None, None));
    }

    #[test]
    fn rename_components_are_windows_safe_and_track_width_is_at_least_two() {
        assert_eq!(sanitize_component("AC/DC: Live").unwrap(), "AC_DC_ Live");
        assert_eq!(sanitize_component("CON").unwrap(), "_CON");
        assert_eq!(decimal_width(9).max(2), 2);
        assert_eq!(decimal_width(101).max(2), 3);
    }

    #[test]
    fn rename_album_uses_optional_disc_and_two_digit_tracks() {
        let parent = std::env::temp_dir().join(format!(
            "aurora-inbox-rename-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let album = parent.join("incoming");
        fs::create_dir_all(&album).expect("create album");
        write_rename_fixture(&album.join("first.mp3"), 1, None, "First");
        write_rename_fixture(&album.join("second.mp3"), 2, None, "Second");

        let result = rename_album(InboxRenameRequest {
            album_path: path_text(&album).expect("album path"),
        })
        .expect("rename album");
        let renamed = PathBuf::from(result.album_path);
        assert!(renamed.ends_with("Test Artist - Test Album (1990)"));
        assert!(renamed.join("01 - Track Artist - First.mp3").is_file());
        assert!(renamed.join("02 - Track Artist - Second.mp3").is_file());
        assert_eq!(result.renamed_tracks, 2);
        fs::remove_dir_all(parent).expect("remove fixture");
    }

    #[test]
    fn rename_album_keeps_multidisc_tracks_in_one_folder() {
        let parent = std::env::temp_dir().join(format!(
            "aurora-inbox-multidisc-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let album = parent.join("incoming");
        fs::create_dir_all(&album).expect("create album");
        write_rename_fixture(&album.join("disc-one.mp3"), 1, Some(1), "Disc One");
        write_rename_fixture(&album.join("disc-two.mp3"), 1, Some(2), "Disc Two");

        let result = rename_album(InboxRenameRequest {
            album_path: path_text(&album).expect("album path"),
        })
        .expect("rename multidisc album");
        let renamed = PathBuf::from(result.album_path);
        assert!(renamed.join("1-01 - Track Artist - Disc One.mp3").is_file());
        assert!(renamed.join("2-01 - Track Artist - Disc Two.mp3").is_file());
        assert_eq!(fs::read_dir(&renamed).expect("read album").count(), 2);
        fs::remove_dir_all(parent).expect("remove fixture");
    }

    #[test]
    fn inbox_artwork_presence_checks_only_the_first_sorted_track() {
        let parent = std::env::temp_dir().join(format!(
            "aurora-inbox-artwork-{}-{}",
            std::process::id(),
            now_ms()
        ));
        fs::create_dir_all(&parent).expect("create album");
        write_artwork_fixture(&parent.join("second.mp3"), 2, true);
        write_artwork_fixture(&parent.join("first.mp3"), 1, false);

        let album = scan_album(&parent).expect("scan album");
        assert!(!album.artwork_present);

        fs::remove_dir_all(parent).expect("remove fixture");
    }

    fn write_artwork_fixture(path: &Path, track: u32, with_picture: bool) {
        File::create(path)
            .expect("create track")
            .write_all(b"FAKE-MPEG-AUDIO")
            .expect("write track");
        let mut tag = Tag::with_version(Version::Id3v24);
        tag.set_album_artist("Test Artist");
        tag.set_artist("Test Artist");
        tag.set_album("Test Album");
        tag.set_title(format!("Track {track}"));
        tag.set_track(track);
        tag.set_total_tracks(2);
        tag.set_genre("Rock");
        tag.set_text("TPUB", "Test Label");
        if with_picture {
            tag.add_frame(Picture {
                mime_type: "image/png".to_owned(),
                picture_type: PictureType::CoverFront,
                description: String::new(),
                data: vec![1, 2, 3],
            });
        }
        tag.write_to_path(path, Version::Id3v24)
            .expect("write track tags");
    }

    fn write_rename_fixture(path: &Path, track: u32, disc: Option<u32>, title: &str) {
        File::create(path)
            .expect("create track")
            .write_all(b"FAKE-MPEG-AUDIO")
            .expect("write track");
        let mut tag = Tag::with_version(Version::Id3v24);
        tag.set_album_artist("Test Artist");
        tag.set_artist("Track Artist");
        tag.set_album("Test Album");
        tag.set_title(title);
        tag.set_year(1990);
        tag.set_track(track);
        tag.set_total_tracks(2);
        if let Some(disc) = disc {
            tag.set_disc(disc);
            tag.set_total_discs(2);
        }
        tag.write_to_path(path, Version::Id3v24)
            .expect("write track tags");
    }

    #[test]
    fn common_values_require_consistency() {
        assert_eq!(
            common_value([Some(1990), Some(1990)].into_iter()),
            Some(1990)
        );
        assert_eq!(common_value([Some(1990), Some(1992)].into_iter()), None);
    }
}
