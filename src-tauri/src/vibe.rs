//! `vibe:` searches by a description of the sound, like `vibe:"dark ambient
//! drone"`, and `sort:vibe` orders by how well results fit it.
//!
//! The description is turned into a CLAP text embedding by Music Library's own
//! worker (`music-clap-worker --text`, about two seconds, then cached) and
//! compared with the audio embeddings Music Library stores in
//! `music-clap.sqlite3`. Text and audio scores are not comparable between
//! descriptions, so `vibe:` keeps a fixed share of the best-fitting results
//! instead of applying a score threshold.
//!
//! - Albums are scored against the mean embedding of their tracks, which the
//!   sound descriptor refresh keeps in `sound_album_vectors`; that takes
//!   milliseconds.
//! - Songs are scored track by track, which reads every embedding (a few
//!   seconds), so the best tracks for a description are kept in memory.
//!
//! Both are exposed to SQL as `aurora_vibe_album(text, album_id)` and
//! `aurora_vibe_track(text, audio_hash)`, which return the score scaled to an
//! integer for results in the kept share, and NULL otherwise.

use base64::Engine;
use rusqlite::{Connection, OpenFlags, OptionalExtension, functions::FunctionFlags};
use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

/// The share of albums, or of tracks, that `vibe:` keeps.
pub(crate) const SHARE: f64 = 0.02;
pub(crate) const MAX_CHARS: usize = 200;
/// Scores are stored as integers so `sort:vibe` pages like the other sorts.
const SCALE: f32 = 1_000_000.0;
const WORKER_TIMEOUT: Duration = Duration::from_secs(120);
/// Album scores are cheap, so they follow Music Library's analysis closely.
const ALBUM_TTL: Duration = Duration::from_secs(5 * 60);
const TRACK_TTL: Duration = Duration::from_secs(30 * 60);
const MAX_CACHED: usize = 8;
const MODEL_FOLDER: &str = "clap-model/larger_clap_music_and_speech-e9fd5ac";
const CLAP_FILE: &str = "music-clap.sqlite3";

/// Collapses spaces and lowercases, so the same description is scored once.
pub(crate) fn normalize(value: &str) -> Result<String, String> {
    let text = value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    if text.is_empty() {
        return Err("vibe: needs a description, like vibe:\"dark ambient drone\".".to_owned());
    }
    if text.chars().count() > MAX_CHARS {
        return Err(format!(
            "vibe: descriptions can be at most {MAX_CHARS} characters."
        ));
    }
    Ok(text)
}

fn sql_literal(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// The album's score for a description, NULL outside the best share.
pub(crate) fn album_sql(text: &str, album_id: &str) -> String {
    format!("aurora_vibe_album({}, {album_id})", sql_literal(text))
}

/// The track's score for a description, NULL outside the best share.
pub(crate) fn track_sql(text: &str, track_alias: &str) -> String {
    format!(
        "(SELECT aurora_vibe_track({}, vibe_track.audio_hash) FROM temp.aurora_sonic_paths AS vibe_track WHERE vibe_track.file_path = {track_alias}.file_path AND vibe_track.filename = {track_alias}.filename LIMIT 1)",
        sql_literal(text)
    )
}

/// Tracks in the best share. One pass over the analyzed paths, then a lookup
/// per track: a million correlated lookups made Songs take 17 s.
pub(crate) fn track_predicate(text: &str, track_alias: &str) -> String {
    format!(
        "({track_alias}.file_path, {track_alias}.filename) IN (SELECT vibe_track.file_path, vibe_track.filename FROM temp.aurora_sonic_paths AS vibe_track WHERE aurora_vibe_track({}, vibe_track.audio_hash) IS NOT NULL)",
        sql_literal(text)
    )
}

pub(crate) fn register(connection: &Connection) -> Result<(), String> {
    for (name, kind) in [
        ("aurora_vibe_album", Kind::Albums),
        ("aurora_vibe_track", Kind::Tracks),
    ] {
        connection
            .create_scalar_function(name, 2, FunctionFlags::SQLITE_UTF8, move |context| {
                // Scored once per statement; the description is a constant.
                let found = context.get_or_create_aux(0, |text| {
                    scores(kind, text.as_str().map_err(|error| error.to_string())?)
                })?;
                let key = context.get_raw(1).as_str_or_null()?;
                Ok(key.and_then(|key| found.get(key).copied()))
            })
            .map_err(|error| format!("Could not prepare vibe: search: {error}"))?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Kind {
    Albums,
    Tracks,
}

type Scores = Arc<HashMap<String, i64>>;

struct Cached {
    at: Instant,
    scores: Scores,
}

fn score_cache() -> &'static Mutex<HashMap<(Kind, String), Cached>> {
    static CACHE: OnceLock<Mutex<HashMap<(Kind, String), Cached>>> = OnceLock::new();
    CACHE.get_or_init(Mutex::default)
}

fn remember(kind: Kind, text: &str, scores: Scores) {
    let mut cache = score_cache()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if cache.len() >= MAX_CACHED
        && let Some(oldest) = cache
            .iter()
            .min_by_key(|(_, cached)| cached.at)
            .map(|(key, _)| key.clone())
    {
        cache.remove(&oldest);
    }
    cache.insert(
        (kind, text.to_owned()),
        Cached {
            at: Instant::now(),
            scores,
        },
    );
}

/// The best share of albums or tracks for a description, by id or audio hash.
fn scores(kind: Kind, text: &str) -> Result<Scores, String> {
    let ttl = match kind {
        Kind::Albums => ALBUM_TTL,
        Kind::Tracks => TRACK_TTL,
    };
    if let Some(cached) = score_cache()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(&(kind, text.to_owned()))
        .filter(|cached| cached.at.elapsed() < ttl)
    {
        return Ok(cached.scores.clone());
    }
    let query = text_embedding(text)?;
    let scores = Arc::new(match kind {
        Kind::Albums => {
            let cache = crate::sound_descriptors::cache_path()
                .filter(|path| path.is_file())
                .ok_or_else(not_ready)?;
            score_albums(&cache, &query)?
        }
        Kind::Tracks => score_tracks(&music_library_dir()?.join(CLAP_FILE), &query)?,
    });
    remember(kind, text, scores.clone());
    Ok(scores)
}

fn not_ready() -> String {
    "vibe: is ready once Aurora has read Music Library's sound search results, a few minutes after Aurora starts. Music Library 0.216.0 or newer makes them during audio analysis.".to_owned()
}

fn music_library_dir() -> Result<PathBuf, String> {
    crate::catalog::default_catalog_path()?
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "Music Library's data folder is missing.".to_owned())
}

/// The best `SHARE` of `scored`, at least one, as integer scores.
fn best_share<K>(mut scored: Vec<(K, f32)>) -> Vec<(K, i64)> {
    if scored.is_empty() {
        return Vec::new();
    }
    let keep = ((scored.len() as f64 * SHARE).ceil() as usize).clamp(1, scored.len());
    scored.select_nth_unstable_by(keep - 1, |a, b| b.1.total_cmp(&a.1));
    scored.truncate(keep);
    scored
        .into_iter()
        .map(|(key, score)| (key, (score * SCALE).round() as i64))
        .collect()
}

fn open_read_only(path: &Path) -> Result<Connection, String> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| format!("Could not open {}: {error}", path.display()))?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|error| error.to_string())?;
    Ok(connection)
}

/// Albums against the mean embedding of their tracks.
fn score_albums(cache: &Path, query: &[f32]) -> Result<HashMap<String, i64>, String> {
    Ok(best_share(album_scores(cache, query)?)
        .into_iter()
        .collect())
}

/// Every album's score, by album id.
fn album_scores(cache: &Path, query: &[f32]) -> Result<Vec<(String, f32)>, String> {
    let connection = open_read_only(cache)?;
    let present: bool = connection
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'sound_album_vectors')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if !present {
        return Err(not_ready());
    }
    let mut statement = connection
        .prepare("SELECT album_id, vector FROM sound_album_vectors")
        .map_err(|error| error.to_string())?;
    let mut rows = statement.query([]).map_err(|error| error.to_string())?;
    let mut scored = Vec::new();
    while let Some(row) = rows.next().map_err(|error| error.to_string())? {
        let bytes = row
            .get_ref(1)
            .and_then(|value| value.as_blob().map_err(Into::into))
            .map_err(|error| error.to_string())?;
        if let Some(score) = dot_f16(bytes, query) {
            scored.push((row.get::<_, String>(0).map_err(|e| e.to_string())?, score));
        }
    }
    if scored.is_empty() {
        return Err(not_ready());
    }
    Ok(scored)
}

/// Every analyzed track; keeps the best share by audio hash.
fn score_tracks(clap: &Path, query: &[f32]) -> Result<HashMap<String, i64>, String> {
    let (connection, scored) = track_scores(clap, query)?;
    let mut hash = connection
        .prepare("SELECT audio_hash FROM clap_audio WHERE rowid = ?1")
        .map_err(|error| error.to_string())?;
    best_share(scored)
        .into_iter()
        .map(|(rowid, score)| {
            hash.query_row([rowid], |row| row.get::<_, String>(0))
                .map(|hash| (hash, score))
                .map_err(|error| error.to_string())
        })
        .collect()
}

/// Every analyzed track's score, by `clap_audio` row id, with the open store.
fn track_scores(clap: &Path, query: &[f32]) -> Result<(Connection, Vec<(i64, f32)>), String> {
    if !clap.is_file() {
        return Err(not_ready());
    }
    let connection = open_read_only(clap)?;
    let model = &crate::sound_descriptors::model().clap_model;
    let mut statement = connection
        .prepare(
            "SELECT rowid, embedding FROM clap_audio WHERE model = ?1 AND embedding IS NOT NULL",
        )
        .map_err(|error| error.to_string())?;
    let mut rows = statement
        .query([model])
        .map_err(|error| error.to_string())?;
    // Row ids first: looking up a few thousand hashes beats keeping a million.
    let mut scored = Vec::new();
    while let Some(row) = rows.next().map_err(|error| error.to_string())? {
        let bytes = row
            .get_ref(1)
            .and_then(|value| value.as_blob().map_err(Into::into))
            .map_err(|error| error.to_string())?;
        if let Some(score) = dot_f16(bytes, query) {
            scored.push((row.get::<_, i64>(0).map_err(|e| e.to_string())?, score));
        }
    }
    drop(rows);
    drop(statement);
    Ok((connection, scored))
}

/// How well one album or track fits a description, against the whole library.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Fit {
    /// The share of the library's albums (or tracks) it fits better, 0 to 100.
    pub(crate) percentile: f64,
    /// Whether `vibe:` with the same description finds it.
    pub(crate) in_vibe: bool,
}

/// `own` placed among every score, `sorted` ascending.
fn fit_among(sorted: &[f32], own: f32) -> Fit {
    let below = sorted.partition_point(|score| *score < own);
    let above = sorted.len() - sorted.partition_point(|score| *score <= own);
    let keep = ((sorted.len() as f64 * SHARE).ceil() as usize).max(1);
    Fit {
        percentile: (1000.0 * below as f64 / sorted.len().max(1) as f64).round() / 10.0,
        in_vibe: above < keep,
    }
}

/// How well these albums fit a description, by album id. Albums without
/// analyzed tracks are left out.
pub(crate) fn album_fits(text: &str, album_ids: &[String]) -> Result<HashMap<String, Fit>, String> {
    let query = text_embedding(&normalize(text)?)?;
    let cache = crate::sound_descriptors::cache_path()
        .filter(|path| path.is_file())
        .ok_or_else(not_ready)?;
    album_fits_at(&cache, &query, album_ids)
}

fn album_fits_at(
    cache: &Path,
    query: &[f32],
    album_ids: &[String],
) -> Result<HashMap<String, Fit>, String> {
    let scored = album_scores(cache, query)?;
    let mut sorted: Vec<f32> = scored.iter().map(|(_, score)| *score).collect();
    sorted.sort_by(f32::total_cmp);
    Ok(scored
        .into_iter()
        .filter(|(id, _)| album_ids.contains(id))
        .map(|(id, score)| (id, fit_among(&sorted, score)))
        .collect())
}

/// How well these tracks fit a description, by audio hash. Reads every
/// embedding, about two seconds for a million tracks.
pub(crate) fn track_fits(
    text: &str,
    audio_hashes: &[String],
) -> Result<HashMap<String, Fit>, String> {
    let query = text_embedding(&normalize(text)?)?;
    track_fits_at(&music_library_dir()?.join(CLAP_FILE), &query, audio_hashes)
}

fn track_fits_at(
    clap: &Path,
    query: &[f32],
    audio_hashes: &[String],
) -> Result<HashMap<String, Fit>, String> {
    let (connection, scored) = track_scores(clap, query)?;
    let model = &crate::sound_descriptors::model().clap_model;
    let mut own = connection
        .prepare("SELECT embedding FROM clap_audio WHERE audio_hash = ?1 AND model = ?2")
        .map_err(|error| error.to_string())?;
    let mut wanted = Vec::new();
    for hash in audio_hashes {
        let bytes: Option<Vec<u8>> = own
            .query_row(rusqlite::params![hash, model], |row| row.get(0))
            .optional()
            .map_err(|error| error.to_string())?
            .flatten();
        if let Some(score) = bytes.and_then(|bytes| dot_f16(&bytes, query)) {
            wanted.push((hash.clone(), score));
        }
    }
    drop(own);
    let mut sorted: Vec<f32> = scored.into_iter().map(|(_, score)| score).collect();
    sorted.sort_by(f32::total_cmp);
    Ok(wanted
        .into_iter()
        .map(|(hash, score)| (hash, fit_among(&sorted, score)))
        .collect())
}

/// The dot product of a little-endian half-float vector with `query`.
fn dot_f16(bytes: &[u8], query: &[f32]) -> Option<f32> {
    if bytes.len() != query.len() * 2 {
        return None;
    }
    let table = crate::sound_descriptors::half_table();
    Some(
        bytes
            .as_chunks::<2>()
            .0
            .iter()
            .zip(query)
            .map(|(pair, value)| table[usize::from(u16::from_le_bytes(*pair))] * value)
            .sum(),
    )
}

fn text_cache() -> &'static Mutex<HashMap<String, Vec<f32>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Vec<f32>>>> = OnceLock::new();
    CACHE.get_or_init(Mutex::default)
}

/// The CLAP text embedding of a normalized description: from memory, from
/// Aurora's descriptor cache, or from Music Library's worker.
fn text_embedding(text: &str) -> Result<Vec<f32>, String> {
    let remembered = || {
        text_cache()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(text)
            .cloned()
    };
    if let Some(found) = remembered() {
        return Ok(found);
    }
    // One worker at a time; a search waiting here may find its answer saved.
    static WORKER: Mutex<()> = Mutex::new(());
    let _running = WORKER.lock().unwrap_or_else(|error| error.into_inner());
    if let Some(found) = remembered() {
        return Ok(found);
    }
    let cache = crate::sound_descriptors::cache_path();
    let saved = cache.as_deref().and_then(|path| {
        crate::sound_descriptors::saved_query(path, text)
            .ok()
            .flatten()
    });
    let embedding = match saved {
        Some(saved) => saved,
        None => {
            let embedding = run_worker(text)?;
            if let Some(path) = cache.as_deref() {
                // The cache is only a shortcut; a failed save costs a rerun.
                let _ = crate::sound_descriptors::save_query(path, text, &embedding);
            }
            embedding
        }
    };
    text_cache()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .insert(text.to_owned(), embedding.clone());
    Ok(embedding)
}

fn run_worker(text: &str) -> Result<Vec<f32>, String> {
    let executable = crate::library_bridge::discover_music_library_executable()?;
    let worker = executable.with_file_name(if cfg!(windows) {
        "music-clap-worker.exe"
    } else {
        "music-clap-worker"
    });
    if !worker.is_file() {
        return Err("vibe: needs Music Library's sound search worker, which comes with Music Library 0.216.0 or newer.".to_owned());
    }
    let model = music_library_dir()?.join(MODEL_FOLDER);
    if !model.is_dir() {
        return Err("vibe: needs Music Library's sound search model. Run audio analysis in Music Library once so it downloads it.".to_owned());
    }
    let mut command = Command::new(&worker);
    command
        .arg("--text")
        .arg(&model)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // No console window.
        command.creation_flags(0x0800_0000);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("Could not start Music Library's sound search worker: {error}"))?;
    child
        .stdin
        .take()
        .ok_or("The sound search worker has no input")?
        .write_all(text.as_bytes())
        .map_err(|error| format!("Could not ask the sound search worker: {error}"))?;
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = send.send(child.wait_with_output());
    });
    let output = receive
        .recv_timeout(WORKER_TIMEOUT)
        .map_err(|_| "The sound search worker did not answer within two minutes.".to_owned())?
        .map_err(|error| format!("The sound search worker failed: {error}"))?;
    if !output.status.success() {
        let message: String = String::from_utf8_lossy(&output.stderr)
            .trim()
            .chars()
            .take(500)
            .collect();
        return Err(format!("The sound search worker failed: {message}"));
    }
    parse_worker_reply(&output.stdout)
}

/// `{"embedding": "<base64 little-endian f32>"}`.
fn parse_worker_reply(stdout: &[u8]) -> Result<Vec<f32>, String> {
    let unreadable = || "The sound search worker sent an unreadable answer.".to_owned();
    let reply: serde_json::Value = serde_json::from_slice(stdout).map_err(|_| unreadable())?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(reply["embedding"].as_str().ok_or_else(unreadable)?)
        .map_err(|_| unreadable())?;
    let dimensions = crate::sound_descriptors::DIMENSIONS;
    if bytes.len() != dimensions * 4 {
        return Err(unreadable());
    }
    Ok(bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| f32::from_le_bytes(*chunk))
        .collect())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use rusqlite::params;

    fn count(c: &Connection, base: &str, query: &str, albums: bool) -> i64 {
        let search = crate::catalog::parse_catalog_search(query).unwrap();
        let mut params = vec![];
        let mut sql = base.to_owned();
        if albums {
            crate::catalog::push_album_search_predicates(&mut sql, &mut params, &search);
        } else {
            crate::catalog::push_track_search_predicates(&mut sql, &mut params, &search);
        }
        c.query_row(&sql, rusqlite::params_from_iter(params), |row| row.get(0))
            .unwrap_or_else(|error| panic!("{query}: {error}"))
    }

    #[test]
    fn vibe_combines_with_the_other_filters() {
        let (_dir, c) = crate::sound_descriptors::tests::search_connection();
        crate::catalog::register_catalog_functions(&c).unwrap();
        remember_albums("wiring albums", &[("B", 900_000), ("A", 100_000)]);
        remember_tracks("wiring tracks", &[("hash5", 700_000), ("hash4", 500_000)]);
        let albums = "SELECT count(*) FROM albums AS a WHERE 1 = 1";
        for (query, expected) in [
            ("vibe:\"Wiring   Albums\"", 2),
            ("vibe:\"wiring albums\" AND sound:synth", 1),
            ("NOT vibe:\"wiring albums\"", 1),
        ] {
            assert_eq!(count(&c, albums, query, true), expected, "{query}");
        }
        let songs = "SELECT count(*) FROM tracks AS t WHERE 1 = 1";
        for (query, expected) in [
            ("vibe:\"wiring tracks\"", 2),
            ("vibe:\"wiring tracks\" AND sound:choir", 0),
            ("vibe:\"wiring tracks\" OR sound:choir", 6),
        ] {
            assert_eq!(count(&c, songs, query, false), expected, "{query}");
        }
        // sort:vibe orders by these same expressions.
        let order: Vec<String> = c
            .prepare(&format!(
                "SELECT a.id FROM albums AS a ORDER BY {} DESC NULLS LAST, a.id",
                album_sql("wiring albums", "a.id")
            ))
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(order, ["B", "A", "C"]);
        let order: Vec<String> = c
            .prepare(&format!(
                "SELECT t.filename FROM tracks AS t WHERE {} ORDER BY {} DESC",
                track_predicate("wiring tracks", "t"),
                track_sql("wiring tracks", "t")
            ))
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(order, ["5.mp3", "4.mp3"]);
    }

    /// Makes `vibe:` find these albums or tracks for `text` without a worker.
    pub(crate) fn remember_albums(text: &str, scores: &[(&str, i64)]) {
        remember(Kind::Albums, text, Arc::new(owned(scores)));
    }

    pub(crate) fn remember_tracks(text: &str, scores: &[(&str, i64)]) {
        remember(Kind::Tracks, text, Arc::new(owned(scores)));
    }

    fn owned(scores: &[(&str, i64)]) -> HashMap<String, i64> {
        scores
            .iter()
            .map(|(key, score)| ((*key).to_owned(), *score))
            .collect()
    }

    #[test]
    fn normalizes_descriptions() {
        assert_eq!(
            normalize("  Dark   AMBIENT\tdrone "),
            Ok("dark ambient drone".to_owned())
        );
        assert!(normalize("   ").is_err());
        assert!(normalize(&"x".repeat(MAX_CHARS + 1)).is_err());
        assert_eq!(
            album_sql("rock 'n' roll", "a.id"),
            "aurora_vibe_album('rock ''n'' roll', a.id)"
        );
    }

    #[test]
    fn keeps_the_best_share() {
        let scored: Vec<(u32, f32)> = (0..200)
            .map(|index| (index, index as f32 / 1000.0))
            .collect();
        let mut best = best_share(scored);
        best.sort();
        // 2% of 200 is four.
        assert_eq!(
            best,
            [
                (196, 196_000),
                (197, 197_000),
                (198, 198_000),
                (199, 199_000)
            ]
        );
        assert_eq!(best_share(vec![("only", 0.5)]), [("only", 500_000)]);
        assert!(best_share(Vec::<(u8, f32)>::new()).is_empty());
    }

    #[test]
    fn reads_the_workers_embedding() {
        let values: Vec<f32> = (0..crate::sound_descriptors::DIMENSIONS)
            .map(|index| index as f32 / 512.0)
            .collect();
        let bytes: Vec<u8> = values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let reply = format!(
            "{{\"embedding\":\"{}\"}}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        );
        assert_eq!(parse_worker_reply(reply.as_bytes()), Ok(values));
        assert!(parse_worker_reply(b"{\"embedding\":\"AAAA\"}").is_err());
        assert!(parse_worker_reply(b"not json").is_err());
    }

    #[test]
    fn scores_albums_and_tracks_from_their_embeddings() {
        let dimensions = crate::sound_descriptors::DIMENSIONS;
        let axis = |index: usize, value: f32| {
            let mut vector = vec![0.0; dimensions];
            vector[index] = value;
            vector
        };
        let directory = tempfile::tempdir().unwrap();

        // Fifty albums: "a49" points along the query the most. Sixty-fourths
        // are exact in f16, so the score is too.
        let cache = directory.path().join("cache.sqlite3");
        let c = Connection::open(&cache).unwrap();
        c.execute_batch(
            "CREATE TABLE sound_album_vectors(album_id TEXT PRIMARY KEY, vector BLOB NOT NULL)",
        )
        .unwrap();
        for index in 0..50 {
            c.execute(
                "INSERT INTO sound_album_vectors VALUES(?1, ?2)",
                params![
                    format!("a{index}"),
                    crate::sound_descriptors::to_f16_bytes(&axis(0, index as f32 / 64.0))
                ],
            )
            .unwrap();
        }
        let query = axis(0, 1.0);
        assert_eq!(
            score_albums(&cache, &query).unwrap(),
            HashMap::from([("a49".to_owned(), 765_625)])
        );

        // A hundred tracks: the best two are kept, by audio hash.
        let clap = directory.path().join("clap.sqlite3");
        let c = Connection::open(&clap).unwrap();
        c.execute_batch(
            "CREATE TABLE clap_audio(audio_hash TEXT NOT NULL, model TEXT NOT NULL, embedding BLOB, PRIMARY KEY(audio_hash, model))",
        )
        .unwrap();
        let model = &crate::sound_descriptors::model().clap_model;
        for index in 0..100 {
            c.execute(
                "INSERT INTO clap_audio VALUES(?1, ?2, ?3)",
                params![
                    format!("h{index}"),
                    model,
                    crate::sound_descriptors::to_f16_bytes(&axis(0, index as f32 / 200.0))
                ],
            )
            .unwrap();
        }
        c.execute("INSERT INTO clap_audio VALUES('failed', ?1, NULL)", [model])
            .unwrap();
        let tracks = score_tracks(&clap, &query).unwrap();
        let mut hashes: Vec<&str> = tracks.keys().map(String::as_str).collect();
        hashes.sort();
        assert_eq!(hashes, ["h98", "h99"]);
        assert!(score_tracks(&directory.path().join("none.sqlite3"), &query).is_err());

        // describe_sound places single albums and tracks among all of them.
        let ids = ["a49", "a25", "a0", "missing"].map(str::to_owned);
        let fits = album_fits_at(&cache, &query, &ids).unwrap();
        assert_eq!(fits.len(), 3);
        assert_eq!(
            fits["a49"],
            Fit {
                percentile: 98.0,
                in_vibe: true
            }
        );
        assert_eq!(
            fits["a25"],
            Fit {
                percentile: 50.0,
                in_vibe: false
            }
        );
        assert_eq!(
            fits["a0"],
            Fit {
                percentile: 0.0,
                in_vibe: false
            }
        );
        let hashes = ["h98", "h10", "failed", "missing"].map(str::to_owned);
        let fits = track_fits_at(&clap, &query, &hashes).unwrap();
        assert_eq!(fits.len(), 2);
        assert_eq!(
            fits["h98"],
            Fit {
                percentile: 98.0,
                in_vibe: true
            }
        );
        assert_eq!(
            fits["h10"],
            Fit {
                percentile: 10.0,
                in_vibe: false
            }
        );
    }

    /// Runs against the real library with a scratch cache:
    /// `AURORA_VIBE_PROOF_CACHE=<file> cargo test --lib real_library -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs Music Library's data and the CLAP worker"]
    fn real_library_proof() {
        let cache = PathBuf::from(std::env::var("AURORA_VIBE_PROOF_CACHE").unwrap());
        let catalog = crate::catalog::default_catalog_path().unwrap();
        let started = Instant::now();
        let report = crate::sound_descriptors::refresh(&cache, &catalog, "proof").unwrap();
        println!("refresh {:?}: {report:?}", started.elapsed());

        let c = Connection::open_with_flags(&catalog, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        // The proof only needs the search projections, not Aurora's state.
        crate::catalog::register_catalog_functions(&c).unwrap();
        crate::sonic::prepare_search(&c, &catalog).unwrap();
        crate::sound_descriptors::prepare_search(&c, Some(&cache)).unwrap();

        for text in [
            "dark ambient drone",
            "heroic orchestral fanfare",
            "8-bit chiptune",
        ] {
            let started = Instant::now();
            let query = run_worker(text).unwrap();
            println!(
                "
{text}: worker {:?}",
                started.elapsed()
            );
            let started = Instant::now();
            let albums = score_albums(&cache, &query).unwrap();
            println!(
                "  {} albums scored in {:?}",
                albums.len(),
                started.elapsed()
            );
            let started = Instant::now();
            let tracks =
                score_tracks(&music_library_dir().unwrap().join(CLAP_FILE), &query).unwrap();
            println!(
                "  {} tracks scored in {:?}",
                tracks.len(),
                started.elapsed()
            );
            remember(Kind::Albums, text, Arc::new(albums));
            remember(Kind::Tracks, text, Arc::new(tracks));

            let search = crate::catalog::parse_catalog_search(&format!(
                "vibe:\"{text}\" AND year:1990..1999"
            ))
            .unwrap();
            let mut params = vec![];
            let mut sql =
                "SELECT a.album_artist_display, a.album FROM albums AS a WHERE 1 = 1".to_owned();
            crate::catalog::push_album_search_predicates(&mut sql, &mut params, &search);
            sql.push_str(&format!(
                " ORDER BY {} DESC LIMIT 5",
                album_sql(text, "a.id")
            ));
            let started = Instant::now();
            let top: Vec<(String, String)> = c
                .prepare(&sql)
                .unwrap()
                .query_map(rusqlite::params_from_iter(params), |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            println!("  1990s, best fit ({:?}):", started.elapsed());
            for (artist, album) in top {
                println!("    {artist} - {album}");
            }
            let search = crate::catalog::parse_catalog_search(&format!("vibe:\"{text}\"")).unwrap();
            let mut params = vec![];
            let mut sql =
                "SELECT t.display_artist, t.title FROM tracks AS t WHERE 1 = 1".to_owned();
            crate::catalog::push_track_search_predicates(&mut sql, &mut params, &search);
            sql.push_str(&format!(" ORDER BY {} DESC LIMIT 5", track_sql(text, "t")));
            let started = Instant::now();
            let top: Vec<(Option<String>, Option<String>)> = c
                .prepare(&sql)
                .unwrap()
                .query_map(rusqlite::params_from_iter(params), |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            println!("  songs, best fit ({:?}):", started.elapsed());
            for (artist, title) in top {
                println!(
                    "    {} - {}",
                    artist.unwrap_or_default(),
                    title.unwrap_or_default()
                );
            }
        }
    }
}
