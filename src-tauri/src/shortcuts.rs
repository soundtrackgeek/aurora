use crate::{
    GlobalShortcutState, PlaybackState, TagState,
    catalog::TrackSummary,
    state_sync,
    tag_model::{LoveState, TagEditRequest, TagValues},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

const SETTINGS_VERSION: u8 = 2;
const LEGACY_SETTINGS_VERSION: u8 = 1;
pub(crate) const RESULT_EVENT: &str = "aurora-global-shortcut-result";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShortcutAction {
    PlayPause,
    Next,
    Rating(u8),
    Love,
}

#[derive(Clone, Copy)]
struct BindingDefinition {
    action: ShortcutAction,
    action_key: &'static str,
    label: &'static str,
    default_accelerator: &'static str,
}

const DEFINITIONS: [BindingDefinition; 9] = [
    BindingDefinition {
        action: ShortcutAction::PlayPause,
        action_key: "playPause",
        label: "Play or pause",
        default_accelerator: "Ctrl+Alt+P",
    },
    BindingDefinition {
        action: ShortcutAction::Next,
        action_key: "next",
        label: "Next track",
        default_accelerator: "Ctrl+Alt+N",
    },
    BindingDefinition {
        action: ShortcutAction::Rating(0),
        action_key: "rating0",
        label: "Clear rating",
        default_accelerator: "Ctrl+Alt+Numpad0",
    },
    BindingDefinition {
        action: ShortcutAction::Rating(1),
        action_key: "rating1",
        label: "Rate 1 star",
        default_accelerator: "Ctrl+Alt+Numpad1",
    },
    BindingDefinition {
        action: ShortcutAction::Rating(2),
        action_key: "rating2",
        label: "Rate 2 stars",
        default_accelerator: "Ctrl+Alt+Numpad2",
    },
    BindingDefinition {
        action: ShortcutAction::Rating(3),
        action_key: "rating3",
        label: "Rate 3 stars",
        default_accelerator: "Ctrl+Alt+Numpad3",
    },
    BindingDefinition {
        action: ShortcutAction::Rating(4),
        action_key: "rating4",
        label: "Rate 4 stars",
        default_accelerator: "Ctrl+Alt+Numpad4",
    },
    BindingDefinition {
        action: ShortcutAction::Rating(5),
        action_key: "rating5",
        label: "Rate 5 stars",
        default_accelerator: "Ctrl+Alt+Numpad5",
    },
    BindingDefinition {
        action: ShortcutAction::Love,
        action_key: "love",
        label: "Toggle Love",
        default_accelerator: "Ctrl+Alt+L",
    },
];

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredBinding {
    action: String,
    accelerator: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ShortcutSettingsFile {
    version: u8,
    enabled: bool,
    bindings: Vec<StoredBinding>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ShortcutBindingInput {
    action: String,
    accelerator: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GlobalShortcutSettingsRequest {
    enabled: bool,
    bindings: Vec<ShortcutBindingInput>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShortcutBinding {
    action: &'static str,
    label: &'static str,
    accelerator: String,
    default_accelerator: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GlobalShortcutStatus {
    enabled: bool,
    registered: bool,
    platform_available: bool,
    error: Option<String>,
    warning: Option<String>,
    bindings: Vec<ShortcutBinding>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GlobalShortcutResult {
    action: String,
    success: bool,
    message: String,
    track: Option<TrackSummary>,
    previous_track: Option<TrackSummary>,
}

pub(crate) struct GlobalShortcutRuntime {
    path: PathBuf,
    enabled: bool,
    bindings: Vec<StoredBinding>,
    registered: bool,
    error: Option<String>,
    warning: Option<String>,
}

impl GlobalShortcutRuntime {
    pub(crate) fn load(path: PathBuf) -> Self {
        let mut warning = None;
        let mut needs_persist = false;
        let defaults = default_bindings();
        let (enabled, bindings) = if path.is_file() {
            match fs::read_to_string(&path)
                .map_err(|error| error.to_string())
                .and_then(|json| {
                    serde_json::from_str::<ShortcutSettingsFile>(&json)
                        .map_err(|error| error.to_string())
                }) {
                Ok(settings) if settings.version == SETTINGS_VERSION => {
                    match validate_bindings(settings.bindings) {
                        Ok(bindings) => (settings.enabled, bindings),
                        Err(error) => {
                            warning = Some(format!(
                                "Aurora found invalid shortcut settings and used the defaults: {error}"
                            ));
                            (true, defaults)
                        }
                    }
                }
                Ok(settings) if settings.version == LEGACY_SETTINGS_VERSION => {
                    match validate_bindings(settings.bindings) {
                        Ok(bindings) => {
                            needs_persist = true;
                            let (bindings, moved_rating_defaults) =
                                migrate_legacy_rating_defaults(bindings);
                            if moved_rating_defaults {
                                warning = Some(
                                    "Aurora moved the default rating shortcuts to the numeric keypad so AltGr characters remain available."
                                        .to_owned(),
                                );
                            }
                            (settings.enabled, bindings)
                        }
                        Err(error) => {
                            warning = Some(format!(
                                "Aurora found invalid shortcut settings and used the defaults: {error}"
                            ));
                            (true, defaults)
                        }
                    }
                }
                Ok(_) => {
                    warning = Some(
                        "Aurora found an unsupported shortcut setting and used the defaults."
                            .to_owned(),
                    );
                    (true, defaults)
                }
                Err(error) => {
                    warning = Some(format!(
                        "Aurora could not read this device's shortcut settings and used the defaults: {error}"
                    ));
                    (true, defaults)
                }
            }
        } else {
            (true, defaults)
        };
        let mut runtime = Self {
            path,
            enabled,
            bindings,
            registered: false,
            error: None,
            warning,
        };
        if needs_persist && let Err(error) = runtime.persist() {
            runtime.warning = Some(format!(
                "Aurora moved the default rating shortcuts to the numeric keypad but could not persist the migration: {error}"
            ));
        }
        runtime
    }

    pub(crate) fn initialize(&mut self, app: &AppHandle) {
        if self.enabled
            && let Err(error) = register_bindings(app, &self.bindings)
        {
            self.error = Some(error);
            return;
        }
        self.registered = self.enabled;
    }

    pub(crate) fn release(&mut self, app: &AppHandle) -> Result<(), String> {
        if !self.registered {
            return Ok(());
        }
        app.global_shortcut().unregister_all().map_err(|error| {
            format!("Aurora could not release its global shortcuts during shutdown: {error}")
        })?;
        self.registered = false;
        Ok(())
    }

    pub(crate) fn status(&self) -> GlobalShortcutStatus {
        GlobalShortcutStatus {
            enabled: self.enabled,
            registered: self.registered,
            platform_available: true,
            error: self.error.clone(),
            warning: self.warning.clone(),
            bindings: DEFINITIONS
                .iter()
                .map(|definition| ShortcutBinding {
                    action: definition.action_key,
                    label: definition.label,
                    accelerator: self
                        .bindings
                        .iter()
                        .find(|binding| binding.action == definition.action_key)
                        .map(|binding| binding.accelerator.clone())
                        .unwrap_or_else(|| definition.default_accelerator.to_owned()),
                    default_accelerator: definition.default_accelerator,
                })
                .collect(),
        }
    }

    pub(crate) fn update(
        &mut self,
        app: &AppHandle,
        request: GlobalShortcutSettingsRequest,
    ) -> Result<GlobalShortcutStatus, String> {
        let requested = validate_bindings(
            request
                .bindings
                .into_iter()
                .map(|binding| StoredBinding {
                    action: binding.action,
                    accelerator: binding.accelerator,
                })
                .collect(),
        )?;
        let previous_enabled = self.enabled;
        let previous_bindings = self.bindings.clone();
        let previous_registered = self.registered;
        let previous_error = self.error.clone();
        let previous_warning = self.warning.clone();

        app.global_shortcut().unregister_all().map_err(|error| {
            format!("Aurora could not release its previous global shortcuts: {error}")
        })?;
        if request.enabled
            && let Err(error) = register_bindings(app, &requested)
        {
            let mut restore_error = None;
            if previous_registered {
                restore_error = register_bindings(app, &previous_bindings).err();
            }
            self.registered = previous_registered && restore_error.is_none();
            if let Some(restore_error) = restore_error {
                self.error = Some(format!(
                    "{error} Aurora also could not restore the previous shortcut set: {restore_error}"
                ));
                return Err(self.error.clone().unwrap_or(error));
            }
            return Err(error);
        }

        self.enabled = request.enabled;
        self.bindings = requested;
        self.registered = request.enabled;
        self.error = None;
        self.warning = None;
        if let Err(error) = self.persist() {
            let _ = app.global_shortcut().unregister_all();
            self.enabled = previous_enabled;
            self.bindings = previous_bindings;
            self.registered = false;
            self.error = previous_error;
            self.warning = previous_warning;
            if previous_registered && register_bindings(app, &self.bindings).is_ok() {
                self.registered = true;
            }
            return Err(error);
        }
        Ok(self.status())
    }

    fn action_for(&self, shortcut_value: &Shortcut) -> Option<ShortcutAction> {
        self.bindings.iter().find_map(|binding| {
            let parsed = Shortcut::from_str(&binding.accelerator).ok()?;
            if parsed != *shortcut_value {
                return None;
            }
            definition_for(&binding.action).map(|definition| definition.action)
        })
    }

    fn persist(&self) -> Result<(), String> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "Aurora's shortcut setting has no parent directory.".to_owned())?;
        fs::create_dir_all(parent).map_err(|error| {
            format!("Could not create Aurora's shortcut settings folder: {error}")
        })?;
        let json = serde_json::to_vec_pretty(&ShortcutSettingsFile {
            version: SETTINGS_VERSION,
            enabled: self.enabled,
            bindings: self.bindings.clone(),
        })
        .map_err(|error| format!("Could not encode Aurora's shortcut settings: {error}"))?;
        write_atomic(&self.path, &json)
    }
}

fn default_bindings() -> Vec<StoredBinding> {
    DEFINITIONS
        .iter()
        .map(|definition| StoredBinding {
            action: definition.action_key.to_owned(),
            accelerator: definition.default_accelerator.to_owned(),
        })
        .collect()
}

fn migrate_legacy_rating_defaults(mut bindings: Vec<StoredBinding>) -> (Vec<StoredBinding>, bool) {
    let mut migrated = false;
    for rating in 0..=5 {
        let action = format!("rating{rating}");
        let legacy = format!("Ctrl+Alt+{rating}");
        if let Some(binding) = bindings.iter_mut().find(|binding| {
            binding.action == action && binding.accelerator.eq_ignore_ascii_case(&legacy)
        }) {
            binding.accelerator = format!("Ctrl+Alt+Numpad{rating}");
            migrated = true;
        }
    }
    (bindings, migrated)
}

fn definition_for(action: &str) -> Option<&'static BindingDefinition> {
    DEFINITIONS
        .iter()
        .find(|definition| definition.action_key == action)
}

fn validate_bindings(bindings: Vec<StoredBinding>) -> Result<Vec<StoredBinding>, String> {
    if bindings.len() != DEFINITIONS.len() {
        return Err("Every Aurora shortcut must have exactly one binding.".to_owned());
    }
    let mut actions = HashSet::new();
    let mut accelerators = HashSet::new();
    let mut validated = Vec::with_capacity(DEFINITIONS.len());
    for definition in DEFINITIONS {
        let binding = bindings
            .iter()
            .find(|binding| binding.action == definition.action_key)
            .ok_or_else(|| format!("{} has no shortcut.", definition.label))?;
        if !actions.insert(binding.action.clone()) {
            return Err(format!("{} is listed more than once.", definition.label));
        }
        let accelerator = normalize_accelerator(&binding.accelerator)?;
        if !accelerators.insert(accelerator.to_ascii_lowercase()) {
            return Err(format!(
                "{accelerator} is assigned to more than one action."
            ));
        }
        Shortcut::from_str(&accelerator).map_err(|error| {
            format!("{accelerator} is not a supported global shortcut: {error}")
        })?;
        validated.push(StoredBinding {
            action: definition.action_key.to_owned(),
            accelerator,
        });
    }
    Ok(validated)
}

fn normalize_accelerator(value: &str) -> Result<String, String> {
    let tokens = value
        .split('+')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    if tokens.len() < 2 || value.len() > 64 {
        return Err("A global shortcut needs at least one modifier and one key.".to_owned());
    }
    let key_count = tokens
        .iter()
        .filter(|token| {
            !matches!(
                token.to_ascii_lowercase().as_str(),
                "ctrl" | "control" | "alt" | "shift" | "super" | "cmd" | "command"
            )
        })
        .count();
    if key_count != 1 {
        return Err("A global shortcut needs exactly one non-modifier key.".to_owned());
    }
    Ok(tokens.join("+"))
}

fn register_bindings(app: &AppHandle, bindings: &[StoredBinding]) -> Result<(), String> {
    for binding in bindings {
        if let Err(error) = app.global_shortcut().register(binding.accelerator.as_str()) {
            let _ = app.global_shortcut().unregister_all();
            return Err(format!(
                "{} is unavailable, probably because MusicBee or another app already registered it: {error}",
                binding.accelerator
            ));
        }
    }
    Ok(())
}

pub(crate) fn handle_shortcut(app: &AppHandle, shortcut_value: &Shortcut, state: ShortcutState) {
    if state != ShortcutState::Pressed {
        return;
    }
    let action = {
        let state = app.state::<GlobalShortcutState>();
        let Ok(runtime) = state.lock() else {
            return;
        };
        runtime.action_for(shortcut_value)
    };
    let Some(action) = action else {
        return;
    };
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = match execute_action(&app, action) {
            Ok(result) => result,
            Err(message) => GlobalShortcutResult {
                action: action_name(action).to_owned(),
                success: false,
                message,
                track: None,
                previous_track: None,
            },
        };
        let _ = app.emit(RESULT_EVENT, result);
    });
}

fn execute_action(app: &AppHandle, action: ShortcutAction) -> Result<GlobalShortcutResult, String> {
    match action {
        ShortcutAction::PlayPause | ShortcutAction::Next => {
            let state = app.state::<PlaybackState>();
            let mut playback = state
                .lock()
                .map_err(|_| "Aurora's playback engine stopped unexpectedly.".to_owned())?;
            let snapshot = if action == ShortcutAction::PlayPause {
                playback.toggle()?
            } else {
                playback.next()?
            };
            let message = match action {
                ShortcutAction::PlayPause => match snapshot.status {
                    crate::playback::PlaybackStatus::Playing => "Playback started",
                    _ => "Playback paused",
                },
                ShortcutAction::Next => "Playing next track",
                _ => unreachable!(),
            };
            Ok(GlobalShortcutResult {
                action: action_name(action).to_owned(),
                success: true,
                message: message.to_owned(),
                track: snapshot.current_track,
                previous_track: None,
            })
        }
        ShortcutAction::Rating(_) | ShortcutAction::Love => {
            let track = {
                let state = app.state::<PlaybackState>();
                let mut playback = state
                    .lock()
                    .map_err(|_| "Aurora's playback engine stopped unexpectedly.".to_owned())?;
                playback.snapshot().current_track.ok_or_else(|| {
                    "Start a track in Aurora before using rating or Love shortcuts.".to_owned()
                })?
            };
            let expected = track.catalog_tag_values();
            let desired = desired_tag_values(&track, action)
                .ok_or_else(|| "This shortcut does not edit track tags.".to_owned())?;
            let previous_track = track.clone();
            let updated = {
                let state = app.state::<TagState>();
                let service = state
                    .lock()
                    .map_err(|_| "Aurora's tag writer stopped unexpectedly.".to_owned())?;
                service.update(TagEditRequest {
                    track_id: track.id,
                    track_key: track.track_key,
                    expected,
                    desired,
                })?
            };
            {
                let state = app.state::<PlaybackState>();
                let mut playback = state
                    .lock()
                    .map_err(|_| "Aurora's playback engine stopped unexpectedly.".to_owned())?;
                playback.refresh_track_metadata(&updated.track);
            }
            let message = match action {
                ShortcutAction::Rating(0) => {
                    format!("Cleared rating for {}", updated.track.title)
                }
                ShortcutAction::Rating(rating) => {
                    format!("Rated {} {} stars", updated.track.title, rating)
                }
                ShortcutAction::Love if updated.track.love_state == LoveState::Loved => {
                    format!("Loved {}", updated.track.title)
                }
                ShortcutAction::Love => format!("Removed Love from {}", updated.track.title),
                _ => unreachable!(),
            };
            Ok(GlobalShortcutResult {
                action: action_name(action).to_owned(),
                success: true,
                message,
                track: Some(updated.track),
                previous_track: Some(previous_track),
            })
        }
    }
}

fn desired_tag_values(track: &TrackSummary, action: ShortcutAction) -> Option<TagValues> {
    let mut values = track.catalog_tag_values();
    match action {
        ShortcutAction::Rating(0) => values.rating = None,
        ShortcutAction::Rating(rating @ 1..=5) => values.rating = Some(f64::from(rating)),
        ShortcutAction::Love => {
            values.love_state = if values.love_state == LoveState::Loved {
                LoveState::Neutral
            } else {
                LoveState::Loved
            };
        }
        _ => return None,
    }
    Some(values)
}

fn action_name(action: ShortcutAction) -> &'static str {
    match action {
        ShortcutAction::PlayPause => "playPause",
        ShortcutAction::Next => "next",
        ShortcutAction::Rating(0) => "rating0",
        ShortcutAction::Rating(1) => "rating1",
        ShortcutAction::Rating(2) => "rating2",
        ShortcutAction::Rating(3) => "rating3",
        ShortcutAction::Rating(4) => "rating4",
        ShortcutAction::Rating(5) => "rating5",
        ShortcutAction::Rating(_) => "rating",
        ShortcutAction::Love => "love",
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Aurora's shortcut setting has no parent directory.".to_owned())?;
    let temporary = parent.join(format!(
        ".aurora-shortcuts-{}-{}.tmp",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let mut file = File::options()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| format!("Could not save Aurora's shortcut settings: {error}"))?;
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "Could not flush Aurora's shortcut settings: {error}"
        ));
    }
    drop(file);
    let result = if path.is_file() {
        state_sync::replace_file_atomic(path, &temporary)
    } else {
        fs::rename(&temporary, path)
            .map_err(|error| format!("Could not install Aurora's shortcut settings: {error}"))
    };
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(rating: Option<f64>, love_state: LoveState) -> TrackSummary {
        TrackSummary {
            id: "track-1".to_owned(),
            track_key: "d:\\music\\artist\\song.mp3".to_owned(),
            album_id: None,
            title: "Song".to_owned(),
            artist: "Artist".to_owned(),
            display_artist: None,
            album: "Album".to_owned(),
            release_year: Some(2026),
            original_year: Some(2026),
            publisher: None,
            rating,
            loved: love_state == LoveState::Loved,
            love_state,
            tag_sync_state: None,
            can_undo_tag_edit: false,
            duration_seconds: Some(180),
            genre: None,
            play_count: None,
            track_number: None,
            track_total: None,
            disc_number: None,
            disc_total: None,
            directory: r"D:\MUSIC\Artist".to_owned(),
            filename: "song.mp3".to_owned(),
            catalog_import_run_id: 1,
        }
    }

    #[test]
    fn defaults_match_the_requested_global_shortcuts() {
        let defaults = default_bindings();
        validate_bindings(defaults.clone()).expect("default shortcuts must parse");
        let accelerators = defaults
            .into_iter()
            .map(|binding| binding.accelerator)
            .collect::<Vec<_>>();
        assert_eq!(
            accelerators,
            [
                "Ctrl+Alt+P",
                "Ctrl+Alt+N",
                "Ctrl+Alt+Numpad0",
                "Ctrl+Alt+Numpad1",
                "Ctrl+Alt+Numpad2",
                "Ctrl+Alt+Numpad3",
                "Ctrl+Alt+Numpad4",
                "Ctrl+Alt+Numpad5",
                "Ctrl+Alt+L",
            ]
        );
    }

    #[test]
    fn duplicate_or_modifierless_custom_bindings_are_rejected() {
        let mut duplicate = default_bindings();
        duplicate[1].accelerator = duplicate[0].accelerator.clone();
        assert!(validate_bindings(duplicate).is_err());

        let mut modifierless = default_bindings();
        modifierless[0].accelerator = "P".to_owned();
        assert!(validate_bindings(modifierless).is_err());
    }

    #[test]
    fn rating_zero_clears_and_whole_star_shortcuts_preserve_other_tags() {
        let current = track(Some(4.5), LoveState::Loved);
        let cleared = desired_tag_values(&current, ShortcutAction::Rating(0)).expect("clear");
        assert_eq!(cleared.rating, None);
        assert_eq!(cleared.love_state, LoveState::Loved);
        assert_eq!(cleared.release_year, Some(2026));

        let rated = desired_tag_values(&current, ShortcutAction::Rating(3)).expect("rate");
        assert_eq!(rated.rating, Some(3.0));
        assert_eq!(rated.love_state, LoveState::Loved);
        assert_eq!(rated.release_year, Some(2026));
    }

    #[test]
    fn love_shortcut_toggles_the_current_tracks_love_state() {
        let loved = desired_tag_values(&track(Some(2.0), LoveState::Neutral), ShortcutAction::Love)
            .expect("love");
        assert_eq!(loved.love_state, LoveState::Loved);

        let neutral = desired_tag_values(&track(Some(2.0), LoveState::Loved), ShortcutAction::Love)
            .expect("unlove");
        assert_eq!(neutral.love_state, LoveState::Neutral);
    }

    #[test]
    fn shortcut_settings_are_device_local_and_persistent() {
        let path = std::env::temp_dir().join(format!(
            "aurora-shortcut-settings-{}-{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let mut runtime = GlobalShortcutRuntime::load(path.clone());
        assert!(runtime.enabled);
        runtime.enabled = false;
        runtime.bindings[0].accelerator = "Ctrl+Shift+P".to_owned();
        runtime.persist().expect("persist shortcut settings");
        let restored = GlobalShortcutRuntime::load(path.clone());
        assert!(!restored.enabled);
        assert_eq!(restored.bindings[0].accelerator, "Ctrl+Shift+P");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn legacy_top_row_rating_defaults_migrate_to_the_numeric_keypad() {
        let path = std::env::temp_dir().join(format!(
            "aurora-legacy-shortcut-settings-{}-{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let mut legacy = ShortcutSettingsFile {
            version: LEGACY_SETTINGS_VERSION,
            enabled: true,
            bindings: DEFINITIONS
                .iter()
                .map(|definition| StoredBinding {
                    action: definition.action_key.to_owned(),
                    accelerator: match definition.action {
                        ShortcutAction::Rating(rating) => format!("Ctrl+Alt+{rating}"),
                        _ => definition.default_accelerator.to_owned(),
                    },
                })
                .collect(),
        };
        legacy.bindings[0].accelerator = "Ctrl+Shift+P".to_owned();
        fs::write(
            &path,
            serde_json::to_vec(&legacy).expect("encode legacy settings"),
        )
        .expect("write legacy settings");

        let restored = GlobalShortcutRuntime::load(path.clone());
        assert_eq!(restored.bindings[0].accelerator, "Ctrl+Shift+P");
        assert_eq!(restored.bindings[4].accelerator, "Ctrl+Alt+Numpad2");
        let persisted: ShortcutSettingsFile =
            serde_json::from_slice(&fs::read(&path).expect("read migrated settings"))
                .expect("decode migrated settings");
        assert_eq!(persisted.version, SETTINGS_VERSION);
        assert_eq!(persisted.bindings[4].accelerator, "Ctrl+Alt+Numpad2");
        let _ = fs::remove_file(path);
    }
}
