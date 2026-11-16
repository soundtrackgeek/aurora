//! Sound descriptors: `sound:` and `group:sound` in the catalog search.
//!
//! Music Library stores one LAION CLAP embedding per analyzed MP3 in
//! `music-clap.sqlite3`. A small linear model (one logistic regression per
//! label, trained by `scripts/train-sound-descriptors.py` and compiled in from
//! `sound_descriptors_model.json`) turns each embedding into descriptors such
//! as Orchestral, Choir, Synth or Chiptune: 12 dot products per track, with no
//! audio model in Aurora.
//!
//! Results are cached in Aurora's own `aurora-sound-descriptors.sqlite3`: one
//! row per audio hash (so identical files share one row) and one per album.
//! The cache follows `clap_audio` by rowid, because Music Library saves with
//! `INSERT OR REPLACE`, so new and re-embedded tracks always get a new rowid.
//! Album rows are rebuilt when the catalog, the analysis bindings or the track
//! rows change. Searches attach the cache read-only; Music Library's files are
//! only ever read.
use music_sonic_core::PROFILE;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde::Deserialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::OnceLock,
    time::Duration,
};

pub(crate) const FILE_NAME: &str = "aurora-sound-descriptors.sqlite3";
const CLAP_FILE: &str = "music-clap.sqlite3";
const ANALYSIS_FILE: &str = "music-analysis.sqlite3";
/// Embeddings read and classified per transaction.
const BATCH: i64 = 4096;
const SCHEMA_VERSION: i32 = 1;
/// Search aliases for label ids.
const ALIASES: [(&str, &str); 9] = [
    ("orchestra", "orchestral"),
    ("choral", "choir"),
    ("string", "strings"),
    ("synthesizer", "synth"),
    ("synths", "synth"),
    ("drums", "percussion"),
    ("chip", "chiptune"),
    ("vocal", "vocals"),
    ("voice", "vocals"),
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelFile {
    version: String,
    clap_model: String,
    dimensions: usize,
    threshold: f32,
    album_share: f64,
    labels: Vec<LabelFile>,
}

#[derive(Deserialize)]
struct LabelFile {
    id: String,
    name: String,
    bias: f32,
    weights: Vec<f32>,
}

pub(crate) struct Label {
    pub(crate) id: String,
    pub(crate) name: String,
    bias: f32,
    weights: Vec<f32>,
}

pub(crate) struct Model {
    /// Changes whenever the labels or weights change; the cache is rebuilt.
    pub(crate) version: String,
    /// Only embeddings from this CLAP model identity are classified.
    pub(crate) clap_model: String,
    dimensions: usize,
    /// The logit of the probability threshold, so classifying needs no `exp`.
    logit_threshold: f32,
    /// An album carries a descriptor when this share of its tracks do.
    pub(crate) album_share: f64,
    pub(crate) labels: Vec<Label>,
}

pub(crate) fn model() -> &'static Model {
    static MODEL: OnceLock<Model> = OnceLock::new();
    MODEL.get_or_init(|| {
        let file: ModelFile = serde_json::from_str(include_str!("sound_descriptors_model.json"))
            .expect("the compiled-in sound descriptor model is valid JSON");
        assert!(
            file.labels.len() <= 31,
            "descriptor masks hold at most 31 labels"
        );
        assert!(
            file.labels
                .iter()
                .all(|label| label.weights.len() == file.dimensions),
            "every descriptor has one weight per embedding dimension"
        );
        Model {
            version: file.version,
            clap_model: file.clap_model,
            dimensions: file.dimensions,
            logit_threshold: (file.threshold / (1.0 - file.threshold)).ln(),
            album_share: file.album_share,
            labels: file
                .labels
                .into_iter()
                .map(|label| Label {
                    id: label.id,
                    name: label.name,
                    bias: label.bias,
                    weights: label.weights,
                })
                .collect(),
        }
    })
}

/// The descriptors of one embedding: a bit per label, and the most likely one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Descriptors {
    pub(crate) mask: u32,
    pub(crate) top: Option<u8>,
}

impl Model {
    pub(crate) fn classify(&self, embedding: &[f32]) -> Descriptors {
        let mut found = Descriptors::default();
        let mut best = f32::NEG_INFINITY;
        for (index, label) in self.labels.iter().enumerate() {
            let logit = label.bias
                + label
                    .weights
                    .iter()
                    .zip(embedding)
                    .map(|(weight, value)| weight * value)
                    .sum::<f32>();
            if logit >= self.logit_threshold {
                found.mask |= 1 << index;
                if logit > best {
                    best = logit;
                    found.top = Some(index as u8);
                }
            }
        }
        found
    }

    /// Music Library stores embeddings as little-endian half floats.
    fn decode(&self, bytes: &[u8]) -> Option<Vec<f32>> {
        (bytes.len() == self.dimensions * 2).then(|| {
            bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| f16_to_f32(u16::from_le_bytes(*pair)))
                .collect()
        })
    }

    pub(crate) fn label_names(&self) -> String {
        self.labels
            .iter()
            .map(|label| label.id.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn f16_to_f32(bits: u16) -> f32 {
    let sign = u32::from(bits >> 15) << 31;
    let exponent = u32::from((bits >> 10) & 0x1f);
    let fraction = u32::from(bits & 0x3ff);
    let value = match exponent {
        0 if fraction == 0 => sign,
        // Subnormal: fraction × 2^-24.
        0 => {
            let magnitude = fraction as f32 * 2f32.powi(-24);
            return if sign == 0 { magnitude } else { -magnitude };
        }
        0x1f => sign | 0x7f80_0000 | (fraction << 13),
        _ => sign | ((exponent + 127 - 15) << 23) | (fraction << 13),
    };
    f32::from_bits(value)
}

/// The label index for a `sound:` value: a label id, its display name or an alias.
pub(crate) fn parse_label(value: &str) -> Result<u8, String> {
    let model = model();
    let wanted = value.trim().to_lowercase();
    let wanted = ALIASES
        .iter()
        .find(|(alias, _)| *alias == wanted)
        .map_or(wanted.as_str(), |(_, id)| id);
    model
        .labels
        .iter()
        .position(|label| label.id == wanted || label.name.to_lowercase() == wanted)
        .map(|index| index as u8)
        .ok_or_else(|| {
            format!(
                "Unknown sound \"{}\". Use sound: with {}.",
                value.trim(),
                model.label_names()
            )
        })
}

/// SQL that turns a label index into its display name, NULL for no label.
pub(crate) fn label_sql(index: &str) -> String {
    let cases = model()
        .labels
        .iter()
        .enumerate()
        .map(|(position, label)| {
            format!("WHEN {position} THEN '{}'", label.name.replace('\'', "''"))
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!("(CASE {index} {cases} END)")
}

/// Aurora's descriptor cache. Tests get no cache unless they attach one.
pub(crate) fn cache_path() -> Option<PathBuf> {
    #[cfg(test)]
    {
        None
    }
    #[cfg(not(test))]
    {
        crate::mcp::app_data_dir()
            .ok()
            .map(|directory| directory.join(FILE_NAME))
    }
}

fn open_cache(path: &Path) -> Result<Connection, String> {
    let c = Connection::open(path)
        .map_err(|error| format!("Could not open the sound descriptor cache: {error}"))?;
    c.busy_timeout(Duration::from_secs(5))
        .map_err(|error| error.to_string())?;
    let version: i32 = c
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|error| error.to_string())?;
    if version != SCHEMA_VERSION {
        c.execute_batch(
            "DROP TABLE IF EXISTS sound_meta; DROP TABLE IF EXISTS sound_audio; DROP TABLE IF EXISTS sound_albums;",
        )
        .map_err(|error| error.to_string())?;
    }
    c.execute_batch(&format!(
        "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;
        CREATE TABLE IF NOT EXISTS sound_meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS sound_audio(audio_hash TEXT PRIMARY KEY, mask INTEGER NOT NULL, top INTEGER) WITHOUT ROWID;
        CREATE TABLE IF NOT EXISTS sound_albums(album_id TEXT PRIMARY KEY, tracks INTEGER NOT NULL, mask INTEGER NOT NULL, top INTEGER) WITHOUT ROWID;
        PRAGMA user_version={SCHEMA_VERSION};"
    ))
    .map_err(|error| format!("Could not prepare the sound descriptor cache: {error}"))?;
    let model = model();
    let identity = format!("{}|{}", model.version, model.clap_model);
    if meta(&c, "model")?.as_deref() != Some(identity.as_str()) {
        c.execute_batch(
            "DELETE FROM sound_meta; DELETE FROM sound_audio; DELETE FROM sound_albums;",
        )
        .map_err(|error| error.to_string())?;
        set_meta(&c, "model", &identity)?;
    }
    Ok(c)
}

fn meta(c: &Connection, key: &str) -> Result<Option<String>, String> {
    c.query_row(
        "SELECT value FROM sound_meta WHERE key = ?1",
        [key],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| error.to_string())
}

fn set_meta(c: &Connection, key: &str, value: &str) -> Result<(), String> {
    c.execute(
        "INSERT OR REPLACE INTO sound_meta(key, value) VALUES(?1, ?2)",
        params![key, value],
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn open_read_only(path: &Path) -> Result<Connection, String> {
    let c = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| format!("Could not open {}: {error}", path.display()))?;
    c.busy_timeout(Duration::from_secs(5))
        .map_err(|error| error.to_string())?;
    Ok(c)
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct RefreshReport {
    /// Embeddings classified in this pass.
    pub(crate) tracks: usize,
    /// Albums written, when the album rows were rebuilt.
    pub(crate) albums: Option<usize>,
}

/// Brings the cache up to date with Music Library's embeddings and catalog.
/// Does nothing until the catalog, its analysis and its CLAP results exist.
pub(crate) fn refresh(
    cache: &Path,
    catalog: &Path,
    revision: &str,
) -> Result<RefreshReport, String> {
    let directory = catalog.parent().ok_or("Catalog directory missing")?;
    let clap_path = directory.join(CLAP_FILE);
    let analysis_path = directory.join(ANALYSIS_FILE);
    if !catalog.is_file() || !clap_path.is_file() || !analysis_path.is_file() {
        return Ok(RefreshReport::default());
    }
    let c = open_cache(cache)?;
    let model = model();
    let mut report = RefreshReport::default();

    let clap = open_read_only(&clap_path)?;
    let newest: i64 = clap
        .query_row(
            "SELECT COALESCE(MAX(rowid), 0) FROM clap_audio",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not read Music Library's sound search results: {error}"))?;
    let mut last: i64 = meta(&c, "clap_rowid")?
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    if newest < last {
        // Music Library's sound search database was replaced: start over.
        c.execute_batch("DELETE FROM sound_audio;")
            .map_err(|error| error.to_string())?;
        last = 0;
    }
    let mut read = clap
        .prepare(
            "SELECT rowid, audio_hash, embedding FROM clap_audio WHERE rowid > ?1 AND model = ?2 ORDER BY rowid LIMIT ?3",
        )
        .map_err(|error| error.to_string())?;
    loop {
        let rows = read
            .query_map(params![last, model.clap_model, BATCH], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<Vec<u8>>>(2)?,
                ))
            })
            .and_then(Iterator::collect::<Result<Vec<_>, _>>)
            .map_err(|error| error.to_string())?;
        let Some((final_rowid, _, _)) = rows.last() else {
            break;
        };
        let final_rowid = *final_rowid;
        let transaction = c
            .unchecked_transaction()
            .map_err(|error| error.to_string())?;
        {
            let mut save = transaction
                .prepare_cached(
                    "INSERT OR REPLACE INTO sound_audio(audio_hash, mask, top) VALUES(?1, ?2, ?3)",
                )
                .map_err(|error| error.to_string())?;
            let mut forget = transaction
                .prepare_cached("DELETE FROM sound_audio WHERE audio_hash = ?1")
                .map_err(|error| error.to_string())?;
            for (_, hash, embedding) in &rows {
                match embedding.as_deref().and_then(|bytes| model.decode(bytes)) {
                    Some(values) => {
                        let found = model.classify(&values);
                        save.execute(params![hash, found.mask, found.top])
                            .map_err(|error| error.to_string())?;
                    }
                    // A track that could not be embedded has no descriptors.
                    None => {
                        forget.execute([hash]).map_err(|error| error.to_string())?;
                    }
                }
            }
        }
        set_meta(&transaction, "clap_rowid", &final_rowid.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
        report.tracks += rows.len();
        last = final_rowid;
    }
    drop(read);

    let analysis = open_read_only(&analysis_path)?;
    let bindings: (i64, i64) = analysis
        .query_row(
            "SELECT COALESCE(MAX(rowid), 0), COUNT(*) FROM sonic_tracks",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| format!("Could not read Music Library's analysis: {error}"))?;
    let signature = format!("{revision}|{last}|{}|{}", bindings.0, bindings.1);
    if meta(&c, "albums_signature")?.as_deref() == Some(signature.as_str()) {
        return Ok(report);
    }
    let albums = album_rollups(catalog, &analysis_path, cache)?;
    let transaction = c
        .unchecked_transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute("DELETE FROM sound_albums", [])
        .map_err(|error| error.to_string())?;
    {
        let mut insert = transaction
            .prepare("INSERT INTO sound_albums(album_id, tracks, mask, top) VALUES(?1, ?2, ?3, ?4)")
            .map_err(|error| error.to_string())?;
        for (album, rollup) in &albums {
            insert
                .execute(params![album, rollup.tracks, rollup.mask, rollup.top])
                .map_err(|error| error.to_string())?;
        }
    }
    set_meta(&transaction, "albums_signature", &signature)?;
    transaction.commit().map_err(|error| error.to_string())?;
    report.albums = Some(albums.len());
    Ok(report)
}

#[derive(Debug, Default, PartialEq, Eq)]
struct AlbumRollup {
    tracks: u32,
    mask: u32,
    top: Option<u8>,
}

/// Counts each album's tracks per descriptor. An album carries a descriptor
/// when at least `album_share` of its described tracks do; its top descriptor
/// is the one the most tracks carry (the earlier label on a tie).
fn album_rollups(
    catalog: &Path,
    analysis: &Path,
    cache: &Path,
) -> Result<HashMap<String, AlbumRollup>, String> {
    let c = open_read_only(catalog)?;
    for (path, name) in [(analysis, "sonic"), (cache, "sound")] {
        c.execute(
            &format!("ATTACH DATABASE ?1 AS {name}"),
            [path.to_string_lossy().as_ref()],
        )
        .map_err(|error| error.to_string())?;
    }
    let labels = model().labels.len();
    let mut counts: HashMap<String, (u32, Vec<u32>)> = HashMap::new();
    let mut rows = c
        .prepare(
            "SELECT t.album_id, d.mask FROM tracks AS t
             JOIN sonic.sonic_tracks AS s ON s.directory = t.file_path AND s.filename = t.filename AND s.profile = ?1
             JOIN sound.sound_audio AS d ON d.audio_hash = s.audio_hash
             WHERE t.album_id IS NOT NULL",
        )
        .map_err(|error| error.to_string())?;
    let mut query = rows.query([PROFILE]).map_err(|error| error.to_string())?;
    while let Some(row) = query.next().map_err(|error| error.to_string())? {
        let album: String = row.get(0).map_err(|error| error.to_string())?;
        let mask: u32 = row.get(1).map_err(|error| error.to_string())?;
        let entry = counts.entry(album).or_insert_with(|| (0, vec![0; labels]));
        entry.0 += 1;
        for (index, count) in entry.1.iter_mut().enumerate() {
            *count += (mask >> index) & 1;
        }
    }
    let share = model().album_share;
    Ok(counts
        .into_iter()
        .map(|(album, (tracks, per_label))| (album, rollup(tracks, &per_label, share)))
        .collect())
}

fn rollup(tracks: u32, per_label: &[u32], share: f64) -> AlbumRollup {
    let mut result = AlbumRollup {
        tracks,
        ..AlbumRollup::default()
    };
    let mut best = 0;
    for (index, &count) in per_label.iter().enumerate() {
        if count > 0 && f64::from(count) >= share * f64::from(tracks) {
            result.mask |= 1 << index;
            if count > best {
                best = count;
                result.top = Some(index as u8);
            }
        }
    }
    result
}

/// Exposes descriptors to searches as `temp.aurora_sound_tracks` (by path) and
/// `temp.aurora_sound_albums`. Without a usable cache, or without Music
/// Library's analysis attached as `sonic`, both are empty, so `sound:` finds
/// nothing instead of failing.
pub(crate) fn prepare_search(c: &Connection, cache: Option<&Path>) -> Result<(), String> {
    let usable = match cache.filter(|path| path.is_file()) {
        Some(path) if attached(c, "sonic")? => attach_cache(c, path)?,
        _ => false,
    };
    let sql = if usable {
        format!(
            "CREATE TEMP VIEW aurora_sound_tracks AS SELECT s.directory AS file_path, s.filename AS filename, d.mask AS mask, d.top AS top FROM sonic.sonic_tracks AS s JOIN sound.sound_audio AS d ON d.audio_hash = s.audio_hash WHERE s.profile = '{PROFILE}';
            CREATE TEMP VIEW aurora_sound_albums AS SELECT album_id, mask, top FROM sound.sound_albums;"
        )
    } else {
        "CREATE TEMP TABLE aurora_sound_tracks(file_path TEXT, filename TEXT, mask INTEGER, top INTEGER);
        CREATE TEMP TABLE aurora_sound_albums(album_id TEXT PRIMARY KEY, mask INTEGER, top INTEGER);"
            .to_owned()
    };
    c.execute_batch(&sql).map_err(|error| error.to_string())
}

fn attached(c: &Connection, name: &str) -> Result<bool, String> {
    let mut statement = c
        .prepare("SELECT name FROM pragma_database_list")
        .map_err(|error| error.to_string())?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(0))
        .and_then(Iterator::collect::<Result<Vec<_>, _>>)
        .map_err(|error| error.to_string())?;
    Ok(names.iter().any(|attached| attached == name))
}

/// Attaches the cache as `sound` when it was built by this model; a cache from
/// another model version (or one still being created) is detached again.
fn attach_cache(c: &Connection, path: &Path) -> Result<bool, String> {
    c.execute(
        "ATTACH DATABASE ?1 AS sound",
        [path.to_string_lossy().as_ref()],
    )
    .map_err(|error| format!("Could not open the sound descriptor cache: {error}"))?;
    let model = model();
    let identity = format!("{}|{}", model.version, model.clap_model);
    let current = c
        .query_row(
            "SELECT (SELECT count(*) FROM sound.sqlite_master WHERE type = 'table' AND name IN ('sound_meta', 'sound_audio', 'sound_albums')) = 3",
            [],
            |row| row.get::<_, bool>(0),
        )
        .unwrap_or(false)
        && c.query_row(
            "SELECT value FROM sound.sound_meta WHERE key = 'model'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .ok()
        .flatten()
        .as_deref()
            == Some(identity.as_str());
    if !current {
        c.execute_batch("DETACH DATABASE sound")
            .map_err(|error| error.to_string())?;
    }
    Ok(current)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// An embedding that points along one label's weights, so that label wins.
    pub(crate) fn embedding_for(label: usize) -> Vec<f32> {
        let weights = &model().labels[label].weights;
        let norm = weights.iter().map(|w| w * w).sum::<f32>().sqrt();
        weights.iter().map(|w| w / norm).collect()
    }

    fn to_f16_bytes(values: &[f32]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| f32_to_f16(*value).to_le_bytes())
            .collect()
    }

    /// Round-to-nearest conversion for normal values, enough for fixtures.
    fn f32_to_f16(value: f32) -> u16 {
        let bits = value.to_bits();
        let sign = ((bits >> 16) & 0x8000) as u16;
        let exponent = ((bits >> 23) & 0xff) as i32 - 127 + 15;
        if value == 0.0 || exponent <= 0 {
            return sign;
        }
        let fraction = ((bits & 0x7f_ffff) + 0x1000) >> 13;
        sign | (((exponent as u16) << 10) + fraction as u16)
    }

    #[test]
    fn the_compiled_in_model_has_twelve_labels() {
        let model = model();
        assert_eq!(model.dimensions, 512);
        let ids: Vec<_> = model.labels.iter().map(|label| label.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "orchestral",
                "choir",
                "strings",
                "piano",
                "guitar",
                "synth",
                "hybrid",
                "ambient",
                "percussion",
                "chiptune",
                "jazz",
                "vocals"
            ]
        );
    }

    #[test]
    fn half_floats_decode() {
        for (bits, value) in [
            (0x3c00, 1.0),
            (0xc000, -2.0),
            (0x3800, 0.5),
            (0x0000, 0.0),
            (0x0001, 2f32.powi(-24)),
        ] {
            assert_eq!(f16_to_f32(bits), value, "{bits:#06x}");
        }
        assert!(f16_to_f32(0x7c00).is_infinite());
        for value in [0.031_25f32, -0.75, 0.2] {
            assert!((f16_to_f32(f32_to_f16(value)) - value).abs() < 1e-3);
        }
    }

    #[test]
    fn an_embedding_along_a_label_gets_that_label_on_top() {
        let model = model();
        for index in 0..model.labels.len() {
            let found = model.classify(&embedding_for(index));
            assert_eq!(found.top, Some(index as u8), "{}", model.labels[index].id);
            assert_ne!(found.mask & (1 << index), 0);
        }
        assert_eq!(model.classify(&vec![0.0; 512]), Descriptors::default());
    }

    #[test]
    fn labels_parse_by_id_name_and_alias() {
        assert_eq!(parse_label("choir"), Ok(1));
        assert_eq!(parse_label("Choir"), Ok(1));
        assert_eq!(parse_label("choral"), Ok(1));
        assert_eq!(parse_label("synthesizer"), Ok(5));
        assert_eq!(parse_label(" chiptune "), Ok(9));
        let error = parse_label("kazoo").unwrap_err();
        assert!(error.contains("orchestral, choir"), "{error}");
    }

    #[test]
    fn albums_need_a_third_of_their_tracks() {
        // 6 tracks: 2 choir (a third), 1 piano, 4 orchestral.
        let found = rollup(6, &[4, 2, 0, 1], 1.0 / 3.0);
        assert_eq!(found.mask, 0b0011);
        assert_eq!(found.top, Some(0));
        assert_eq!(
            rollup(3, &[0, 0, 0, 0], 1.0 / 3.0),
            AlbumRollup {
                tracks: 3,
                mask: 0,
                top: None
            }
        );
        // Ties go to the earlier label.
        assert_eq!(rollup(2, &[0, 1, 1], 1.0 / 3.0).top, Some(1));
    }

    /// A catalog, analysis and CLAP database beside each other, as Music
    /// Library keeps them: album A is choir, album B synth with one choir track.
    pub(crate) fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let catalog = dir.path().join("music-library.sqlite3");
        let c = Connection::open(&catalog).unwrap();
        c.execute_batch(
            "CREATE TABLE tracks(id INTEGER PRIMARY KEY, title TEXT, album_artist_display TEXT, album TEXT, release_year INTEGER, normalized_rating INTEGER, love TEXT, time_seconds INTEGER, canonical_genre TEXT, album_id TEXT, file_path TEXT, filename TEXT, import_run_id INTEGER, display_artist TEXT, year INTEGER, publisher TEXT);
            CREATE TABLE albums(id TEXT PRIMARY KEY, album TEXT, album_artist_display TEXT);
            INSERT INTO albums VALUES('A', 'Choir Album', 'Composer'), ('B', 'Synth Album', 'Composer'), ('C', 'Unanalyzed', 'Composer');",
        )
        .unwrap();
        let analysis = Connection::open(dir.path().join(ANALYSIS_FILE)).unwrap();
        analysis
            .execute_batch("CREATE TABLE sonic_tracks(track_key TEXT PRIMARY KEY, directory TEXT NOT NULL, filename TEXT NOT NULL, audio_hash TEXT NOT NULL, profile TEXT NOT NULL, size INTEGER NOT NULL, modified TEXT NOT NULL, analyzed_at TEXT NOT NULL);")
            .unwrap();
        let clap = Connection::open(dir.path().join(CLAP_FILE)).unwrap();
        clap.execute_batch("CREATE TABLE clap_audio(audio_hash TEXT NOT NULL, model TEXT NOT NULL, embedding BLOB, windows TEXT, error TEXT, analyzed_at TEXT NOT NULL, PRIMARY KEY(audio_hash, model));")
            .unwrap();
        // (track, album, label) — None means no embedding at all.
        let tracks: [(i64, &str, Option<usize>); 7] = [
            (1, "A", Some(1)),
            (2, "A", Some(1)),
            (3, "A", Some(1)),
            (4, "B", Some(5)),
            (5, "B", Some(5)),
            (6, "B", Some(1)),
            (7, "C", None),
        ];
        for (id, album, label) in tracks {
            let filename = format!("{id}.mp3");
            c.execute(
                "INSERT INTO tracks VALUES(?1, 'Cue', 'Composer', 'Album', 2000, NULL, NULL, 120, 'Sci-Fi', ?2, 'D:\\Scores', ?3, 1, 'Composer', 2000, 'Label')",
                params![id, album, filename],
            )
            .unwrap();
            let Some(label) = label else { continue };
            analysis
                .execute(
                    "INSERT INTO sonic_tracks VALUES(?1, 'D:\\Scores', ?2, ?3, ?4, 1, '1', 'now')",
                    params![format!("key{id}"), filename, format!("hash{id}"), PROFILE],
                )
                .unwrap();
            clap.execute(
                "INSERT INTO clap_audio VALUES(?1, ?2, ?3, '[]', NULL, 'now')",
                params![
                    format!("hash{id}"),
                    model().clap_model,
                    to_f16_bytes(&embedding_for(label))
                ],
            )
            .unwrap();
        }
        let cache = dir.path().join("aurora").join(FILE_NAME);
        std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
        (dir, catalog, cache)
    }

    fn albums(cache: &Path) -> Vec<(String, u32, u32, Option<u8>)> {
        let c = Connection::open(cache).unwrap();
        let mut statement = c
            .prepare("SELECT album_id, tracks, mask, top FROM sound_albums ORDER BY album_id")
            .unwrap();
        statement
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn refresh_classifies_new_embeddings_and_rolls_up_albums() {
        let (dir, catalog, cache) = fixture();
        let report = refresh(&cache, &catalog, "r1").unwrap();
        assert_eq!(
            report,
            RefreshReport {
                tracks: 6,
                albums: Some(2)
            }
        );
        let found = albums(&cache);
        assert_eq!(found[0].0, "A");
        assert_eq!((found[0].1, found[0].3), (3, Some(1)));
        // B: two synth tracks and one choir track, a third, so both.
        assert_eq!(found[1].0, "B");
        assert_eq!(found[1].3, Some(5));
        assert_ne!(found[1].2 & (1 << 1), 0);

        // Nothing changed: nothing is read or rebuilt.
        assert_eq!(
            refresh(&cache, &catalog, "r1").unwrap(),
            RefreshReport::default()
        );
        // A new catalog revision rebuilds the albums only.
        assert_eq!(
            refresh(&cache, &catalog, "r2").unwrap(),
            RefreshReport {
                tracks: 0,
                albums: Some(2)
            }
        );

        // Re-embedding replaces the row with a new rowid, which is picked up.
        let clap = Connection::open(dir.path().join(CLAP_FILE)).unwrap();
        clap.execute(
            "INSERT OR REPLACE INTO clap_audio VALUES('hash6', ?1, ?2, '[]', NULL, 'later')",
            params![model().clap_model, to_f16_bytes(&embedding_for(5))],
        )
        .unwrap();
        let report = refresh(&cache, &catalog, "r2").unwrap();
        assert_eq!(
            report,
            RefreshReport {
                tracks: 1,
                albums: Some(2)
            }
        );
        assert_eq!(albums(&cache)[1].2 & (1 << 1), 0, "B lost its choir track");
    }

    #[test]
    fn refresh_waits_for_music_library_files() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join(FILE_NAME);
        let report = refresh(&cache, &dir.path().join("music-library.sqlite3"), "r").unwrap();
        assert_eq!(report, RefreshReport::default());
        assert!(!cache.exists());
    }

    /// The fixture catalog opened like a search: analysis as `sonic`, then the cache.
    pub(crate) fn search_connection() -> (tempfile::TempDir, Connection) {
        let (dir, catalog, cache) = fixture();
        refresh(&cache, &catalog, "r1").unwrap();
        let c = Connection::open(&catalog).unwrap();
        c.execute(
            "ATTACH DATABASE ?1 AS sonic",
            [dir.path().join(ANALYSIS_FILE).to_string_lossy().as_ref()],
        )
        .unwrap();
        prepare_search(&c, Some(&cache)).unwrap();
        (dir, c)
    }

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
    fn songs_match_their_own_descriptors() {
        let (_dir, c) = search_connection();
        let songs = "SELECT count(*) FROM tracks AS t WHERE 1 = 1";
        for (query, expected) in [
            ("sound:choir", 4),
            ("sound:synth", 2),
            ("sound:choral", 4),
            ("sound:choir OR synth", 6),
            ("NOT sound:choir", 3),
            ("sound:chiptune", 0),
            ("sound:synth AND year:2000", 2),
        ] {
            assert_eq!(count(&c, songs, query, false), expected, "{query}");
        }
        assert!(crate::catalog::parse_catalog_search("sound:kazoo").is_err());
    }

    #[test]
    fn albums_match_when_a_third_of_their_tracks_do() {
        let (_dir, c) = search_connection();
        let albums = "SELECT count(*) FROM albums AS a WHERE 1 = 1";
        for (query, expected) in [
            // A is all choir; B has one choir track of three, a third.
            ("sound:choir", 2),
            ("sound:synth", 1),
            // C has no analysis, so it has no descriptors.
            ("NOT sound:choir", 1),
            ("sound:piano", 0),
        ] {
            assert_eq!(count(&c, albums, query, true), expected, "{query}");
        }
    }

    #[test]
    fn group_sound_names_each_albums_top_descriptor() {
        use crate::catalog_view::{CatalogGroup, GroupField, GroupTarget, group_sql};
        let (_dir, c) = search_connection();
        let group = CatalogGroup {
            field: GroupField::Sound,
            descending: false,
        };
        for (target, sql) in [
            (
                GroupTarget::Albums,
                "SELECT {label} FROM albums AS a ORDER BY {key}, a.id",
            ),
            (
                GroupTarget::Tracks,
                "SELECT DISTINCT {label} FROM tracks AS t ORDER BY {key}",
            ),
        ] {
            let grouping = group_sql(group, target, |column| column.to_owned());
            let sql = sql
                .replace("{label}", &grouping.label)
                .replace("{key}", &grouping.key);
            let mut statement = c.prepare(&sql).unwrap();
            let labels: Vec<String> = statement
                .query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            assert_eq!(
                labels,
                ["Choir", "Synth", "No sound descriptor"],
                "{target:?}"
            );
        }
    }

    /// Builds a cache from a real Music Library store (opened read-only) into
    /// `AURORA_SOUND_PROOF_CACHE` and prints timings and sample results.
    #[test]
    #[ignore = "reads a real Music Library store; set AURORA_SOUND_PROOF_LIBRARY and AURORA_SOUND_PROOF_CACHE"]
    fn sound_descriptor_library_proof() {
        let library = PathBuf::from(std::env::var_os("AURORA_SOUND_PROOF_LIBRARY").unwrap());
        let cache = PathBuf::from(std::env::var_os("AURORA_SOUND_PROOF_CACHE").unwrap());
        assert_ne!(
            cache.parent(),
            Some(library.as_path()),
            "Keep the proof cache out of Music Library's folder"
        );
        let catalog = library.join("music-library.sqlite3");
        let started = std::time::Instant::now();
        let report = refresh(&cache, &catalog, "proof").unwrap();
        println!("first pass: {report:?} in {:?}", started.elapsed());
        let started = std::time::Instant::now();
        println!(
            "idle pass: {:?} in {:?}",
            refresh(&cache, &catalog, "proof").unwrap(),
            started.elapsed()
        );

        let c = Connection::open_with_flags(&catalog, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        c.execute(
            "ATTACH DATABASE ?1 AS sonic",
            [library.join(ANALYSIS_FILE).to_string_lossy().as_ref()],
        )
        .unwrap();
        prepare_search(&c, Some(&cache)).unwrap();
        for label in &model().labels {
            let query = format!("sound:{}", label.id);
            let started = std::time::Instant::now();
            let albums = count(
                &c,
                "SELECT count(*) FROM albums AS a WHERE 1 = 1",
                &query,
                true,
            );
            let elapsed = started.elapsed();
            let search = crate::catalog::parse_catalog_search(&query).unwrap();
            let mut params = vec![];
            let mut sql = "SELECT a.album_artist_display || ' - ' || a.album FROM albums AS a WHERE lower(a.canonical_genre) IN ('sci-fi', 'video game', 'horror')".to_owned();
            crate::catalog::push_album_search_predicates(&mut sql, &mut params, &search);
            sql.push_str(" ORDER BY random() LIMIT 4");
            let mut statement = c.prepare(&sql).unwrap();
            let sample: Vec<String> = statement
                .query_map(rusqlite::params_from_iter(params), |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            println!(
                "{query}: {albums} albums in {elapsed:?}; e.g. {}",
                sample.join(" | ")
            );
        }
        let started = std::time::Instant::now();
        let songs = "SELECT count(*) FROM (SELECT t.id FROM tracks AS t WHERE 1 = 1";
        let search = crate::catalog::parse_catalog_search("sound:chiptune").unwrap();
        let mut params = vec![];
        let mut sql = songs.to_owned();
        crate::catalog::push_track_search_predicates(&mut sql, &mut params, &search);
        sql.push_str(" ORDER BY t.id DESC LIMIT 50)");
        let first: i64 = c
            .query_row(&sql, rusqlite::params_from_iter(params), |row| row.get(0))
            .unwrap();
        println!(
            "Songs sound:chiptune first page ({first}) in {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn searches_without_a_cache_find_nothing() {
        let c = Connection::open_in_memory().unwrap();
        prepare_search(&c, None).unwrap();
        let count: i64 = c
            .query_row("SELECT (SELECT count(*) FROM temp.aurora_sound_tracks) + (SELECT count(*) FROM temp.aurora_sound_albums)", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn a_cache_from_another_model_is_not_used() {
        let (dir, catalog, cache) = fixture();
        refresh(&cache, &catalog, "r1").unwrap();
        Connection::open(&cache)
            .unwrap()
            .execute(
                "UPDATE sound_meta SET value = 'old' WHERE key = 'model'",
                [],
            )
            .unwrap();
        let c = Connection::open(&catalog).unwrap();
        c.execute(
            "ATTACH DATABASE ?1 AS sonic",
            [dir.path().join(ANALYSIS_FILE).to_string_lossy().as_ref()],
        )
        .unwrap();
        prepare_search(&c, Some(&cache)).unwrap();
        let count: i64 = c
            .query_row("SELECT count(*) FROM temp.aurora_sound_tracks", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }
}
