use crate::{catalog, explorer, state_store::StateStore};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub(crate) async fn reveal_catalog_item(
    app: AppHandle,
    track_id: Option<String>,
    track_key: Option<String>,
    album_id: Option<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<StateStore>();
        let (path, select_file) = match (track_id, track_key, album_id) {
            (Some(id), Some(key), None) => {
                (catalog::resolve_track(&id, &key, &store)?.audio_path, true)
            }
            (None, None, Some(id)) => {
                let detail = explorer::load_local_album_detail(id, &store)?;
                let track = detail
                    .tracks
                    .first()
                    .ok_or("This album has no available local songs.")?;
                (PathBuf::from(&track.directory), false)
            }
            _ => return Err("Choose one catalog song or album to show.".into()),
        };
        reveal_path(&existing_path(&path, select_file)?, select_file)
    })
    .await
    .map_err(|error| format!("Could not open the file browser: {error}"))?
}

fn existing_path(path: &Path, file: bool) -> Result<PathBuf, String> {
    let resolved = path
        .canonicalize()
        .map_err(|error| format!("This local path is unavailable: {error}"))?;
    if (file && !resolved.is_file()) || (!file && !resolved.is_dir()) {
        return Err("The selected local file or folder is unavailable.".into());
    }
    #[cfg(windows)]
    let resolved = shell_path(&resolved);
    Ok(resolved)
}

#[cfg(windows)]
fn shell_path(path: &Path) -> PathBuf {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    let wide = path.as_os_str().encode_wide().collect::<Vec<_>>();
    let unc_prefix = r"\\?\UNC\".encode_utf16().collect::<Vec<_>>();
    let verbatim_prefix = r"\\?\".encode_utf16().collect::<Vec<_>>();
    let normalized = if wide.starts_with(&unc_prefix) {
        vec![u16::from(b'\\'), u16::from(b'\\')]
            .into_iter()
            .chain(wide[unc_prefix.len()..].iter().copied())
            .collect::<Vec<_>>()
    } else if wide.starts_with(&verbatim_prefix) {
        wide[verbatim_prefix.len()..].to_vec()
    } else {
        wide
    };
    std::ffi::OsString::from_wide(&normalized).into()
}

#[cfg(windows)]
fn reveal_path(path: &Path, select_file: bool) -> Result<(), String> {
    if !select_file {
        std::process::Command::new("explorer.exe")
            .arg(path)
            .spawn()
            .map_err(|error| format!("Could not open Windows Explorer: {error}"))?;
        return Ok(());
    }
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        Win32::{
            System::Com::{
                COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize,
            },
            UI::Shell::{Common::ITEMIDLIST, SHOpenFolderAndSelectItems, SHParseDisplayName},
        },
        core::PCWSTR,
    };
    let wide = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    // The shell parses the catalog path directly; filenames never become command arguments.
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|error| format!("Could not initialize Windows Explorer: {error}"))?;
        let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
        let result = SHParseDisplayName(PCWSTR(wide.as_ptr()), None, &mut pidl, 0, None)
            .and_then(|()| SHOpenFolderAndSelectItems(pidl, None, 0));
        if !pidl.is_null() {
            CoTaskMemFree(Some(pidl.cast()));
        }
        CoUninitialize();
        result.map_err(|error| format!("Could not open Windows Explorer: {error}"))
    }
}

#[cfg(not(windows))]
fn reveal_path(path: &Path, select_file: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = std::process::Command::new("open");
        if select_file {
            command.arg("-R");
        }
        command.arg(path);
        command
    };
    #[cfg(not(target_os = "macos"))]
    let mut command = {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(if select_file {
            path.parent().ok_or("This file has no parent folder.")?
        } else {
            path
        });
        command
    };
    command
        .spawn()
        .map_err(|error| format!("Could not open the file browser: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn shell_paths_remove_verbatim_prefixes_and_preserve_drive_and_unc_paths() {
        assert_eq!(
            shell_path(Path::new(r"\\?\C:\Music\Björk, 'song' & spaces.mp3")),
            PathBuf::from(r"C:\Music\Björk, 'song' & spaces.mp3")
        );
        assert_eq!(
            shell_path(Path::new(r"\\?\UNC\server\share\Music\Song.mp3")),
            PathBuf::from(r"\\server\share\Music\Song.mp3")
        );
        let normal = Path::new(r"C:\Music\Song.mp3");
        assert_eq!(shell_path(normal), normal);
    }

    #[test]
    fn reveal_requires_an_existing_path_of_the_right_kind() {
        let directory = tempfile::tempdir().unwrap();
        let song = directory.path().join("Song, 'quotes' & spaces.mp3");
        std::fs::write(&song, b"test").unwrap();
        assert!(existing_path(&song, true).is_ok());
        #[cfg(windows)]
        assert!(!existing_path(&song, true).unwrap().starts_with(r"\\?\"));
        assert!(existing_path(directory.path(), false).is_ok());
        assert!(existing_path(&song, false).is_err());
        assert!(existing_path(directory.path(), true).is_err());
        assert!(existing_path(&directory.path().join("missing.mp3"), true).is_err());
    }
}
