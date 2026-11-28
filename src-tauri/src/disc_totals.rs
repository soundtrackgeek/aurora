//! Device-local Disc Total cache behind the `disc:` search. Music Library's
//! catalog stores Disc# but not Disc Total, so Aurora reads TPOS from the files
//! in the background and keeps the result here, keyed by catalog path.
//!
//! A row with `disc_total = NULL` is a file whose tag was read and has no Disc
//! Total; a file without a row has not been read yet and matches no `disc:` term.
use crate::{device_mode, state_store::StateStore};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use std::{
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant, UNIX_EPOCH},
};

const FILE_NAME: &str = "aurora-disc-totals.sqlite3";
const BATCH: i64 = 500;
/// How long the catalog walk waits before checking again for new tracks.
const FILL_REST: Duration = Duration::from_secs(10 * 60);
/// How often every cached file is checked for changes made outside Aurora.
const VERIFY_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// Aurora's own tag writes are re-read this often while they wait for import.
const PENDING_EVERY: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Activity {
    Starting,
    /// Reading catalog tracks that have no cached Disc Total.
    Reading,
    /// Checking cached files for changes made outside Aurora.
    Checking,
    /// Every catalog track has been looked at; waiting for new ones.
    Resting,
    /// Network or Laptop Mode: files are not read on this device.
    Paused,
}

struct Progress {
    activity: Activity,
    session_reads: u64,
    first_read: Option<Instant>,
    error: Option<String>,
}

static PROGRESS: Mutex<Progress> = Mutex::new(Progress {
    activity: Activity::Starting,
    session_reads: 0,
    first_read: None,
    error: None,
});

fn progress() -> std::sync::MutexGuard<'static, Progress> {
    PROGRESS.lock().unwrap_or_else(|error| error.into_inner())
}

fn set_activity(activity: Activity) {
    progress().activity = activity;
}

fn count_reads(count: usize) {
    if count == 0 {
        return;
    }
    let mut progress = progress();
    progress.first_read.get_or_insert_with(Instant::now);
    progress.session_reads += count as u64;
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScanStatus {
    pub(crate) activity: Activity,
    /// Files with a cached Disc Total, capped at the catalog's file count.
    pub(crate) read_files: i64,
    pub(crate) catalog_files: i64,
    /// Read rate since Aurora started, once there is enough to measure.
    pub(crate) files_per_minute: Option<f64>,
    pub(crate) error: Option<String>,
}

/// What the scanner is doing and how much of the catalog it has read.
pub(crate) fn status() -> Result<ScanStatus, String> {
    let catalog_path = crate::catalog::default_catalog_path()?;
    let catalog_files = if catalog_path.is_file() {
        crate::catalog::open_catalog_for_lookup(&catalog_path)?
            .query_row(
                "SELECT count(*) FROM tracks WHERE file_path IS NOT NULL AND filename IS NOT NULL",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| format!("Could not count catalog files: {error}"))?
    } else {
        0
    };
    let cached = match cache_path().filter(|path| path.is_file()) {
        Some(path) => open_cache(&path)?
            .query_row("SELECT count(*) FROM disc_totals", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|error| error.to_string())?,
        None => 0,
    };
    let progress = progress();
    let files_per_minute = progress.first_read.and_then(|first| {
        let minutes = first.elapsed().as_secs_f64() / 60.0;
        (minutes >= 1.0 && progress.session_reads >= 50)
            .then(|| progress.session_reads as f64 / minutes)
    });
    Ok(ScanStatus {
        activity: progress.activity,
        read_files: cached.min(catalog_files),
        catalog_files,
        files_per_minute,
        error: progress.error.clone(),
    })
}

pub(crate) fn cache_path() -> Option<PathBuf> {
    #[cfg(test)]
    {
        std::env::var_os("AURORA_PROOF_DISC_CACHE").map(PathBuf::from)
    }
    #[cfg(not(test))]
    {
        crate::mcp::app_data_dir()
            .ok()
            .map(|directory| directory.join(FILE_NAME))
    }
}

fn open_cache(path: &Path) -> Result<Connection, String> {
    let connection = Connection::open(path)
        .map_err(|error| format!("Could not open the Disc Total cache: {error}"))?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|error| error.to_string())?;
    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS disc_totals (
               directory TEXT NOT NULL, filename TEXT NOT NULL, disc_total INTEGER,
               size_bytes INTEGER NOT NULL, modified_ns INTEGER NOT NULL,
               PRIMARY KEY (directory, filename)
             ) WITHOUT ROWID;
             CREATE INDEX IF NOT EXISTS disc_totals_value ON disc_totals(disc_total);",
        )
        .map_err(|error| format!("Could not prepare the Disc Total cache: {error}"))?;
    Ok(connection)
}

/// Exposes the cache to searches as `temp.aurora_disc_totals` (by catalog
/// path). Without a cache the table is empty and `disc:` matches nothing.
pub(crate) fn prepare_search(c: &Connection, cache: Option<&Path>) -> Result<(), String> {
    if let Some(path) = cache.filter(|path| path.is_file()) {
        c.execute(
            "ATTACH DATABASE ?1 AS disc_cache",
            [path.to_string_lossy().as_ref()],
        )
        .map_err(|error| format!("Could not open the Disc Total cache: {error}"))?;
        let ready = c
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM disc_cache.sqlite_master WHERE type = 'table' AND name = 'disc_totals')",
                [],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|error| error.to_string())?;
        if ready {
            return c
                .execute_batch(
                    "CREATE TEMP VIEW aurora_disc_totals AS
                     SELECT directory AS file_path, filename, disc_total FROM disc_cache.disc_totals;",
                )
                .map_err(|error| error.to_string());
        }
        c.execute_batch("DETACH DATABASE disc_cache")
            .map_err(|error| error.to_string())?;
    }
    c.execute_batch(
        "CREATE TEMP TABLE aurora_disc_totals (
           file_path TEXT, filename TEXT, disc_total INTEGER, PRIMARY KEY (file_path, filename)
         );",
    )
    .map_err(|error| error.to_string())
}

/// The Disc Total in a TPOS value such as `1/3`; `1` or an empty total is none.
fn parse_total(text: &str) -> Option<u32> {
    let (_, total) = text.split_once(['/', '\0'])?;
    total
        .trim_matches(|c: char| c.is_whitespace() || c == '\0')
        .parse()
        .ok()
}

fn synchsafe(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(0, |value, byte| (value << 7) | u64::from(byte & 0x7f))
}

fn big_endian(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(0, |value, byte| (value << 8) | u64::from(*byte))
}

fn decode_text(body: &[u8]) -> Option<String> {
    let (&encoding, text) = body.split_first()?;
    let utf16 = |bytes: &[u8], big: bool| {
        let units = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&pair| {
                if big {
                    u16::from_be_bytes(pair)
                } else {
                    u16::from_le_bytes(pair)
                }
            })
            .collect::<Vec<_>>();
        String::from_utf16(&units).ok()
    };
    match encoding {
        0 => Some(text.iter().map(|&byte| char::from(byte)).collect()),
        1 => match text {
            [0xff, 0xfe, rest @ ..] => utf16(rest, false),
            [0xfe, 0xff, rest @ ..] => utf16(rest, true),
            _ => utf16(text, true),
        },
        2 => utf16(text, true),
        3 => String::from_utf8(text.to_vec()).ok(),
        _ => None,
    }
}

/// Walks the ID3v2 frame headers and decodes only TPOS, so a file's cover art is
/// skipped instead of read. `Ok(None)` asks the full tag reader to decide: the
/// tag is unsynchronised, compressed, or not laid out as its header says.
fn scan_disc_total(file: &mut (impl Read + Seek)) -> io::Result<Option<Option<u32>>> {
    let mut header = [0_u8; 10];
    match file.read_exact(&mut header) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(Some(None)),
        Err(error) => return Err(error),
    }
    if &header[..3] != b"ID3" {
        return Ok(Some(None));
    }
    let version = header[3];
    let flags = header[5];
    if !(2..=4).contains(&version) || flags & 0x80 != 0 || (version == 2 && flags & 0x40 != 0) {
        return Ok(None);
    }
    let end = 10 + synchsafe(&header[6..10]);
    let mut position = 10;
    if flags & 0x40 != 0 {
        let mut size = [0_u8; 4];
        file.read_exact(&mut size)?;
        position += if version == 4 {
            synchsafe(&size)
        } else {
            4 + big_endian(&size)
        };
    }
    let (id_length, header_length) = if version == 2 { (3, 6) } else { (4, 10) };
    let wanted: &[u8] = if version == 2 { b"TPA" } else { b"TPOS" };
    while position + header_length <= end {
        file.seek(SeekFrom::Start(position))?;
        let mut frame = [0_u8; 10];
        file.read_exact(&mut frame[..header_length as usize])?;
        let id = &frame[..id_length];
        if id[0] == 0 {
            break;
        }
        if !id
            .iter()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        {
            return Ok(None);
        }
        let size = match version {
            2 => big_endian(&frame[3..6]),
            3 => big_endian(&frame[4..8]),
            _ => synchsafe(&frame[4..8]),
        };
        position += header_length;
        if position + size > end {
            return Ok(None);
        }
        if id == wanted {
            let format = if version == 2 { 0 } else { frame[9] };
            // v2.3 compression/encryption, v2.4 compression/encryption/unsync/length.
            let unusual = if version == 3 { 0xc0 } else { 0x0f };
            if format & unusual != 0 || size > 4096 {
                return Ok(None);
            }
            let mut body = vec![0_u8; size as usize];
            file.read_exact(&mut body)?;
            return Ok(decode_text(&body).map(|text| parse_total(&text)));
        }
        position += size;
    }
    Ok(Some(None))
}

fn read_disc_total(path: &Path) -> io::Result<Option<u32>> {
    let mut file = File::open(path)?;
    if let Some(total) = scan_disc_total(&mut file)? {
        return Ok(total);
    }
    use id3::TagLike;
    match id3::Tag::read_from_path(path) {
        Ok(tag) => Ok(tag
            .get("TPOS")
            .and_then(|frame| frame.content().text())
            .and_then(parse_total)),
        Err(error) if matches!(error.kind, id3::ErrorKind::NoTag) => Ok(None),
        Err(error) => Err(io::Error::other(error.to_string())),
    }
}

fn file_stamp(path: &Path) -> io::Result<(i64, i64)> {
    let metadata = fs::metadata(path)?;
    let modified = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map(|age| i64::try_from(age.as_nanos()).unwrap_or(i64::MAX))
        .unwrap_or(0);
    Ok((i64::try_from(metadata.len()).unwrap_or(i64::MAX), modified))
}

struct Reading {
    directory: String,
    filename: String,
    disc_total: Option<u32>,
    stamp: (i64, i64),
}

/// Reads one catalog file; `None` when it cannot be read right now.
fn read_catalog_file(directory: &str, filename: &str) -> Option<Reading> {
    let path = device_mode::resolve_device_path(Path::new(directory)).join(filename);
    let stamp = file_stamp(&path).ok()?;
    let disc_total = read_disc_total(&path).ok()?;
    Some(Reading {
        directory: directory.to_owned(),
        filename: filename.to_owned(),
        disc_total,
        stamp,
    })
}

fn save(
    cache: &mut Connection,
    readings: &[Reading],
    gone: &[(String, String)],
) -> Result<(), String> {
    if readings.is_empty() && gone.is_empty() {
        return Ok(());
    }
    let transaction = cache.transaction().map_err(|error| error.to_string())?;
    for reading in readings {
        transaction
            .execute(
                "INSERT INTO disc_totals VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(directory, filename) DO UPDATE SET disc_total = excluded.disc_total,
                   size_bytes = excluded.size_bytes, modified_ns = excluded.modified_ns",
                params![
                    reading.directory,
                    reading.filename,
                    reading.disc_total,
                    reading.stamp.0,
                    reading.stamp.1
                ],
            )
            .map_err(|error| format!("Could not save a Disc Total: {error}"))?;
    }
    for (directory, filename) in gone {
        transaction
            .execute(
                "DELETE FROM disc_totals WHERE directory = ?1 AND filename = ?2",
                params![directory, filename],
            )
            .map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())
}

fn cached_stamp(
    cache: &Connection,
    directory: &str,
    filename: &str,
) -> Result<Option<(i64, i64)>, String> {
    cache
        .query_row(
            "SELECT size_bytes, modified_ns FROM disc_totals WHERE directory = ?1 AND filename = ?2",
            params![directory, filename],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())
}

/// Re-reads a file when it changed since it was cached, or was never read.
fn refresh_if_changed(
    cache: &Connection,
    directory: &str,
    filename: &str,
    readings: &mut Vec<Reading>,
    gone: &mut Vec<(String, String)>,
) -> Result<(), String> {
    let cached = cached_stamp(cache, directory, filename)?;
    let path = device_mode::resolve_device_path(Path::new(directory)).join(filename);
    match file_stamp(&path) {
        Ok(stamp) if cached == Some(stamp) => {}
        Ok(_) => readings.extend(read_catalog_file(directory, filename)),
        Err(error) if error.kind() == io::ErrorKind::NotFound && cached.is_some() => {
            gone.push((directory.to_owned(), filename.to_owned()));
        }
        Err(_) => {}
    }
    Ok(())
}

#[derive(Debug)]
enum Phase {
    /// Reading every catalog track that has no cached row, in id order.
    Fill {
        after_id: i64,
    },
    /// Checking every cached file for changes, in key order.
    Verify {
        after: Option<(String, String)>,
    },
    Rest {
        until: Instant,
    },
}

/// Fills and refreshes the cache in short slices on a background thread.
pub(crate) struct Scanner {
    phase: Phase,
    next_verify: Instant,
    next_pending: Instant,
}

impl Default for Scanner {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            phase: Phase::Fill { after_id: 0 },
            next_verify: now + VERIFY_EVERY,
            next_pending: now,
        }
    }
}

impl Scanner {
    /// Does up to `budget` of work. Files on a share are not read in Network or
    /// Laptop Mode; the cache then keeps what the home PC read.
    pub(crate) fn run_slice(&mut self, store: &StateStore, budget: Duration) -> Result<(), String> {
        if crate::connections::network_mode() || device_mode::laptop_mode_enabled() {
            set_activity(Activity::Paused);
            return Ok(());
        }
        let outcome = self.work(store, budget);
        let mut progress = progress();
        progress.activity = match self.phase {
            Phase::Fill { .. } => Activity::Reading,
            Phase::Verify { .. } => Activity::Checking,
            Phase::Rest { .. } => Activity::Resting,
        };
        progress.error = outcome.as_ref().err().cloned();
        outcome
    }

    fn work(&mut self, store: &StateStore, budget: Duration) -> Result<(), String> {
        let Some(cache_path) = cache_path() else {
            return Ok(());
        };
        let catalog_path = crate::catalog::default_catalog_path()?;
        if !catalog_path.is_file() {
            return Ok(());
        }
        let catalog = crate::catalog::open_catalog_for_lookup(&catalog_path)?;
        let mut cache = open_cache(&cache_path)?;
        let deadline = Instant::now() + budget;
        if Instant::now() >= self.next_pending {
            self.next_pending = Instant::now() + PENDING_EVERY;
            refresh_pending(&catalog, &mut cache, store)?;
        }
        while Instant::now() < deadline {
            match &self.phase {
                Phase::Rest { until } => {
                    if Instant::now() < *until {
                        return Ok(());
                    }
                    self.phase = if Instant::now() >= self.next_verify {
                        Phase::Verify { after: None }
                    } else {
                        Phase::Fill { after_id: 0 }
                    };
                }
                Phase::Fill { after_id } => {
                    let after_id = *after_id;
                    self.phase = match fill_batch(&catalog, &mut cache, after_id, deadline)? {
                        Some(last) => Phase::Fill { after_id: last },
                        None => Phase::Rest {
                            until: Instant::now() + FILL_REST,
                        },
                    };
                }
                Phase::Verify { after } => {
                    let after = after.clone();
                    self.phase = match verify_batch(&mut cache, after, deadline)? {
                        Some(last) => Phase::Verify { after: Some(last) },
                        None => {
                            self.next_verify = Instant::now() + VERIFY_EVERY;
                            Phase::Fill { after_id: 0 }
                        }
                    };
                }
            }
        }
        Ok(())
    }
}

/// Reads the catalog tracks after `after_id` that have no cached row. Returns
/// the last id looked at, or `None` once the whole catalog has been walked.
fn fill_batch(
    catalog: &Connection,
    cache: &mut Connection,
    after_id: i64,
    deadline: Instant,
) -> Result<Option<i64>, String> {
    let rows = catalog
        .prepare_cached(
            "SELECT id, file_path, filename FROM tracks
             WHERE id > ?1 AND file_path IS NOT NULL AND filename IS NOT NULL
             ORDER BY id LIMIT ?2",
        )
        .and_then(|mut statement| {
            statement
                .query_map(params![after_id, BATCH], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(|error| format!("Could not read catalog tracks for Disc Total: {error}"))?;
    if rows.is_empty() {
        return Ok(None);
    }
    let mut readings = Vec::new();
    let mut reached = after_id;
    for (id, directory, filename) in rows {
        if Instant::now() >= deadline && reached > after_id {
            break;
        }
        reached = id;
        if cached_stamp(cache, &directory, &filename)?.is_none() {
            readings.extend(read_catalog_file(&directory, &filename));
        }
    }
    save(cache, &readings, &[])?;
    count_reads(readings.len());
    Ok(Some(reached))
}

fn verify_batch(
    cache: &mut Connection,
    after: Option<(String, String)>,
    deadline: Instant,
) -> Result<Option<(String, String)>, String> {
    let (directory, filename) = after.clone().unwrap_or_default();
    let rows = cache
        .prepare(
            "SELECT directory, filename FROM disc_totals
             WHERE ?1 = 0 OR (directory, filename) > (?2, ?3)
             ORDER BY directory, filename LIMIT ?4",
        )
        .and_then(|mut statement| {
            statement
                .query_map(
                    params![after.is_some(), directory, filename, BATCH],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )?
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(|error| error.to_string())?;
    if rows.is_empty() {
        return Ok(None);
    }
    let mut readings = Vec::new();
    let mut gone = Vec::new();
    let mut reached = None;
    for (directory, filename) in rows {
        if Instant::now() >= deadline && reached.is_some() {
            break;
        }
        refresh_if_changed(cache, &directory, &filename, &mut readings, &mut gone)?;
        reached = Some((directory, filename));
    }
    save(cache, &readings, &gone)?;
    Ok(reached)
}

/// Re-reads the files Aurora wrote that wait for Music Library, so a Disc Total
/// edited in Aurora is searchable before the next full pass.
fn refresh_pending(
    catalog: &Connection,
    cache: &mut Connection,
    store: &StateStore,
) -> Result<(), String> {
    let mut files = Vec::new();
    for pending in store.pending_library_folder_sync_targets(64)? {
        match pending.filename {
            Some(filename) => files.push((pending.directory, filename)),
            None => {
                let mut statement = catalog
                    .prepare_cached(
                        "SELECT filename FROM tracks WHERE file_path = ?1 AND filename IS NOT NULL",
                    )
                    .map_err(|error| error.to_string())?;
                let names = statement
                    .query_map([&pending.directory], |row| row.get::<_, String>(0))
                    .and_then(Iterator::collect::<Result<Vec<_>, _>>)
                    .map_err(|error| error.to_string())?;
                files.extend(
                    names
                        .into_iter()
                        .map(|name| (pending.directory.clone(), name)),
                );
            }
        }
    }
    let mut readings = Vec::new();
    let mut gone = Vec::new();
    for (directory, filename) in files.iter().take(2000) {
        refresh_if_changed(cache, directory, filename, &mut readings, &mut gone)?;
    }
    save(cache, &readings, &gone)
}

#[cfg(test)]
mod tests {
    use super::*;
    use id3::{Frame, Tag, TagLike, Version, frame::Picture, frame::PictureType};
    use std::io::Cursor;

    fn tagged(version: Version, tpos: Option<&str>) -> Vec<u8> {
        let mut tag = Tag::new();
        tag.set_title("Song");
        tag.add_frame(Picture {
            mime_type: "image/jpeg".to_owned(),
            picture_type: PictureType::CoverFront,
            description: String::new(),
            data: vec![0x55; 300_000],
        });
        if let Some(tpos) = tpos {
            tag.add_frame(Frame::text("TPOS", tpos));
        }
        let mut bytes = Vec::new();
        tag.write_to(&mut bytes, version).unwrap();
        bytes.extend_from_slice(&[0xff, 0xfb, 0x90, 0x00]);
        bytes
    }

    #[test]
    fn reads_disc_total_after_cover_art_without_full_tag_reader() {
        for version in [Version::Id3v23, Version::Id3v24] {
            let scan = |tpos| scan_disc_total(&mut Cursor::new(tagged(version, tpos))).unwrap();
            assert_eq!(scan(Some("2/3")), Some(Some(3)), "{version:?}");
            assert_eq!(scan(Some("1/0")), Some(Some(0)), "{version:?}");
            assert_eq!(scan(Some("1")), Some(None), "{version:?}");
            assert_eq!(scan(Some("1/")), Some(None), "{version:?}");
            assert_eq!(scan(None), Some(None), "{version:?}");
        }
        assert_eq!(
            scan_disc_total(&mut Cursor::new(b"\xff\xfb\x90\x00".to_vec())).unwrap(),
            Some(None)
        );
        assert_eq!(
            scan_disc_total(&mut Cursor::new(Vec::new())).unwrap(),
            Some(None)
        );
    }

    #[test]
    fn unusual_tags_fall_back_to_the_full_reader() {
        let mut bytes = tagged(Version::Id3v23, Some("1/2"));
        bytes[5] |= 0x80;
        assert_eq!(scan_disc_total(&mut Cursor::new(bytes)).unwrap(), None);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("a.mp3");
        let mut tag = Tag::new();
        tag.add_frame(Frame::text("TPOS", "1/4"));
        fs::write(&path, b"\xff\xfb\x90\x00").unwrap();
        tag.write_to_path(&path, Version::Id3v24).unwrap();
        assert_eq!(read_disc_total(&path).unwrap(), Some(4));
    }

    #[test]
    fn decodes_every_text_encoding() {
        assert_eq!(decode_text(b"\x001/2").as_deref(), Some("1/2"));
        assert_eq!(decode_text(b"\x031/2").as_deref(), Some("1/2"));
        assert_eq!(
            decode_text(b"\x01\xff\xfe1\x00/\x002\x00").as_deref(),
            Some("1/2")
        );
        assert_eq!(decode_text(b"\x02\x001\x00/\x002").as_deref(), Some("1/2"));
        assert_eq!(parse_total("1/2\0"), Some(2));
        assert_eq!(parse_total(" 1 / 2 "), Some(2));
    }

    #[test]
    fn fill_reads_new_tracks_and_verify_rereads_changed_files() {
        let directory = tempfile::tempdir().unwrap();
        let folder = directory.path().to_string_lossy().into_owned();
        let write = |name: &str, tpos: Option<&str>| {
            fs::write(directory.path().join(name), tagged(Version::Id3v24, tpos)).unwrap();
        };
        write("01.mp3", Some("1/2"));
        write("02.mp3", None);
        let catalog = Connection::open_in_memory().unwrap();
        catalog
            .execute_batch(
                "CREATE TABLE tracks (id INTEGER PRIMARY KEY, file_path TEXT, filename TEXT);",
            )
            .unwrap();
        for (id, name) in [(1, "01.mp3"), (2, "02.mp3"), (3, "missing.mp3")] {
            catalog
                .execute(
                    "INSERT INTO tracks VALUES (?1, ?2, ?3)",
                    params![id, folder, name],
                )
                .unwrap();
        }
        let mut cache = open_cache(&directory.path().join(FILE_NAME)).unwrap();
        let far = Instant::now() + Duration::from_secs(60);
        assert_eq!(fill_batch(&catalog, &mut cache, 0, far).unwrap(), Some(3));
        assert_eq!(fill_batch(&catalog, &mut cache, 3, far).unwrap(), None);
        let totals = |cache: &Connection| {
            cache
                .prepare("SELECT filename, disc_total FROM disc_totals ORDER BY filename")
                .unwrap()
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, Option<u32>>(1)?))
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert_eq!(
            totals(&cache),
            [("01.mp3".to_owned(), Some(2)), ("02.mp3".to_owned(), None)]
        );

        write("02.mp3", Some("1/0"));
        fs::remove_file(directory.path().join("01.mp3")).unwrap();
        let mut after = None;
        while let Some(last) = verify_batch(&mut cache, after, far).unwrap() {
            after = Some(last);
        }
        assert_eq!(totals(&cache), [("02.mp3".to_owned(), Some(0))]);
    }

    #[test]
    fn status_serializes_for_the_settings_panel() {
        let status = ScanStatus {
            activity: Activity::Reading,
            read_files: 2,
            catalog_files: 3,
            files_per_minute: None,
            error: None,
        };
        assert_eq!(
            serde_json::to_value(status).unwrap(),
            serde_json::json!({
                "activity": "reading", "readFiles": 2, "catalogFiles": 3,
                "filesPerMinute": null, "error": null
            })
        );
    }

    #[test]
    fn search_sees_the_cache_and_an_absent_cache_matches_nothing() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(FILE_NAME);
        let mut cache = open_cache(&path).unwrap();
        save(
            &mut cache,
            &[Reading {
                directory: "D:\\Music\\A".to_owned(),
                filename: "01.mp3".to_owned(),
                disc_total: Some(2),
                stamp: (1, 1),
            }],
            &[],
        )
        .unwrap();
        let c = Connection::open_in_memory().unwrap();
        prepare_search(&c, Some(&path)).unwrap();
        let count = |c: &Connection| {
            c.query_row(
                "SELECT count(*) FROM temp.aurora_disc_totals WHERE disc_total = 2",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
        };
        assert_eq!(count(&c), 1);
        let empty = Connection::open_in_memory().unwrap();
        prepare_search(&empty, Some(&directory.path().join("missing.sqlite3"))).unwrap();
        assert_eq!(count(&empty), 0);
    }
}
