use crate::state_sync;
use serde::{Deserialize, Serialize};
use std::{
    env,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static LAPTOP_MODE: AtomicBool = AtomicBool::new(false);

const PATH_MAPPINGS: [(&str, &str); 3] = [
    (r"D:\MUSIC", r"Y:\MUSIC"),
    (r"G:\_BACKUP\SCORES", r"V:\_BACKUP\SCORES"),
    (r"H:\Synthwave", r"U:\Synthwave"),
];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeviceSettingsFile {
    laptop_mode: bool,
    #[serde(default)]
    device_id: Option<String>,
    #[serde(default)]
    device_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PathMappingStatus {
    desktop_root: &'static str,
    laptop_root: &'static str,
    active_root: &'static str,
    available: bool,
}

pub(crate) struct DeviceModeStore {
    path: PathBuf,
    laptop_mode: bool,
    device_id: String,
    device_name: String,
    warning: Option<String>,
}

impl DeviceModeStore {
    pub(crate) fn load(path: PathBuf) -> Self {
        let mut warning = None;
        let loaded = if path.is_file() {
            match fs::read_to_string(&path)
                .map_err(|error| error.to_string())
                .and_then(|json| {
                    serde_json::from_str::<DeviceSettingsFile>(&json)
                        .map_err(|error| error.to_string())
                }) {
                Ok(settings) => Some(settings),
                Err(error) => {
                    warning = Some(format!(
                        "Aurora could not read this device's Laptop Mode setting and used Desktop Mode: {error}"
                    ));
                    None
                }
            }
        } else {
            None
        };
        let laptop_mode = loaded.as_ref().is_some_and(|settings| settings.laptop_mode);
        let device_name = loaded
            .as_ref()
            .and_then(|settings| settings.device_name.as_deref())
            .filter(|name| valid_device_name(name))
            .map(str::to_owned)
            .unwrap_or_else(default_device_name);
        let device_id = loaded
            .as_ref()
            .and_then(|settings| settings.device_id.as_deref())
            .filter(|id| valid_device_id(id))
            .map(str::to_owned)
            .unwrap_or_else(|| new_device_id(&device_name));
        let needs_identity_save = loaded.as_ref().is_none_or(|settings| {
            settings.device_id.as_deref() != Some(device_id.as_str())
                || settings.device_name.as_deref() != Some(device_name.as_str())
        });
        set_laptop_mode_runtime(laptop_mode);
        let mut store = Self {
            path,
            laptop_mode,
            device_id,
            device_name,
            warning,
        };
        if needs_identity_save && let Err(error) = store.persist() {
            store.warning = Some(match store.warning.take() {
                Some(existing) => {
                    format!("{existing} Aurora could not save this device's identity: {error}")
                }
                None => format!("Aurora could not save this device's identity: {error}"),
            });
        }
        store
    }

    pub(crate) fn laptop_mode(&self) -> bool {
        self.laptop_mode
    }

    pub(crate) fn warning(&self) -> Option<&str> {
        self.warning.as_deref()
    }

    pub(crate) fn device_id(&self) -> &str {
        &self.device_id
    }

    pub(crate) fn device_name(&self) -> &str {
        &self.device_name
    }

    pub(crate) fn set_laptop_mode(&mut self, enabled: bool) -> Result<(), String> {
        let previous = self.laptop_mode;
        self.laptop_mode = enabled;
        if let Err(error) = self.persist() {
            self.laptop_mode = previous;
            return Err(error);
        }
        self.warning = None;
        set_laptop_mode_runtime(enabled);
        Ok(())
    }

    pub(crate) fn adopt_device_id(&mut self, device_id: &str) -> Result<(), String> {
        if !valid_device_id(device_id) {
            return Err("Aurora's recovered device identity is invalid.".to_owned());
        }
        if self.device_id == device_id {
            return Ok(());
        }
        let previous = std::mem::replace(&mut self.device_id, device_id.to_owned());
        if let Err(error) = self.persist() {
            self.device_id = previous;
            return Err(error);
        }
        Ok(())
    }

    fn persist(&self) -> Result<(), String> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "Aurora's device setting has no parent directory.".to_owned())?;
        fs::create_dir_all(parent).map_err(|error| {
            format!("Could not create Aurora's device settings folder: {error}")
        })?;
        let json = serde_json::to_vec_pretty(&DeviceSettingsFile {
            laptop_mode: self.laptop_mode,
            device_id: Some(self.device_id.clone()),
            device_name: Some(self.device_name.clone()),
        })
        .map_err(|error| format!("Could not encode Aurora's device setting: {error}"))?;
        let temporary = parent.join(format!(
            ".aurora-device-{}-{}.tmp",
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
            .map_err(|error| format!("Could not save Aurora's device setting: {error}"))?;
        if let Err(error) = file.write_all(&json).and_then(|_| file.sync_all()) {
            drop(file);
            let _ = fs::remove_file(&temporary);
            return Err(format!("Could not flush Aurora's device setting: {error}"));
        }
        drop(file);
        let result = if self.path.is_file() {
            state_sync::replace_file_atomic(&self.path, &temporary)
        } else {
            fs::rename(&temporary, &self.path)
                .map_err(|error| format!("Could not install Aurora's device setting: {error}"))
        };
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

fn valid_device_id(value: &str) -> bool {
    (8..=96).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn valid_device_name(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 80 && !value.chars().any(char::is_control)
}

fn default_device_name() -> String {
    env::var("COMPUTERNAME")
        .ok()
        .filter(|name| valid_device_name(name))
        .unwrap_or_else(|| "This computer".to_owned())
}

fn new_device_id(device_name: &str) -> String {
    let slug = device_name
        .chars()
        .filter_map(|character| {
            if character.is_ascii_alphanumeric() {
                Some(character.to_ascii_lowercase())
            } else if character == '-' || character == '_' {
                Some(character)
            } else {
                None
            }
        })
        .take(32)
        .collect::<String>();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "device-{}-{}-{nanos}",
        if slug.is_empty() { "computer" } else { &slug },
        std::process::id()
    )
}

pub(crate) fn laptop_mode_enabled() -> bool {
    LAPTOP_MODE.load(Ordering::SeqCst)
}

fn set_laptop_mode_runtime(enabled: bool) {
    LAPTOP_MODE.store(enabled, Ordering::SeqCst);
}

pub(crate) fn resolve_device_path(path: &Path) -> PathBuf {
    remap_path(path, laptop_mode_enabled())
}

pub(crate) fn catalog_path_for_device_path(path: &Path) -> PathBuf {
    remap_path(path, false)
}

pub(crate) fn path_mapping_statuses() -> Vec<PathMappingStatus> {
    let laptop_mode = laptop_mode_enabled();
    PATH_MAPPINGS
        .iter()
        .map(|(desktop_root, laptop_root)| {
            let active_root = if laptop_mode {
                *laptop_root
            } else {
                *desktop_root
            };
            PathMappingStatus {
                desktop_root,
                laptop_root,
                active_root,
                available: Path::new(active_root).is_dir(),
            }
        })
        .collect()
}

fn remap_path(path: &Path, laptop_mode: bool) -> PathBuf {
    let raw = path.to_string_lossy().replace('/', "\\");
    let raw = raw.strip_prefix(r"\\?\").unwrap_or(&raw);
    for (desktop_root, laptop_root) in PATH_MAPPINGS {
        let target = if laptop_mode {
            laptop_root
        } else {
            desktop_root
        };
        for candidate in [desktop_root, laptop_root] {
            if let Some(suffix) = path_suffix(raw, candidate) {
                return PathBuf::from(format!("{target}{suffix}"));
            }
        }
    }
    PathBuf::from(raw)
}

fn path_suffix<'a>(path: &'a str, root: &str) -> Option<&'a str> {
    if path.eq_ignore_ascii_case(root) {
        return Some("");
    }
    if path.len() > root.len()
        && path
            .get(..root.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(root))
        && path.as_bytes().get(root.len()) == Some(&b'\\')
    {
        return Some(&path[root.len()..]);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_settings_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "aurora-device-settings-{}-{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ))
    }

    #[test]
    fn laptop_mode_maps_only_complete_catalog_roots() {
        assert_eq!(
            remap_path(Path::new(r"D:\MUSIC\M83\Track.mp3"), true),
            PathBuf::from(r"Y:\MUSIC\M83\Track.mp3")
        );
        assert_eq!(
            remap_path(Path::new(r"g:\_backup\scores\Arrival\Track.mp3"), true),
            PathBuf::from(r"V:\_BACKUP\SCORES\Arrival\Track.mp3")
        );
        assert_eq!(
            remap_path(Path::new(r"H:\Synthwave"), true),
            PathBuf::from(r"U:\Synthwave")
        );
        assert_eq!(
            remap_path(Path::new(r"D:\MUSICIAN\Track.mp3"), true),
            PathBuf::from(r"D:\MUSICIAN\Track.mp3")
        );
    }

    #[test]
    fn desktop_mode_maps_laptop_journal_paths_back_to_catalog_roots() {
        assert_eq!(
            remap_path(Path::new(r"Y:\MUSIC\M83\Track.mp3"), false),
            PathBuf::from(r"D:\MUSIC\M83\Track.mp3")
        );
        assert_eq!(
            catalog_path_for_device_path(Path::new(r"u:\Synthwave\M83\Track.mp3")),
            PathBuf::from(r"H:\Synthwave\M83\Track.mp3")
        );
    }

    #[test]
    fn device_setting_survives_restart_without_entering_shared_state() {
        let path = temporary_settings_path();
        let mut settings = DeviceModeStore::load(path.clone());
        assert!(!settings.laptop_mode());
        settings.set_laptop_mode(true).expect("enable laptop mode");
        let device_id = settings.device_id().to_owned();
        let restored = DeviceModeStore::load(path.clone());
        assert!(restored.laptop_mode());
        assert_eq!(restored.device_id(), device_id);
        let _ = fs::remove_file(path);
        set_laptop_mode_runtime(false);
    }

    #[test]
    fn legacy_device_setting_gains_a_stable_identity() {
        let path = temporary_settings_path();
        fs::write(&path, r#"{ "laptopMode": true }"#).expect("legacy setting");
        let migrated = DeviceModeStore::load(path.clone());
        let device_id = migrated.device_id().to_owned();
        assert!(migrated.laptop_mode());
        assert!(valid_device_id(&device_id));
        assert_eq!(DeviceModeStore::load(path.clone()).device_id(), device_id);
        let _ = fs::remove_file(path);
        set_laptop_mode_runtime(false);
    }
}
