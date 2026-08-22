use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
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
    warning: Option<String>,
}

impl DeviceModeStore {
    pub(crate) fn load(path: PathBuf) -> Self {
        let mut warning = None;
        let laptop_mode = if path.is_file() {
            match fs::read_to_string(&path)
                .map_err(|error| error.to_string())
                .and_then(|json| {
                    serde_json::from_str::<DeviceSettingsFile>(&json)
                        .map_err(|error| error.to_string())
                }) {
                Ok(settings) => settings.laptop_mode,
                Err(error) => {
                    warning = Some(format!(
                        "Aurora could not read this device's Laptop Mode setting and used Desktop Mode: {error}"
                    ));
                    false
                }
            }
        } else {
            false
        };
        set_laptop_mode_runtime(laptop_mode);
        Self {
            path,
            laptop_mode,
            warning,
        }
    }

    pub(crate) fn laptop_mode(&self) -> bool {
        self.laptop_mode
    }

    pub(crate) fn warning(&self) -> Option<&str> {
        self.warning.as_deref()
    }

    pub(crate) fn set_laptop_mode(&mut self, enabled: bool) -> Result<(), String> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "Aurora's device setting has no parent directory.".to_owned())?;
        fs::create_dir_all(parent).map_err(|error| {
            format!("Could not create Aurora's device settings folder: {error}")
        })?;
        let json = serde_json::to_vec_pretty(&DeviceSettingsFile {
            laptop_mode: enabled,
        })
        .map_err(|error| format!("Could not encode Aurora's device setting: {error}"))?;
        let mut file = File::create(&self.path)
            .map_err(|error| format!("Could not save Aurora's device setting: {error}"))?;
        file.write_all(&json)
            .and_then(|_| file.sync_all())
            .map_err(|error| format!("Could not flush Aurora's device setting: {error}"))?;
        self.laptop_mode = enabled;
        self.warning = None;
        set_laptop_mode_runtime(enabled);
        Ok(())
    }
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
        let restored = DeviceModeStore::load(path.clone());
        assert!(restored.laptop_mode());
        let _ = fs::remove_file(path);
        set_laptop_mode_runtime(false);
    }
}
