use std::{fs, path::Path};

pub(crate) fn remove_verified_mp3(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Aurora could not verify the MP3 before deletion: {error}"))?;
    if !metadata.file_type().is_file() {
        return Err("Aurora refused to delete a path that is not a regular MP3 file.".to_owned());
    }
    let is_mp3 = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mp3"));
    if !is_mp3 {
        return Err("Aurora refused to delete a file that is not an MP3.".to_owned());
    }

    match fs::remove_file(path) {
        Ok(()) => {}
        Err(_error) if !path.exists() => {}
        Err(error) => {
            return Err(format!(
                "Aurora could not delete the MP3 from disk: {error}"
            ));
        }
    }
    if path.exists() {
        return Err("Aurora could not verify that the MP3 was deleted from disk.".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn removes_only_a_regular_mp3() {
        let directory = TempDir::new().expect("temporary album");
        let track = directory.path().join("Bonus Track.MP3");
        fs::write(&track, b"fixture").expect("fixture track");

        remove_verified_mp3(&track).expect("delete track");

        assert!(!track.exists());
    }

    #[test]
    fn refuses_non_mp3_files_and_directories() {
        let directory = TempDir::new().expect("temporary album");
        let text = directory.path().join("notes.txt");
        fs::write(&text, b"fixture").expect("fixture text");

        assert!(remove_verified_mp3(&text).is_err());
        assert!(remove_verified_mp3(directory.path()).is_err());
        assert!(text.exists());
    }
}
