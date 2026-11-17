//! Genre check that listens: albums whose sound disagrees with their genre
//! tag, sent to "Review genre changes" with a confidence and evidence.
//!
//! Every album has a sound vector, the mean of its tracks' CLAP embeddings
//! (`sound_album_vectors`). The check only ever suggests genres already used
//! in the library, so suggestions stay within the user's own genre names:
//! Rate Your Music subgenres (`rym_genres`) on at least `MIN_RYM_SUGGESTED`
//! albums, and other genres on at least `MIN_ALBUMS`. Albums of rarer RYM
//! subgenres, down to `MIN_RYM_CHECKED` albums, are still checked against
//! their genre's other albums. Two passes:
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
//! checked or suggested. Broad genres (RYM's main genres and catch-all names)
//! and alternative spellings are never suggested either.
use crate::{
    catalog,
    control::{self, AssistantProposals, ProposedChange},
    live_genres,
    rym_genres::{self, Level, fold},
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
/// Genres that are not Rate Your Music subgenres need this many albums to be
/// checked or suggested: fewer have no reliable average sound.
const MIN_ALBUMS: usize = 30;
/// A Rate Your Music subgenre is checked from this many albums: each album is
/// compared with the average of at least two others.
const MIN_RYM_CHECKED: usize = 3;
/// A Rate Your Music subgenre is suggested to other albums from this many.
/// A smaller genre's average is mostly one artist's sound.
const MIN_RYM_SUGGESTED: usize = 10;
/// Retag: how much closer the other genre must be, in cosine similarity.
const MEDIUM_GAP: f32 = 0.25;
const HIGH_GAP: f32 = 0.4;
/// Retag: the album is among this share of its genre's least typical albums.
const OWN_SHARE: f64 = 0.05;
/// Retag: it fits the other genre at least as well as this share of its albums.
const OTHER_SHARE: f64 = 0.5;
/// Refine: neighbours that vote, and the share that must agree.
const NEIGHBOURS: usize = 15;
/// Sources compared with each candidate together; their vectors stay in cache.
const SOURCE_BLOCK: usize = 32;
const MEDIUM_VOTE: f64 = 0.6;
const HIGH_VOTE: f64 = 0.8;
/// Evidence: albums of the suggested genre to take tracks from, and tracks.
const EVIDENCE_ALBUMS: usize = 5;
const EVIDENCE_TRACKS: usize = 3;
const MAX_FLAGS: usize = 500;
/// Other genres listed with each album, and offered in the review.
const CLOSEST_GENRES: usize = 3;

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

/// Top-level genres, and catch-all names that are not RYM genres. Albums
/// tagged with these, or with any main genre of `rym_genres`, are refined,
/// never suggested.
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
    /// A chosen album that sounds like its genre, or whose broad genre has no
    /// clear specific one. Reported only when checking chosen albums.
    Fits,
}

/// A genre the album sounds close to.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NearGenre {
    pub(crate) genre: String,
    /// The share of the genre's albums this one is closer to its average
    /// sound than, 0 to 100: how typical of the genre it would be.
    pub(crate) percentile: u32,
}

/// Why the check suggests a genre, shown in the review.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Evidence {
    pub(crate) kind: Kind,
    /// None for `Kind::Fits`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) confidence: Option<Confidence>,
    pub(crate) summary: String,
    /// The suggested genre's tracks that sound most like this album.
    pub(crate) tracks: Vec<EvidenceTrack>,
    /// The other specific genres it sounds closest to, closest first.
    pub(crate) closest: Vec<NearGenre>,
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
    pub(crate) confidence: Option<Confidence>,
    /// Retag: the gap in similarity. Refine: the share of agreeing neighbours.
    pub(crate) strength: f64,
    pub(crate) summary: String,
    /// Albums of `genre` that sound most like this one, closest first.
    pub(crate) neighbours: Vec<usize>,
    pub(crate) closest: Vec<NearGenre>,
}

#[derive(Clone, Copy, PartialEq)]
enum Role {
    /// Never checked, never suggested.
    Skip,
    /// Refined; never suggested.
    Broad,
    /// Checked and suggested.
    Specific,
    /// A Rate Your Music subgenre on a few albums: checked against its other
    /// albums, never suggested.
    Rare,
    /// Too few albums to know its sound; left alone.
    Small,
}

impl Role {
    /// Whether albums with this genre are compared with its average sound.
    fn has_sound(self) -> bool {
        matches!(self, Role::Specific | Role::Rare)
    }
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
            let level = rym_genres::level(genre);
            let role = if NOT_A_SOUND.contains(&lower.as_str()) {
                Role::Skip
            } else if BROAD.contains(&lower.as_str())
                || level == Some(Level::Main)
                || by_fold[&fold(genre)].0 != *genre
            {
                // A less common spelling of another genre is refined towards it.
                Role::Broad
            } else if *count >= MIN_ALBUMS
                || (level == Some(Level::Sub) && *count >= MIN_RYM_SUGGESTED)
            {
                Role::Specific
            } else if level == Some(Level::Sub) && *count >= MIN_RYM_CHECKED {
                Role::Rare
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
fn closest_albums(albums: &[Album], members: &[usize], vector: &[f32], count: usize) -> Vec<usize> {
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

/// Each genre's role in the check, by the albums that carry it.
fn album_roles(albums: &[Album]) -> HashMap<String, Role> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for album in albums {
        *counts.entry(album.genre.as_str()).or_default() += 1;
    }
    roles(&counts)
}

/// Every album that sounds unlike its genre, tag errors first, at most
/// `MAX_FLAGS`.
#[cfg(test)]
pub(crate) fn find(albums: &[Album]) -> Vec<Flag> {
    find_for(albums, None)
}

/// The specific genres' sounds and how typical each member is of its own.
struct Genres<'a> {
    names: Vec<&'a str>,
    /// Whether the genre may be suggested to other albums (not `Role::Rare`).
    suggested: Vec<bool>,
    members: Vec<Vec<usize>>,
    centroids: Vec<Vec<f32>>,
    /// Each member's similarity to its own genre without itself.
    own: HashMap<usize, f32>,
    own_sorted: Vec<Vec<f32>>,
    /// Members' similarity to their own genre, to rank outsiders against.
    full_sorted: Vec<Vec<f32>>,
}

impl<'a> Genres<'a> {
    fn new(albums: &'a [Album], roles: &HashMap<String, Role>) -> Self {
        let distinct: HashSet<&str> = albums.iter().map(|album| album.genre.as_str()).collect();
        let mut names: Vec<&str> = distinct
            .into_iter()
            .filter(|genre| roles[*genre].has_sound())
            .collect();
        names.sort_unstable();
        let suggested = names
            .iter()
            .map(|genre| roles[*genre] == Role::Specific)
            .collect();
        let index: HashMap<&str, usize> = names.iter().enumerate().map(|(i, g)| (*g, i)).collect();
        let mut members: Vec<Vec<usize>> = vec![Vec::new(); names.len()];
        let mut sums = vec![vec![0f32; DIMENSIONS]; names.len()];
        for (position, album) in albums.iter().enumerate() {
            if let Some(&genre) = index.get(album.genre.as_str()) {
                members[genre].push(position);
                for (sum, value) in sums[genre].iter_mut().zip(&album.vector) {
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
        let mut own = HashMap::new();
        let mut own_sorted = Vec::with_capacity(names.len());
        for (genre, indices) in members.iter().enumerate() {
            let mut values = Vec::with_capacity(indices.len());
            for &position in indices {
                let rest: Vec<f32> = sums[genre]
                    .iter()
                    .zip(&albums[position].vector)
                    .map(|(sum, value)| sum - value)
                    .collect();
                let similarity =
                    dot(&albums[position].vector, &rest) / norm(&rest).max(f32::MIN_POSITIVE);
                own.insert(position, similarity);
                values.push(similarity);
            }
            values.sort_by(f32::total_cmp);
            own_sorted.push(values);
        }
        let full_sorted = members
            .iter()
            .zip(&centroids)
            .map(|(indices, centroid)| {
                let mut values: Vec<f32> = indices
                    .iter()
                    .map(|&position| dot(&albums[position].vector, centroid))
                    .collect();
                values.sort_by(f32::total_cmp);
                values
            })
            .collect();
        Self {
            names,
            suggested,
            members,
            centroids,
            own,
            own_sorted,
            full_sorted,
        }
    }

    /// "of Hard Rock albums", or "of the 8 Neoclassical New Age albums" for a
    /// genre on few albums, where each album weighs in the percentage.
    fn albums_of(&self, genre: usize) -> String {
        let name = self.names[genre];
        let count = self.members[genre].len();
        if count < MIN_ALBUMS {
            format!("of the {count} {name} albums")
        } else {
            format!("of {name} albums")
        }
    }

    /// The suggestable genres other than `except` this sound would be most
    /// typical of, by the share of each genre's albums it is closer than.
    fn closest(&self, vector: &[f32], except: Option<usize>) -> Vec<NearGenre> {
        let mut near: Vec<(f64, f32, usize)> = self
            .centroids
            .iter()
            .enumerate()
            .filter(|(genre, _)| Some(*genre) != except && self.suggested[*genre])
            .map(|(genre, centroid)| {
                let similarity = dot(vector, centroid);
                (
                    share_below(&self.full_sorted[genre], similarity),
                    similarity,
                    genre,
                )
            })
            .collect();
        near.sort_by(|a, b| b.0.total_cmp(&a.0).then(b.1.total_cmp(&a.1)));
        near.into_iter()
            .take(CLOSEST_GENRES)
            .map(|(share, _, genre)| NearGenre {
                genre: self.names[genre].to_owned(),
                percentile: percent(share),
            })
            .collect()
    }
}

/// Like `find`, checking only the albums in `only` when given. The whole
/// library still sets each genre's sound and the neighbours that vote. A
/// check of chosen albums also reports the ones that fit, as `Kind::Fits`.
pub(crate) fn find_for(albums: &[Album], only: Option<&HashSet<usize>>) -> Vec<Flag> {
    let roles = album_roles(albums);
    let genres = Genres::new(albums, &roles);
    let wanted = |position: usize| only.is_none_or(|only| only.contains(&position));
    let report_fits = only.is_some();
    let mut flags = retag(albums, &genres, &wanted, report_fits);
    let broad: Vec<usize> = albums
        .iter()
        .enumerate()
        .filter(|(position, album)| roles[album.genre.as_str()] == Role::Broad && wanted(*position))
        .map(|(position, _)| position)
        .collect();
    flags.extend(refine(albums, &genres, &broad, report_fits));

    // Tag errors, then refinements, then albums that fit; the most confident first.
    let rank = |kind: Kind| match kind {
        Kind::Retag => 0,
        Kind::Refine => 1,
        Kind::Fits => 2,
    };
    flags.sort_by(|a, b| {
        rank(a.kind)
            .cmp(&rank(b.kind))
            .then(b.confidence.cmp(&a.confidence))
            .then(b.strength.total_cmp(&a.strength))
            .then(a.album.cmp(&b.album))
    });
    flags.truncate(MAX_FLAGS);
    flags
}

fn retag(
    albums: &[Album],
    genres: &Genres,
    wanted: &dyn Fn(usize) -> bool,
    report_fits: bool,
) -> Vec<Flag> {
    let mut flags = Vec::new();
    for (genre, indices) in genres.members.iter().enumerate() {
        for &position in indices.iter().filter(|position| wanted(**position)) {
            let vector = &albums[position].vector;
            let own_similarity = genres.own[&position];
            let own_share = share_below(&genres.own_sorted[genre], own_similarity);
            let name = genres.names[genre];
            let best = genres
                .centroids
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != genre && genres.suggested[*other])
                .map(|(other, centroid)| (other, dot(vector, centroid)))
                .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)));
            let flagged = best.and_then(|(other, best)| {
                let gap = best - own_similarity;
                let other_share = share_below(&genres.full_sorted[other], best);
                (gap >= MEDIUM_GAP && own_share <= OWN_SHARE && other_share >= OTHER_SHARE)
                    .then_some((other, gap, other_share))
            });
            if let Some((other, gap, other_share)) = flagged {
                let to = genres.names[other];
                flags.push(Flag {
                    album: position,
                    genre: to.to_owned(),
                    kind: Kind::Retag,
                    confidence: Some(if gap >= HIGH_GAP {
                        Confidence::High
                    } else {
                        Confidence::Medium
                    }),
                    strength: f64::from(gap),
                    summary: format!(
                        "Sounds more like {to} than {name}: closer to {to}'s average sound than {}% {}, and further from {name}'s than {}% {}.",
                        percent(other_share),
                        genres.albums_of(other),
                        percent(1.0 - own_share),
                        genres.albums_of(genre),
                    ),
                    neighbours: closest_albums(
                        albums,
                        &genres.members[other],
                        vector,
                        EVIDENCE_ALBUMS,
                    ),
                    closest: genres.closest(vector, Some(genre)),
                });
            } else if report_fits {
                flags.push(Flag {
                    album: position,
                    genre: name.to_owned(),
                    kind: Kind::Fits,
                    confidence: None,
                    strength: own_share,
                    summary: format!(
                        "Sounds like {name}: closer to its average sound than {}% {}.",
                        percent(own_share),
                        genres.albums_of(genre),
                    ),
                    neighbours: Vec::new(),
                    closest: genres.closest(vector, Some(genre)),
                });
            }
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
                    // A block of sources meets each candidate while it is in
                    // cache, instead of streaming the whole pool per source.
                    let mut results = Vec::with_capacity(part.len());
                    for block in part.chunks(SOURCE_BLOCK) {
                        let mut best: Vec<Vec<(f32, usize)>> =
                            vec![Vec::with_capacity(NEIGHBOURS + 1); block.len()];
                        for &candidate in pool {
                            let candidate_vector = &albums[candidate].vector;
                            for (&source, best) in block.iter().zip(best.iter_mut()) {
                                let similarity = dot(&albums[source].vector, candidate_vector);
                                if best.len() == NEIGHBOURS && similarity <= best[NEIGHBOURS - 1].0
                                {
                                    continue;
                                }
                                let at = best.partition_point(|(s, _)| *s >= similarity);
                                best.insert(at, (similarity, candidate));
                                best.truncate(NEIGHBOURS);
                            }
                        }
                        results.extend(best);
                    }
                    results
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().unwrap_or_default())
            .collect()
    })
}

fn refine(albums: &[Album], genres: &Genres, sources: &[usize], report_fits: bool) -> Vec<Flag> {
    let pool: Vec<usize> = genres
        .members
        .iter()
        .zip(&genres.suggested)
        .filter(|(_, suggested)| **suggested)
        .flat_map(|(members, _)| members.iter().copied())
        .collect();
    if pool.len() < NEIGHBOURS {
        return Vec::new();
    }
    let neighbours = nearest(albums, sources, &pool);
    let mut flags = Vec::new();
    for (&source, near) in sources.iter().zip(neighbours) {
        let mut votes: HashMap<&str, (usize, f32)> = HashMap::new();
        for &(similarity, position) in &near {
            let vote = votes.entry(albums[position].genre.as_str()).or_default();
            vote.0 += 1;
            vote.1 += similarity;
        }
        let mut ranked: Vec<(&str, usize, f32)> = votes
            .into_iter()
            .map(|(genre, (count, similarity))| (genre, count, similarity))
            .collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.total_cmp(&a.2)).then(a.0.cmp(b.0)));
        let Some(&(genre, count, _)) = ranked.first() else {
            continue;
        };
        let share = count as f64 / near.len() as f64;
        let from = &albums[source].genre;
        if share >= MEDIUM_VOTE {
            flags.push(Flag {
                album: source,
                genre: genre.to_owned(),
                kind: Kind::Refine,
                confidence: Some(if share >= HIGH_VOTE {
                    Confidence::High
                } else {
                    Confidence::Medium
                }),
                strength: share,
                summary: format!(
                    "{from} is a broad genre. {count} of the {} albums that sound most like this one, among albums with a specific genre, are {genre}.",
                    near.len(),
                ),
                neighbours: near
                    .iter()
                    .filter(|(_, position)| albums[*position].genre == genre)
                    .take(EVIDENCE_ALBUMS)
                    .map(|(_, position)| *position)
                    .collect(),
                closest: genres.closest(&albums[source].vector, None),
            });
        } else if report_fits {
            let split = ranked
                .iter()
                .take(CLOSEST_GENRES)
                .map(|(genre, count, _)| format!("{genre} {count}"))
                .collect::<Vec<_>>()
                .join(", ");
            flags.push(Flag {
                album: source,
                genre: from.clone(),
                kind: Kind::Fits,
                confidence: None,
                strength: share,
                summary: format!(
                    "{from} is a broad genre, and the {} albums that sound most like this one do not agree on a specific one: {split}.",
                    near.len(),
                ),
                neighbours: Vec::new(),
                closest: genres.closest(&albums[source].vector, None),
            });
        }
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

/// The analyzed tracks of each album, with their audio hashes.
type Candidates = Vec<Vec<(EvidenceTrack, String)>>;

fn candidate_tracks(catalog: &Connection, album_ids: &[&str]) -> Result<Candidates, String> {
    let mut tracks = catalog
        .prepare_cached(
            "SELECT t.id, t.title, COALESCE(t.display_artist, t.album_artist_display), t.album, COALESCE(t.year, t.release_year), p.audio_hash FROM tracks AS t JOIN temp.aurora_sonic_paths AS p ON p.file_path = t.file_path AND p.filename = t.filename WHERE t.album_id = ?1",
        )
        .map_err(|error| error.to_string())?;
    album_ids
        .iter()
        .map(|album_id| {
            tracks
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
                .map_err(|error| error.to_string())
        })
        .collect()
}

/// Of `candidates`, the tracks that sound most like `vector`, at most one
/// per album.
fn best_tracks(
    clap: &Connection,
    candidates: Candidates,
    vector: &[f32],
) -> Result<Vec<EvidenceTrack>, String> {
    let model = &sound_descriptors::model().clap_model;
    let table = sound_descriptors::half_table();
    let mut embedding = clap
        .prepare_cached("SELECT embedding FROM clap_audio WHERE audio_hash = ?1 AND model = ?2")
        .map_err(|error| error.to_string())?;
    let mut best: Vec<(f32, EvidenceTrack)> = Vec::new();
    for album in candidates {
        let mut album_best: Option<(f32, EvidenceTrack)> = None;
        for (track, hash) in album {
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

/// Evidence tracks for every job, by worker threads with their own
/// read-only connection: the embeddings are random reads in a large file.
fn evidence_for(
    clap: Option<&Path>,
    jobs: Vec<(Candidates, &[f32])>,
) -> Result<Vec<Vec<EvidenceTrack>>, String> {
    let Some(clap) = clap else {
        return Ok(vec![Vec::new(); jobs.len()]);
    };
    if jobs.iter().all(|(candidates, _)| candidates.is_empty()) {
        return Ok(vec![Vec::new(); jobs.len()]);
    }
    let threads = std::thread::available_parallelism()
        .map_or(1, |count| count.get())
        .min(jobs.len().max(1));
    let chunk = jobs.len().div_ceil(threads).max(1);
    let mut parts: Vec<Vec<(Candidates, &[f32])>> = Vec::new();
    let mut jobs = jobs.into_iter().peekable();
    while jobs.peek().is_some() {
        parts.push(jobs.by_ref().take(chunk).collect());
    }
    std::thread::scope(|scope| {
        let workers: Vec<_> = parts
            .into_iter()
            .map(|part| {
                scope.spawn(move || -> Result<Vec<Vec<EvidenceTrack>>, String> {
                    let connection = open_read_only(clap)?;
                    part.into_iter()
                        .map(|(candidates, vector)| best_tracks(&connection, candidates, vector))
                        .collect()
                })
            })
            .collect();
        let mut results = Vec::new();
        for worker in workers {
            results.extend(
                worker
                    .join()
                    .map_err(|_| "The genre check stopped unexpectedly.".to_owned())??,
            );
        }
        Ok(results)
    })
}

/// What a check found, for the message beside its button.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SoundCheck {
    /// The findings, open in "Review genre changes"; `None` when no album
    /// stands out.
    pub(crate) proposal: Option<control::GenreProposal>,
    pub(crate) checked: usize,
    /// Checked albums that sound like their genre.
    pub(crate) fitting: usize,
    /// Albums without analyzed tracks or without a genre.
    pub(crate) unanalyzed: usize,
    /// Albums with a genre the check leaves alone: a score or other non-sound
    /// genre, or one with too few albums to know its sound.
    pub(crate) not_checked: usize,
}

#[derive(Debug, Default, PartialEq)]
struct Counts {
    checked: usize,
    fitting: usize,
    unanalyzed: usize,
    not_checked: usize,
}

/// Runs the check on the whole library, or on `album_ids`, and sends its
/// findings to "Review genre changes".
pub(crate) fn check(
    store: &StateStore,
    proposals: &AssistantProposals,
    album_ids: Option<&[String]>,
) -> Result<SoundCheck, String> {
    if let Some(ids) = album_ids {
        if ids.is_empty() {
            return Err("Choose at least one album to check.".to_owned());
        }
        if ids.len() > MAX_FLAGS {
            return Err(format!(
                "Check at most {MAX_FLAGS} albums at a time, or use Genres → Check genres by sound for the whole library."
            ));
        }
    }
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
    let (changes, counts) = check_with(&connection, clap_path.as_deref(), &cache, album_ids)?;
    let proposal = if changes.is_empty() {
        None
    } else {
        let reason = if album_ids.is_some() {
            "The albums you chose, judged by sound against your whole library. Albums that fit their genre keep it and start unticked; pick one of the close genres to change it. The tracks are the suggested genre's songs that sound most like each album."
        } else {
            "Albums whose sound fits another of your genres better than their own, or a specific genre better than a broad one. The tracks are the suggested genre's songs that sound most like each album."
        };
        Some(proposals.create_checked(SOURCE, reason, changes)?)
    };
    Ok(SoundCheck {
        proposal,
        checked: counts.checked,
        fitting: counts.fitting,
        unanalyzed: counts.unanalyzed,
        not_checked: counts.not_checked,
    })
}

/// How many of `indices` the check looks at, and how many it leaves alone.
fn count_roles(
    albums: &[Album],
    roles: &HashMap<String, Role>,
    indices: impl Iterator<Item = usize>,
    counts: &mut Counts,
) {
    for index in indices {
        match roles[albums[index].genre.as_str()] {
            Role::Skip | Role::Small => counts.not_checked += 1,
            Role::Broad | Role::Specific | Role::Rare => counts.checked += 1,
        }
    }
}

/// The check's changes with their evidence, strongest first, and what
/// happened to the albums it was asked about.
fn check_with(
    catalog: &Connection,
    clap: Option<&Path>,
    cache: &Path,
    album_ids: Option<&[String]>,
) -> Result<(Vec<(ProposedChange, Evidence)>, Counts), String> {
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
    let wanted: Option<HashSet<&str>> =
        album_ids.map(|ids| ids.iter().map(String::as_str).collect());
    let only: Option<HashSet<usize>> = wanted.as_ref().map(|wanted| {
        albums
            .iter()
            .enumerate()
            .filter(|(_, album)| wanted.contains(album.id.as_str()))
            .map(|(index, _)| index)
            .collect()
    });
    let flags = find_for(&albums, only.as_ref());
    let roles = album_roles(&albums);
    let mut counts = Counts::default();
    match (&only, &wanted) {
        (Some(only), Some(wanted)) => {
            counts.unanalyzed = wanted.len() - only.len();
            count_roles(&albums, &roles, only.iter().copied(), &mut counts);
        }
        _ => count_roles(&albums, &roles, 0..albums.len(), &mut counts),
    }
    let mut seen = HashSet::new();
    let flags: Vec<Flag> = flags
        .into_iter()
        .filter(|flag| seen.insert(flag.album))
        .collect();
    let jobs = flags
        .iter()
        .map(|flag| {
            let neighbours: Vec<&str> = flag
                .neighbours
                .iter()
                .map(|index| albums[*index].id.as_str())
                .collect();
            Ok((
                candidate_tracks(catalog, &neighbours)?,
                albums[flag.album].vector.as_slice(),
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let tracks = evidence_for(clap, jobs)?;
    let changes: Vec<(ProposedChange, Evidence)> = flags
        .into_iter()
        .zip(tracks)
        .map(|(flag, tracks)| {
            (
                ProposedChange {
                    album_id: albums[flag.album].id.clone(),
                    genre: flag.genre,
                },
                Evidence {
                    kind: flag.kind,
                    confidence: flag.confidence,
                    summary: flag.summary,
                    tracks,
                    closest: flag.closest,
                },
            )
        })
        .collect();
    let flagged = changes
        .iter()
        .filter(|(_, evidence)| evidence.kind != Kind::Fits)
        .count();
    counts.fitting = counts.checked.saturating_sub(flagged);
    Ok((changes, counts))
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
            ("Ambient", 495),
            ("New Age", 21),
            ("Doom Metal", 12),
            ("Neoclassical New Age", 8),
            ("Celtic New Age", 2),
        ]);
        let roles = roles(&counts);
        // Rate Your Music's main genres are broad; its subgenres count from
        // a few albums, other genres from 30.
        assert!(roles["Ambient"] == Role::Broad);
        assert!(roles["New Age"] == Role::Broad);
        assert!(roles["Doom Metal"] == Role::Specific);
        assert!(roles["Neoclassical New Age"] == Role::Rare);
        assert!(roles["Celtic New Age"] == Role::Small);
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
        genre(&mut albums, "Dark Ambient", 2, 40);
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
                "Dark Ambient",
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
        assert_eq!(mistagged.confidence, Some(Confidence::High));
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
        assert_eq!(rock.confidence, Some(Confidence::High));
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

        // Checking chosen albums reports only those, judged against the
        // whole library.
        let index = |id: &str| albums.iter().position(|album| album.id == id).unwrap();
        let only = HashSet::from([index("rock"), index("leaning"), index("Synthwave-3")]);
        let chosen = find_for(&albums, Some(&only));
        assert_eq!(chosen.len(), 3);
        assert_eq!(chosen[0], *rock);
        assert!(rock.closest.iter().any(|near| near.genre == "Hard Rock"));
        for fits in &chosen[1..] {
            assert_eq!(fits.kind, Kind::Fits);
            assert_eq!(fits.genre, "Synthwave");
            assert_eq!(fits.confidence, None);
            assert!(
                fits.summary
                    .starts_with("Sounds like Synthwave: closer to its average sound than")
            );
            // Every other suggestable genre: Hard Rock and Dark Ambient.
            assert_eq!(fits.closest.len(), 2);
            assert!(fits.closest.iter().all(|near| near.genre != "Synthwave"));
        }
        // The album that leans towards Hard Rock lists it first.
        let leaning = chosen
            .iter()
            .find(|flag| albums[flag.album].id == "leaning")
            .unwrap();
        assert_eq!(leaning.closest[0].genre, "Hard Rock");
        // A broad album without a clear specific genre shows the split vote.
        let vague = find_for(&albums, Some(&HashSet::from([index("vague")])));
        assert_eq!(vague.len(), 1);
        assert_eq!(vague[0].kind, Kind::Fits);
        assert_eq!(vague[0].genre, "Rock");
        assert!(
            vague[0]
                .summary
                .contains("do not agree on a specific one: Hard Rock 8, Dark Ambient 7"),
            "{}",
            vague[0].summary
        );
    }

    #[test]
    fn checks_rare_genres_but_never_suggests_them() {
        let mut albums = Vec::new();
        genre(&mut albums, "Synthwave", 0, 40);
        genre(&mut albums, "Hard Rock", 1, 40);
        // A Rate Your Music subgenre on six albums, one of them hard rock.
        genre(&mut albums, "Neoclassical New Age", 7, 5);
        albums.push(album(
            "rock-as-new-age".into(),
            "Neoclassical New Age",
            vector(1, 102, 0.1),
        ));
        // A synthwave album that sounds like the rare genre.
        albums.push(album(
            "new-age-as-synthwave".into(),
            "Synthwave",
            vector(7, 101, 0.1),
        ));
        let flags = find(&albums);
        let rock = flags
            .iter()
            .find(|flag| albums[flag.album].id == "rock-as-new-age")
            .expect("the rare genre's odd album is checked");
        assert_eq!(rock.genre, "Hard Rock");
        assert!(
            rock.summary
                .ends_with("further from Neoclassical New Age's than 100% of the 6 Neoclassical New Age albums."),
            "{}",
            rock.summary
        );
        assert!(
            flags
                .iter()
                .all(|flag| flag.genre != "Neoclassical New Age")
        );
        assert!(
            flags
                .iter()
                .flat_map(|flag| &flag.closest)
                .all(|near| near.genre != "Neoclassical New Age")
        );

        // A chosen album of the rare genre that fits it says so.
        let fits = find_for(&albums, Some(&HashSet::from([albums.len() - 3])));
        assert_eq!(fits.len(), 1);
        assert_eq!(fits[0].kind, Kind::Fits);
        assert!(
            fits[0]
                .summary
                .starts_with("Sounds like Neoclassical New Age: closer to its average sound than")
                && fits[0]
                    .summary
                    .ends_with("of the 6 Neoclassical New Age albums."),
            "{}",
            fits[0].summary
        );
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
        let clap = catalog_path.parent().unwrap().join("music-clap.sqlite3");
        // A few chosen albums, as from the Albums page's menu.
        let chosen: Vec<String> = connection
            .prepare("SELECT id FROM (SELECT id, canonical_genre, ROW_NUMBER() OVER (PARTITION BY canonical_genre ORDER BY id) AS n FROM albums WHERE canonical_genre IN ('Synthwave', 'Rock', 'Hard Rock', 'Drama', 'Neoclassical New Age')) WHERE n <= 3")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let started = std::time::Instant::now();
        let (picked, counts) =
            check_with(&connection, Some(clap.as_path()), &cache, Some(&chosen)).unwrap();
        println!(
            "chosen {} in {:?}: {counts:?}",
            chosen.len(),
            started.elapsed()
        );
        for (change, evidence) in &picked {
            let album = crate::genre_rule::load_album(&connection, &change.album_id)
                .unwrap()
                .unwrap();
            let closest: Vec<String> = evidence
                .closest
                .iter()
                .map(|near| format!("{} {}%", near.genre, near.percentile))
                .collect();
            println!(
                "  {} - {} -> {} [{:?}]\n    {}\n    closest: {}",
                album.artist.unwrap_or_default(),
                album.title.unwrap_or_default(),
                change.genre,
                evidence.kind,
                evidence.summary,
                closest.join(", ")
            );
        }

        let started = std::time::Instant::now();
        let (changes, counts) =
            check_with(&connection, Some(clap.as_path()), &cache, None).unwrap();
        println!("{counts:?}");
        println!(
            "{} albums flagged in {:?}",
            changes.len(),
            started.elapsed()
        );
        let count = |kind, confidence| {
            changes
                .iter()
                .filter(|(_, e)| e.kind == kind && e.confidence == Some(confidence))
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
