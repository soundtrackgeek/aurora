//! Genre check that listens: albums whose sound disagrees with their genre
//! tag, sent to "Review genre changes" with a confidence and evidence.
//!
//! Every album has a sound vector, the mean of its tracks' CLAP embeddings
//! (`sound_album_vectors`). The check only ever suggests genres already used
//! on at least `MIN_ALBUMS` albums of the library, so suggestions stay within
//! the user's own (Rate Your Music) genre names. Two passes:
//!
//! - **Retag.** For each specific genre, the normalized sum of its albums'
//!   vectors is its average sound. An album is flagged when it sits clearly
//!   closer to another specific genre's average than to its own (its own
//!   genre's average without itself), is among the least typical of its own
//!   genre, and sounds at least as typical as the median album of the other.
//! - **Refine.** Albums tagged with a broad genre such as Rock or Pop get the
//!   specific genre most of their nearest-sounding albums carry, when most of
//!   them agree.
//!
//! Score genres are named after the kind of film or game they accompany, not
//! a sound, so they, Soundtrack and a few other non-sound genres are never
//! checked or suggested. Broad, non-RYM and alternative-spelling genres are
//! never suggested either.
use crate::{
    catalog,
    control::{self, AssistantProposals, ProposedChange},
    live_genres,
    sound_descriptors::{self, DIMENSIONS},
    state_store::StateStore,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    time::Duration,
};

pub(crate) const SOURCE: &str = "genre-check";
/// Genres with fewer albums have no reliable average sound.
const MIN_ALBUMS: usize = 30;
/// Retag: how much closer the other genre must be, in cosine similarity.
const MEDIUM_GAP: f32 = 0.25;
const HIGH_GAP: f32 = 0.4;
/// Retag: the album is among this share of its genre's least typical albums.
const OWN_SHARE: f64 = 0.05;
/// Retag: it fits the other genre at least as well as this share of its albums.
const OTHER_SHARE: f64 = 0.5;
/// Refine: neighbours that vote, and the share that must agree.
const NEIGHBOURS: usize = 15;
const MEDIUM_VOTE: f64 = 0.6;
const HIGH_VOTE: f64 = 0.8;
/// Evidence: albums of the suggested genre to take tracks from, and tracks.
const EVIDENCE_ALBUMS: usize = 5;
const EVIDENCE_TRACKS: usize = 3;
const MAX_FLAGS: usize = 500;

/// Named after a kind of film, game or occasion rather than a sound.
const NOT_A_SOUND: [&str; 17] = [
    "action",
    "animation",
    "anime",
    "comedy",
    "documentary",
    "drama",
    "fantasy",
    "horror",
    "sci-fi",
    "thriller",
    "tv",
    "video game",
    "western",
    "soundtrack",
    "trailer",
    "christmas music",
    "children's music",
];

/// Top-level genres in Rate Your Music's hierarchy, and catch-all names that
/// are not RYM genres. Albums tagged with these are refined, never suggested.
const BROAD: [&str; 25] = [
    "rock",
    "pop",
    "electronic",
    "metal",
    "jazz",
    "country",
    "blues",
    "folk",
    "classical",
    "classical music",
    "hip hop",
    "rap",
    "dance",
    "punk",
    "r&b",
    "experimental",
    "singer/songwriter",
    "regional music",
    "electronica",
    "oldies",
    "funk / soul",
    "classic",
    "vocal",
    "visa",
    "instrumental",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Confidence {
    Medium,
    High,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Kind {
    /// The album sounds like a different specific genre.
    Retag,
    /// The album has a broad genre; a specific one fits its sound.
    Refine,
}

/// Why the check suggests a genre, shown in the review.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Evidence {
    pub(crate) kind: Kind,
    pub(crate) confidence: Confidence,
    pub(crate) summary: String,
    /// The suggested genre's tracks that sound most like this album.
    pub(crate) tracks: Vec<EvidenceTrack>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EvidenceTrack {
    pub(crate) track_id: String,
    pub(crate) title: Option<String>,
    pub(crate) artist: Option<String>,
    pub(crate) album: Option<String>,
    pub(crate) year: Option<i64>,
}

// ---------------------------------------------------------------------------
// The check, on vectors
// ---------------------------------------------------------------------------

/// One album: its genre tag and its unit-length sound vector.
pub(crate) struct Album {
    pub(crate) id: String,
    pub(crate) genre: String,
    pub(crate) vector: Vec<f32>,
}

#[derive(Debug, PartialEq)]
pub(crate) struct Flag {
    pub(crate) album: usize,
    pub(crate) genre: String,
    pub(crate) kind: Kind,
    pub(crate) confidence: Confidence,
    /// Retag: the gap in similarity. Refine: the share of agreeing neighbours.
    pub(crate) strength: f64,
    pub(crate) summary: String,
    /// Albums of `genre` that sound most like this one, closest first.
    pub(crate) neighbours: Vec<usize>,
}

#[derive(Clone, Copy, PartialEq)]
enum Role {
    /// Never checked, never suggested.
    Skip,
    /// Refined; never suggested.
    Broad,
    /// Checked and suggested.
    Specific,
    /// Too few albums to know its sound; left alone.
    Small,
}

/// Lower case, `&` as "and", letters and digits only: "Rock & Roll" and
/// "Rock And Roll" fold to the same key.
fn fold(genre: &str) -> String {
    genre
        .to_lowercase()
        .replace('&', "and")
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

fn roles(counts: &HashMap<&str, usize>) -> HashMap<String, Role> {
    let mut by_fold: HashMap<String, (&str, usize)> = HashMap::new();
    for (genre, count) in counts {
        let entry = by_fold.entry(fold(genre)).or_insert((genre, *count));
        if (*count, std::cmp::Reverse(*genre)) > (entry.1, std::cmp::Reverse(entry.0)) {
            *entry = (genre, *count);
        }
    }
    counts
        .iter()
        .map(|(genre, count)| {
            let lower = genre.to_lowercase();
            let role = if NOT_A_SOUND.contains(&lower.as_str()) {
                Role::Skip
            } else if BROAD.contains(&lower.as_str()) || by_fold[&fold(genre)].0 != *genre {
                // A less common spelling of another genre is refined towards it.
                Role::Broad
            } else if *count >= MIN_ALBUMS {
                Role::Specific
            } else {
                Role::Small
            };
            ((*genre).to_owned(), role)
        })
        .collect()
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    let (a8, a_rest) = a.as_chunks::<8>();
    let (b8, b_rest) = b.as_chunks::<8>();
    let mut lanes = [0f32; 8];
    for (x, y) in a8.iter().zip(b8) {
        for lane in 0..8 {
            lanes[lane] += x[lane] * y[lane];
        }
    }
    let rest: f32 = a_rest.iter().zip(b_rest).map(|(x, y)| x * y).sum();
    lanes.iter().sum::<f32>() + rest
}

fn norm(vector: &[f32]) -> f32 {
    dot(vector, vector).sqrt()
}

/// The share of `sorted` (ascending) below `value`.
fn share_below(sorted: &[f32], value: f32) -> f64 {
    sorted.partition_point(|x| *x < value) as f64 / sorted.len().max(1) as f64
}

fn percent(share: f64) -> u32 {
    (share * 100.0).round() as u32
}

/// The `count` albums among `members` closest to `vector`, closest first.
fn closest(albums: &[Album], members: &[usize], vector: &[f32], count: usize) -> Vec<usize> {
    let mut scored: Vec<(f32, usize)> = members
        .iter()
        .map(|&index| (dot(&albums[index].vector, vector), index))
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    scored
        .into_iter()
        .take(count)
        .map(|(_, index)| index)
        .collect()
}

/// Every album that sounds unlike its genre, tag errors first, at most
/// `MAX_FLAGS`.
pub(crate) fn find(albums: &[Album]) -> Vec<Flag> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for album in albums {
        *counts.entry(album.genre.as_str()).or_default() += 1;
    }
    let roles = roles(&counts);
    let mut targets: Vec<&str> = counts
        .keys()
        .copied()
        .filter(|genre| roles[*genre] == Role::Specific)
        .collect();
    targets.sort_unstable();
    let target_index: HashMap<&str, usize> =
        targets.iter().enumerate().map(|(i, g)| (*g, i)).collect();
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); targets.len()];
    let mut sums = vec![vec![0f32; DIMENSIONS]; targets.len()];
    for (index, album) in albums.iter().enumerate() {
        if let Some(&target) = target_index.get(album.genre.as_str()) {
            members[target].push(index);
            for (sum, value) in sums[target].iter_mut().zip(&album.vector) {
                *sum += value;
            }
        }
    }
    let centroids: Vec<Vec<f32>> = sums
        .iter()
        .map(|sum| {
            let length = norm(sum).max(f32::MIN_POSITIVE);
            sum.iter().map(|value| value / length).collect()
        })
        .collect();

    let mut flags = retag(albums, &targets, &members, &sums, &centroids);
    let broad: Vec<usize> = albums
        .iter()
        .enumerate()
        .filter(|(_, album)| roles[album.genre.as_str()] == Role::Broad)
        .map(|(index, _)| index)
        .collect();
    let specific: Vec<usize> = members.iter().flatten().copied().collect();
    flags.extend(refine(albums, &broad, &specific));

    // Tag errors before refinements, then the most confident.
    flags.sort_by(|a, b| {
        (a.kind == Kind::Refine)
            .cmp(&(b.kind == Kind::Refine))
            .then(b.confidence.cmp(&a.confidence))
            .then(b.strength.total_cmp(&a.strength))
            .then(a.album.cmp(&b.album))
    });
    flags.truncate(MAX_FLAGS);
    flags
}

fn retag(
    albums: &[Album],
    targets: &[&str],
    members: &[Vec<usize>],
    sums: &[Vec<f32>],
    centroids: &[Vec<f32>],
) -> Vec<Flag> {
    // Each member's similarity to its own genre without itself.
    let mut own = HashMap::new();
    let mut own_sorted = Vec::with_capacity(targets.len());
    for (target, indices) in members.iter().enumerate() {
        let mut values = Vec::with_capacity(indices.len());
        for &index in indices {
            let rest: Vec<f32> = sums[target]
                .iter()
                .zip(&albums[index].vector)
                .map(|(sum, value)| sum - value)
                .collect();
            let similarity = dot(&albums[index].vector, &rest) / norm(&rest).max(f32::MIN_POSITIVE);
            own.insert(index, similarity);
            values.push(similarity);
        }
        values.sort_by(f32::total_cmp);
        own_sorted.push(values);
    }
    // Each member's similarity to its own genre, to rank outsiders against.
    let full_sorted: Vec<Vec<f32>> = members
        .iter()
        .zip(centroids)
        .map(|(indices, centroid)| {
            let mut values: Vec<f32> = indices
                .iter()
                .map(|&index| dot(&albums[index].vector, centroid))
                .collect();
            values.sort_by(f32::total_cmp);
            values
        })
        .collect();

    let mut flags = Vec::new();
    for (target, indices) in members.iter().enumerate() {
        for &index in indices {
            let vector = &albums[index].vector;
            let Some((other, best)) = centroids
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != target)
                .map(|(other, centroid)| (other, dot(vector, centroid)))
                .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
            else {
                continue;
            };
            let own_similarity = own[&index];
            let gap = best - own_similarity;
            if gap < MEDIUM_GAP {
                continue;
            }
            let own_share = share_below(&own_sorted[target], own_similarity);
            let other_share = share_below(&full_sorted[other], best);
            if own_share > OWN_SHARE || other_share < OTHER_SHARE {
                continue;
            }
            let (from, to) = (targets[target], targets[other]);
            flags.push(Flag {
                album: index,
                genre: to.to_owned(),
                kind: Kind::Retag,
                confidence: if gap >= HIGH_GAP {
                    Confidence::High
                } else {
                    Confidence::Medium
                },
                strength: f64::from(gap),
                summary: format!(
                    "Sounds more like {to} than {from}: closer to {to}'s average sound than {}% of {to} albums, and further from {from}'s than {}% of {from} albums.",
                    percent(other_share),
                    percent(1.0 - own_share),
                ),
                neighbours: closest(albums, &members[other], vector, EVIDENCE_ALBUMS),
            });
        }
    }
    flags
}

/// The `NEIGHBOURS` most similar of `pool` to each album of `sources`, by
/// worker threads: a broad genre can hold thousands of albums.
fn nearest(albums: &[Album], sources: &[usize], pool: &[usize]) -> Vec<Vec<(f32, usize)>> {
    let threads = std::thread::available_parallelism()
        .map_or(1, |count| count.get())
        .min(sources.len().max(1));
    let chunk = sources.len().div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        let workers: Vec<_> = sources
            .chunks(chunk)
            .map(|part| {
                scope.spawn(move || {
                    part.iter()
                        .map(|&source| {
                            let vector = &albums[source].vector;
                            let mut best: Vec<(f32, usize)> = Vec::with_capacity(NEIGHBOURS + 1);
                            for &candidate in pool {
                                let similarity = dot(vector, &albums[candidate].vector);
                                if best.len() == NEIGHBOURS && similarity <= best[NEIGHBOURS - 1].0
                                {
                                    continue;
                                }
                                let at = best.partition_point(|(s, _)| *s >= similarity);
                                best.insert(at, (similarity, candidate));
                                best.truncate(NEIGHBOURS);
                            }
                            best
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().unwrap_or_default())
            .collect()
    })
}

fn refine(albums: &[Album], sources: &[usize], pool: &[usize]) -> Vec<Flag> {
    if pool.len() < NEIGHBOURS {
        return Vec::new();
    }
    let neighbours = nearest(albums, sources, pool);
    let mut flags = Vec::new();
    for (&source, near) in sources.iter().zip(neighbours) {
        let mut votes: HashMap<&str, (usize, f32)> = HashMap::new();
        for &(similarity, index) in &near {
            let vote = votes.entry(albums[index].genre.as_str()).or_default();
            vote.0 += 1;
            vote.1 += similarity;
        }
        let Some((genre, (count, _))) = votes.into_iter().max_by(|a, b| {
            (a.1.0, a.1.1)
                .partial_cmp(&(b.1.0, b.1.1))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.0.cmp(a.0))
        }) else {
            continue;
        };
        let share = count as f64 / near.len() as f64;
        if share < MEDIUM_VOTE {
            continue;
        }
        let from = &albums[source].genre;
        flags.push(Flag {
            album: source,
            genre: genre.to_owned(),
            kind: Kind::Refine,
            confidence: if share >= HIGH_VOTE {
                Confidence::High
            } else {
                Confidence::Medium
            },
            strength: share,
            summary: format!(
                "{from} is a broad genre. {count} of the {} albums that sound most like this one, among albums with a specific genre, are {genre}.",
                near.len(),
            ),
            neighbours: near
                .iter()
                .filter(|(_, index)| albums[*index].genre == genre)
                .take(EVIDENCE_ALBUMS)
                .map(|(_, index)| *index)
                .collect(),
        });
    }
    flags
}

// ---------------------------------------------------------------------------
// The check on the library
// ---------------------------------------------------------------------------

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

fn not_ready() -> String {
    "The genre check needs Music Library's audio analysis. Aurora reads it a few minutes after it starts; try again then.".to_owned()
}

/// Every album's unit sound vector, by album id.
fn album_vectors(cache: &Path) -> Result<HashMap<String, Vec<f32>>, String> {
    let connection = open_read_only(cache)?;
    let ready: bool = connection
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'sound_album_vectors')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if !ready {
        return Err(not_ready());
    }
    let table = sound_descriptors::half_table();
    let mut statement = connection
        .prepare("SELECT album_id, vector FROM sound_album_vectors")
        .map_err(|error| error.to_string())?;
    let mut rows = statement.query([]).map_err(|error| error.to_string())?;
    let mut vectors = HashMap::new();
    while let Some(row) = rows.next().map_err(|error| error.to_string())? {
        let bytes = row
            .get_ref(1)
            .and_then(|value| value.as_blob().map_err(Into::into))
            .map_err(|error| error.to_string())?;
        if bytes.len() != DIMENSIONS * 2 {
            continue;
        }
        let vector: Vec<f32> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| table[usize::from(u16::from_le_bytes(*pair))])
            .collect();
        let length = norm(&vector);
        if length > 0.0 && length.is_finite() {
            vectors.insert(
                row.get::<_, String>(0).map_err(|error| error.to_string())?,
                vector.into_iter().map(|value| value / length).collect(),
            );
        }
    }
    Ok(vectors)
}

/// Each album's genre as Aurora shows it, with pending edits, by album id.
fn album_genres(catalog: &Connection) -> Result<HashMap<String, String>, String> {
    let sql = format!(
        "SELECT a.id, {} FROM albums AS a",
        live_genres::album_sql("a")
    );
    let mut statement = catalog.prepare(&sql).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .map_err(|error| error.to_string())?;
    let mut genres = HashMap::new();
    for row in rows {
        let (id, genre) = row.map_err(|error| error.to_string())?;
        // The first genre, as Music Library's canonical genre takes it.
        if let Some(genre) = genre
            .as_deref()
            .and_then(|genre| genre.split([';', '|']).next())
            .map(str::trim)
            .filter(|genre| !genre.is_empty())
        {
            genres.insert(id, genre.to_owned());
        }
    }
    Ok(genres)
}

/// The suggested genre's tracks that sound most like `vector`, at most one
/// per album.
fn evidence_tracks(
    catalog: &Connection,
    clap: Option<&Connection>,
    album_ids: &[&str],
    vector: &[f32],
) -> Result<Vec<EvidenceTrack>, String> {
    let Some(clap) = clap else {
        return Ok(Vec::new());
    };
    let model = &sound_descriptors::model().clap_model;
    let table = sound_descriptors::half_table();
    let mut tracks = catalog
        .prepare_cached(
            "SELECT t.id, t.title, COALESCE(t.display_artist, t.album_artist_display), t.album, COALESCE(t.year, t.release_year), p.audio_hash FROM tracks AS t JOIN temp.aurora_sonic_paths AS p ON p.file_path = t.file_path AND p.filename = t.filename WHERE t.album_id = ?1",
        )
        .map_err(|error| error.to_string())?;
    let mut embedding = clap
        .prepare_cached("SELECT embedding FROM clap_audio WHERE audio_hash = ?1 AND model = ?2")
        .map_err(|error| error.to_string())?;
    let mut best: Vec<(f32, EvidenceTrack)> = Vec::new();
    for album_id in album_ids {
        let rows: Vec<(EvidenceTrack, String)> = tracks
            .query_map([album_id], |row| {
                Ok((
                    EvidenceTrack {
                        track_id: row.get::<_, i64>(0)?.to_string(),
                        title: row.get(1)?,
                        artist: row.get(2)?,
                        album: row.get(3)?,
                        year: row.get(4)?,
                    },
                    row.get(5)?,
                ))
            })
            .and_then(Iterator::collect)
            .map_err(|error| error.to_string())?;
        let mut album_best: Option<(f32, EvidenceTrack)> = None;
        for (track, hash) in rows {
            let bytes: Option<Vec<u8>> = embedding
                .query_row(rusqlite::params![hash, model], |row| row.get(0))
                .optional()
                .map_err(|error| error.to_string())?
                .flatten();
            let Some(bytes) = bytes.filter(|bytes| bytes.len() == DIMENSIONS * 2) else {
                continue;
            };
            let values: Vec<f32> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| table[usize::from(u16::from_le_bytes(*pair))])
                .collect();
            let length = norm(&values);
            if length <= 0.0 || length.is_nan() {
                continue;
            }
            let similarity = dot(&values, vector) / length;
            if album_best
                .as_ref()
                .is_none_or(|(best, _)| similarity > *best)
            {
                album_best = Some((similarity, track));
            }
        }
        best.extend(album_best);
    }
    best.sort_by(|a, b| b.0.total_cmp(&a.0));
    Ok(best
        .into_iter()
        .take(EVIDENCE_TRACKS)
        .map(|(_, track)| track)
        .collect())
}

/// Runs the check and sends its findings to "Review genre changes". `None`
/// when no album stands out.
pub(crate) fn check(
    store: &StateStore,
    proposals: &AssistantProposals,
) -> Result<Option<control::GenreProposal>, String> {
    let cache = sound_descriptors::cache_path()
        .filter(|path| path.is_file())
        .ok_or_else(not_ready)?;
    let catalog_path = catalog::default_catalog_path()?;
    let connection = catalog::open_catalog(&catalog_path)?;
    catalog::attach_local_aurora_state(&connection, store)?;
    let clap_path = catalog_path
        .parent()
        .map(|directory| directory.join("music-clap.sqlite3"))
        .filter(|path| path.is_file());
    let clap = clap_path.as_deref().map(open_read_only).transpose()?;
    let flags = check_with(&connection, clap.as_ref(), &cache)?;
    if flags.is_empty() {
        return Ok(None);
    }
    proposals.create_checked(
        SOURCE,
        "Albums whose sound fits another of your genres better than their own, or a specific genre better than a broad one. The tracks are the suggested genre's songs that sound most like each album.",
        flags,
    )
    .map(Some)
}

/// The check's changes with their evidence, strongest first.
fn check_with(
    catalog: &Connection,
    clap: Option<&Connection>,
    cache: &Path,
) -> Result<Vec<(ProposedChange, Evidence)>, String> {
    let mut vectors = album_vectors(cache)?;
    if vectors.is_empty() {
        return Err(not_ready());
    }
    let mut albums: Vec<Album> = album_genres(catalog)?
        .into_iter()
        .filter_map(|(id, genre)| {
            let vector = vectors.remove(&id)?;
            Some(Album { id, genre, vector })
        })
        .collect();
    albums.sort_by(|a, b| a.id.cmp(&b.id));
    let flags = find(&albums);
    let mut seen = HashSet::new();
    flags
        .into_iter()
        .filter(|flag| seen.insert(flag.album))
        .map(|flag| {
            let album = &albums[flag.album];
            let neighbours: Vec<&str> = flag
                .neighbours
                .iter()
                .map(|index| albums[*index].id.as_str())
                .collect();
            Ok((
                ProposedChange {
                    album_id: album.id.clone(),
                    genre: flag.genre,
                },
                Evidence {
                    kind: flag.kind,
                    confidence: flag.confidence,
                    summary: flag.summary,
                    tracks: evidence_tracks(catalog, clap, &neighbours, &album.vector)?,
                },
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit vector mostly along `axis`, nudged along `nudge`.
    fn vector(axis: usize, nudge: usize, amount: f32) -> Vec<f32> {
        let mut vector = vec![0f32; DIMENSIONS];
        vector[axis] = 1.0;
        vector[nudge] += amount;
        let length = norm(&vector);
        vector.iter().map(|value| value / length).collect()
    }

    fn album(id: String, genre: &str, vector: Vec<f32>) -> Album {
        Album {
            id,
            genre: genre.to_owned(),
            vector,
        }
    }

    /// `count` albums of a genre that sound alike (along `axis`), each a
    /// little different.
    fn genre(albums: &mut Vec<Album>, name: &str, axis: usize, count: usize) {
        for index in 0..count {
            albums.push(album(
                format!("{name}-{index}"),
                name,
                vector(axis, 100 + index % 50, 0.1 + (index % 20) as f32 * 0.02),
            ));
        }
    }

    #[test]
    fn folds_spellings_and_assigns_roles() {
        assert_eq!(fold("Rock & Roll"), fold("Rock And Roll"));
        assert_eq!(fold("Post-Punk"), fold("post punk"));
        let counts = HashMap::from([
            ("Rock & Roll", 106),
            ("Rock And Roll", 30),
            ("Rock", 3000),
            ("Hard Rock", 2500),
            ("Drama", 4000),
            ("Krautrock", 12),
        ]);
        let roles = roles(&counts);
        assert!(roles["Rock & Roll"] == Role::Specific);
        assert!(roles["Rock And Roll"] == Role::Broad);
        assert!(roles["Rock"] == Role::Broad);
        assert!(roles["Hard Rock"] == Role::Specific);
        assert!(roles["Drama"] == Role::Skip);
        assert!(roles["Krautrock"] == Role::Small);
    }

    #[test]
    fn flags_mistagged_albums_and_refines_broad_ones() {
        let mut albums = Vec::new();
        genre(&mut albums, "Synthwave", 0, 40);
        genre(&mut albums, "Hard Rock", 1, 40);
        genre(&mut albums, "Ambient", 2, 40);
        // Scores sound like anything; they are never checked or suggested.
        genre(&mut albums, "Drama", 3, 40);
        albums.push(album("drama-ambient".into(), "Drama", vector(2, 101, 0.3)));
        // A hard rock album tagged Synthwave, and one that only leans that way.
        albums.push(album("mistagged".into(), "Synthwave", vector(1, 102, 0.1)));
        albums.push(album("leaning".into(), "Synthwave", vector(0, 1, 0.6)));
        // Broad genres are refined by their neighbours, never suggested.
        albums.push(album("rock".into(), "Rock", vector(1, 103, 0.2)));
        // Its nearest albums split 8 to 7 between two genres.
        for index in 0..8 {
            albums.push(album(
                format!("split-hr-{index}"),
                "Hard Rock",
                vector(6, 150 + index, 0.2),
            ));
        }
        for index in 0..7 {
            albums.push(album(
                format!("split-am-{index}"),
                "Ambient",
                vector(6, 160 + index, 0.2),
            ));
        }
        albums.push(album("vague".into(), "Rock", vector(6, 170, 0.2)));
        for index in 0..20 {
            albums.push(album(format!("pop-{index}"), "Pop", vector(1, 104, 0.25)));
        }
        let flags = find(&albums);
        let by_id: HashMap<&str, &Flag> = flags
            .iter()
            .map(|flag| (albums[flag.album].id.as_str(), flag))
            .collect();

        let mistagged = by_id["mistagged"];
        assert_eq!(mistagged.genre, "Hard Rock");
        assert_eq!(mistagged.kind, Kind::Retag);
        assert_eq!(mistagged.confidence, Confidence::High);
        assert_eq!(mistagged.neighbours.len(), EVIDENCE_ALBUMS);
        assert!(
            mistagged
                .neighbours
                .iter()
                .all(|i| albums[*i].genre == "Hard Rock")
        );
        assert!(
            mistagged
                .summary
                .starts_with("Sounds more like Hard Rock than Synthwave")
        );
        assert!(!by_id.contains_key("leaning"));
        assert!(!by_id.contains_key("drama-ambient"));

        let rock = by_id["rock"];
        assert_eq!(rock.genre, "Hard Rock");
        assert_eq!(rock.kind, Kind::Refine);
        assert_eq!(rock.confidence, Confidence::High);
        // Pop albums are refined too, never suggested to others.
        assert!(
            flags
                .iter()
                .all(|flag| !["Pop", "Rock", "Drama"].contains(&flag.genre.as_str()))
        );
        // An album between genres has no majority.
        assert!(!by_id.contains_key("vague"));
        // Tag errors lead.
        assert_eq!(flags[0].kind, Kind::Retag);
    }

    #[test]
    fn nearest_keeps_the_closest_in_order() {
        let albums: Vec<Album> = (0..40)
            .map(|index| album(index.to_string(), "X", vector(0, 1, index as f32 / 10.0)))
            .collect();
        let pool: Vec<usize> = (1..40).collect();
        let near = nearest(&albums, &[0], &pool);
        assert_eq!(near.len(), 1);
        let order: Vec<usize> = near[0].iter().map(|(_, index)| *index).collect();
        assert_eq!(order, (1..=NEIGHBOURS).collect::<Vec<_>>());
    }

    /// Prints the check on the real library. Read-only.
    /// `AURORA_PROOF_SOUND_CACHE=<cache> cargo test --release genre_check::tests::real_library_proof -- --ignored --nocapture`
    #[test]
    #[ignore = "reads the real library"]
    fn real_library_proof() {
        let cache = sound_descriptors::cache_path().expect("set AURORA_PROOF_SOUND_CACHE");
        let catalog_path = catalog::default_catalog_path().unwrap();
        let connection = catalog::open_catalog(&catalog_path).unwrap();
        let state = crate::mcp::app_data_dir().unwrap();
        let store = StateStore::open_existing(state.join("aurora-state.sqlite3")).unwrap();
        catalog::attach_local_aurora_state(&connection, &store).unwrap();
        let clap =
            open_read_only(&catalog_path.parent().unwrap().join("music-clap.sqlite3")).unwrap();
        let started = std::time::Instant::now();
        let changes = check_with(&connection, Some(&clap), &cache).unwrap();
        println!(
            "{} albums flagged in {:?}",
            changes.len(),
            started.elapsed()
        );
        let count = |kind, confidence| {
            changes
                .iter()
                .filter(|(_, e)| e.kind == kind && e.confidence == confidence)
                .count()
        };
        println!(
            "retag high {} medium {}, refine high {} medium {}",
            count(Kind::Retag, Confidence::High),
            count(Kind::Retag, Confidence::Medium),
            count(Kind::Refine, Confidence::High),
            count(Kind::Refine, Confidence::Medium),
        );
        for (change, evidence) in changes.iter().step_by(changes.len().div_ceil(25).max(1)) {
            let album = crate::genre_rule::load_album(&connection, &change.album_id)
                .unwrap()
                .unwrap();
            println!(
                "{} - {} ({:?}) -> {} [{:?} {:?}]\n    {}",
                album.artist.unwrap_or_default(),
                album.title.unwrap_or_default(),
                album.year,
                change.genre,
                evidence.kind,
                evidence.confidence,
                evidence.summary
            );
            for track in &evidence.tracks {
                println!(
                    "      {} - {} ({})",
                    track.artist.clone().unwrap_or_default(),
                    track.title.clone().unwrap_or_default(),
                    track.album.clone().unwrap_or_default()
                );
            }
        }
    }
}
