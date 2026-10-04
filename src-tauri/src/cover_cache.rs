//! A shared budget for derived cover thumbnails, never for source artwork.
use std::{
    collections::{BTreeSet, HashMap},
    fs::{self, File, FileTimes},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) const COVER_CACHE_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_THUMBNAIL_BYTES: u64 = 4 * 1024 * 1024;
const DIRECTORIES: [&str; 4] = [
    "covers",
    "embedded-album-covers",
    "inbox-covers",
    "selected-covers",
];
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Entry {
    bytes: u64,
    used: SystemTime,
}

#[derive(Default)]
struct Index {
    entries: HashMap<PathBuf, Entry>,
    oldest: BTreeSet<(SystemTime, PathBuf)>,
    bytes: u64,
}

impl Index {
    fn forget(&mut self, path: &Path) {
        if let Some(entry) = self.entries.remove(path) {
            self.bytes -= entry.bytes;
            self.oldest.remove(&(entry.used, path.to_owned()));
        }
    }

    fn record(&mut self, path: PathBuf, bytes: u64, used: SystemTime) {
        self.forget(&path);
        self.bytes += bytes;
        self.oldest.insert((used, path.clone()));
        self.entries.insert(path, Entry { bytes, used });
    }

    fn evict_to(&mut self, budget: u64) {
        // Failed removals stay counted. Try other entries rather than spinning
        // on a file held open by another process (especially on Windows).
        let mut failed = Vec::new();
        while self.bytes > budget {
            let Some((used, path)) = self.oldest.pop_first() else {
                break;
            };
            if !path.parent().is_some_and(regular_directory) {
                failed.push((used, path));
                continue;
            }
            match fs::remove_file(&path) {
                Ok(()) => self.forget(&path),
                Err(error) if error.kind() == io::ErrorKind::NotFound => self.forget(&path),
                Err(_) => failed.push((used, path)),
            }
        }
        self.oldest.extend(failed);
    }
}

pub(crate) struct CoverCache {
    root: PathBuf,
    budget: u64,
    index: Mutex<Option<Index>>,
}

fn thumbnail_name(name: &str) -> bool {
    let Some((fingerprint, size)) = name
        .strip_suffix(".webp")
        .and_then(|name| name.split_once('-'))
    else {
        return false;
    };
    fingerprint.len() == 16
        && fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit())
        && matches!(size, "64" | "128" | "256" | "512")
}

fn staged_name(path: &Path) -> bool {
    let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };
    // with_extension("pid-sequence.tmp") produces a multi-part extension.
    let Some((stem, sequence)) = stem.rsplit_once('.') else {
        return false;
    };
    let Some((pid, number)) = sequence.split_once('-') else {
        return false;
    };
    extension == "tmp"
        && thumbnail_name(&format!("{stem}.webp"))
        && !pid.is_empty()
        && pid.bytes().all(|byte| byte.is_ascii_digit())
        && !number.is_empty()
        && number.bytes().all(|byte| byte.is_ascii_digit())
}

fn regular_directory(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir() && !linked(&metadata))
}

fn linked(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Includes junctions as well as symbolic links.
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

impl CoverCache {
    pub(crate) fn new(root: PathBuf, budget: u64) -> Self {
        Self {
            root,
            budget,
            index: Mutex::new(None),
        }
    }

    fn initialize(&self) -> Index {
        let mut index = Index::default();
        for directory in DIRECTORIES {
            let root = self.root.join(directory);
            if !regular_directory(&root) {
                continue;
            }
            let Ok(files) = fs::read_dir(&root) else {
                continue;
            };
            for file in files.flatten() {
                let path = file.path();
                let Ok(metadata) = fs::symlink_metadata(&path) else {
                    continue;
                };
                if !metadata.file_type().is_file() || linked(&metadata) {
                    continue;
                }
                let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                if thumbnail_name(name) {
                    index.record(
                        path,
                        metadata.len(),
                        metadata.modified().unwrap_or(UNIX_EPOCH),
                    );
                } else if staged_name(&path) {
                    // Startup runs before any writes. Only Aurora's own staging
                    // names in these four device-local directories are removed.
                    let _ = fs::remove_file(path);
                }
            }
        }
        index.evict_to(self.budget);
        index
    }

    pub(crate) fn sweep(&self) {
        let mut guard = self.index.lock().unwrap_or_else(|error| error.into_inner());
        guard
            .get_or_insert_with(|| self.initialize())
            .evict_to(self.budget);
    }

    fn path(&self, directory: &str, filename: &str) -> Option<PathBuf> {
        (DIRECTORIES.contains(&directory) && thumbnail_name(filename))
            .then(|| self.root.join(directory).join(filename))
    }

    pub(crate) fn read(&self, directory: &str, filename: &str) -> Option<Vec<u8>> {
        let path = self.path(directory, filename)?;
        let mut guard = self.index.lock().unwrap_or_else(|error| error.into_inner());
        let index = guard.get_or_insert_with(|| self.initialize());
        let entry = index.entries.get(&path)?;
        if entry.bytes == 0
            || entry.bytes > MAX_THUMBNAIL_BYTES
            || !path.parent().is_some_and(regular_directory)
            || !fs::symlink_metadata(&path)
                .is_ok_and(|metadata| metadata.is_file() && !linked(&metadata))
        {
            return None;
        }
        let mut bytes = Vec::new();
        let result = File::open(&path)
            .and_then(|file| file.take(MAX_THUMBNAIL_BYTES + 1).read_to_end(&mut bytes));
        match result {
            Ok(_) if !bytes.is_empty() && bytes.len() as u64 <= MAX_THUMBNAIL_BYTES => {}
            Ok(_) => return None,
            Err(error) => {
                if error.kind() == io::ErrorKind::NotFound {
                    index.forget(&path);
                }
                return None;
            }
        }
        let used = SystemTime::now();
        // mtime is our access timestamp, including on filesystems where atime is
        // disabled. Persist it so LRU ordering survives an app restart.
        if let Ok(file) = File::options().write(true).open(&path) {
            let _ = file.set_times(FileTimes::new().set_modified(used));
        }
        index.record(path, bytes.len() as u64, used);
        Some(bytes)
    }

    pub(crate) fn store(&self, directory: &str, filename: &str, bytes: &[u8]) -> io::Result<()> {
        let path = self
            .path(directory, filename)
            .ok_or_else(|| io::Error::other("Invalid cover-cache path"))?;
        let size = bytes.len() as u64;
        if size == 0 || size > self.budget || size > MAX_THUMBNAIL_BYTES {
            return Ok(());
        }
        let mut guard = self.index.lock().unwrap_or_else(|error| error.into_inner());
        let index = guard.get_or_insert_with(|| self.initialize());
        let parent = path.parent().expect("cache path has a parent");
        fs::create_dir_all(parent)?;
        if !regular_directory(parent) {
            return Err(io::Error::other("Cover-cache directory is a link"));
        }
        // A concurrent decode may have already published this fingerprint.
        if let Ok(metadata) = fs::symlink_metadata(&path) {
            if !metadata.is_file() || linked(&metadata) {
                return Err(io::Error::other("Cover-cache entry is not a regular file"));
            }
            if index.entries.contains_key(&path)
                && metadata.len() > 0
                && metadata.len() <= MAX_THUMBNAIL_BYTES
            {
                return Ok(());
            }
            // Replace an empty/oversized cache entry rather than decoding the
            // same source on every request. A failed removal stays counted.
            fs::remove_file(&path)?;
        }
        index.forget(&path);
        index.evict_to(self.budget - size);
        if index.bytes > self.budget - size {
            return Ok(());
        }
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = path.with_extension(format!("{}-{sequence}.tmp", std::process::id()));
        let mut file = File::options()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let result = file.write_all(bytes);
        drop(file);
        let result = result.and_then(|()| fs::rename(&temporary, &path));
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result?;
        index.record(path, size, SystemTime::now());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::Arc, thread, time::Duration};

    fn name(number: u64) -> String {
        format!("{number:016x}-64.webp")
    }

    fn seed(root: &Path, directory: &str, number: u64, bytes: usize, age: u64) -> PathBuf {
        let path = root.join(directory).join(name(number));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, vec![1; bytes]).unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(age)))
            .unwrap();
        path
    }

    fn disk_bytes(root: &Path) -> u64 {
        DIRECTORIES
            .iter()
            .map(|directory| {
                fs::read_dir(root.join(directory))
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter(|file| thumbnail_name(&file.file_name().to_string_lossy()))
                    .map(|file| file.metadata().unwrap().len())
                    .sum::<u64>()
            })
            .sum()
    }

    #[test]
    fn startup_evicts_oldest_across_all_directories_and_only_cleans_owned_files() {
        let root = tempfile::tempdir().unwrap();
        let old = seed(root.path(), "covers", 1, 4, 1);
        let embedded = seed(root.path(), "embedded-album-covers", 2, 4, 2);
        let inbox = seed(root.path(), "inbox-covers", 3, 4, 3);
        let selected = seed(root.path(), "selected-covers", 4, 4, 4);
        let stage = old.with_extension("999-1.tmp");
        fs::write(&stage, b"unfinished").unwrap();
        let unrelated = root.path().join("covers/source.jpg");
        fs::write(&unrelated, b"source").unwrap();
        let unrelated_tmp = root.path().join("covers/other.tmp");
        fs::write(&unrelated_tmp, b"other").unwrap();
        let other_cache = seed(root.path(), "artist-images", 5, 20, 0);
        let cache = CoverCache::new(root.path().to_owned(), 8);
        cache.sweep();
        assert!(!old.exists());
        assert!(!embedded.exists());
        assert!(inbox.exists() && selected.exists());
        assert!(!stage.exists());
        assert!(unrelated.exists() && unrelated_tmp.exists() && other_cache.exists());
        assert_eq!(disk_bytes(root.path()), 8);
    }

    #[test]
    fn reads_refresh_lru_order_and_persist_it_across_restart() {
        let root = tempfile::tempdir().unwrap();
        let hot = seed(root.path(), "covers", 1, 4, 1);
        let cold = seed(root.path(), "embedded-album-covers", 2, 4, 2);
        let cache = CoverCache::new(root.path().to_owned(), 8);
        assert_eq!(cache.read("covers", &name(1)), Some(vec![1; 4]));
        drop(cache);
        let cache = CoverCache::new(root.path().to_owned(), 8);
        cache.store("inbox-covers", &name(3), &[2; 4]).unwrap();
        assert!(hot.exists());
        assert!(!cold.exists());
        assert_eq!(cache.read("inbox-covers", &name(3)), Some(vec![2; 4]));
        assert_eq!(disk_bytes(root.path()), 8);
    }

    #[test]
    fn concurrent_writes_remain_within_one_budget_without_staging_orphans() {
        let root = tempfile::tempdir().unwrap();
        let cache = Arc::new(CoverCache::new(root.path().to_owned(), 32));
        let threads = (0..3)
            .map(|worker| {
                let cache = Arc::clone(&cache);
                thread::spawn(move || {
                    for number in 0..20 {
                        cache
                            .store(DIRECTORIES[worker], &name(number), &[1; 8])
                            .unwrap();
                    }
                })
            })
            .collect::<Vec<_>>();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(disk_bytes(root.path()), 32);
        for directory in DIRECTORIES {
            for file in fs::read_dir(root.path().join(directory))
                .into_iter()
                .flatten()
                .flatten()
            {
                assert!(thumbnail_name(&file.file_name().to_string_lossy()));
            }
        }
    }

    #[test]
    fn oversized_entries_and_paths_outside_the_cache_are_not_stored() {
        let root = tempfile::tempdir().unwrap();
        let cache = CoverCache::new(root.path().to_owned(), 4);
        cache.store("covers", &name(1), &[1; 5]).unwrap();
        assert_eq!(disk_bytes(root.path()), 0);
        assert!(cache.store("../archive", &name(1), &[1]).is_err());
        assert!(cache.store("covers", "../source.jpg", &[1]).is_err());
        assert_eq!(cache.read("covers", "../source.jpg"), None);
    }

    #[cfg(windows)]
    #[test]
    fn locked_files_stay_counted_and_writes_resume_after_unlock() {
        use std::os::windows::fs::OpenOptionsExt;
        let root = tempfile::tempdir().unwrap();
        let old = seed(root.path(), "covers", 1, 8, 1);
        let cache = CoverCache::new(root.path().to_owned(), 8);
        cache.sweep();
        let locked = File::options().read(true).share_mode(0).open(&old).unwrap();
        cache.store("inbox-covers", &name(2), &[2; 4]).unwrap();
        assert!(!root.path().join("inbox-covers").join(name(2)).exists());
        assert_eq!(cache.index.lock().unwrap().as_ref().unwrap().bytes, 8);
        drop(locked);
        cache.store("inbox-covers", &name(2), &[2; 4]).unwrap();
        assert!(!old.exists());
        assert_eq!(disk_bytes(root.path()), 4);
    }
}
