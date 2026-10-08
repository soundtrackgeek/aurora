//! Shared derived index adapter. Keep this file aligned with Aurora.
//! SQL generations and bounded change logs bridge immutable tree snapshots.
use music_sonic_core::{
    AlbumAccumulator, Analysis, DIMENSIONS, PROFILE,
    index::{Index, Point},
};
use rusqlite::{Connection, OpenFlags, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant, SystemTime},
};

const FILE_NAME: &str = "sonic-index.bin";
const DELTA_LIMIT: usize = 8192;
const MAX_HEADER: u64 = 16 * 1024 * 1024;
const MAX_FILE: u64 = 1536 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct Generation {
    catalog: String,
    analysis: String,
    catalog_seq: i64,
    analysis_seq: i64,
}
#[derive(Serialize, Deserialize)]
struct Header {
    version: u32,
    profile: String,
    generation: Generation,
    weights: Vec<f32>,
    coverage: Vec<(String, usize, usize)>,
    #[serde(default)]
    counts: HashMap<String, i64>,
}
struct Snapshot {
    header: Header,
    tracks: Index,
    albums: Index,
}
#[derive(Default)]
struct Cache {
    snapshot: Option<Arc<Snapshot>>,
    stamp: Option<(u64, SystemTime)>,
    building: bool,
    attempted: Option<Instant>,
    loaded: bool,
}
static CACHE: OnceLock<Mutex<HashMap<PathBuf, Cache>>> = OnceLock::new();
#[cfg(test)]
static INDEXED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(test)]
static FALLBACKS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub(crate) fn fallback() {
    #[cfg(test)]
    {
        FALLBACKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}
#[cfg(test)]
pub(crate) fn proof_stats() -> serde_json::Value {
    use std::sync::atomic::Ordering;
    serde_json::json!({"indexedCandidateRequests":INDEXED.load(Ordering::Relaxed),"exactFallbacks":FALLBACKS.load(Ordering::Relaxed)})
}
type CountKey = (Generation, String);
static COUNTS: OnceLock<Mutex<HashMap<CountKey, i64>>> = OnceLock::new();
/// Coverage counts have the same source generations as candidate retrieval.
/// Cache their SQL result; never recount a million paths on every radio refill.
pub(crate) fn count(
    c: &Connection,
    name: &str,
    sql: &str,
    persisted: bool,
) -> rusqlite::Result<i64> {
    let current = generation(c);
    let key = current.clone().map(|mut generation| {
        // A completed analysis checkpoint does not change catalog MP3 totals.
        if name == "mp3-total" {
            generation.analysis_seq = 0;
        }
        (generation, name.to_string())
    });
    let counts = COUNTS.get_or_init(Default::default);
    if let Some(value) = key.as_ref().and_then(|key| {
        counts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(key)
            .copied()
    }) {
        return Ok(value);
    }
    let saved = if persisted {
        current.as_ref().and_then(|generation| {
            let snapshot = snapshot(c, generation)?;
            if snapshot.header.generation == *generation
                || name == "mp3-total"
                    && snapshot.header.generation.catalog_seq == generation.catalog_seq
            {
                snapshot.header.counts.get(name).copied()
            } else {
                None
            }
        })
    } else {
        None
    };
    let value = if let Some(value) = saved {
        value
    } else {
        let mut q = c.prepare(sql)?;
        if q.parameter_count() == 0 {
            q.query_row([], |r| r.get(0))?
        } else {
            q.query_row([PROFILE], |r| r.get(0))?
        }
    };
    let mut counts = counts.lock().unwrap_or_else(|e| e.into_inner());
    if counts.len() > 16 {
        counts.clear();
    }
    if let Some(key) = key {
        counts.insert(key, value);
    }
    Ok(value)
}
fn cache() -> &'static Mutex<HashMap<PathBuf, Cache>> {
    CACHE.get_or_init(Default::default)
}

/// Called only by Music Library's migrations/writer. Aurora never changes either source.
#[allow(dead_code)] // The companion reader only uses this in fixtures.
pub(crate) fn install_catalog(c: &Connection) -> Result<(), String> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS sonic_index_identity(id INTEGER PRIMARY KEY CHECK(id=1),identity TEXT NOT NULL);
      INSERT OR IGNORE INTO sonic_index_identity VALUES(1,lower(hex(randomblob(16))));
      CREATE TABLE IF NOT EXISTS sonic_index_changes(seq INTEGER PRIMARY KEY AUTOINCREMENT,directory TEXT,filename TEXT,album_id TEXT);
      CREATE TRIGGER IF NOT EXISTS sonic_index_prune AFTER INSERT ON sonic_index_changes WHEN new.seq%1024=0 BEGIN DELETE FROM sonic_index_changes WHERE seq<new.seq-65536; END;
      CREATE TRIGGER IF NOT EXISTS sonic_index_track_insert AFTER INSERT ON tracks BEGIN
        INSERT INTO sonic_index_changes(directory,filename,album_id) VALUES(new.file_path,new.filename,new.album_id); END;
      CREATE TRIGGER IF NOT EXISTS sonic_index_track_delete AFTER DELETE ON tracks BEGIN
        INSERT INTO sonic_index_changes(directory,filename,album_id) VALUES(old.file_path,old.filename,old.album_id); END;
      CREATE TRIGGER IF NOT EXISTS sonic_index_track_update AFTER UPDATE OF file_path,filename,album_id,love ON tracks
      WHEN old.file_path IS NOT new.file_path OR old.filename IS NOT new.filename OR old.album_id IS NOT new.album_id OR old.love IS NOT new.love BEGIN
        INSERT INTO sonic_index_changes(directory,filename,album_id) VALUES(old.file_path,old.filename,old.album_id);
        INSERT INTO sonic_index_changes(directory,filename,album_id) VALUES(new.file_path,new.filename,new.album_id); END;")
        .map_err(|e| e.to_string())
}
#[allow(dead_code)] // The companion reader only uses this in fixtures.
pub(crate) fn install_analysis(c: &Connection) -> Result<(), String> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS sonic_index_identity(id INTEGER PRIMARY KEY CHECK(id=1),identity TEXT NOT NULL);
      INSERT OR IGNORE INTO sonic_index_identity VALUES(1,lower(hex(randomblob(16))));
      CREATE TABLE IF NOT EXISTS sonic_index_changes(seq INTEGER PRIMARY KEY AUTOINCREMENT,directory TEXT,filename TEXT,album_id TEXT);
      CREATE TRIGGER IF NOT EXISTS sonic_index_prune AFTER INSERT ON sonic_index_changes WHEN new.seq%1024=0 BEGIN DELETE FROM sonic_index_changes WHERE seq<new.seq-65536; END;
      CREATE INDEX IF NOT EXISTS sonic_tracks_audio ON sonic_tracks(audio_hash,profile);
      CREATE TRIGGER IF NOT EXISTS sonic_index_binding_insert AFTER INSERT ON sonic_tracks BEGIN
        INSERT INTO sonic_index_changes(directory,filename) VALUES(new.directory,new.filename); END;
      CREATE TRIGGER IF NOT EXISTS sonic_index_binding_delete AFTER DELETE ON sonic_tracks BEGIN
        INSERT INTO sonic_index_changes(directory,filename) VALUES(old.directory,old.filename); END;
      CREATE TRIGGER IF NOT EXISTS sonic_index_binding_update AFTER UPDATE ON sonic_tracks BEGIN
        INSERT INTO sonic_index_changes(directory,filename) VALUES(old.directory,old.filename);
        INSERT INTO sonic_index_changes(directory,filename) VALUES(new.directory,new.filename); END;
      CREATE TRIGGER IF NOT EXISTS sonic_index_audio_update AFTER UPDATE ON sonic_audio BEGIN
        INSERT INTO sonic_index_changes(directory,filename) SELECT directory,filename FROM sonic_tracks WHERE (audio_hash=old.audio_hash AND profile=old.profile) OR (audio_hash=new.audio_hash AND profile=new.profile); END;
      CREATE TRIGGER IF NOT EXISTS sonic_index_audio_insert AFTER INSERT ON sonic_audio BEGIN
        INSERT INTO sonic_index_changes(directory,filename) SELECT directory,filename FROM sonic_tracks WHERE audio_hash=new.audio_hash AND profile=new.profile; END;
      CREATE TRIGGER IF NOT EXISTS sonic_index_audio_delete BEFORE DELETE ON sonic_audio BEGIN
        INSERT INTO sonic_index_changes(directory,filename) SELECT directory,filename FROM sonic_tracks WHERE audio_hash=old.audio_hash AND profile=old.profile; END;")
        .map_err(|e|e.to_string())
}
fn generation(c: &Connection) -> Option<Generation> {
    let identity = |schema: &str| -> Option<(String, i64)> {
        c.query_row(&format!("SELECT identity,COALESCE((SELECT MAX(seq) FROM {schema}.sonic_index_changes),0) FROM {schema}.sonic_index_identity WHERE id=1"),[],|r|Ok((r.get(0)?,r.get(1)?))).ok()
    };
    let (mut catalog, catalog_seq) = identity("main")?;
    let (mut analysis, analysis_seq) = identity("sonic")?;
    let (catalog_path, analysis_path) = paths(c)?;
    // A replaced/copied database can carry the same SQL identity/counter.
    // Its filesystem incarnation must also match the index's source.
    fn incarnation(path: &Path) -> Option<String> {
        let m = fs::metadata(path).ok()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Some(format!("{}:{}", m.dev(), m.ino()))
        }
        #[cfg(not(unix))]
        {
            Some(
                m.created()
                    .ok()?
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .ok()?
                    .as_nanos()
                    .to_string(),
            )
        }
    }
    catalog.push_str(&incarnation(&catalog_path)?);
    analysis.push_str(&incarnation(&analysis_path)?);
    Some(Generation {
        catalog,
        analysis,
        catalog_seq,
        analysis_seq,
    })
}
fn paths(c: &Connection) -> Option<(PathBuf, PathBuf)> {
    let mut q = c.prepare("PRAGMA database_list").ok()?;
    let list = q
        .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
        .ok()?
        .collect::<rusqlite::Result<HashMap<_, _>>>()
        .ok()?;
    let catalog = PathBuf::from(list.get("main")?);
    let analysis = PathBuf::from(list.get("sonic")?);
    if !catalog.is_absolute() || !analysis.is_absolute() {
        return None;
    }
    Some((catalog, analysis))
}
fn stamp(path: &Path) -> Option<(u64, SystemTime)> {
    let m = fs::metadata(path).ok()?;
    Some((m.len(), m.modified().ok()?))
}
fn error(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn load(path: &Path) -> io::Result<Snapshot> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    if !(40..=MAX_FILE).contains(&size) {
        return Err(error("Invalid index size"));
    }
    let mut hash = Sha256::new();
    io::copy(&mut (&file).take(size - 32), &mut hash)?;
    file.seek(SeekFrom::Start(size - 32))?;
    let mut expected = [0; 32];
    file.read_exact(&mut expected)?;
    if hash.finalize()[..] != expected {
        return Err(error("Index checksum mismatch"));
    }
    file.rewind()?;
    let mut reader = BufReader::new(file.take(size - 32));
    let mut length = [0; 4];
    reader.read_exact(&mut length)?;
    let length = u32::from_le_bytes(length) as u64;
    if length > MAX_HEADER {
        return Err(error("Invalid index header"));
    }
    let mut json = vec![0; length as usize];
    reader.read_exact(&mut json)?;
    let header: Header = serde_json::from_slice(&json).map_err(io::Error::other)?;
    if header.version != 1 || header.profile != PROFILE || header.counts.values().any(|v| *v < 0) {
        return Err(error("Unsupported index profile/format"));
    }
    let tracks = Index::read(&mut reader)?;
    let albums = Index::read(&mut reader)?;
    let analysis = Analysis {
        profile: header.profile.clone(),
        features: vec![0.; DIMENSIONS],
        weights: header.weights.clone(),
    };
    if !tracks.compatible(&analysis)
        || !albums.compatible(&analysis)
        || reader.read(&mut [0; 1])? != 0
    {
        return Err(error("Incompatible index metric"));
    }
    Ok(Snapshot {
        header,
        tracks,
        albums,
    })
}
fn save(path: &Path, snapshot: &Snapshot) -> io::Result<()> {
    let mut temp = tempfile::NamedTempFile::new_in(
        path.parent()
            .ok_or_else(|| error("Missing index directory"))?,
    )?;
    {
        let mut writer = BufWriter::new(temp.as_file_mut());
        let header = serde_json::to_vec(&snapshot.header).map_err(io::Error::other)?;
        if header.len() as u64 > MAX_HEADER {
            return Err(error("Index header too large"));
        }
        writer.write_all(&(header.len() as u32).to_le_bytes())?;
        writer.write_all(&header)?;
        snapshot.tracks.write(&mut writer)?;
        snapshot.albums.write(&mut writer)?;
        writer.flush()?;
    }
    if temp.as_file().metadata()?.len() > MAX_FILE - 32 {
        return Err(error("Index file too large"));
    }
    temp.as_file_mut().rewind()?;
    let mut hash = Sha256::new();
    io::copy(temp.as_file_mut(), &mut hash)?;
    temp.as_file_mut().write_all(&hash.finalize())?;
    temp.as_file_mut().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}
pub(crate) fn rebuild(catalog: &Path, analysis: &Path) -> Result<(), String> {
    let dir = analysis.parent().ok_or("Analysis directory missing")?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join("sonic-index.lock"))
        .map_err(|e| e.to_string())?;
    lock.try_lock()
        .map_err(|_| "Similarity index is already being built")?;
    // Preserve an artifact written by a newer index format. A corrupt current
    // artifact can be rebuilt without touching completed analysis.
    if let Ok(mut file) = File::open(dir.join(FILE_NAME)) {
        let mut length = [0; 4];
        if file.read_exact(&mut length).is_ok() {
            let length = u32::from_le_bytes(length) as u64;
            if length <= MAX_HEADER {
                let mut json = vec![0; length as usize];
                if file.read_exact(&mut json).is_ok()
                    && serde_json::from_slice::<serde_json::Value>(&json)
                        .ok()
                        .and_then(|value| value["version"].as_u64())
                        .is_some_and(|version| version > 1)
                {
                    return Err("Similarity index needs a newer app version".into());
                }
            }
        }
    }
    let c = Connection::open_with_flags(
        catalog,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|e| e.to_string())?;
    c.busy_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let mut uri = url::Url::from_file_path(analysis).map_err(|_| "Invalid analysis cache path")?;
    uri.set_query(Some("mode=ro"));
    c.execute("ATTACH DATABASE ?1 AS sonic", [uri.as_str()])
        .map_err(|e| e.to_string())?;
    c.execute_batch("PRAGMA query_only=ON")
        .map_err(|e| e.to_string())?;
    let tx = c.unchecked_transaction().map_err(|e| e.to_string())?;
    let generation =
        generation(&tx).ok_or("Index generations are unavailable; update Music Library first")?;
    let weights: String = tx
        .query_row(
            "SELECT weights FROM sonic.sonic_profiles WHERE profile=?1",
            [PROFILE],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let analysis = Analysis {
        profile: PROFILE.into(),
        features: vec![0.; DIMENSIONS],
        weights: serde_json::from_str(&weights).map_err(|e| e.to_string())?,
    };
    let mut tracks = vec![];
    let mut rolls: HashMap<String, AlbumAccumulator> = HashMap::new();
    {
        let mut q=tx.prepare("SELECT s.track_key,t.album_id,a.features,COALESCE(t.love,'') FROM sonic.sonic_tracks s CROSS JOIN tracks t ON t.file_path=s.directory AND t.filename=s.filename JOIN sonic.sonic_audio a USING(audio_hash,profile) WHERE s.profile=?1 AND lower(t.filename) LIKE '%.mp3' ORDER BY t.album_id,t.id").map_err(|e|e.to_string())?;
        for row in q
            .query_map([PROFILE], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?
        {
            let (key, id, json, love) = row.map_err(|e| e.to_string())?;
            let features: Vec<f32> = serde_json::from_str(&json).map_err(|e| e.to_string())?;
            if let Some(point) = Point::new(key, &features) {
                tracks.push(point);
                if love != "B" {
                    rolls.entry(id).or_default().add(&features);
                }
            }
        }
    }
    let mut coverage = vec![];
    let mut albums = vec![];
    let mut totals = tx
        .prepare("SELECT COUNT(*) FROM tracks WHERE album_id=?1 AND lower(filename) LIKE '%.mp3'")
        .map_err(|e| e.to_string())?;
    for (id, roll) in rolls {
        let total = totals
            .query_row([&id], |r| r.get::<_, i64>(0))
            .map_err(|e| e.to_string())? as usize;
        coverage.push((id.clone(), total, roll.count()));
        if let Some(point) = roll.mean().and_then(|features| Point::new(id, &features)) {
            albums.push(point);
        }
    }
    coverage.sort_by(|a, b| a.0.cmp(&b.0));
    let total = tx
        .query_row(
            "SELECT COUNT(*) FROM tracks WHERE lower(filename) LIKE '%.mp3'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?;
    let (all, unbanned, mp3) = tx.query_row("SELECT COUNT(*),COALESCE(SUM(CASE WHEN COALESCE(t.love,'')!='B' THEN 1 ELSE 0 END),0),COALESCE(SUM(CASE WHEN lower(t.filename) LIKE '%.mp3' THEN 1 ELSE 0 END),0)
        FROM sonic.sonic_tracks s CROSS JOIN tracks t ON s.directory=t.file_path AND s.filename=t.filename JOIN sonic.sonic_audio a USING(audio_hash,profile) WHERE s.profile=?1",[PROFILE],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?))).map_err(|e|e.to_string())?;
    let counts = HashMap::from([
        ("mp3-total".into(), total),
        ("tracks-all".into(), all),
        ("tracks-unbanned".into(), unbanned),
        ("journey".into(), mp3),
    ]);
    let snapshot = Snapshot {
        header: Header {
            version: 1,
            profile: PROFILE.into(),
            generation,
            weights: analysis.weights.clone(),
            coverage,
            counts,
        },
        tracks: Index::build(&analysis, tracks).ok_or("Unsupported index metric")?,
        albums: Index::build(&analysis, albums).ok_or("Unsupported index metric")?,
    };
    save(&dir.join(FILE_NAME), &snapshot).map_err(|e| e.to_string())
}
fn schedule(catalog: PathBuf, analysis: PathBuf, key: PathBuf) {
    std::thread::spawn(move || {
        if let Err(error) = rebuild(&catalog, &analysis) {
            eprintln!("Could not rebuild similarity index: {error}");
        }
        let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = cache.get_mut(&key) {
            entry.building = false;
            entry.attempted = Some(Instant::now());
        }
    });
}
fn snapshot(c: &Connection, current: &Generation) -> Option<Arc<Snapshot>> {
    let (catalog, analysis) = paths(c)?;
    let key = analysis.parent()?.join(FILE_NAME);
    let observed = stamp(&key);
    let mut cached = cache().lock().unwrap_or_else(|e| e.into_inner());
    // Keep at most two library snapshots resident. No app writes the other app's state.
    if !cached.contains_key(&key) && cached.len() >= 2 {
        cached.retain(|_, v| v.building);
    }
    let entry = cached.entry(key.clone()).or_default();
    if entry.stamp != observed {
        entry.snapshot = None;
        entry.stamp = observed;
        entry.loaded = false;
    }
    if !entry.loaded && entry.stamp.is_some() {
        entry.snapshot = load(&key).ok().map(Arc::new);
        entry.loaded = true;
    }
    let compatible = entry.snapshot.as_ref().is_some_and(|s| {
        s.header.generation.catalog == current.catalog
            && s.header.generation.analysis == current.analysis
            && s.header.generation.catalog_seq <= current.catalog_seq
            && s.header.generation.analysis_seq <= current.analysis_seq
    });
    let stale = !compatible
        || entry.snapshot.as_ref().is_some_and(|s| {
            current.catalog_seq - s.header.generation.catalog_seq + current.analysis_seq
                - s.header.generation.analysis_seq
                > 1024
        });
    if stale
        && !entry.building
        && entry
            .attempted
            .is_none_or(|t| t.elapsed() > Duration::from_secs(30))
    {
        entry.building = true;
        schedule(catalog, analysis, key);
    }
    compatible.then(|| entry.snapshot.clone()).flatten()
}
struct Delta {
    tracks: HashSet<String>,
    albums: HashSet<String>,
}
fn delta(c: &Connection, base: &Generation, current: &Generation) -> Option<Delta> {
    if current.catalog_seq - base.catalog_seq + current.analysis_seq - base.analysis_seq
        > DELTA_LIMIT as i64
    {
        return None;
    }
    let mut delta = Delta {
        tracks: HashSet::new(),
        albums: HashSet::new(),
    };
    let mut count = 0;
    for (schema, after, through) in [
        ("main", base.catalog_seq, current.catalog_seq),
        ("sonic", base.analysis_seq, current.analysis_seq),
    ] {
        let mut q=c.prepare(&format!("SELECT directory,filename,album_id FROM {schema}.sonic_index_changes WHERE seq>?1 AND seq<=?2 ORDER BY seq LIMIT {}",DELTA_LIMIT+1)).ok()?;
        let mut album = c
            .prepare("SELECT DISTINCT album_id FROM tracks WHERE file_path=?1 AND filename=?2")
            .ok()?;
        for row in q
            .query_map(params![after, through], |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })
            .ok()?
        {
            count += 1;
            if count > DELTA_LIMIT {
                return None;
            }
            let (directory, filename, id) = row.ok()?;
            if let Some(id) = id {
                delta.albums.insert(id);
            }
            if let (Some(directory), Some(filename)) = (directory, filename) {
                delta
                    .tracks
                    .insert(music_sonic_core::track_key(&directory, &filename));
                for id in album
                    .query_map(params![directory, filename], |r| r.get::<_, String>(0))
                    .ok()?
                {
                    delta.albums.insert(id.ok()?);
                }
            }
        }
    }
    Some(delta)
}
/// A candidate prefix from the exact tree plus every changed identity. The
/// caller still resolves current SQL rows, applies overlays, and checks files.
/// Sparse filters or freshness failures retry the streamed exact baseline.
pub(crate) struct Candidates {
    pub keys: String,
    pub frontiers: Vec<f64>,
}
pub(crate) fn candidates(
    c: &Connection,
    analysis: &Analysis,
    targets: &[Vec<f32>],
    albums: bool,
    limit: usize,
    eligible: Option<&HashSet<String>>,
    extra_dirty: &HashSet<String>,
) -> Option<Candidates> {
    let current = generation(c)?;
    let snapshot = snapshot(c, &current)?;
    let index = if albums {
        &snapshot.albums
    } else {
        &snapshot.tracks
    };
    if !index.compatible(analysis) {
        return None;
    }
    let mut delta = delta(c, &snapshot.header.generation, &current)?;
    let dirty = if albums {
        &mut delta.albums
    } else {
        &mut delta.tracks
    };
    dirty.extend(extra_dirty.iter().cloned());
    let allowed = |key: &str| eligible.is_none_or(|e| e.contains(key));
    let mut keys = HashSet::new();
    let mut frontiers = Vec::new();
    for target in targets {
        let found = index.nearest(target, limit, |key| !dirty.contains(key) && allowed(key));
        frontiers.push(if found.points.len() < limit {
            f64::INFINITY
        } else {
            found.points.last()?.1
        });
        for (point, _) in found.points {
            keys.insert(point.key.clone());
        }
    }
    keys.extend(dirty.iter().filter(|key| allowed(key)).cloned());
    #[cfg(test)]
    {
        INDEXED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    Some(Candidates {
        keys: serde_json::to_string(&keys).ok()?,
        frontiers,
    })
}
pub(crate) fn override_albums(
    c: &Connection,
    keys: impl Iterator<Item = String>,
) -> HashSet<String> {
    let mut ids = HashSet::new();
    if let Ok(mut q)=c.prepare("SELECT DISTINCT t.album_id FROM sonic.sonic_tracks s JOIN tracks t ON t.file_path=s.directory AND t.filename=s.filename WHERE s.track_key=?1") {
        for key in keys {if let Ok(rows)=q.query_map([key],|r|r.get::<_,String>(0)) {ids.extend(rows.filter_map(Result::ok));}}
    }
    ids
}
pub(crate) fn album_count(
    c: &Connection,
    minimum: u32,
    overrides: &HashMap<String, bool>,
) -> Option<usize> {
    let current = generation(c)?;
    let snapshot = snapshot(c, &current)?;
    let mut dirty = delta(c, &snapshot.header.generation, &current)?.albums;
    dirty.extend(override_albums(c, overrides.keys().cloned()));
    let mut count = snapshot
        .header
        .coverage
        .iter()
        .filter(|(id, total, analyzed)| {
            !dirty.contains(id) && music_sonic_core::album_ready(*analyzed, *total, minimum)
        })
        .count();
    for id in dirty {
        let (analyzed, total) = crate::sonic_albums::index_coverage(c, &id, overrides).ok()?;
        if music_sonic_core::album_ready(analyzed, total, minimum) {
            count += 1;
        }
    }
    Some(count)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn fixture() -> (tempfile::TempDir, Connection, Analysis) {
        let dir = tempfile::tempdir().unwrap();
        let c = Connection::open(dir.path().join("music-library.sqlite3")).unwrap();
        c.execute_batch("CREATE TABLE tracks(id INTEGER PRIMARY KEY,title TEXT,album_artist_display TEXT,album TEXT,release_year INTEGER,normalized_rating INTEGER,love TEXT,time_seconds INTEGER,canonical_genre TEXT,album_id TEXT,file_path TEXT,filename TEXT,import_run_id INTEGER,display_artist TEXT,year INTEGER,publisher TEXT);
          CREATE INDEX paths ON tracks(file_path,filename);CREATE INDEX albums ON tracks(album_id);").unwrap();
        install_catalog(&c).unwrap();
        let mut cache = Connection::open(dir.path().join("music-analysis.sqlite3")).unwrap();
        cache.execute_batch("CREATE TABLE sonic_profiles(profile TEXT PRIMARY KEY,weights TEXT);CREATE TABLE sonic_audio(audio_hash TEXT,profile TEXT,features TEXT,PRIMARY KEY(audio_hash,profile));CREATE TABLE sonic_tracks(track_key TEXT PRIMARY KEY,directory TEXT,filename TEXT,audio_hash TEXT,profile TEXT,size INTEGER,modified TEXT,analyzed_at TEXT);CREATE INDEX paths ON sonic_tracks(directory,filename);PRAGMA user_version=1;").unwrap();
        install_analysis(&cache).unwrap();
        let mut weights = vec![0.; DIMENSIONS * DIMENSIONS];
        for i in 0..DIMENSIONS {
            weights[i * DIMENSIONS + i] = 1.;
        }
        let analysis = Analysis {
            profile: PROFILE.into(),
            features: vec![0.; DIMENSIONS],
            weights,
        };
        cache
            .execute(
                "INSERT INTO sonic_profiles VALUES(?1,?2)",
                params![PROFILE, serde_json::to_string(&analysis.weights).unwrap()],
            )
            .unwrap();
        let tx = c.unchecked_transaction().unwrap();
        let atx = cache.transaction().unwrap();
        for i in 0..4096 {
            let name = format!("{i:05}.mp3");
            let path = dir.path().join(&name);
            fs::write(&path, [9; 256]).unwrap();
            let (size, modified) = music_sonic_core::file_signature(&path).unwrap();
            let mut features = vec![0.; DIMENSIONS];
            features[0] = (i / 8) as f32 / 512.;
            features[1] = (i % 8) as f32 / 8.;
            let directory = dir.path().to_string_lossy();
            let key = music_sonic_core::track_key(&directory, &name);
            tx.execute("INSERT INTO tracks VALUES(?1,?2,?3,?4,2000,80,'',180,'Pop',?4,?5,?2,1,?3,2000,'Label')",params![i+1,name,format!("Artist {}",i/32),format!("album-{:03}",i/8),directory]).unwrap();
            atx.execute(
                "INSERT INTO sonic_audio VALUES(?1,?2,?3)",
                params![name, PROFILE, serde_json::to_string(&features).unwrap()],
            )
            .unwrap();
            atx.execute(
                "INSERT INTO sonic_tracks VALUES(?1,?2,?3,?3,?4,?5,?6,'now')",
                params![key, directory, name, PROFILE, size as i64, modified],
            )
            .unwrap();
        }
        tx.commit().unwrap();
        atx.commit().unwrap();
        drop(cache);
        rebuild(
            &dir.path().join("music-library.sqlite3"),
            &dir.path().join("music-analysis.sqlite3"),
        )
        .unwrap();
        c.execute(
            "ATTACH DATABASE ?1 AS sonic",
            [dir.path()
                .join("music-analysis.sqlite3")
                .to_string_lossy()
                .as_ref()],
        )
        .unwrap();
        (dir, c, analysis)
    }
    fn albums(c: &Connection, overrides: &HashMap<String, bool>) -> serde_json::Value {
        let request = crate::sonic_albums::SonicAlbumRequest {
            album_id: "album-000".into(),
            limit: 20,
            minimum_coverage: 80,
        };
        let exact =
            crate::sonic_albums::query_indexed(c, true, &request, overrides, false).unwrap();
        let indexed =
            crate::sonic_albums::query_indexed(c, true, &request, overrides, true).unwrap();
        let exact = serde_json::to_value(exact).unwrap();
        assert_eq!(serde_json::to_value(indexed).unwrap(), exact);
        exact
    }
    #[test]
    fn indexed_albums_and_journeys_survive_new_analysis_edits_deletes_and_overlays() {
        let (dir, c, analysis) = fixture();
        assert!(
            candidates(
                &c,
                &analysis,
                std::slice::from_ref(&analysis.features),
                false,
                50,
                None,
                &HashSet::new()
            )
            .is_some()
        );
        let empty = HashMap::new();
        albums(&c, &empty);
        let visits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = visits.clone();
        c.create_scalar_function(
            "count_probe",
            0,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8,
            move |_| {
                observed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Ok(true)
            },
        )
        .unwrap();
        let count_sql = "SELECT COUNT(*) FROM tracks WHERE count_probe()";
        assert_eq!(
            count(&c, "tracks-all", "SELECT count_probe()", true).unwrap(),
            4096
        );
        assert_eq!(visits.load(std::sync::atomic::Ordering::Relaxed), 0);
        assert_eq!(count(&c, "mp3-total", count_sql, false).unwrap(), 4096);
        c.execute(
            "UPDATE sonic.sonic_tracks SET analyzed_at='new checkpoint' WHERE filename='00000.mp3'",
            [],
        )
        .unwrap();
        assert_eq!(count(&c, "mp3-total", count_sql, false).unwrap(), 4096);
        assert_eq!(visits.load(std::sync::atomic::Ordering::Relaxed), 4096);
        // Completed work becomes usable before the persisted tree is rebuilt.
        let (_, new_analysis) = crate::sonic_albums::seed(&c, "album-004", 50, &empty).unwrap();
        let name = "new-analysis.mp3";
        fs::write(dir.path().join(name), [9; 256]).unwrap();
        let (size, modified) = music_sonic_core::file_signature(&dir.path().join(name)).unwrap();
        let directory = dir.path().to_string_lossy();
        c.execute("INSERT INTO tracks VALUES(9001,'New analysis','New artist','album-new',2000,80,'',180,'Pop','album-new',?1,?2,1,'New artist',2000,'Label')",params![directory,name]).unwrap();
        c.execute(
            "INSERT INTO sonic.sonic_audio VALUES(?1,?2,?3)",
            params![
                name,
                PROFILE,
                serde_json::to_string(&new_analysis.unwrap().features).unwrap()
            ],
        )
        .unwrap();
        c.execute(
            "INSERT INTO sonic.sonic_tracks VALUES(?1,?2,?3,?3,?4,?5,?6,'new checkpoint')",
            params![
                music_sonic_core::track_key(&directory, name),
                directory,
                name,
                PROFILE,
                size as i64,
                modified
            ],
        )
        .unwrap();
        assert_eq!(count(&c, "mp3-total", count_sql, false).unwrap(), 4097);
        assert_eq!(visits.load(std::sync::atomic::Ordering::Relaxed), 8193);
        let completed = albums(&c, &empty);
        assert!(
            completed["albums"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["albumId"] == "album-new")
        );
        let stop_keys = [0, 64, 128, 192, 256]
            .map(|i| {
                music_sonic_core::track_key(&dir.path().to_string_lossy(), &format!("{i:05}.mp3"))
            })
            .to_vec();
        let request = crate::sonic_journey::JourneyRequest {
            stop_keys,
            connecting_tracks: 3,
            minimum_rating: Some(70),
            same_genre: true,
        };
        let exact = crate::sonic_journey::query_indexed(&c, true, &request, &HashMap::new(), false)
            .unwrap();
        let indexed =
            crate::sonic_journey::query_indexed(&c, true, &request, &HashMap::new(), true).unwrap();
        assert_eq!(
            serde_json::to_value(exact).unwrap(),
            serde_json::to_value(indexed).unwrap()
        );
        c.execute(
            "UPDATE sonic.sonic_audio SET features=?1 WHERE audio_hash='00008.mp3'",
            [serde_json::to_string(&vec![5.; DIMENSIONS]).unwrap()],
        )
        .unwrap();
        c.execute("UPDATE tracks SET love='B' WHERE id=17", [])
            .unwrap();
        c.execute(
            "DELETE FROM sonic.sonic_tracks WHERE filename IN ('00024.mp3','00025.mp3')",
            [],
        )
        .unwrap();
        c.execute("UPDATE tracks SET id=id+10000 WHERE id>1000", [])
            .unwrap();
        fs::write(dir.path().join("00040.mp3"), [9; 257]).unwrap();
        let overrides = HashMap::from([(
            music_sonic_core::track_key(&dir.path().to_string_lossy(), "00032.mp3"),
            true,
        )]);
        let result = albums(&c, &overrides);
        assert!(
            !result["albums"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["albumId"] == "album-003")
        );
        let base = generation(&c).unwrap();
        c.execute(
            "UPDATE sonic.sonic_tracks SET modified='changed' WHERE filename!='new-analysis.mp3'",
            [],
        )
        .unwrap();
        let current = generation(&c).unwrap();
        assert!(delta(&c, &base, &current).is_some());
        c.execute("UPDATE tracks SET love='B' WHERE love!='B'", [])
            .unwrap();
        assert!(delta(&c, &base, &generation(&c).unwrap()).is_none());
    }
    #[test]
    fn replacement_generation_and_old_source_store_use_exact_fallback() {
        let (dir, c, analysis) = fixture();
        c.execute(
            "UPDATE sonic.sonic_index_identity SET identity='replacement'",
            [],
        )
        .unwrap();
        assert!(
            candidates(
                &c,
                &analysis,
                std::slice::from_ref(&analysis.features),
                false,
                50,
                None,
                &HashSet::new()
            )
            .is_none()
        );
        // Older source stores have no generations and remain fully readable.
        c.execute_batch("DROP TABLE sonic.sonic_index_identity;")
            .unwrap();
        assert!(generation(&c).is_none());
        assert_eq!(albums(&c, &HashMap::new())["seedReady"], true);
        // Wait for a triggered builder to release the disposable source files.
        let deadline = Instant::now() + Duration::from_secs(5);
        while cache()
            .lock()
            .unwrap()
            .get(&dir.path().join(FILE_NAME))
            .is_some_and(|v| v.building)
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    #[ignore = "writes derived generation tables and index in an explicitly disposable benchmark copy"]
    fn sonic_index_build_proof() {
        assert_eq!(
            std::env::var("MUSIC_SONIC_BENCH_DISPOSABLE").as_deref(),
            Ok("1")
        );
        let dir = PathBuf::from(std::env::var_os("MUSIC_SONIC_BENCH_DIR").unwrap());
        let dir = dir.canonicalize().unwrap();
        let live = PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default())
            .join("com.local.musiclibrary");
        assert_ne!(
            Some(dir.clone()),
            live.canonicalize().ok(),
            "Never benchmark by changing the live store"
        );
        let catalog = dir.join("music-library.sqlite3");
        let analysis = dir.join("music-analysis.sqlite3");
        let c = Connection::open(&catalog).unwrap();
        install_catalog(&c).unwrap();
        drop(c);
        let c = Connection::open(&analysis).unwrap();
        install_analysis(&c).unwrap();
        drop(c);
        let start = Instant::now();
        rebuild(&catalog, &analysis).unwrap();
        let build_ms = start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        let snapshot = load(&dir.join(FILE_NAME)).unwrap();
        let load_ms = start.elapsed().as_secs_f64() * 1000.;
        let value = serde_json::json!({"profile":PROFILE,"tracks":snapshot.tracks.len(),"albums":snapshot.albums.len(),"buildMs":build_ms,"loadMs":load_ms,"bytes":fs::metadata(dir.join(FILE_NAME)).unwrap().len()});
        fs::write(
            dir.join("index-build.json"),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();
        println!("{value}");
    }
    #[test]
    #[ignore = "creates one-byte audio fixtures only inside a marked synthetic benchmark directory"]
    fn sonic_index_materialize_proof() {
        assert_eq!(
            std::env::var("MUSIC_SONIC_BENCH_DISPOSABLE").as_deref(),
            Ok("1")
        );
        let dir = PathBuf::from(std::env::var_os("MUSIC_SONIC_BENCH_DIR").unwrap())
            .canonicalize()
            .unwrap();
        let marker: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("synthetic-fixture.json")).unwrap()).unwrap();
        assert_eq!(marker["synthetic"], true);
        let root = PathBuf::from(marker["audioDirectory"].as_str().unwrap());
        assert!(root.is_absolute());
        let canonical_root = root.canonicalize().unwrap();
        assert_eq!(
            canonical_root,
            dir.join("fixture-audio").canonicalize().unwrap()
        );
        let config: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("seeds.json")).unwrap()).unwrap();
        let stops: Vec<String> = serde_json::from_value(config["stops"].clone()).unwrap();
        let snapshot = load(&dir.join(FILE_NAME)).unwrap();
        let c = Connection::open(dir.join("music-library.sqlite3")).unwrap();
        c.execute(
            "ATTACH DATABASE ?1 AS sonic",
            [dir.join("music-analysis.sqlite3")
                .to_string_lossy()
                .as_ref()],
        )
        .unwrap();
        let mut points = vec![];
        let mut keys = HashSet::new();
        for key in stops {
            let features:String=c.query_row("SELECT a.features FROM sonic.sonic_tracks s JOIN sonic.sonic_audio a USING(audio_hash,profile) WHERE s.track_key=?1 AND s.profile=?2",params![key,PROFILE],|r|r.get(0)).unwrap();
            let features: Vec<f32> = serde_json::from_str(&features).unwrap();
            for (point, _) in snapshot.tracks.nearest(&features, 4096, |_| true).points {
                keys.insert(point.key.clone());
            }
            keys.insert(key.clone());
            points.push(music_sonic_core::JourneyStop {
                key,
                features,
                data: (),
            });
        }
        let analysis = Analysis {
            profile: PROFILE.into(),
            features: points[0].features.clone(),
            weights: snapshot.header.weights.clone(),
        };
        let builder = music_sonic_core::JourneyBuilder::new(&analysis, points, 3).unwrap();
        for target in builder.targets() {
            for (point, _) in snapshot.tracks.nearest(&target, 1024, |_| true).points {
                keys.insert(point.key.clone());
            }
        }
        let album_id = config["album"].as_str().unwrap();
        let album = snapshot
            .albums
            .points()
            .iter()
            .find(|p| p.key == album_id)
            .unwrap();
        let mut albums = snapshot
            .albums
            .nearest(&album.features, 160, |id| id != album_id)
            .points
            .iter()
            .map(|(p, _)| p.key.clone())
            .collect::<HashSet<_>>();
        albums.insert(album_id.into());
        let mut q=c.prepare("SELECT s.track_key FROM tracks t JOIN sonic.sonic_tracks s ON s.directory=t.file_path AND s.filename=t.filename WHERE t.album_id IN (SELECT value FROM json_each(?1)) AND s.profile=?2").unwrap();
        keys.extend(
            q.query_map(
                params![serde_json::to_string(&albums).unwrap(), PROFILE],
                |r| r.get::<_, String>(0),
            )
            .unwrap()
            .map(Result::unwrap),
        );
        drop(q);
        let mut q=c.prepare("SELECT directory,filename FROM sonic.sonic_tracks WHERE track_key=?1 AND profile=?2").unwrap();
        let tx = c.unchecked_transaction().unwrap();
        for key in &keys {
            let (directory, filename): (String, String) = q
                .query_row(params![key, PROFILE], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap();
            let directory = PathBuf::from(directory);
            assert!(directory.starts_with(&root));
            let filename_path = Path::new(&filename);
            assert_eq!(filename_path.components().count(), 1);
            assert!(matches!(
                filename_path.components().next(),
                Some(std::path::Component::Normal(_))
            ));
            fs::create_dir_all(&directory).unwrap();
            assert!(
                directory
                    .canonicalize()
                    .unwrap()
                    .starts_with(&canonical_root)
            );
            let path = directory.join(filename);
            assert!(!path.exists(), "Never overwrite a real file");
            fs::write(&path, [9]).unwrap();
            let (size, modified) = music_sonic_core::file_signature(&path).unwrap();
            tx.execute(
                "UPDATE sonic.sonic_tracks SET size=?2,modified=?3 WHERE track_key=?1",
                params![key, size as i64, modified],
            )
            .unwrap();
        }
        tx.commit().unwrap();
        drop(q);
        drop(c);
        rebuild(
            &dir.join("music-library.sqlite3"),
            &dir.join("music-analysis.sqlite3"),
        )
        .unwrap();
        let info =
            serde_json::json!({"synthetic":true,"materializedFiles":keys.len(),"bytesPerFile":1});
        fs::write(
            dir.join("materialization.json"),
            serde_json::to_vec_pretty(&info).unwrap(),
        )
        .unwrap();
        println!("{info}");
    }
    #[test]
    fn persisted_index_checksum_and_metric_are_checked() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        let mut weights = vec![0.; DIMENSIONS * DIMENSIONS];
        for i in 0..DIMENSIONS {
            weights[i * DIMENSIONS + i] = 1.;
        }
        let analysis = Analysis {
            profile: PROFILE.into(),
            features: vec![0.; DIMENSIONS],
            weights,
        };
        let snapshot = Snapshot {
            header: Header {
                version: 1,
                profile: PROFILE.into(),
                generation: Generation {
                    catalog: "c".into(),
                    analysis: "a".into(),
                    catalog_seq: 0,
                    analysis_seq: 0,
                },
                weights: analysis.weights.clone(),
                coverage: vec![],
                counts: HashMap::new(),
            },
            tracks: Index::build(
                &analysis,
                vec![Point::new("test".into(), &analysis.features).unwrap()],
            )
            .unwrap(),
            albums: Index::build(&analysis, vec![]).unwrap(),
        };
        save(&path, &snapshot).unwrap();
        assert_eq!(load(&path).unwrap().tracks.len(), 1);
        let mut bytes = fs::read(&path).unwrap();
        bytes[8] ^= 1;
        fs::write(&path, bytes).unwrap();
        assert!(load(&path).is_err());
    }
}
