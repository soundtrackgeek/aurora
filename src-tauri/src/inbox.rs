use crate::{
    artwork::{
        CanonicalCover, canonical_cover_from_image, canonical_cover_from_picture,
        canonical_front_cover_fingerprint, cover_digest, front_cover_matches, validate_cover_bytes,
    },
    lastfm, state_sync,
    tag_model::{EditableTagField, EditableTagValues},
    tagging::{
        apply_editor_tag_changes, audio_payload_hash, editor_non_target_frames, merge_editor_patch,
        read_editable_tag_values, read_tag_for_write, verify_editor_written_file,
    },
};
use id3::{
    Frame, TagLike,
    frame::{Picture, PictureType},
};
use keyring::Entry;
use lofty::{config::ParseOptions, file::AudioFile, probe::Probe};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Mutex, OnceLock},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

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
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

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
    quality_cache: HashMap<PathBuf, CachedAudioQuality>,
}

#[derive(Clone)]
struct CachedAudioQuality {
    size_bytes: u64,
    modified_ns: u64,
    bitrate_kbps: Option<u32>,
    duration_ms: Option<u64>,
    scan_error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InboxSettingsStatus {
    monitored_folders: Vec<String>,
    discogs_configured: bool,
    discogs_auth_mode: Option<DiscogsAuthMode>,
    discogs_incomplete_consumer_key: bool,
    last_fm_configured: bool,
    last_fm_secret_configured: bool,
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
    formats: Vec<String>,
    total_size_bytes: u64,
    avg_bitrate_kbps: Option<u32>,
    duration_ms: u64,
    audio_scan_error_count: usize,
    lossless_track_count: usize,
    artwork_present: bool,
    artwork_source_path: Option<String>,
    artwork_track_count: usize,
    artwork_ready: bool,
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
    format: String,
    size_bytes: u64,
    bitrate_kbps: Option<u32>,
    duration_ms: Option<u64>,
    scan_error: Option<String>,
    album_artist: Option<String>,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    genre: Option<String>,
    publisher: Option<String>,
    rating: Option<f64>,
    year: Option<i32>,
    release_year: Option<i32>,
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
    original_year: Option<i32>,
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
    #[serde(default)]
    prefer_original_edition: bool,
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
    #[serde(default)]
    remove_track_paths: Vec<String>,
    #[serde(default)]
    pub(crate) artwork_token: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InboxTagApplyResult {
    changed_tracks: usize,
    renamed_tracks: usize,
    removed_tracks: usize,
    recovery_path: Option<String>,
    album_path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct InboxCoverEmbedRequest {
    album_path: String,
    image_path: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InboxCoverEmbedResult {
    changed_tracks: usize,
    track_count: usize,
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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct InboxBatchRenameRequest {
    album_paths: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InboxRenameFailure {
    album_path: String,
    message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InboxBatchRenameResult {
    renamed_tracks: usize,
    renamed_albums: usize,
    renamed_folders: usize,
    failures: Vec<InboxRenameFailure>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct InboxConvertRequest {
    album_path: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InboxConversionFailure {
    file_name: String,
    message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InboxConvertResult {
    converted_tracks: usize,
    deleted_sources: usize,
    failures: Vec<InboxConversionFailure>,
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

struct PreparedRemoval {
    source: PathBuf,
    recovery: PathBuf,
}

struct AlbumRenameCandidate {
    requested_path: String,
    path: PathBuf,
    scanned: InboxAlbum,
    folder_name: String,
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
            quality_cache: HashMap::new(),
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
            last_fm_configured: lastfm::configured(),
            last_fm_secret_configured: lastfm::secret_configured(),
            warning: self.warning.clone(),
        }
    }

    pub(crate) fn monitored_roots(&self) -> Vec<PathBuf> {
        self.settings
            .monitored_folders
            .iter()
            .filter_map(|root| fs::canonicalize(root).ok())
            .collect()
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

    pub(crate) fn resolve_album_directory(&self, value: &str) -> Result<PathBuf, String> {
        if value.trim().is_empty() || value.chars().count() > 32_768 {
            return Err("The Inbox album path is invalid.".to_owned());
        }
        let album = canonical_directory(value)?;
        let allowed = self.settings.monitored_folders.iter().any(|root| {
            fs::canonicalize(root).is_ok_and(|canonical_root| album.starts_with(canonical_root))
        });
        allowed
            .then_some(album)
            .ok_or_else(|| "The Inbox album is outside the monitored folders.".to_owned())
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

    pub(crate) fn scan(&mut self) -> Result<InboxSnapshot, String> {
        let mut album_directories = HashSet::new();
        let mut visited = 0usize;
        for root in &self.settings.monitored_folders {
            collect_album_directories(Path::new(root), &mut album_directories, &mut visited)?;
        }
        let mut active_tracks = HashSet::new();
        let mut albums = album_directories
            .into_iter()
            .filter_map(|path| {
                match scan_album_cached(&path, &mut self.quality_cache, &mut active_tracks) {
                    Ok(album) => Some(album),
                    Err(error) => {
                        eprintln!("Inbox skipped {}: {error}", path.display());
                        None
                    }
                }
            })
            .collect::<Vec<_>>();
        self.quality_cache
            .retain(|path, _| active_tracks.contains(path));
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
        let left_original = left.year.is_some() && left.year == left.original_year;
        let right_original = right.year.is_some() && right.year == right.original_year;
        let left_track_difference = track_count_difference(left.track_count, request.track_count);
        let right_track_difference = track_count_difference(right.track_count, request.track_count);
        (if request.prefer_original_edition {
            right_original.cmp(&left_original)
        } else {
            std::cmp::Ordering::Equal
        })
        .then_with(|| left_track_difference.cmp(&right_track_difference))
        .then_with(|| right.score.cmp(&left.score))
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

pub(crate) fn apply_tags(
    request: InboxTagApplyRequest,
    cover: Option<&CanonicalCover>,
    recovery_root: &Path,
) -> Result<InboxTagApplyResult, String> {
    if (request.fields.is_empty() && cover.is_none())
        || (request.tracks.is_empty() && cover.is_none())
    {
        return Err(
            "Choose at least one field, track, or replacement album cover before applying tags."
                .to_owned(),
        );
    }
    let unique_fields = request.fields.iter().copied().collect::<HashSet<_>>();
    if unique_fields.len() != request.fields.len() {
        return Err("The Inbox tag edit contains duplicate fields.".to_owned());
    }
    let album = canonical_directory(&request.album_path)?;
    if request.remove_track_paths.len() > 100 {
        return Err("Choose at most 100 unmatched Inbox tracks to remove.".to_owned());
    }
    let mut removal_targets = Vec::with_capacity(request.remove_track_paths.len());
    let mut removal_set = HashSet::with_capacity(request.remove_track_paths.len());
    for path in &request.remove_track_paths {
        let target = fs::canonicalize(path)
            .map_err(|error| format!("Could not open an unmatched Inbox track: {error}"))?;
        if target.parent() != Some(album.as_path()) || !is_mp3(&target) {
            return Err(
                "An unmatched Inbox track is outside the selected album folder.".to_owned(),
            );
        }
        if !removal_set.insert(target.clone()) {
            return Err("The Inbox removal selection contains a duplicate track.".to_owned());
        }
        removal_targets.push(target);
    }
    let mut patches = HashMap::with_capacity(request.tracks.len());
    for patch in &request.tracks {
        let target = fs::canonicalize(&patch.path)
            .map_err(|error| format!("Could not open an Inbox track: {error}"))?;
        if target.parent() != Some(album.as_path()) || !is_mp3(&target) {
            return Err("An Inbox track is outside the selected album folder.".to_owned());
        }
        if removal_set.contains(&target) {
            return Err(
                "An Inbox track cannot be tagged and removed in the same batch.".to_owned(),
            );
        }
        if patches.insert(target, patch).is_some() {
            return Err("The Inbox tag selection contains a duplicate track.".to_owned());
        }
    }
    let mut targets = if cover.is_some() {
        fs::read_dir(&album)
            .map_err(|error| format!("Could not read the Inbox album: {error}"))?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                entry
                    .file_type()
                    .ok()
                    .filter(|kind| kind.is_file() && !kind.is_symlink())
                    .map(|_| entry.path())
            })
            .filter(|path| is_mp3(path) && !removal_set.contains(path))
            .collect::<Vec<_>>()
    } else {
        patches.keys().cloned().collect::<Vec<_>>()
    };
    targets.sort_by_key(|path| path.file_name().map(|name| name.to_os_string()));
    if targets.is_empty() {
        return Err("The Inbox album contains no MP3 files.".to_owned());
    }
    let expected_artwork_fingerprint = cover.map(canonical_front_cover_fingerprint);
    let mut prepared = Vec::new();
    let sequence = now_ms();
    for (index, target) in targets.into_iter().enumerate() {
        let patch = patches.get(&target).copied();
        let (mut tag, version) = read_tag_for_write(&target)?;
        let before = read_editable_tag_values(&tag)?;
        let after = patch.map_or_else(
            || Ok(before.clone()),
            |patch| merge_editor_patch(&before, &request.fields, &patch.values.clone().normalize()),
        )?;
        let artwork_changed = cover.is_some_and(|cover| !front_cover_matches(&tag, &cover.digest));
        if before == after && !artwork_changed {
            continue;
        }
        let track_fields = if patch.is_some() {
            request.fields.as_slice()
        } else {
            &[]
        };
        let preserved_frames = editor_non_target_frames(&tag, track_fields, artwork_changed);
        let original_hash = audio_payload_hash(&target)?;
        let temporary = album.join(format!(".aurora-inbox-{sequence}-{index}.tmp.mp3"));
        let backup = album.join(format!(".aurora-inbox-{sequence}-{index}.backup.mp3"));
        fs::copy(&target, &temporary)
            .map_err(|error| format!("Could not prepare an Inbox tag edit: {error}"))?;
        apply_editor_tag_changes(&mut tag, version, track_fields, &after)?;
        if artwork_changed {
            tag.remove_picture_by_type(PictureType::CoverFront);
            tag.add_frame(
                cover
                    .expect("an Inbox artwork change was prepared with a cover")
                    .picture
                    .clone(),
            );
        }
        if let Err(error) = tag.write_to_path(&temporary, version) {
            let _ = fs::remove_file(&temporary);
            cleanup_prepared(&prepared);
            return Err(format!("Could not write staged Inbox tags: {error}"));
        }
        if verify_editor_written_file(
            &temporary,
            &after,
            track_fields,
            artwork_changed,
            expected_artwork_fingerprint.as_ref(),
            &preserved_frames,
            &original_hash,
        )
        .is_err()
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
    let (prepared_removals, recovery_directory) =
        prepare_inbox_recovery(&album, &removal_targets, recovery_root, sequence)?;
    for item in &prepared {
        if let Err(error) = fs::copy(&item.target, &item.backup) {
            cleanup_prepared(&prepared);
            cleanup_recovery(&recovery_directory);
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
            cleanup_recovery(&recovery_directory);
            return Err(format!(
                "Aurora could not finish the Inbox tag batch and restored earlier tracks: {error}"
            ));
        }
        installed += 1;
    }
    let mut removed = 0usize;
    for item in &prepared_removals {
        if let Err(error) = crate::track_deletion::remove_verified_mp3(&item.source) {
            for previous in prepared.iter().rev() {
                let _ = state_sync::replace_file_atomic(&previous.target, &previous.backup);
            }
            let restore_errors = restore_removed_tracks(&prepared_removals, removed);
            cleanup_prepared(&prepared);
            if restore_errors.is_empty() {
                cleanup_recovery(&recovery_directory);
                return Err(format!(
                    "Aurora could not remove an unmatched Inbox track and restored the album: {error}"
                ));
            }
            return Err(format!(
                "Aurora could not remove an unmatched Inbox track. Recovery copies were retained at {} because rollback also failed: {}",
                recovery_directory.as_ref().map_or_else(
                    || "the Inbox recovery folder".to_owned(),
                    |path| path.display().to_string()
                ),
                restore_errors.join("; ")
            ));
        }
        removed += 1;
    }
    let rename_result = if request.rename_after_apply {
        match rename_album_path(&album) {
            Ok(result) => Some(result),
            Err(error) => {
                for item in prepared.iter().rev() {
                    let _ = state_sync::replace_file_atomic(&item.target, &item.backup);
                }
                let restore_errors = restore_removed_tracks(&prepared_removals, removed);
                cleanup_prepared(&prepared);
                if restore_errors.is_empty() {
                    cleanup_recovery(&recovery_directory);
                }
                return Err(format!(
                    "Aurora could not rename the tagged album and restored its original tags{}: {error}",
                    if restore_errors.is_empty() {
                        " and unmatched tracks"
                    } else {
                        "; unmatched-track recovery requires manual attention"
                    }
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
        removed_tracks: removed,
        recovery_path: recovery_directory.as_deref().map(path_text).transpose()?,
        album_path: path_text(&final_album)?,
    })
}

pub(crate) fn embed_album_cover(
    request: InboxCoverEmbedRequest,
    monitored_roots: &[PathBuf],
) -> Result<InboxCoverEmbedResult, String> {
    let album = canonical_directory(&request.album_path)?;
    let allowed = monitored_roots.iter().any(|root| {
        fs::canonicalize(root).is_ok_and(|canonical_root| album.starts_with(canonical_root))
    });
    if !allowed {
        return Err("The Inbox album is outside the monitored folders.".to_owned());
    }

    let scanned = scan_album(&album)?;
    if scanned.tracks.is_empty() {
        return Err("The Inbox album contains no MP3 files.".to_owned());
    }
    let cover = match request.image_path.as_deref() {
        Some(path) => canonical_cover_from_image(Path::new(path))?,
        None => canonical_cover_from_tracks(&scanned.tracks)?,
    };

    let sequence = now_ms();
    let mut prepared = Vec::new();
    for (index, track) in scanned.tracks.iter().enumerate() {
        let next = (|| -> Result<Option<PreparedWrite>, String> {
            let target = fs::canonicalize(&track.path)
                .map_err(|error| format!("Could not open an Inbox track: {error}"))?;
            if target.parent() != Some(album.as_path()) || !is_mp3(&target) {
                return Err("An Inbox track is outside the selected album folder.".to_owned());
            }
            let (mut tag, version) = read_tag_for_write(&target)?;
            let front_pictures = tag
                .pictures()
                .filter(|picture| picture.picture_type == PictureType::CoverFront)
                .collect::<Vec<_>>();
            if front_pictures.len() == 1
                && cover_digest(front_pictures[0]).is_ok_and(|digest| digest == cover.digest)
            {
                return Ok(None);
            }

            let preserved_frames = non_front_cover_frames(&tag);
            let original_hash = audio_payload_hash(&target)?;
            let temporary = album.join(format!(".aurora-inbox-cover-{sequence}-{index}.tmp.mp3"));
            let backup = album.join(format!(".aurora-inbox-cover-{sequence}-{index}.backup.mp3"));
            fs::copy(&target, &temporary)
                .map_err(|error| format!("Could not prepare the embedded cover update: {error}"))?;
            tag.remove_picture_by_type(PictureType::CoverFront);
            tag.add_frame(cover.picture.clone());
            if let Err(error) = tag.write_to_path(&temporary, version) {
                let _ = fs::remove_file(&temporary);
                return Err(format!(
                    "Could not write the staged embedded cover: {error}"
                ));
            }
            if let Err(error) = verify_embedded_cover_write(
                &temporary,
                &cover.digest,
                &preserved_frames,
                &original_hash,
            ) {
                let _ = fs::remove_file(&temporary);
                return Err(format!(
                    "Aurora could not verify the staged embedded cover. No originals were changed: {error}"
                ));
            }
            Ok(Some(PreparedWrite {
                target,
                temporary,
                backup,
            }))
        })();
        match next {
            Ok(Some(item)) => prepared.push(item),
            Ok(None) => {}
            Err(error) => {
                cleanup_prepared(&prepared);
                return Err(error);
            }
        }
    }

    for item in &prepared {
        if let Err(error) = fs::copy(&item.target, &item.backup) {
            cleanup_prepared(&prepared);
            return Err(format!(
                "Could not create an embedded-cover safety backup: {error}"
            ));
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
                "Aurora could not finish the embedded-cover batch and restored earlier tracks: {error}"
            ));
        }
        installed += 1;
    }
    cleanup_prepared(&prepared);
    let verified = scan_album(&album)?;
    if !verified.artwork_ready {
        return Err(
            "Aurora installed the embedded covers, but the album did not pass its final artwork verification."
                .to_owned(),
        );
    }
    Ok(InboxCoverEmbedResult {
        changed_tracks: installed,
        track_count: verified.track_count,
    })
}

pub(crate) fn convert_lossless_album(
    request: InboxConvertRequest,
    monitored_roots: &[PathBuf],
) -> Result<InboxConvertResult, String> {
    static CONVERSION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _guard = CONVERSION_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "Aurora's Inbox converter stopped unexpectedly.".to_owned())?;
    let album = canonical_directory(&request.album_path)?;
    let allowed = monitored_roots.iter().any(|root| {
        fs::canonicalize(root).is_ok_and(|canonical_root| album.starts_with(canonical_root))
    });
    if !allowed {
        return Err("The Inbox album is outside the monitored folders.".to_owned());
    }

    let mut sources = fs::read_dir(&album)
        .map_err(|error| format!("Could not read the Inbox album: {error}"))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .file_type()
                .ok()
                .filter(|kind| kind.is_file() && !kind.is_symlink())
                .map(|_| entry.path())
        })
        .filter(|path| is_lossless_source(path))
        .collect::<Vec<_>>();
    sources.sort_by_key(|path| path.file_name().map(|name| name.to_os_string()));
    if sources.is_empty() {
        return Err("The Inbox album contains no FLAC or APE files to convert.".to_owned());
    }

    let ffmpeg = discover_ffmpeg_executable()?;
    let sequence = now_ms();
    let mut result = InboxConvertResult {
        converted_tracks: 0,
        deleted_sources: 0,
        failures: Vec::new(),
    };
    for (index, source) in sources.iter().enumerate() {
        let file_name = source
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("lossless track")
            .to_owned();
        match convert_lossless_track(&ffmpeg, source, &album, sequence, index) {
            Ok(()) => {
                result.converted_tracks += 1;
                result.deleted_sources += 1;
            }
            Err(message) => result
                .failures
                .push(InboxConversionFailure { file_name, message }),
        }
    }
    Ok(result)
}

fn convert_lossless_track(
    ffmpeg: &Path,
    source: &Path,
    album: &Path,
    sequence: u64,
    index: usize,
) -> Result<(), String> {
    let destination = source.with_extension("mp3");
    if destination.exists() {
        return Err("An MP3 with the same name already exists; nothing was changed.".to_owned());
    }
    let temporary = album.join(format!(
        ".aurora-convert-{}-{sequence}-{index}.tmp.mp3",
        std::process::id()
    ));
    let _ = fs::remove_file(&temporary);
    let output = hidden_command(ffmpeg)
        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-n", "-i"])
        .arg(source)
        .args([
            "-map",
            "0:a:0",
            "-map",
            "0:v?",
            "-map_metadata",
            "0",
            "-c:a",
            "libmp3lame",
            "-b:a",
            "320k",
            "-c:v",
            "copy",
            "-id3v2_version",
            "3",
            "-write_id3v1",
            "1",
        ])
        .arg(&temporary)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("Could not start FFmpeg: {error}"))?;
    if !output.status.success() {
        let _ = fs::remove_file(&temporary);
        let detail = String::from_utf8_lossy(&output.stderr)
            .trim()
            .chars()
            .take(800)
            .collect::<String>();
        return Err(if detail.is_empty() {
            "FFmpeg could not convert this track; the source was kept.".to_owned()
        } else {
            format!("FFmpeg could not convert this track; the source was kept: {detail}")
        });
    }

    let quality = inspect_audio_quality(&temporary, &mut HashMap::new());
    let verified = temporary.is_file()
        && quality.size_bytes > 0
        && quality.duration_ms.is_some_and(|duration| duration > 0)
        && quality
            .bitrate_kbps
            .is_some_and(|bitrate| (300..=340).contains(&bitrate))
        && quality.scan_error.is_none();
    if !verified {
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "Aurora could not verify a 320 kbps MP3 output; the source was kept (size {}, duration {:?}, bitrate {:?}, scan error {:?}).",
            quality.size_bytes, quality.duration_ms, quality.bitrate_kbps, quality.scan_error
        ));
    }
    fs::rename(&temporary, &destination).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("Could not install the verified MP3; the source was kept: {error}")
    })?;
    if let Err(error) = fs::remove_file(source) {
        let rollback = fs::remove_file(&destination);
        return Err(match rollback {
            Ok(()) => format!("Could not delete the source, so the new MP3 was removed: {error}"),
            Err(rollback_error) => format!(
                "Could not delete the source or roll back the new MP3: {error}; rollback failed: {rollback_error}"
            ),
        });
    }
    Ok(())
}

fn discover_ffmpeg_executable() -> Result<PathBuf, String> {
    let executable_name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    let mut candidates = Vec::new();
    if let Some(override_path) = std::env::var_os("AURORA_FFMPEG_PATH") {
        candidates.push(PathBuf::from(override_path));
    }
    if let Ok(current) = std::env::current_exe()
        && let Some(parent) = current.parent()
    {
        candidates.push(parent.join(executable_name));
    }
    if let Some(path_value) = std::env::var_os("PATH") {
        candidates.extend(
            std::env::split_paths(&path_value).map(|directory| directory.join(executable_name)),
        );
    }
    #[cfg(windows)]
    candidates.push(PathBuf::from(r"C:\ffmpeg\bin\ffmpeg.exe"));

    for candidate in candidates {
        let available = hidden_command(&candidate)
            .arg("-version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if available {
            return Ok(candidate);
        }
    }
    Err(
        "Aurora needs FFmpeg to convert FLAC or APE files. Install FFmpeg or place ffmpeg.exe beside Aurora, then try again."
            .to_owned(),
    )
}

fn hidden_command(executable: &Path) -> Command {
    let mut command = Command::new(executable);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

pub(crate) fn rename_album(request: InboxRenameRequest) -> Result<InboxRenameResult, String> {
    let album = canonical_directory(&request.album_path)?;
    rename_album_path(&album)
}

pub(crate) fn rename_albums(
    request: InboxBatchRenameRequest,
    monitored_roots: &[PathBuf],
) -> Result<InboxBatchRenameResult, String> {
    if request.album_paths.len() > MAX_SCANNED_DIRECTORIES {
        return Err("Too many Inbox albums were selected for one rename.".to_owned());
    }
    let mut failures = Vec::new();
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    for requested_path in request.album_paths {
        match album_rename_candidate(requested_path.clone()) {
            Ok(candidate) if seen.insert(path_key(&candidate.path)) => candidates.push(candidate),
            Ok(_) => {}
            Err(message) => failures.push(InboxRenameFailure {
                album_path: requested_path,
                message,
            }),
        }
    }

    let mut groups: Vec<Vec<AlbumRenameCandidate>> = Vec::new();
    let mut group_indices = HashMap::new();
    for candidate in candidates {
        let parent = candidate
            .path
            .parent()
            .ok_or_else(|| "The Inbox album has no parent folder.".to_owned())?;
        let group_key = format!(
            "{}\u{0}{}",
            path_key(parent),
            candidate.folder_name.to_lowercase()
        );
        let group_index = *group_indices.entry(group_key).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[group_index].push(candidate);
    }

    let mut renamed_tracks = 0;
    let mut renamed_albums = 0;
    let mut renamed_folders = 0;
    for group in groups {
        let result = if group.len() > 1 {
            merge_album_group(&group, monitored_roots)
        } else {
            let candidate = &group[0];
            let parent_is_monitored = candidate
                .path
                .parent()
                .is_some_and(|parent| monitored_roots.iter().any(|root| path_eq(root, parent)));
            if !parent_is_monitored && disc_folder_number(&candidate.path).is_some() {
                Err("Select every disc folder for this release before renaming it. Aurora will merge the discs into one album folder.".to_owned())
            } else {
                rename_album_path(&candidate.path)
            }
        };
        match result {
            Ok(result) => {
                renamed_tracks += result.renamed_tracks;
                renamed_albums += group.len();
                renamed_folders += usize::from(result.folder_renamed);
            }
            Err(message) => {
                failures.extend(group.into_iter().map(|candidate| InboxRenameFailure {
                    album_path: candidate.requested_path,
                    message: message.clone(),
                }))
            }
        }
    }

    Ok(InboxBatchRenameResult {
        renamed_tracks,
        renamed_albums,
        renamed_folders,
        failures,
    })
}

fn album_rename_candidate(requested_path: String) -> Result<AlbumRenameCandidate, String> {
    let path = canonical_directory(&requested_path)?;
    let scanned = scan_album(&path)?;
    let album_artist = required_component(scanned.artist.as_deref(), "Album Artist")?;
    let album_title = required_component(scanned.album.as_deref(), "Album")?;
    let year = scanned
        .year
        .filter(|value| *value > 0)
        .ok_or_else(|| "Add a Year tag before renaming this album.".to_owned())?;
    let folder_name = sanitize_component(&format!("{album_artist} - {album_title} ({year})"))?;
    Ok(AlbumRenameCandidate {
        requested_path,
        path,
        scanned,
        folder_name,
    })
}

fn merge_album_group(
    group: &[AlbumRenameCandidate],
    monitored_roots: &[PathBuf],
) -> Result<InboxRenameResult, String> {
    let common_parent = group[0]
        .path
        .parent()
        .ok_or_else(|| "The selected disc folder has no parent folder.".to_owned())?
        .to_path_buf();
    if group.iter().any(|candidate| {
        candidate
            .path
            .parent()
            .is_none_or(|parent| !path_eq(parent, &common_parent))
    }) {
        return Err(
            "The selected discs must be sibling folders before Aurora can merge them.".to_owned(),
        );
    }

    let selected_directories = group
        .iter()
        .map(|candidate| path_key(&candidate.path))
        .collect::<HashSet<_>>();
    let parent_is_monitored = monitored_roots
        .iter()
        .any(|root| path_eq(root, &common_parent));
    if !parent_is_monitored {
        for entry in fs::read_dir(&common_parent)
            .map_err(|error| format!("Could not inspect the multi-disc album folder: {error}"))?
        {
            let entry = entry.map_err(|error| {
                format!("Could not inspect the multi-disc album folder: {error}")
            })?;
            let kind = entry.file_type().map_err(|error| {
                format!("Could not inspect the multi-disc album folder: {error}")
            })?;
            if kind.is_symlink() {
                return Err(format!(
                    "Aurora cannot flatten this release because it contains the folder link {}.",
                    entry.path().display()
                ));
            }
            if kind.is_dir() && !selected_directories.contains(&path_key(&entry.path())) {
                return Err(format!(
                    "Select every disc folder before renaming this release. The unselected folder is {}.",
                    entry.path().display()
                ));
            }
        }
    }

    let final_folder = if parent_is_monitored {
        common_parent.join(&group[0].folder_name)
    } else {
        common_parent
            .parent()
            .ok_or_else(|| "The multi-disc album folder has no parent folder.".to_owned())?
            .join(&group[0].folder_name)
    };
    let mut created_destination = false;
    let destination_root = if parent_is_monitored {
        if final_folder.exists() {
            let canonical = fs::canonicalize(&final_folder).map_err(|error| {
                format!("Could not inspect the destination album folder: {error}")
            })?;
            if !selected_directories.contains(&path_key(&canonical)) {
                return Err(format!(
                    "The destination album folder already exists: {}",
                    final_folder.display()
                ));
            }
            canonical
        } else {
            created_destination = true;
            final_folder.clone()
        }
    } else {
        if !path_eq(&common_parent, &final_folder) && final_folder.exists() {
            return Err(format!(
                "The destination album folder already exists: {}",
                final_folder.display()
            ));
        }
        common_parent.clone()
    };

    let mut extra_files = Vec::new();
    let mut source_files = HashSet::new();
    for candidate in group {
        for entry in fs::read_dir(&candidate.path)
            .map_err(|error| format!("Could not inspect a selected disc folder: {error}"))?
        {
            let entry = entry
                .map_err(|error| format!("Could not inspect a selected disc folder: {error}"))?;
            let kind = entry
                .file_type()
                .map_err(|error| format!("Could not inspect a selected disc folder: {error}"))?;
            if kind.is_symlink() || kind.is_dir() {
                return Err(format!(
                    "Aurora cannot flatten {} because it contains another folder or link.",
                    candidate.path.display()
                ));
            }
            if kind.is_file() {
                source_files.insert(path_key(&entry.path()));
                if !is_mp3(&entry.path()) {
                    extra_files.push(entry.path());
                }
            }
        }
    }

    let track_width = group
        .iter()
        .flat_map(|candidate| candidate.scanned.tracks.iter())
        .filter_map(|track| track.track_number.or(track.track_total))
        .max()
        .map_or(2, decimal_width)
        .max(2);
    let sequence = now_ms();
    let mut next_index = 0usize;
    let mut destinations = HashSet::new();
    let mut prepared = Vec::new();
    let mut renamed_tracks = 0;
    for candidate in group {
        for track in &candidate.scanned.tracks {
            let track_number = track.track_number.ok_or_else(|| {
                format!(
                    "Add a Track Number tag to {} before renaming.",
                    track.file_name
                )
            })?;
            let disc_number = track.disc_number.ok_or_else(|| {
                format!(
                    "Add a Disc Number tag to {} before merging this multi-disc release.",
                    track.file_name
                )
            })?;
            let artist = required_component(track.artist.as_deref(), "Artist")?;
            let title = required_component(track.title.as_deref(), "Title")?;
            let position = format!("{disc_number}-{track_number:0track_width$}");
            let file_name = sanitize_component(&format!("{position} - {artist} - {title}"))?;
            let source = PathBuf::from(&track.path);
            let destination = destination_root.join(format!("{file_name}.mp3"));
            if prepare_merged_file_rename(
                &mut prepared,
                &mut destinations,
                &source_files,
                source,
                destination,
                sequence,
                next_index,
            )? {
                renamed_tracks += 1;
            }
            next_index += 1;
        }
    }
    for source in extra_files {
        let file_name = source
            .file_name()
            .ok_or_else(|| "A selected disc file has no filename.".to_owned())?;
        let destination = destination_root.join(file_name);
        prepare_merged_file_rename(
            &mut prepared,
            &mut destinations,
            &source_files,
            source,
            destination,
            sequence,
            next_index,
        )?;
        next_index += 1;
    }

    if created_destination {
        fs::create_dir(&destination_root)
            .map_err(|error| format!("Could not create the destination album folder: {error}"))?;
    }
    if let Err(error) = stage_renames(&prepared) {
        if created_destination {
            let _ = fs::remove_dir(&destination_root);
        }
        return Err(error);
    }

    let directories_to_remove = group
        .iter()
        .map(|candidate| candidate.path.clone())
        .filter(|path| !path_eq(path, &destination_root))
        .collect::<Vec<_>>();
    let mut removed_directories = Vec::new();
    for directory in &directories_to_remove {
        if let Err(error) = fs::remove_dir(directory) {
            for removed in &removed_directories {
                let _ = fs::create_dir_all(removed);
            }
            rollback_renames(&prepared, prepared.len());
            if created_destination {
                let _ = fs::remove_dir(&destination_root);
            }
            return Err(format!(
                "Could not remove the empty disc folder {}: {error}",
                directory.display()
            ));
        }
        removed_directories.push(directory.clone());
    }

    let final_album = if parent_is_monitored || path_eq(&common_parent, &final_folder) {
        destination_root.clone()
    } else {
        if let Err(error) = fs::rename(&common_parent, &final_folder) {
            for removed in &removed_directories {
                let _ = fs::create_dir_all(removed);
            }
            rollback_renames(&prepared, prepared.len());
            return Err(format!(
                "Could not rename the multi-disc album folder: {error}"
            ));
        }
        final_folder
    };

    Ok(InboxRenameResult {
        album_path: path_text(&final_album)?,
        renamed_tracks,
        folder_renamed: !directories_to_remove.is_empty()
            || created_destination
            || !path_eq(&common_parent, &final_album),
    })
}

fn prepare_merged_file_rename(
    prepared: &mut Vec<PreparedRename>,
    destinations: &mut HashSet<String>,
    sources: &HashSet<String>,
    source: PathBuf,
    destination: PathBuf,
    sequence: u64,
    index: usize,
) -> Result<bool, String> {
    let destination_key = path_key(&destination);
    if !destinations.insert(destination_key.clone()) {
        return Err(format!(
            "More than one selected disc file would be named {}.",
            destination.display()
        ));
    }
    if destination.exists() && !sources.contains(&destination_key) {
        return Err(format!(
            "The destination file already exists: {}",
            destination.display()
        ));
    }
    if path_eq(&source, &destination) {
        return Ok(false);
    }
    let temporary = source
        .parent()
        .ok_or_else(|| "A selected disc file has no parent folder.".to_owned())?
        .join(format!(".aurora-rename-{sequence}-{index}.tmp"));
    if temporary.exists() {
        return Err(format!(
            "Aurora's temporary rename file already exists: {}",
            temporary.display()
        ));
    }
    prepared.push(PreparedRename {
        source,
        temporary,
        destination,
    });
    Ok(true)
}

fn disc_folder_number(path: &Path) -> Option<u32> {
    let compact = path
        .file_name()?
        .to_str()?
        .chars()
        .filter(|character| !character.is_ascii_whitespace() && !matches!(*character, '-' | '_'))
        .collect::<String>()
        .to_ascii_lowercase();
    compact
        .strip_prefix("cd")
        .or_else(|| compact.strip_prefix("disc"))?
        .parse()
        .ok()
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

fn prepare_inbox_recovery(
    album: &Path,
    tracks: &[PathBuf],
    recovery_root: &Path,
    sequence: u64,
) -> Result<(Vec<PreparedRemoval>, Option<PathBuf>), String> {
    if tracks.is_empty() {
        return Ok((Vec::new(), None));
    }
    let album_key = hex_hash(album.to_string_lossy().as_bytes());
    let directory = recovery_root.join(format!("{sequence}-{}", &album_key[..12]));
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Could not create Aurora's Inbox recovery folder: {error}"))?;
    let mut prepared = Vec::with_capacity(tracks.len());
    for source in tracks {
        let result = (|| -> Result<PreparedRemoval, String> {
            let file_name = source
                .file_name()
                .ok_or_else(|| "An unmatched Inbox track has no filename.".to_owned())?;
            let recovery = directory.join(file_name);
            if recovery.exists() {
                return Err("Aurora's Inbox recovery destination already exists.".to_owned());
            }
            fs::copy(source, &recovery).map_err(|error| {
                format!("Could not copy an unmatched track into Inbox recovery: {error}")
            })?;
            File::options()
                .read(true)
                .write(true)
                .open(&recovery)
                .and_then(|file| file.sync_all())
                .map_err(|error| format!("Could not flush an Inbox recovery copy: {error}"))?;
            if file_sha256(source)? != file_sha256(&recovery)? {
                return Err("Aurora could not verify an Inbox recovery copy.".to_owned());
            }
            Ok(PreparedRemoval {
                source: source.clone(),
                recovery,
            })
        })();
        match result {
            Ok(item) => prepared.push(item),
            Err(error) => {
                let _ = fs::remove_dir_all(&directory);
                return Err(error);
            }
        }
    }
    Ok((prepared, Some(directory)))
}

fn restore_removed_tracks(items: &[PreparedRemoval], removed: usize) -> Vec<String> {
    let mut errors = Vec::new();
    for item in items[..removed].iter().rev() {
        if item.source.exists() {
            continue;
        }
        if let Err(error) = fs::copy(&item.recovery, &item.source) {
            errors.push(format!(
                "Could not restore {}: {error}",
                item.source.display()
            ));
            continue;
        }
        match (file_sha256(&item.recovery), file_sha256(&item.source)) {
            (Ok(expected), Ok(actual)) if expected == actual => {}
            _ => errors.push(format!(
                "Could not verify restored track {}",
                item.source.display()
            )),
        }
    }
    errors
}

fn cleanup_recovery(directory: &Option<PathBuf>) {
    if let Some(directory) = directory {
        let _ = fs::remove_dir_all(directory);
    }
}

fn file_sha256(path: &Path) -> Result<[u8; 32], String> {
    let mut file = File::open(path)
        .map_err(|error| format!("Could not open a file for recovery verification: {error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("Could not read a file for recovery verification: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().into())
}

fn cleanup_prepared(items: &[PreparedWrite]) {
    for item in items {
        let _ = fs::remove_file(&item.temporary);
        let _ = fs::remove_file(&item.backup);
    }
}

fn cached_cover_digest(
    picture: &Picture,
    validation_cache: &mut HashMap<[u8; 32], bool>,
) -> Option<[u8; 32]> {
    let digest: [u8; 32] = Sha256::digest(&picture.data).into();
    let valid = *validation_cache
        .entry(digest)
        .or_insert_with(|| validate_cover_bytes(&picture.data).is_ok());
    valid.then_some(digest)
}

fn canonical_cover_from_tracks(tracks: &[InboxTrack]) -> Result<CanonicalCover, String> {
    for prefer_front in [true, false] {
        for track in tracks {
            let (tag, _) = read_tag_for_write(Path::new(&track.path))?;
            for picture in tag
                .pictures()
                .filter(|picture| (picture.picture_type == PictureType::CoverFront) == prefer_front)
            {
                if let Ok(cover) = canonical_cover_from_picture(picture) {
                    return Ok(cover);
                }
            }
        }
    }
    Err("No usable embedded album cover was found. Choose an image file first.".to_owned())
}

fn non_front_cover_frames(tag: &id3::Tag) -> Vec<Frame> {
    tag.frames()
        .filter(|frame| {
            frame
                .content()
                .picture()
                .is_none_or(|picture| picture.picture_type != PictureType::CoverFront)
        })
        .cloned()
        .collect()
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

fn verify_embedded_cover_write(
    path: &Path,
    expected_digest: &[u8; 32],
    preserved_frames: &[Frame],
    expected_payload_hash: &[u8; 32],
) -> Result<(), String> {
    let (tag, _) = read_tag_for_write(path)?;
    let front_pictures = tag
        .pictures()
        .filter(|picture| picture.picture_type == PictureType::CoverFront)
        .collect::<Vec<_>>();
    if front_pictures.len() != 1 || cover_digest(front_pictures[0]).as_ref() != Ok(expected_digest)
    {
        return Err("the embedded front cover did not round-trip".to_owned());
    }
    if !same_frames(&non_front_cover_frames(&tag), preserved_frames) {
        return Err("a non-cover ID3 frame changed".to_owned());
    }
    if &audio_payload_hash(path)? != expected_payload_hash {
        return Err("the MP3 audio payload changed".to_owned());
    }
    Ok(())
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
        let mut has_audio = false;
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
            } else if kind.is_file() && is_inbox_audio(&path) {
                has_audio = true;
            }
        }
        if has_audio {
            albums.insert(directory);
        }
        pending.extend(children);
    }
    Ok(())
}

fn scan_album(directory: &Path) -> Result<InboxAlbum, String> {
    scan_album_cached(directory, &mut HashMap::new(), &mut HashSet::new())
}

fn scan_album_cached(
    directory: &Path,
    quality_cache: &mut HashMap<PathBuf, CachedAudioQuality>,
    active_tracks: &mut HashSet<PathBuf>,
) -> Result<InboxAlbum, String> {
    let canonical = fs::canonicalize(directory).map_err(|error| error.to_string())?;
    let mut tracks = fs::read_dir(&canonical)
        .map_err(|error| error.to_string())?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_inbox_audio(path))
        .map(|path| scan_track(&path, quality_cache, active_tracks))
        .collect::<Result<Vec<_>, _>>()?;
    tracks.sort_by(|left, right| {
        left.disc_number
            .cmp(&right.disc_number)
            .then_with(|| left.track_number.cmp(&right.track_number))
            .then_with(|| left.file_name.cmp(&right.file_name))
    });
    let mut tags = Vec::new();
    let mut front_cover_source = None;
    let mut fallback_cover_source = None;
    let mut artwork_track_count = 0usize;
    let mut artwork_digests = HashSet::new();
    let mut artwork_validation_cache = HashMap::new();
    let mut duplicate_front_cover = false;
    let mut modified_at_ms = 0;
    for track in &tracks {
        let path = Path::new(&track.path);
        if !is_mp3(path) {
            tags.push(EditableTagValues::default());
            modified_at_ms = modified_at_ms.max(
                fs::metadata(path)
                    .ok()
                    .and_then(|value| value.modified().ok())
                    .and_then(system_time_ms)
                    .unwrap_or(0),
            );
            continue;
        }
        let (tag, _) = read_tag_for_write(path)?;
        let front_pictures = tag
            .pictures()
            .filter(|picture| picture.picture_type == PictureType::CoverFront)
            .collect::<Vec<_>>();
        if front_pictures.len() > 1 {
            duplicate_front_cover = true;
        }
        if let Some(digest) = front_pictures
            .iter()
            .find_map(|picture| cached_cover_digest(picture, &mut artwork_validation_cache))
        {
            artwork_track_count += 1;
            artwork_digests.insert(digest);
            front_cover_source.get_or_insert_with(|| track.path.clone());
        }
        if fallback_cover_source.is_none()
            && tag.pictures().any(|picture| {
                cached_cover_digest(picture, &mut artwork_validation_cache).is_some()
            })
        {
            fallback_cover_source = Some(track.path.clone());
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
    let formats = {
        let mut values = tracks
            .iter()
            .map(|track| track.format.clone())
            .collect::<Vec<_>>();
        values.sort();
        values.dedup();
        values
    };
    let total_size_bytes = tracks.iter().map(|track| track.size_bytes).sum();
    let duration_ms = tracks.iter().filter_map(|track| track.duration_ms).sum();
    let bitrates = tracks
        .iter()
        .filter_map(|track| track.bitrate_kbps)
        .collect::<Vec<_>>();
    let avg_bitrate_kbps = (!bitrates.is_empty()).then(|| {
        (bitrates.iter().map(|value| u64::from(*value)).sum::<u64>() / bitrates.len() as u64) as u32
    });
    let audio_scan_error_count = tracks
        .iter()
        .filter(|track| track.scan_error.is_some())
        .count();
    let lossless_track_count = tracks
        .iter()
        .filter(|track| !track.format.eq_ignore_ascii_case("MP3"))
        .count();
    let inconsistent_artwork = artwork_digests.len() > 1;
    let artwork_ready = lossless_track_count == 0
        && artwork_track_count == tracks.len()
        && !duplicate_front_cover
        && !inconsistent_artwork;
    let artwork_source_path = front_cover_source.or(fallback_cover_source);
    let artwork_present = artwork_source_path.is_some();
    let mut issues = Vec::new();
    if lossless_track_count > 0 {
        issues.push(format!(
            "Convert {lossless_track_count} FLAC/APE {} to 320 kbps MP3",
            if lossless_track_count == 1 {
                "track"
            } else {
                "tracks"
            }
        ));
    }
    if lossless_track_count == 0 {
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
        if year.is_none() || year.is_some_and(|value| value <= 0) {
            issues.push("Year is missing or inconsistent".to_owned());
        }
        if audio_scan_error_count > 0 {
            issues.push(format!(
                "Audio properties could not be read from {audio_scan_error_count} {}",
                if audio_scan_error_count == 1 {
                    "track"
                } else {
                    "tracks"
                }
            ));
        }
        let missing_artwork_tracks = tracks.len().saturating_sub(artwork_track_count);
        if missing_artwork_tracks == tracks.len() {
            issues.push("Embedded front cover is missing".to_owned());
        } else if missing_artwork_tracks > 0 {
            issues.push(format!(
                "Embedded front cover is missing or invalid on {missing_artwork_tracks} {}",
                if missing_artwork_tracks == 1 {
                    "track"
                } else {
                    "tracks"
                }
            ));
        }
        if duplicate_front_cover {
            issues.push("One or more tracks contain multiple embedded front covers".to_owned());
        }
        if inconsistent_artwork {
            issues.push("Embedded front covers are inconsistent".to_owned());
        }
        issues.extend(organization_issues(
            &canonical,
            artist.as_deref(),
            album.as_deref(),
            year,
            &tracks,
        ));
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
        formats,
        total_size_bytes,
        avg_bitrate_kbps,
        duration_ms,
        audio_scan_error_count,
        lossless_track_count,
        artwork_present,
        artwork_source_path,
        artwork_track_count,
        artwork_ready,
        modified_at_ms,
        readiness: InboxReadiness {
            ready: issues.is_empty(),
            issues,
        },
        tracks,
    })
}

fn organization_issues(
    album_path: &Path,
    album_artist: Option<&str>,
    album_title: Option<&str>,
    year: Option<i32>,
    tracks: &[InboxTrack],
) -> Vec<String> {
    let mut issues = Vec::new();
    if let (Some(album_artist), Some(album_title), Some(year)) =
        (album_artist, album_title, year.filter(|value| *value > 0))
        && let Ok(expected) =
            sanitize_component(&format!("{album_artist} - {album_title} ({year})"))
        && album_path
            .file_name()
            .and_then(|value| value.to_str())
            .is_none_or(|actual| !actual.eq_ignore_ascii_case(&expected))
    {
        issues.push("Album folder is not organized as Album Artist - Album (Year)".to_owned());
    }

    let track_width = tracks
        .iter()
        .filter_map(|track| track.track_number.or(track.track_total))
        .max()
        .map_or(2, decimal_width)
        .max(2);
    let filenames_ready = tracks.iter().all(|track| {
        let (Some(track_number), Some(artist), Some(title)) = (
            track.track_number,
            track.artist.as_deref(),
            track.title.as_deref(),
        ) else {
            return true;
        };
        let position = match track.disc_number {
            Some(disc) => format!("{disc}-{track_number:0track_width$}"),
            None => format!("{track_number:0track_width$}"),
        };
        sanitize_component(&format!("{position} - {artist} - {title}"))
            .is_ok_and(|stem| track.file_name.eq_ignore_ascii_case(&format!("{stem}.mp3")))
    });
    if !filenames_ready {
        issues.push("One or more track filenames are not organized from their tags".to_owned());
    }
    issues
}

fn scan_track(
    path: &Path,
    quality_cache: &mut HashMap<PathBuf, CachedAudioQuality>,
    active_tracks: &mut HashSet<PathBuf>,
) -> Result<InboxTrack, String> {
    let canonical = fs::canonicalize(path).map_err(|error| error.to_string())?;
    active_tracks.insert(canonical.clone());
    let quality = inspect_audio_quality(&canonical, quality_cache);
    let values = if is_mp3(&canonical) {
        let (tag, _) = read_tag_for_write(&canonical)?;
        read_editable_tag_values(&tag)?
    } else {
        EditableTagValues::default()
    };
    Ok(InboxTrack {
        path: path_text(&canonical)?,
        file_name: canonical
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("track")
            .to_owned(),
        format: audio_format(&canonical).unwrap_or("Unknown").to_owned(),
        size_bytes: quality.size_bytes,
        bitrate_kbps: quality.bitrate_kbps,
        duration_ms: quality.duration_ms,
        scan_error: quality.scan_error,
        album_artist: values.album_artist,
        title: values.title,
        artist: values.artist,
        album: values.album,
        genre: values.genre,
        publisher: values.publisher,
        rating: values.rating,
        year: values.year,
        release_year: values.release_year,
        track_number: values.track_number,
        track_total: values.track_total,
        disc_number: values.disc_number,
        disc_total: values.disc_total,
    })
}

fn inspect_audio_quality(
    path: &Path,
    cache: &mut HashMap<PathBuf, CachedAudioQuality>,
) -> CachedAudioQuality {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => {
            return CachedAudioQuality {
                size_bytes: 0,
                modified_ns: 0,
                bitrate_kbps: None,
                duration_ms: None,
                scan_error: Some(format!("Could not read file metadata: {error}")),
            };
        }
    };
    let size_bytes = metadata.len();
    let modified_ns = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_nanos().min(u64::MAX as u128) as u64)
        .unwrap_or_default();
    if let Some(cached) = cache.get(path)
        && cached.size_bytes == size_bytes
        && cached.modified_ns == modified_ns
    {
        return cached.clone();
    }
    let inspected = (|| {
        let options = ParseOptions::new()
            .read_properties(true)
            .read_tags(false)
            .read_cover_art(false);
        let tagged = Probe::open(path)
            .map_err(|error| error.to_string())?
            .options(options)
            .read()
            .map_err(|error| error.to_string())?;
        let properties = tagged.properties();
        Ok::<_, String>((
            properties
                .audio_bitrate()
                .or_else(|| properties.overall_bitrate()),
            u64::try_from(properties.duration().as_millis()).ok(),
        ))
    })();
    let quality = match inspected {
        Ok((bitrate_kbps, duration_ms)) => CachedAudioQuality {
            size_bytes,
            modified_ns,
            bitrate_kbps,
            duration_ms,
            scan_error: (size_bytes == 0).then(|| "The MP3 file is empty".to_owned()),
        },
        Err(error) => CachedAudioQuality {
            size_bytes,
            modified_ns,
            bitrate_kbps: None,
            duration_ms: None,
            scan_error: Some(error),
        },
    };
    cache.insert(path.to_path_buf(), quality.clone());
    quality
}

fn search_musicbrainz(request: &ReleaseSearchRequest) -> Result<Vec<ReleaseCandidate>, String> {
    wait_for_musicbrainz();
    let query = format!(
        "artist:\"{}\" AND release:\"{}\"",
        escape_lucene(&request.artist),
        escape_lucene(&request.album),
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
                original_year: row
                    .get("release-group")
                    .and_then(|group| text(group, "first-release-date"))
                    .and_then(|value| parse_year(&value)),
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
            Some(ReleaseCandidate {
                source: MetadataSource::Discogs,
                id,
                score: 85,
                title,
                artist,
                year: number(row, "year").map(|value| value as i32),
                original_year: None,
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
        original_year: value
            .get("release-group")
            .and_then(|group| text(group, "first-release-date"))
            .and_then(|value| parse_year(&value)),
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
                track_number: Some(normalize_discogs_track_number(position, index, total)),
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
    let original_year = discogs_master_year(&value, &auth).ok().flatten();
    let candidate = ReleaseCandidate {
        source: MetadataSource::Discogs,
        id: id.to_owned(),
        score: 100,
        title: text(&value, "title").unwrap_or_default(),
        artist,
        year: number(&value, "year").map(|value| value as i32),
        original_year,
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

fn discogs_master_year(value: &Value, auth: &DiscogsAuth) -> Result<Option<i32>, String> {
    let Some(master_id) = number(value, "master_id").filter(|value| *value > 0) else {
        return Ok(None);
    };
    let url = format!("https://api.discogs.com/masters/{master_id}");
    let response = http_client()?
        .get(url)
        .header(reqwest::header::AUTHORIZATION, discogs_authorization(auth))
        .send()
        .map_err(|_| "Aurora could not connect to the Discogs master release.".to_owned())?;
    let status = response.status();
    let master: Value = response
        .error_for_status()
        .map_err(|_| format!("Discogs master lookup failed with HTTP {status}."))?
        .json()
        .map_err(|error| format!("Discogs returned invalid master data: {error}"))?;
    Ok(number(&master, "year").map(|year| year as i32))
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
fn is_lossless_source(path: &Path) -> bool {
    audio_format(path).is_some_and(|format| matches!(format, "FLAC" | "APE"))
}
fn is_inbox_audio(path: &Path) -> bool {
    !path
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.starts_with(".aurora-"))
        && audio_format(path).is_some()
}
fn audio_format(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "mp3" => Some("MP3"),
        "flac" => Some("FLAC"),
        "ape" => Some("APE"),
        _ => None,
    }
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
fn track_count_difference(candidate: Option<u32>, requested: Option<u32>) -> u32 {
    match (candidate, requested) {
        (Some(candidate), Some(requested)) => candidate.abs_diff(requested),
        _ => u32::MAX,
    }
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
                    let name = row.get("name").and_then(Value::as_str).or_else(|| {
                        row.get("artist")
                            .and_then(|artist| artist.get("name"))
                            .and_then(Value::as_str)
                    })?;
                    let join_phrase = row
                        .get("joinphrase")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    Some(format!("{name}{join_phrase}"))
                })
                .collect::<Vec<_>>()
                .join("")
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
fn normalize_discogs_track_number(position: Option<u32>, index: usize, total: u32) -> u32 {
    position
        .filter(|position| (1..=total).contains(position))
        .unwrap_or((index + 1) as u32)
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
    use std::io::Cursor;

    #[test]
    fn discogs_positions_support_multidisc_and_plain_tracks() {
        assert_eq!(parse_disc_track("2-03"), (Some(2), Some(3)));
        assert_eq!(parse_disc_track("3"), (None, Some(3)));
        assert_eq!(parse_disc_track("A1"), (None, None));
        assert_eq!(parse_disc_track("B1"), (None, None));
    }

    #[test]
    fn discogs_positions_outside_the_track_total_follow_release_order() {
        assert_eq!(normalize_discogs_track_number(Some(12), 11, 13), 12);
        assert_eq!(normalize_discogs_track_number(Some(41), 12, 13), 13);
        assert_eq!(normalize_discogs_track_number(Some(0), 0, 13), 1);
        assert_eq!(normalize_discogs_track_number(None, 4, 13), 5);
    }

    #[test]
    fn rename_components_are_windows_safe_and_track_width_is_at_least_two() {
        assert_eq!(sanitize_component("AC/DC: Live").unwrap(), "AC_DC_ Live");
        assert_eq!(sanitize_component("CON").unwrap(), "_CON");
        assert_eq!(decimal_width(9).max(2), 2);
        assert_eq!(decimal_width(101).max(2), 3);
    }

    #[test]
    fn inbox_audio_detection_accepts_mp3_flac_and_ape_only() {
        assert!(is_inbox_audio(Path::new("track.mp3")));
        assert!(is_inbox_audio(Path::new("track.FLAC")));
        assert!(is_inbox_audio(Path::new("track.Ape")));
        assert!(!is_inbox_audio(Path::new("track.wav")));
        assert!(!is_inbox_audio(Path::new(".aurora-convert.tmp.mp3")));
    }

    #[test]
    fn converting_flac_installs_verified_mp3_before_deleting_source() {
        let Ok(ffmpeg) = discover_ffmpeg_executable() else {
            return;
        };
        let root = tempfile::tempdir().expect("temporary Inbox root");
        let album = root.path().join("Lossless Album");
        fs::create_dir(&album).expect("album folder");
        let source = album.join("01 - Test Track.flac");
        let generated = hidden_command(&ffmpeg)
            .args([
                "-nostdin",
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=1",
                "-c:a",
                "flac",
                "-metadata",
                "album_artist=Test Artist",
                "-metadata",
                "artist=Test Artist",
                "-metadata",
                "album=Test Album",
                "-metadata",
                "title=Test Track",
                "-metadata",
                "date=1990",
                "-metadata",
                "track=1/1",
            ])
            .arg(&source)
            .stdin(Stdio::null())
            .status()
            .expect("generate FLAC fixture");
        assert!(generated.success());

        let before = scan_album(&album).expect("scan lossless album");
        assert_eq!(before.lossless_track_count, 1);
        assert_eq!(before.formats, vec!["FLAC"]);
        assert_eq!(
            before.readiness.issues,
            vec!["Convert 1 FLAC/APE track to 320 kbps MP3"]
        );

        let result = convert_lossless_album(
            InboxConvertRequest {
                album_path: path_text(&album).expect("album path"),
            },
            &[root.path().to_path_buf()],
        )
        .expect("convert FLAC album");
        assert!(result.failures.is_empty(), "{:?}", result.failures);
        assert_eq!(result.converted_tracks, 1);
        assert_eq!(result.deleted_sources, 1);
        assert!(!source.exists());

        let destination = album.join("01 - Test Track.mp3");
        assert!(destination.is_file());
        let quality = inspect_audio_quality(&destination, &mut HashMap::new());
        assert!(
            quality
                .bitrate_kbps
                .is_some_and(|bitrate| (300..=340).contains(&bitrate))
        );
        assert!(quality.duration_ms.is_some_and(|duration| duration > 0));
        let after = scan_album(&album).expect("scan converted album");
        assert_eq!(after.lossless_track_count, 0);
        assert_eq!(after.formats, vec!["MP3"]);
        assert_eq!(after.artist.as_deref(), Some("Test Artist"));
        assert_eq!(after.album.as_deref(), Some("Test Album"));
        assert_eq!(after.year, Some(1990));
        assert_eq!(after.tracks[0].title.as_deref(), Some("Test Track"));
    }

    #[test]
    fn conversion_never_overwrites_an_existing_mp3() {
        let root = tempfile::tempdir().expect("temporary Inbox root");
        let source = root.path().join("track.flac");
        let destination = root.path().join("track.mp3");
        fs::write(&source, b"lossless source").expect("source fixture");
        fs::write(&destination, b"existing mp3").expect("destination fixture");

        let error = convert_lossless_track(
            Path::new("missing-ffmpeg"),
            &source,
            root.path(),
            now_ms(),
            0,
        )
        .expect_err("collision must fail");

        assert!(error.contains("already exists"));
        assert_eq!(fs::read(&source).expect("source kept"), b"lossless source");
        assert_eq!(fs::read(&destination).expect("MP3 kept"), b"existing mp3");
    }

    #[test]
    fn musicbrainz_artist_credit_preserves_join_phrases() {
        let value = serde_json::json!({
            "artist-credit": [
                { "name": "X‐Ecutioners", "joinphrase": " featuring " },
                { "name": "Large Professor", "joinphrase": "" }
            ]
        });

        assert_eq!(
            artist_credit(&value).as_deref(),
            Some("X‐Ecutioners featuring Large Professor")
        );
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
        let organized = scan_album(&renamed).expect("scan organized album");
        assert!(
            organized
                .readiness
                .issues
                .iter()
                .all(|issue| !issue.contains("not organized"))
        );
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
    fn batch_rename_flattens_selected_disc_folders_into_one_album() {
        let monitored_root = std::env::temp_dir().join(format!(
            "aurora-inbox-disc-folders-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let incoming = monitored_root.join("Test Album with CD folders");
        let cd1 = incoming.join("CD1");
        let cd2 = incoming.join("CD2");
        fs::create_dir_all(&cd1).expect("create CD1");
        fs::create_dir_all(&cd2).expect("create CD2");
        File::create(incoming.join("cover.jpg")).expect("create cover");
        write_rename_fixture(&cd1.join("one.mp3"), 1, Some(1), "Disc One First");
        write_rename_fixture(&cd1.join("two.mp3"), 2, Some(1), "Disc One Second");
        write_rename_fixture(&cd2.join("one.mp3"), 1, Some(2), "Disc Two First");
        write_rename_fixture(&cd2.join("two.mp3"), 2, Some(2), "Disc Two Second");

        let result = rename_albums(
            InboxBatchRenameRequest {
                album_paths: vec![
                    path_text(&cd1).expect("CD1 path"),
                    path_text(&cd2).expect("CD2 path"),
                ],
            },
            std::slice::from_ref(&monitored_root),
        )
        .expect("rename selected discs");

        let renamed = monitored_root.join("Test Artist - Test Album (1990)");
        assert_eq!(result.renamed_albums, 2);
        assert_eq!(result.renamed_tracks, 4);
        assert_eq!(result.renamed_folders, 1);
        assert!(result.failures.is_empty());
        assert!(
            renamed
                .join("1-01 - Track Artist - Disc One First.mp3")
                .is_file()
        );
        assert!(
            renamed
                .join("1-02 - Track Artist - Disc One Second.mp3")
                .is_file()
        );
        assert!(
            renamed
                .join("2-01 - Track Artist - Disc Two First.mp3")
                .is_file()
        );
        assert!(
            renamed
                .join("2-02 - Track Artist - Disc Two Second.mp3")
                .is_file()
        );
        assert!(renamed.join("cover.jpg").is_file());
        assert!(!renamed.join("CD1").exists());
        assert!(!renamed.join("CD2").exists());
        assert_eq!(
            fs::read_dir(&renamed)
                .expect("read merged album")
                .filter_map(Result::ok)
                .filter(|entry| entry.path().is_dir())
                .count(),
            0
        );
        fs::remove_dir_all(monitored_root).expect("remove fixture");
    }

    #[test]
    fn batch_rename_recovers_a_partially_nested_multidisc_album() {
        let monitored_root = std::env::temp_dir().join(format!(
            "aurora-inbox-partial-disc-rename-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let incoming = monitored_root.join("Old multi-disc folder");
        let cd1 = incoming.join("CD1");
        let wrongly_nested = incoming.join("Test Artist - Test Album (1990)");
        fs::create_dir_all(&cd1).expect("create CD1");
        fs::create_dir_all(&wrongly_nested).expect("create partial rename");
        write_rename_fixture(&cd1.join("one.mp3"), 1, Some(1), "Disc One");
        write_rename_fixture(&wrongly_nested.join("two.mp3"), 1, Some(2), "Disc Two");

        let result = rename_albums(
            InboxBatchRenameRequest {
                album_paths: vec![
                    path_text(&cd1).expect("CD1 path"),
                    path_text(&wrongly_nested).expect("nested path"),
                ],
            },
            std::slice::from_ref(&monitored_root),
        )
        .expect("recover partial rename");

        let renamed = monitored_root.join("Test Artist - Test Album (1990)");
        assert_eq!(result.renamed_albums, 2);
        assert_eq!(result.renamed_tracks, 2);
        assert!(renamed.join("1-01 - Track Artist - Disc One.mp3").is_file());
        assert!(renamed.join("2-01 - Track Artist - Disc Two.mp3").is_file());
        assert_eq!(
            fs::read_dir(&renamed)
                .expect("read recovered album")
                .filter_map(Result::ok)
                .filter(|entry| entry.path().is_dir())
                .count(),
            0
        );
        fs::remove_dir_all(monitored_root).expect("remove fixture");
    }

    #[test]
    fn inbox_artwork_uses_any_track_as_a_repair_source_but_requires_every_track() {
        let parent = std::env::temp_dir().join(format!(
            "aurora-inbox-artwork-{}-{}",
            std::process::id(),
            now_ms()
        ));
        fs::create_dir_all(&parent).expect("create album");
        write_artwork_fixture(&parent.join("second.mp3"), 2, Some(1));
        write_artwork_fixture(&parent.join("first.mp3"), 1, None);

        let album = scan_album(&parent).expect("scan album");
        assert!(album.artwork_present);
        assert!(
            album
                .artwork_source_path
                .as_deref()
                .is_some_and(|path| path.ends_with("second.mp3"))
        );
        assert_eq!(album.artwork_track_count, 1);
        assert!(!album.artwork_ready);
        assert!(
            album
                .readiness
                .issues
                .iter()
                .any(|issue| { issue == "Embedded front cover is missing or invalid on 1 track" })
        );

        fs::remove_dir_all(parent).expect("remove fixture");
    }

    #[test]
    fn embedded_cover_repair_updates_every_track_and_preserves_audio() {
        let parent = std::env::temp_dir().join(format!(
            "aurora-inbox-cover-repair-{}-{}",
            std::process::id(),
            now_ms()
        ));
        fs::create_dir_all(&parent).expect("create album");
        let first = parent.join("first.mp3");
        let second = parent.join("second.mp3");
        write_artwork_fixture(&first, 1, None);
        write_artwork_fixture(&second, 2, Some(7));
        let first_audio = audio_payload_hash(&first).expect("first audio before");
        let second_audio = audio_payload_hash(&second).expect("second audio before");

        let result = embed_album_cover(
            InboxCoverEmbedRequest {
                album_path: path_text(&parent).expect("album path"),
                image_path: None,
            },
            std::slice::from_ref(&parent),
        )
        .expect("embed cover");

        assert_eq!(result.changed_tracks, 1);
        assert_eq!(result.track_count, 2);
        let album = scan_album(&parent).expect("scan repaired album");
        assert!(album.artwork_ready);
        assert_eq!(album.artwork_track_count, 2);
        let first_tag = Tag::read_from_path(&first).expect("first tag");
        let second_tag = Tag::read_from_path(&second).expect("second tag");
        let first_cover = first_tag
            .pictures()
            .find(|picture| picture.picture_type == PictureType::CoverFront)
            .expect("first cover");
        let second_cover = second_tag
            .pictures()
            .find(|picture| picture.picture_type == PictureType::CoverFront)
            .expect("second cover");
        assert_eq!(first_cover.data, second_cover.data);
        assert_eq!(
            audio_payload_hash(&first).expect("first audio after"),
            first_audio
        );
        assert_eq!(
            audio_payload_hash(&second).expect("second audio after"),
            second_audio
        );
        fs::remove_dir_all(parent).expect("remove fixture");
    }

    #[test]
    fn manual_tag_cover_save_updates_the_complete_album_from_one_selected_track() {
        let parent = std::env::temp_dir().join(format!(
            "aurora-inbox-manual-cover-{}-{}",
            std::process::id(),
            now_ms()
        ));
        fs::create_dir_all(&parent).expect("create album");
        let first = parent.join("first.mp3");
        let second = parent.join("second.mp3");
        let image = parent.join("replacement.png");
        write_artwork_fixture(&first, 1, Some(1));
        write_artwork_fixture(&second, 2, Some(2));
        fs::write(&image, cover_fixture_bytes(9)).expect("write replacement cover");
        let first_audio = audio_payload_hash(&first).expect("first audio before");
        let second_audio = audio_payload_hash(&second).expect("second audio before");
        let cover = canonical_cover_from_image(&image).expect("selected cover");

        let result = apply_tags(
            InboxTagApplyRequest {
                album_path: path_text(&parent).expect("album path"),
                fields: Vec::new(),
                tracks: vec![InboxTrackPatch {
                    path: path_text(&first).expect("track path"),
                    values: EditableTagValues::default(),
                }],
                rename_after_apply: false,
                remove_track_paths: Vec::new(),
                artwork_token: Some("fixture-cover".to_owned()),
            },
            Some(&cover),
            &parent.join("recovery"),
        )
        .expect("save album cover");

        assert_eq!(result.changed_tracks, 2);
        for track in [&first, &second] {
            let tag = Tag::read_from_path(track).expect("read updated tag");
            let picture = tag
                .pictures()
                .find(|picture| picture.picture_type == PictureType::CoverFront)
                .expect("front cover");
            assert_eq!(cover_digest(picture).expect("cover digest"), cover.digest);
        }
        assert_eq!(
            audio_payload_hash(&first).expect("first audio after"),
            first_audio
        );
        assert_eq!(
            audio_payload_hash(&second).expect("second audio after"),
            second_audio
        );
        fs::remove_dir_all(parent).expect("remove fixture");
    }

    #[test]
    fn tag_apply_moves_reviewed_extra_tracks_to_verified_recovery() {
        let root = std::env::temp_dir().join(format!(
            "aurora-inbox-extra-recovery-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let album = root.join("Animal Man");
        let recovery = root.join("recovery");
        fs::create_dir_all(&album).expect("create album");
        let retained = album.join("01 Progress.mp3");
        let extra = album.join("02 Bonus.mp3");
        write_artwork_fixture(&retained, 1, Some(1));
        write_artwork_fixture(&extra, 2, Some(1));
        let extra_hash = file_sha256(&extra).expect("extra hash");

        let result = apply_tags(
            InboxTagApplyRequest {
                album_path: path_text(&album).expect("album path"),
                fields: vec![EditableTagField::Genre],
                tracks: vec![InboxTrackPatch {
                    path: path_text(&retained).expect("track path"),
                    values: EditableTagValues {
                        genre: Some("Heavy Metal".to_owned()),
                        ..EditableTagValues::default()
                    },
                }],
                rename_after_apply: false,
                remove_track_paths: vec![path_text(&extra).expect("extra path")],
                artwork_token: None,
            },
            None,
            &recovery,
        )
        .expect("apply tags and recover extra");

        assert_eq!(result.changed_tracks, 1);
        assert_eq!(result.removed_tracks, 1);
        assert!(!extra.exists());
        let recovery_path = PathBuf::from(result.recovery_path.expect("recovery path"));
        let recovered = recovery_path.join("02 Bonus.mp3");
        assert!(recovered.is_file());
        assert_eq!(file_sha256(&recovered).expect("recovered hash"), extra_hash);
        assert_eq!(
            Tag::read_from_path(&retained)
                .expect("retained tag")
                .genre()
                .map(str::to_owned),
            Some("Heavy Metal".to_owned())
        );
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn inconsistent_embedded_front_covers_block_readiness() {
        let parent = std::env::temp_dir().join(format!(
            "aurora-inbox-cover-mismatch-{}-{}",
            std::process::id(),
            now_ms()
        ));
        fs::create_dir_all(&parent).expect("create album");
        write_artwork_fixture(&parent.join("first.mp3"), 1, Some(1));
        write_artwork_fixture(&parent.join("second.mp3"), 2, Some(2));

        let album = scan_album(&parent).expect("scan album");
        assert_eq!(album.artwork_track_count, 2);
        assert!(!album.artwork_ready);
        assert!(
            album
                .readiness
                .issues
                .contains(&"Embedded front covers are inconsistent".to_owned())
        );
        fs::remove_dir_all(parent).expect("remove fixture");
    }

    fn cover_fixture_bytes(red: u8) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([red, 0, 0, 255]));
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .expect("encode cover");
        bytes.into_inner()
    }

    fn write_artwork_fixture(path: &Path, track: u32, picture_seed: Option<u8>) {
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
        if let Some(seed) = picture_seed {
            tag.add_frame(Picture {
                mime_type: "image/png".to_owned(),
                picture_type: PictureType::CoverFront,
                description: String::new(),
                data: cover_fixture_bytes(seed),
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
