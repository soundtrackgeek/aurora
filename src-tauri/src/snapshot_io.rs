//! SQLite never opens the configured Mac network snapshot directory directly.
//! Published snapshots are closed, self-contained files; active local WAL databases
//! always keep their original path and SQLite locking behavior.
use rusqlite::{Connection, OpenFlags};
use std::{
    fs,
    io::Write,
    ops::Deref,
    path::{Path, PathBuf},
};

pub(crate) struct LocalSnapshot {
    pub(crate) path: PathBuf,
    _directory: Option<tempfile::TempDir>,
}
impl LocalSnapshot {
    pub(crate) fn new(path: &Path) -> Result<Self, String> {
        let configured = crate::connections::active().sync_folder;
        let remote = cfg!(target_os = "macos")
            && (path.starts_with("/Volumes")
                || (!configured.is_empty() && path.starts_with(&configured)));
        if !remote {
            return Self::with_staging(path, false);
        }
        // SMB can block inside open(), beyond SQLite's busy timeout. Limit both
        // the caller's wait and the number of outstanding filesystem workers.
        static IN_FLIGHT: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());
        let mut in_flight = IN_FLIGHT.lock().map_err(|e| e.to_string())?;
        if in_flight.len() >= 4 || in_flight.iter().any(|p| p == path) {
            return Err("A sync snapshot is still being downloaded; Aurora will retry.".to_owned());
        }
        let path = path.to_owned();
        in_flight.push(path.clone());
        drop(in_flight);
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let result = Self::with_staging(&path, true);
            if let Ok(mut pending) = IN_FLIGHT.lock() {
                pending.retain(|p| p != &path);
            }
            let _ = sender.send(result);
        });
        receiver.recv_timeout(std::time::Duration::from_secs(3))
            .map_err(|_| "The sync share did not respond within 3 seconds. Local data remains available; Aurora will retry.".to_owned())?
    }
    fn with_staging(path: &Path, remote: bool) -> Result<Self, String> {
        if !remote {
            return Ok(Self {
                path: path.to_owned(),
                _directory: None,
            });
        }
        let directory = tempfile::tempdir().map_err(|e| e.to_string())?;
        let copy = directory.path().join("snapshot.sqlite3");
        fs::copy(path, &copy).map_err(|e| format!("Could not download the sync snapshot: {e}"))?;
        Ok(Self {
            path: copy,
            _directory: Some(directory),
        })
    }
}

pub(crate) struct SnapshotConnection {
    // Close SQLite before removing its staging directory (fields drop in order).
    connection: Connection,
    _snapshot: LocalSnapshot,
}
impl Deref for SnapshotConnection {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        &self.connection
    }
}
pub(crate) fn open_read(path: &Path) -> Result<SnapshotConnection, String> {
    let snapshot = LocalSnapshot::new(path)?;
    let connection = Connection::open_with_flags(
        &snapshot.path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|e| e.to_string())?;
    Ok(SnapshotConnection {
        connection,
        _snapshot: snapshot,
    })
}

// Copy into an exclusively-created file on the destination filesystem before
// atomic publication. Dropping the temporary removes failed/incomplete transfers.
pub(crate) fn upload(source: &Path, directory: &Path) -> Result<tempfile::TempPath, String> {
    let mut staged = tempfile::NamedTempFile::new_in(directory).map_err(|e| e.to_string())?;
    let mut input = fs::File::open(source).map_err(|e| e.to_string())?;
    std::io::copy(&mut input, &mut staged).map_err(|e| e.to_string())?;
    staged.flush().map_err(|e| e.to_string())?;
    staged.as_file().sync_all().map_err(|e| e.to_string())?;
    // Close the writable handle before another SMB reader validates the file.
    Ok(staged.into_temp_path())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn staged_snapshot_is_independent_and_upload_is_complete() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("state.sqlite3");
        let db = Connection::open(&source).unwrap();
        db.execute_batch("CREATE TABLE marker(value); INSERT INTO marker VALUES(42)")
            .unwrap();
        drop(db);
        let copy = LocalSnapshot::with_staging(&source, true).unwrap();
        assert_ne!(copy.path, source);
        fs::remove_file(&source).unwrap();
        let db = Connection::open(&copy.path).unwrap();
        assert_eq!(
            db.query_row("SELECT value FROM marker", [], |r| r.get::<_, i32>(0))
                .unwrap(),
            42
        );
        drop(db);
        let staged = upload(&copy.path, dir.path()).unwrap();
        assert_eq!(fs::read(&staged).unwrap(), fs::read(&copy.path).unwrap());
    }
}
