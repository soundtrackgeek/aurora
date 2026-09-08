use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

static ACTIVE: OnceLock<ConnectionSettings> = OnceLock::new();

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RootMapping {
    pub(crate) catalog_root: String,
    pub(crate) mounted_root: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ConnectionSettings {
    pub(crate) network_mode: bool,
    pub(crate) catalog_path: String,
    pub(crate) sync_folder: String,
    pub(crate) music_roots: Vec<RootMapping>,
}

#[allow(
    clippy::derivable_impls,
    reason = "Network Mode defaults to true on macOS"
)]
impl Default for ConnectionSettings {
    fn default() -> Self {
        Self {
            network_mode: cfg!(target_os = "macos"),
            catalog_path: String::new(),
            sync_folder: String::new(),
            music_roots: Vec::new(),
        }
    }
}

pub(crate) fn initialize(directory: &Path) -> Result<(), String> {
    let value = read(directory)?;
    ACTIVE
        .set(value)
        .map_err(|_| "Connection settings were already initialized.".to_owned())
}

pub(crate) fn read(directory: &Path) -> Result<ConnectionSettings, String> {
    let path = directory.join("aurora-connections.json");
    if !path.exists() {
        return Ok(ConnectionSettings::default());
    }
    let value: ConnectionSettings =
        serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("Could not read connection settings: {e}"))?;
    validate(&value)?;
    Ok(value)
}

pub(crate) fn save(directory: &Path, value: &ConnectionSettings) -> Result<(), String> {
    validate(value)?;
    fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let target = directory.join("aurora-connections.json");
    let mut temporary = tempfile::NamedTempFile::new_in(directory).map_err(|e| e.to_string())?;
    use std::io::Write;
    temporary
        .write_all(&serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    temporary.persist(target).map_err(|e| e.to_string())?;
    Ok(())
}

fn validate(value: &ConnectionSettings) -> Result<(), String> {
    for path in [&value.catalog_path, &value.sync_folder] {
        if !path.is_empty() && !Path::new(path).is_absolute() {
            return Err("Choose absolute paths for the catalog and sync folder.".to_owned());
        }
    }
    #[cfg(target_os = "macos")]
    if value.catalog_path.starts_with("/Volumes/") {
        return Err(
            "The working catalog must be local. Copy a verified snapshot to this Mac first."
                .to_owned(),
        );
    }
    if value.music_roots.len() > 32 {
        return Err("At most 32 music roots are supported.".to_owned());
    }
    for (i, mapping) in value.music_roots.iter().enumerate() {
        let source = mapping.catalog_root.replace('/', "\\");
        if source.len() < 3
            || source.as_bytes()[1] != b':'
            || source.as_bytes()[2] != b'\\'
            || !source.as_bytes()[0].is_ascii_alphabetic()
            || source.split('\\').any(|p| p == ".." || p == ".")
            || !Path::new(&mapping.mounted_root).is_absolute()
        {
            return Err(
                "Each mapping needs a Windows drive root and an absolute mounted folder."
                    .to_owned(),
            );
        }
        if value.music_roots[..i].iter().any(|other| {
            other
                .catalog_root
                .trim_end_matches(['\\', '/'])
                .eq_ignore_ascii_case(source.trim_end_matches('\\'))
                || other
                    .mounted_root
                    .trim_end_matches('/')
                    .eq_ignore_ascii_case(mapping.mounted_root.trim_end_matches('/'))
        }) {
            return Err("Each catalog root and mounted folder must be unique.".to_owned());
        }
    }
    Ok(())
}

pub(crate) fn active() -> ConnectionSettings {
    ACTIVE.get().cloned().unwrap_or_default()
}
pub(crate) fn network_mode() -> bool {
    ACTIVE.get().is_some_and(|s| s.network_mode)
}
pub(crate) fn require_music_writes() -> Result<(), String> {
    if network_mode() {
        Err("Network Mode is read-only for music files. Make edits on the home PC.".to_owned())
    } else {
        Ok(())
    }
}

pub(crate) fn native_catalog_path() -> Result<PathBuf, String> {
    let configured = active().catalog_path;
    if !configured.is_empty() {
        return Ok(PathBuf::from(configured));
    }
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|p| p.join("Library/Application Support"));
    #[cfg(not(target_os = "macos"))]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    Ok(base
        .ok_or_else(|| "Application data directory is unavailable.".to_owned())?
        .join("com.local.musiclibrary")
        .join("music-library.sqlite3"))
}

// Canonical catalog identity remains Windows-shaped; only filesystem access is translated.
pub(crate) fn resolve(path: &Path, mappings: &[RootMapping], reverse: bool) -> Option<PathBuf> {
    let native = path.to_string_lossy();
    let windows = native.replace('/', "\\");
    let windows = windows.strip_prefix(r"\\?\").unwrap_or(&windows);
    let mut ordered: Vec<_> = mappings.iter().collect();
    ordered.sort_by_key(|m| {
        std::cmp::Reverse(if reverse {
            m.mounted_root.len()
        } else {
            m.catalog_root.len()
        })
    });
    for mapping in ordered {
        let source = mapping.catalog_root.replace('/', "\\");
        let source = source.trim_end_matches('\\');
        let mounted = mapping.mounted_root.trim_end_matches('/');
        let (raw, prefix, separator) = if reverse {
            (native.as_ref(), mounted, '/')
        } else {
            (windows, source, '\\')
        };
        let matched = raw.get(..prefix.len()).is_some_and(|p| {
            if reverse {
                p == prefix
            } else {
                p.eq_ignore_ascii_case(prefix)
            }
        });
        if !matched || (raw.len() > prefix.len() && raw.as_bytes()[prefix.len()] != separator as u8)
        {
            continue;
        }
        let suffix = raw[prefix.len()..].trim_start_matches(separator);
        if suffix.split(['/', '\\']).any(|p| p == ".." || p == ".") {
            return None;
        }
        return Some(if reverse {
            PathBuf::from(if suffix.is_empty() {
                source.to_owned()
            } else {
                format!("{source}\\{}", suffix.replace('/', "\\"))
            })
        } else {
            PathBuf::from(mounted).join(suffix.replace('\\', "/"))
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maps_windows_identity_to_native_paths_and_back_without_prefix_collisions() {
        let mappings = vec![RootMapping {
            catalog_root: r"D:\MUSIC".into(),
            mounted_root: "/Volumes/PC/MUSIC".into(),
        }];
        let native = Path::new("/Volumes/PC/MUSIC/Björk/01 - Jóga.mp3");
        assert_eq!(
            resolve(Path::new(r"d:\music\Björk\01 - Jóga.mp3"), &mappings, false).unwrap(),
            native
        );
        assert_eq!(
            resolve(native, &mappings, true).unwrap(),
            Path::new(r"D:\MUSIC\Björk\01 - Jóga.mp3")
        );
        assert!(resolve(Path::new(r"D:\MUSICIAN\song.mp3"), &mappings, false).is_none());
        assert!(resolve(Path::new(r"D:\MUSIC\..\private.mp3"), &mappings, false).is_none());
    }
    #[test]
    fn settings_survive_save_and_allow_disconnected_roots() {
        let dir = tempfile::tempdir().unwrap();
        let value = ConnectionSettings {
            network_mode: true,
            sync_folder: dir
                .path()
                .join("offline-sync")
                .to_string_lossy()
                .into_owned(),
            ..Default::default()
        };
        save(dir.path(), &value).unwrap();
        assert_eq!(read(dir.path()).unwrap(), value);
        assert!(
            validate(&ConnectionSettings {
                catalog_path: "relative.sqlite3".into(),
                ..value
            })
            .is_err()
        );
    }
}
