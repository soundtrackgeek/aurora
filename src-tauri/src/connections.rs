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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) smb_share: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RootStatus {
    pub(crate) catalog_root: String,
    pub(crate) mounted_root: String,
    pub(crate) smb_share: Option<String>,
    pub(crate) active_root: String,
    pub(crate) available: bool,
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
    let mut value: ConnectionSettings =
        serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("Could not read connection settings: {e}"))?;
    for mapping in &mut value.music_roots {
        if mapping.smb_share.is_none() {
            mapping.smb_share = infer_legacy_smb_share(mapping);
        }
    }
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
            || Path::new(&mapping.mounted_root).components().any(|part| {
                matches!(
                    part,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            })
        {
            return Err(
                "Each mapping needs a Windows drive root and an absolute mounted folder."
                    .to_owned(),
            );
        }
        if mapping
            .smb_share
            .as_deref()
            .is_some_and(|share| parse_smb_share(share).is_none())
        {
            return Err(
                "SMB shares must use smb://host/share without a user name or subfolder.".to_owned(),
            );
        }
        if mapping.smb_share.is_some() && !mapping.mounted_root.starts_with("/Volumes/") {
            return Err("An SMB mapping needs a mounted folder under /Volumes.".to_owned());
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
        let Some(mounted_root) = active_mounted_root(mapping) else {
            continue;
        };
        let mounted = mounted_root.to_str()?.trim_end_matches('/');
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

fn parse_smb_share(value: &str) -> Option<(&str, &str)> {
    let (host, share) = value.strip_prefix("smb://")?.split_once('/')?;
    if host.is_empty()
        || share.is_empty()
        || host
            .chars()
            .any(|c| !c.is_ascii_alphanumeric() && !matches!(c, '.' | '-'))
        || share
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | '?' | '#' | '@'))
        || matches!(share, "." | "..")
    {
        return None;
    }
    Some((host, share))
}

fn infer_legacy_smb_share(mapping: &RootMapping) -> Option<String> {
    let volume = mapping
        .mounted_root
        .strip_prefix("/Volumes/")?
        .split('/')
        .next()?;
    let host = volume
        .rsplit_once('-')
        .filter(|(_, suffix)| {
            !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
        })
        .map_or(volume, |(host, _)| host);
    host.parse::<std::net::Ipv4Addr>().ok()?;
    let drive = mapping.catalog_root.as_bytes().first().copied()?;
    if !drive.is_ascii_alphabetic() {
        return None;
    }
    Some(format!(
        "smb://{host}/{}",
        (drive as char).to_ascii_uppercase()
    ))
}

#[cfg(target_os = "macos")]
#[derive(Clone, Debug)]
struct SmbMount {
    host: String,
    share: String,
    path: PathBuf,
}

#[cfg(target_os = "macos")]
fn smb_mounts() -> Vec<SmbMount> {
    use std::ffi::CStr;
    let mut entries = std::ptr::null_mut();
    // SAFETY: getmntinfo supplies an OS-owned array of `count` statfs entries.
    let count = unsafe { libc::getmntinfo(&mut entries, libc::MNT_NOWAIT) };
    if count <= 0 || entries.is_null() {
        return Vec::new();
    }
    // SAFETY: Each statfs string is NUL-terminated and the array stays valid until the next getmntinfo call.
    unsafe { std::slice::from_raw_parts(entries, count as usize) }
        .iter()
        .filter_map(|entry| {
            let fs_type = unsafe { CStr::from_ptr(entry.f_fstypename.as_ptr()) };
            if fs_type.to_bytes() != b"smbfs" {
                return None;
            }
            let source = unsafe { CStr::from_ptr(entry.f_mntfromname.as_ptr()) }
                .to_str()
                .ok()?;
            let (server, share) = source.strip_prefix("//")?.split_once('/')?;
            let host = server.rsplit_once('@').map_or(server, |(_, host)| host);
            let path = unsafe { CStr::from_ptr(entry.f_mntonname.as_ptr()) }
                .to_str()
                .ok()?;
            Some(SmbMount {
                host: host.to_owned(),
                share: share.to_owned(),
                path: PathBuf::from(path),
            })
        })
        .collect()
}

fn active_mounted_root(mapping: &RootMapping) -> Option<PathBuf> {
    if mapping.smb_share.is_none() {
        return Some(PathBuf::from(&mapping.mounted_root));
    }
    #[cfg(target_os = "macos")]
    {
        mounted_root_from_mounts(mapping, &smb_mounts())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Some(PathBuf::from(&mapping.mounted_root))
    }
}

#[cfg(target_os = "macos")]
fn mounted_root_from_mounts(mapping: &RootMapping, mounts: &[SmbMount]) -> Option<PathBuf> {
    let (host, share) = parse_smb_share(mapping.smb_share.as_deref()?)?;
    let saved = Path::new(&mapping.mounted_root);
    let volume = saved.strip_prefix("/Volumes").ok()?.components().next()?;
    let saved_mount = Path::new("/Volumes").join(volume.as_os_str());
    let suffix = saved.strip_prefix(saved_mount).ok()?;
    mounts
        .iter()
        .find(|mount| {
            mount.host.eq_ignore_ascii_case(host) && mount.share.eq_ignore_ascii_case(share)
        })
        .map(|mount| mount.path.join(suffix))
}

pub(crate) fn root_statuses(mappings: &[RootMapping]) -> Vec<RootStatus> {
    mappings
        .iter()
        .map(|mapping| {
            let active = active_mounted_root(mapping);
            RootStatus {
                catalog_root: mapping.catalog_root.clone(),
                mounted_root: mapping.mounted_root.clone(),
                smb_share: mapping.smb_share.clone(),
                available: active.as_deref().is_some_and(Path::is_dir),
                active_root: active
                    .unwrap_or_else(|| PathBuf::from(&mapping.mounted_root))
                    .to_string_lossy()
                    .into_owned(),
            }
        })
        .collect()
}

pub(crate) fn unavailable_share(path: &Path) -> Option<String> {
    let settings = active();
    let windows = path.to_string_lossy().replace('/', "\\");
    settings.music_roots.iter().find_map(|mapping| {
        let source = mapping.catalog_root.replace('/', "\\");
        let source = source.trim_end_matches('\\');
        let prefix = windows.get(..source.len())?;
        if !prefix.eq_ignore_ascii_case(source)
            || (windows.len() > source.len() && windows.as_bytes()[source.len()] != b'\\')
            || active_mounted_root(mapping).is_some()
        {
            return None;
        }
        mapping.smb_share.clone()
    })
}

#[cfg(target_os = "macos")]
pub(crate) fn connect_music_shares(settings: &ConnectionSettings) -> Result<usize, String> {
    if !settings.network_mode {
        return Ok(0);
    }
    let mounts = smb_mounts();
    let mut opened = std::collections::HashSet::new();
    for mapping in &settings.music_roots {
        let Some(url) = mapping.smb_share.as_deref() else {
            continue;
        };
        let (host, share) = parse_smb_share(url).ok_or("The SMB share URL is invalid.")?;
        if mounts.iter().any(|mount| {
            mount.host.eq_ignore_ascii_case(host) && mount.share.eq_ignore_ascii_case(share)
        }) || !opened.insert(url.to_ascii_lowercase())
        {
            continue;
        }
        let result = std::process::Command::new("/usr/bin/open")
            .arg(url)
            .status()
            .map_err(|error| format!("Could not ask Finder to connect {url}: {error}"))?;
        if !result.success() {
            return Err(format!("Finder could not open the music share {url}."));
        }
    }
    Ok(opened.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maps_windows_identity_to_native_paths_and_back_without_prefix_collisions() {
        let mappings = vec![RootMapping {
            catalog_root: r"D:\MUSIC".into(),
            mounted_root: "/Volumes/PC/MUSIC".into(),
            smb_share: None,
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

    #[test]
    fn legacy_roots_load_without_smb_share_and_invalid_urls_are_rejected() {
        let legacy: RootMapping = serde_json::from_str(
            r#"{"catalogRoot":"D:\\MUSIC","mountedRoot":"/Volumes/Old/MUSIC"}"#,
        )
        .unwrap();
        assert_eq!(legacy.smb_share, None);
        let valid = ConnectionSettings {
            music_roots: vec![RootMapping {
                smb_share: Some("smb://home-pc/D".into()),
                ..legacy.clone()
            }],
            ..Default::default()
        };
        assert!(validate(&valid).is_ok());
        assert!(
            validate(&ConnectionSettings {
                music_roots: vec![RootMapping {
                    smb_share: Some("smb://user@home-pc/D".into()),
                    ..legacy
                }],
                ..Default::default()
            })
            .is_err()
        );

        let dir = tempfile::tempdir().unwrap();
        let settings = ConnectionSettings {
            music_roots: vec![RootMapping {
                catalog_root: r"D:\MUSIC".into(),
                mounted_root: "/Volumes/192.0.2.10-2/MUSIC".into(),
                smb_share: None,
            }],
            ..Default::default()
        };
        save(dir.path(), &settings).unwrap();
        assert_eq!(
            read(dir.path()).unwrap().music_roots[0]
                .smb_share
                .as_deref(),
            Some("smb://192.0.2.10/D")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn smb_share_rebinds_to_current_mount_name_without_confusing_drives() {
        let mapping = RootMapping {
            catalog_root: r"D:\MUSIC".into(),
            mounted_root: "/Volumes/musicbox-1/MUSIC".into(),
            smb_share: Some("smb://musicbox.local/D".into()),
        };
        let mounts = vec![
            SmbMount {
                host: "musicbox.local".into(),
                share: "C".into(),
                path: "/Volumes/musicbox-1".into(),
            },
            SmbMount {
                host: "musicbox.local".into(),
                share: "D".into(),
                path: "/Volumes/musicbox-2".into(),
            },
        ];
        assert_eq!(
            mounted_root_from_mounts(&mapping, &mounts),
            Some(PathBuf::from("/Volumes/musicbox-2/MUSIC"))
        );
        assert_eq!(mounted_root_from_mounts(&mapping, &mounts[..1]), None);
        assert_eq!(parse_smb_share("smb://host/D/other"), None);
        let mappings = vec![
            RootMapping {
                smb_share: Some("smb://missing.invalid/D".into()),
                ..mapping
            },
            RootMapping {
                catalog_root: r"G:\".into(),
                mounted_root: "/Volumes/G".into(),
                smb_share: None,
            },
        ];
        assert_eq!(
            resolve(Path::new(r"G:\Album\song.mp3"), &mappings, false),
            Some(PathBuf::from("/Volumes/G/Album/song.mp3"))
        );
    }
}
