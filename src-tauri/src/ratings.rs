use crate::{
    catalog::{self, TrackSummary},
    state_store::{StateStore, TagOverlay},
    tag_model::LoveState,
};
use rusqlite::{Connection, OptionalExtension, named_params, params};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};

const MAX_ALBUMS: usize = 14;
const MAX_QUEUE: usize = 100;

const TRACK_RATING_SQL: &str = r#"COALESCE(normalized_rating, CASE trim(rating_raw)
  WHEN '0.5' THEN 10 WHEN '1' THEN 20 WHEN '1.0' THEN 20
  WHEN '1.5' THEN 30 WHEN '2' THEN 40 WHEN '2.0' THEN 40
  WHEN '2.5' THEN 50 WHEN '3' THEN 60 WHEN '3.0' THEN 60
  WHEN '3.5' THEN 70 WHEN '4' THEN 80 WHEN '4.0' THEN 80
  WHEN '4.5' THEN 90 WHEN '5' THEN 100 WHEN '5.0' THEN 100 END)"#;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RatingMode {
    Tracks,
    Albums,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CompletionKind {
    AlmostComplete,
    PartiallyRated,
    Unrated,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RatingBand {
    pub(crate) rating: Option<f64>,
    pub(crate) count: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompletionCounts {
    pub(crate) almost_complete: i64,
    pub(crate) partially_rated: i64,
    pub(crate) unrated: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RatingAlbum {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) original_year: Option<i64>,
    pub(crate) release_year: Option<i64>,
    pub(crate) genre: Option<String>,
    pub(crate) total_tracks: i64,
    pub(crate) rated_tracks: i64,
    pub(crate) loved_tracks: i64,
    pub(crate) duration_seconds: i64,
    pub(crate) remaining_tracks: i64,
    pub(crate) effective_rating: Option<f64>,
    pub(crate) provisional_rating: Option<f64>,
    pub(crate) album_score: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RatingAlbumPage {
    pub(crate) kind: CompletionKind,
    pub(crate) total: i64,
    pub(crate) albums: Vec<RatingAlbum>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RatingsOverview {
    pub(crate) track_bands: Vec<RatingBand>,
    pub(crate) album_bands: Vec<RatingBand>,
    pub(crate) completion: CompletionCounts,
    pub(crate) rated_albums: i64,
    pub(crate) five_star_albums: Vec<RatingAlbum>,
    pub(crate) initial_page: RatingAlbumPage,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RatingCollectionRequest {
    pub(crate) mode: RatingMode,
    pub(crate) rating: Option<f64>,
    pub(crate) limit: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RatingAlbumQueueRequest {
    pub(crate) album_id: String,
    pub(crate) unrated_only: bool,
    pub(crate) limit: usize,
}

#[derive(Clone, Debug)]
struct AlbumSnapshot {
    album: RatingAlbum,
    completion: Option<CompletionKind>,
    rating_band: Option<i64>,
}

#[derive(Clone, Debug)]
struct AlbumComparison {
    base_completion: Option<CompletionKind>,
    base_rating_band: Option<i64>,
    live: AlbumSnapshot,
}

fn rating_points(value: Option<f64>) -> Option<i64> {
    value.map(|rating| (rating.clamp(0.0, 5.0) * 20.0).round() as i64)
}

fn rated_bucket(points: Option<i64>) -> Option<i64> {
    points.filter(|value| *value > 0)
}

fn album_band(points: Option<i64>) -> Option<i64> {
    rated_bucket(points).map(|value| ((value as f64 / 10.0).round() as i64 * 10).clamp(10, 100))
}

fn completion_kind(total: i64, rated: i64) -> Option<CompletionKind> {
    let remaining = (total - rated).max(0);
    if rated == 0 {
        Some(CompletionKind::Unrated)
    } else if remaining == 0 {
        None
    } else if remaining <= 3 {
        Some(CompletionKind::AlmostComplete)
    } else {
        Some(CompletionKind::PartiallyRated)
    }
}

fn empty_band_counts() -> BTreeMap<Option<i64>, i64> {
    let mut counts = BTreeMap::from([(None, 0)]);
    for points in (10..=100).step_by(10) {
        counts.insert(Some(points), 0);
    }
    counts
}

fn bands_from_counts(counts: BTreeMap<Option<i64>, i64>) -> Vec<RatingBand> {
    let mut bands = Vec::with_capacity(11);
    bands.push(RatingBand {
        rating: None,
        count: counts.get(&None).copied().unwrap_or_default().max(0),
    });
    for points in (10..=100).step_by(10) {
        bands.push(RatingBand {
            rating: Some(points as f64 / 20.0),
            count: counts
                .get(&Some(points))
                .copied()
                .unwrap_or_default()
                .max(0),
        });
    }
    bands
}

fn query_track_bands(
    connection: &Connection,
    overlays: &[TagOverlay],
) -> Result<Vec<RatingBand>, String> {
    let sql = format!(
        "SELECT {TRACK_RATING_SQL} AS rating_value, COUNT(*) FROM tracks GROUP BY rating_value"
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("Could not prepare the track-rating constellation: {error}"))?;
    let mut counts = empty_band_counts();
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|error| format!("Could not read the track-rating constellation: {error}"))?;
    for row in rows {
        let (points, count) = row
            .map_err(|error| format!("Could not decode the track-rating constellation: {error}"))?;
        *counts.entry(rated_bucket(points)).or_default() += count;
    }
    for overlay in overlays {
        let before = rated_bucket(rating_points(overlay.catalog_values.rating));
        let after = rated_bucket(rating_points(overlay.values.rating));
        if before == after {
            continue;
        }
        *counts.entry(before).or_default() -= 1;
        *counts.entry(after).or_default() += 1;
    }
    Ok(bands_from_counts(counts))
}

fn query_album_bands(connection: &Connection) -> Result<BTreeMap<Option<i64>, i64>, String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT CASE WHEN effective_album_rating IS NULL OR effective_album_rating <= 0 THEN NULL
                        ELSE CAST(ROUND(effective_album_rating / 10.0) AS INTEGER) * 10 END,
                   COUNT(*)
            FROM albums
            GROUP BY 1
            "#,
        )
        .map_err(|error| format!("Could not prepare the album-rating constellation: {error}"))?;
    let mut counts = empty_band_counts();
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|error| format!("Could not read the album-rating constellation: {error}"))?;
    for row in rows {
        let (points, count) = row
            .map_err(|error| format!("Could not decode the album-rating constellation: {error}"))?;
        counts.insert(points, count);
    }
    Ok(counts)
}

fn query_completion_counts(connection: &Connection) -> Result<CompletionCounts, String> {
    connection
        .query_row(
            r#"
            SELECT SUM(CASE WHEN rated_tracks > 0 AND total_tracks - rated_tracks BETWEEN 1 AND 3 THEN 1 ELSE 0 END),
                   SUM(CASE WHEN rated_tracks > 0 AND total_tracks - rated_tracks > 3 THEN 1 ELSE 0 END),
                   SUM(CASE WHEN rated_tracks = 0 THEN 1 ELSE 0 END)
            FROM albums
            "#,
            [],
            |row| {
                Ok(CompletionCounts {
                    almost_complete: row.get(0)?,
                    partially_rated: row.get(1)?,
                    unrated: row.get(2)?,
                })
            },
        )
        .map_err(|error| format!("Could not read album-rating progress: {error}"))
}

fn count_for_kind(counts: &CompletionCounts, kind: CompletionKind) -> i64 {
    match kind {
        CompletionKind::AlmostComplete => counts.almost_complete,
        CompletionKind::PartiallyRated => counts.partially_rated,
        CompletionKind::Unrated => counts.unrated,
    }
}

fn adjust_completion(counts: &mut CompletionCounts, kind: CompletionKind, delta: i64) {
    let target = match kind {
        CompletionKind::AlmostComplete => &mut counts.almost_complete,
        CompletionKind::PartiallyRated => &mut counts.partially_rated,
        CompletionKind::Unrated => &mut counts.unrated,
    };
    *target = (*target + delta).max(0);
}

fn query_album_snapshot(
    connection: &Connection,
    album_id: &str,
    overlays: &HashMap<String, TagOverlay>,
) -> Result<AlbumSnapshot, String> {
    let metadata = connection
        .query_row(
            r#"
            SELECT COALESCE(NULLIF(TRIM(album), ''), 'Unknown Album'),
                   COALESCE(NULLIF(TRIM(album_artist_display), ''), 'Unknown Artist'),
                   year, release_year, canonical_genre, total_tracks, total_seconds, album_rating
            FROM albums WHERE id = ?1
            "#,
            [album_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                ))
            },
        )
        .map_err(|error| format!("Could not read this rating album: {error}"))?;

    let sql = format!(
        r#"
        SELECT {TRACK_RATING_SQL}, love, COALESCE(time_seconds, 0), file_path, filename
        FROM tracks WHERE album_id = ?1
        "#
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("Could not prepare this album's rating calculation: {error}"))?;
    let mut rows = statement
        .query([album_id])
        .map_err(|error| format!("Could not read this album's rating calculation: {error}"))?;
    let mut rated_tracks = 0_i64;
    let mut rating_sum = 0_i64;
    let mut loved_tracks = 0_i64;
    let mut five_star_seconds = 0_i64;
    while let Some(row) = rows
        .next()
        .map_err(|error| format!("Could not decode this album's rating calculation: {error}"))?
    {
        let catalog_rating = row
            .get::<_, Option<i64>>(0)
            .map_err(|error| error.to_string())?;
        let catalog_love = LoveState::from_catalog(
            row.get::<_, Option<String>>(1)
                .map_err(|error| error.to_string())?
                .as_deref(),
        );
        let duration = row
            .get::<_, i64>(2)
            .map_err(|error| error.to_string())?
            .max(0);
        let directory = row.get::<_, String>(3).map_err(|error| error.to_string())?;
        let filename = row.get::<_, String>(4).map_err(|error| error.to_string())?;
        let key = catalog::normalize_track_key(&directory, &filename);
        let (rating, love) = overlays
            .get(&key)
            .map(|overlay| {
                (
                    rating_points(overlay.values.rating),
                    overlay.values.love_state,
                )
            })
            .unwrap_or((catalog_rating, catalog_love));
        let rating = rated_bucket(rating);
        if let Some(points) = rating {
            rated_tracks += 1;
            rating_sum += points;
            if points == 100 {
                five_star_seconds += duration;
            }
        }
        if love == LoveState::Loved {
            loved_tracks += 1;
        }
    }

    let (
        title,
        artist,
        original_year,
        release_year,
        genre,
        total_tracks,
        total_seconds,
        explicit_rating,
    ) = metadata;
    let provisional_points = (rated_tracks > 0).then_some(rating_sum as f64 / rated_tracks as f64);
    let calculated_rating = (total_tracks > 0 && rated_tracks == total_tracks)
        .then_some((rating_sum as f64 / rated_tracks as f64).round() as i64);
    let effective_points = rated_bucket(explicit_rating).or(calculated_rating);
    let ratio = if total_seconds > 0 {
        five_star_seconds as f64 / total_seconds as f64
    } else {
        0.0
    };
    let score = effective_points.map(|rating| {
        ((rating as f64 * 0.5) + (ratio * 100.0) + (five_star_seconds as f64 / 60.0 * 0.3)) / 10.0
            + loved_tracks as f64 * 100.0
    });
    let completion = completion_kind(total_tracks, rated_tracks);
    Ok(AlbumSnapshot {
        rating_band: album_band(effective_points),
        completion,
        album: RatingAlbum {
            id: album_id.to_owned(),
            title,
            artist,
            original_year,
            release_year,
            genre,
            total_tracks,
            rated_tracks,
            loved_tracks,
            duration_seconds: total_seconds,
            remaining_tracks: (total_tracks - rated_tracks).max(0),
            effective_rating: effective_points.map(|rating| rating as f64 / 20.0),
            provisional_rating: provisional_points.map(|rating| rating / 20.0),
            album_score: score,
        },
    })
}

pub(crate) fn live_album_from_connection(
    connection: &Connection,
    album_id: &str,
    store: &StateStore,
) -> Result<RatingAlbum, String> {
    let overlays = store
        .all_overlays()?
        .into_iter()
        .map(|overlay| (overlay.track_key.clone(), overlay))
        .collect::<HashMap<_, _>>();
    query_album_snapshot(connection, album_id, &overlays).map(|snapshot| snapshot.album)
}

fn affected_album_comparisons(
    connection: &Connection,
    overlays: &[TagOverlay],
) -> Result<HashMap<String, AlbumComparison>, String> {
    if overlays.is_empty() {
        return Ok(HashMap::new());
    }
    let overlay_map = overlays
        .iter()
        .cloned()
        .map(|overlay| (overlay.track_key.clone(), overlay))
        .collect::<HashMap<_, _>>();
    let mut album_lookup = connection
        .prepare_cached("SELECT album_id FROM tracks WHERE file_path = ?1 AND filename = ?2")
        .map_err(|error| format!("Could not prepare the rating-overlay lookup: {error}"))?;
    let mut album_ids = HashSet::new();
    for overlay in overlays {
        if let Some(album_id) = album_lookup
            .query_row(params![overlay.directory, overlay.filename], |row| {
                row.get::<_, Option<String>>(0)
            })
            .optional()
            .map_err(|error| format!("Could not resolve a rating overlay: {error}"))?
            .flatten()
        {
            album_ids.insert(album_id);
        }
    }
    let mut comparisons = HashMap::new();
    for album_id in album_ids {
        let (total, rated, effective) = connection
            .query_row(
                "SELECT total_tracks, rated_tracks, effective_album_rating FROM albums WHERE id = ?1",
                [&album_id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, Option<i64>>(2)?)),
            )
            .map_err(|error| format!("Could not read an overlaid album: {error}"))?;
        let live = query_album_snapshot(connection, &album_id, &overlay_map)?;
        comparisons.insert(
            album_id,
            AlbumComparison {
                base_completion: completion_kind(total, rated),
                base_rating_band: album_band(effective),
                live,
            },
        );
    }
    Ok(comparisons)
}

fn apply_album_deltas(
    completion: &mut CompletionCounts,
    album_counts: &mut BTreeMap<Option<i64>, i64>,
    comparisons: &HashMap<String, AlbumComparison>,
) {
    for comparison in comparisons.values() {
        if comparison.base_completion != comparison.live.completion {
            if let Some(kind) = comparison.base_completion {
                adjust_completion(completion, kind, -1);
            }
            if let Some(kind) = comparison.live.completion {
                adjust_completion(completion, kind, 1);
            }
        }
        if comparison.base_rating_band != comparison.live.rating_band {
            *album_counts.entry(comparison.base_rating_band).or_default() -= 1;
            *album_counts.entry(comparison.live.rating_band).or_default() += 1;
        }
    }
}

fn album_matches(kind: CompletionKind, album: &RatingAlbum) -> bool {
    completion_kind(album.total_tracks, album.rated_tracks) == Some(kind)
}

fn candidate_predicate(kind: CompletionKind) -> &'static str {
    match kind {
        CompletionKind::AlmostComplete => {
            "a.rated_tracks > 0 AND a.total_tracks - a.rated_tracks BETWEEN 1 AND 3"
        }
        CompletionKind::PartiallyRated => {
            "a.rated_tracks > 0 AND a.total_tracks - a.rated_tracks > 3"
        }
        CompletionKind::Unrated => "a.rated_tracks = 0 AND a.total_tracks BETWEEN 1 AND 30",
    }
}

fn query_candidate_ids(
    connection: &Connection,
    kind: CompletionKind,
    limit: usize,
) -> Result<Vec<String>, String> {
    let order = match kind {
        CompletionKind::AlmostComplete => {
            "a.total_tracks - a.rated_tracks, a.loved_tracks DESC, COALESCE(a.album_score, -1) DESC, a.id"
        }
        CompletionKind::PartiallyRated => {
            "a.rating_completeness DESC, a.loved_tracks DESC, COALESCE(a.album_score, -1) DESC, a.id"
        }
        CompletionKind::Unrated => "COALESCE(a.year, -1) DESC, a.total_tracks, a.id",
    };
    let sql = format!(
        "SELECT a.id FROM albums AS a WHERE {} ORDER BY {order} LIMIT ?1",
        candidate_predicate(kind)
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("Could not prepare the album-completion shelf: {error}"))?;
    statement
        .query_map([limit as i64], |row| row.get::<_, String>(0))
        .map_err(|error| format!("Could not read the album-completion shelf: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode the album-completion shelf: {error}"))
}

fn sort_albums(kind: CompletionKind, albums: &mut [RatingAlbum]) {
    albums.sort_by(|left, right| {
        let score_order = right
            .album_score
            .partial_cmp(&left.album_score)
            .unwrap_or(Ordering::Equal);
        match kind {
            CompletionKind::AlmostComplete => left
                .remaining_tracks
                .cmp(&right.remaining_tracks)
                .then_with(|| right.loved_tracks.cmp(&left.loved_tracks))
                .then(score_order)
                .then_with(|| left.id.cmp(&right.id)),
            CompletionKind::PartiallyRated => {
                let left_ratio = left.rated_tracks as f64 / left.total_tracks.max(1) as f64;
                let right_ratio = right.rated_tracks as f64 / right.total_tracks.max(1) as f64;
                right_ratio
                    .partial_cmp(&left_ratio)
                    .unwrap_or(Ordering::Equal)
                    .then_with(|| right.loved_tracks.cmp(&left.loved_tracks))
                    .then(score_order)
                    .then_with(|| left.id.cmp(&right.id))
            }
            CompletionKind::Unrated => right
                .original_year
                .cmp(&left.original_year)
                .then_with(|| left.total_tracks.cmp(&right.total_tracks))
                .then_with(|| left.id.cmp(&right.id)),
        }
    });
}

fn query_album_page(
    connection: &Connection,
    kind: CompletionKind,
    completion: &CompletionCounts,
    overlays: &[TagOverlay],
    comparisons: &HashMap<String, AlbumComparison>,
) -> Result<RatingAlbumPage, String> {
    let overlay_map = overlays
        .iter()
        .cloned()
        .map(|overlay| (overlay.track_key.clone(), overlay))
        .collect::<HashMap<_, _>>();
    let mut ids = query_candidate_ids(connection, kind, MAX_ALBUMS * 3)?;
    ids.extend(
        comparisons
            .iter()
            .filter(|(_, comparison)| comparison.live.completion == Some(kind))
            .map(|(id, _)| id.clone()),
    );
    let mut seen = HashSet::new();
    let mut albums = Vec::new();
    for id in ids {
        if !seen.insert(id.clone()) {
            continue;
        }
        let album = comparisons
            .get(&id)
            .map(|comparison| comparison.live.album.clone())
            .map(Ok)
            .unwrap_or_else(|| {
                query_album_snapshot(connection, &id, &overlay_map).map(|value| value.album)
            })?;
        if album_matches(kind, &album) {
            albums.push(album);
        }
    }
    sort_albums(kind, &mut albums);
    albums.truncate(MAX_ALBUMS);
    Ok(RatingAlbumPage {
        kind,
        total: count_for_kind(completion, kind),
        albums,
    })
}

fn query_five_star_album_ids(connection: &Connection) -> Result<Vec<String>, String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT t.album_id
            FROM tracks AS t
            JOIN albums AS a ON a.id = t.album_id
            WHERE t.normalized_rating = 100 AND t.album_id IS NOT NULL
            GROUP BY t.album_id
            ORDER BY COUNT(*) DESC, a.loved_tracks DESC, COALESCE(a.album_score, -1) DESC, t.album_id
            LIMIT 8
            "#,
        )
        .map_err(|error| format!("Could not prepare the 5 Star Collection: {error}"))?;
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| format!("Could not read the 5 Star Collection: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode the 5 Star Collection: {error}"))
}

fn query_overview(connection: &Connection, store: &StateStore) -> Result<RatingsOverview, String> {
    let overlays = store.all_overlays()?;
    let comparisons = affected_album_comparisons(connection, &overlays)?;
    let track_bands = query_track_bands(connection, &overlays)?;
    let mut album_counts = query_album_bands(connection)?;
    let mut completion = query_completion_counts(connection)?;
    apply_album_deltas(&mut completion, &mut album_counts, &comparisons);
    let overlay_map = overlays
        .iter()
        .cloned()
        .map(|overlay| (overlay.track_key.clone(), overlay))
        .collect::<HashMap<_, _>>();
    let five_star_albums = query_five_star_album_ids(connection)?
        .into_iter()
        .map(|id| {
            query_album_snapshot(connection, &id, &overlay_map).map(|snapshot| snapshot.album)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let initial_page = query_album_page(
        connection,
        CompletionKind::AlmostComplete,
        &completion,
        &overlays,
        &comparisons,
    )?;
    let rated_albums = album_counts
        .iter()
        .filter(|(key, _)| key.is_some())
        .map(|(_, count)| *count)
        .sum();
    Ok(RatingsOverview {
        track_bands,
        album_bands: bands_from_counts(album_counts),
        completion,
        rated_albums,
        five_star_albums,
        initial_page,
    })
}

pub(crate) fn load_ratings_overview(store: &StateStore) -> Result<RatingsOverview, String> {
    let path = catalog::default_catalog_path()?;
    let connection = catalog::open_catalog(&path)?;
    query_overview(&connection, store)
}

pub(crate) fn load_rating_album_page(
    kind: CompletionKind,
    store: &StateStore,
) -> Result<RatingAlbumPage, String> {
    let path = catalog::default_catalog_path()?;
    let connection = catalog::open_catalog(&path)?;
    let overlays = store.all_overlays()?;
    let comparisons = affected_album_comparisons(&connection, &overlays)?;
    let mut completion = query_completion_counts(&connection)?;
    let mut album_counts = query_album_bands(&connection)?;
    apply_album_deltas(&mut completion, &mut album_counts, &comparisons);
    query_album_page(&connection, kind, &completion, &overlays, &comparisons)
}

fn validate_rating_request(request: &RatingCollectionRequest) -> Result<Option<i64>, String> {
    if request.limit == 0 || request.limit > MAX_QUEUE {
        return Err("A rating collection must request between 1 and 100 tracks.".to_owned());
    }
    let points = rating_points(request.rating);
    if request
        .rating
        .is_some_and(|rating| !(0.5..=5.0).contains(&rating))
    {
        return Err("The selected rating is invalid.".to_owned());
    }
    Ok(points)
}

pub(crate) fn load_rating_collection(
    request: RatingCollectionRequest,
    store: &StateStore,
) -> Result<Vec<TrackSummary>, String> {
    let points = validate_rating_request(&request)?;
    let path = catalog::default_catalog_path()?;
    let connection = catalog::open_catalog(&path)?;
    let predicate = match request.mode {
        RatingMode::Tracks => format!(
            "((:rating_points IS NULL AND ({TRACK_RATING_SQL} IS NULL OR {TRACK_RATING_SQL} <= 0)) OR {TRACK_RATING_SQL} = :rating_points)"
        ),
        RatingMode::Albums => {
            "((:rating_points IS NULL AND (a.effective_album_rating IS NULL OR a.effective_album_rating <= 0)) OR (:rating_points IS NOT NULL AND CAST(ROUND(a.effective_album_rating / 10.0) AS INTEGER) * 10 = :rating_points))".to_owned()
        }
    };
    let sql = format!(
        r#"
        WITH page AS MATERIALIZED (
          SELECT t.id, t.title, t.album_artist_display, t.album, t.release_year,
                 {TRACK_RATING_SQL} AS rating_value, t.love, t.time_seconds,
                 t.canonical_genre, t.album_id, t.file_path, t.filename, t.import_run_id
          FROM tracks AS t
          LEFT JOIN albums AS a ON a.id = t.album_id
          WHERE {predicate}
          ORDER BY (t.love = 'L') DESC, rating_value DESC, t.album_id, t.disc_number, t.track_number, t.id
          LIMIT :candidate_limit
        )
        SELECT p.id, p.title, p.album_artist_display, p.album, p.release_year,
               p.rating_value, p.love, p.time_seconds, p.canonical_genre,
               l.play_count, p.album_id, p.file_path, p.filename, p.import_run_id
        FROM page AS p
        LEFT JOIN lastfm_track_popularity AS l
          ON l.artist_key = lower(trim(p.album_artist_display))
         AND l.track_key = lower(trim(p.title))
        ORDER BY (p.love = 'L') DESC, p.rating_value DESC, p.album_id, p.id
        "#
    );
    let mut tracks = catalog::query_tracks(
        &connection,
        &sql,
        named_params! {
            ":rating_points": points,
            ":candidate_limit": (request.limit * 3) as i64,
        },
        "rating collection",
        Some(store),
    )?;
    if request.mode == RatingMode::Tracks {
        tracks.retain(|track| rated_bucket(rating_points(track.rating)) == points);
    }
    tracks.truncate(request.limit);
    Ok(tracks)
}

pub(crate) fn load_rating_album_queue(
    request: RatingAlbumQueueRequest,
    store: &StateStore,
) -> Result<Vec<TrackSummary>, String> {
    if request.album_id.trim().is_empty() || request.album_id.chars().count() > 256 {
        return Err("Album identity is invalid.".to_owned());
    }
    if request.limit == 0 || request.limit > MAX_QUEUE {
        return Err("Album playback must request between 1 and 100 tracks.".to_owned());
    }
    let path = catalog::default_catalog_path()?;
    let connection = catalog::open_catalog(&path)?;
    let sql = format!(
        r#"
        SELECT t.id, t.title, t.album_artist_display, t.album, t.release_year,
               {TRACK_RATING_SQL}, t.love, t.time_seconds, t.canonical_genre,
               l.play_count, t.album_id, t.file_path, t.filename, t.import_run_id
        FROM tracks AS t
        LEFT JOIN lastfm_track_popularity AS l
          ON l.artist_key = lower(trim(t.album_artist_display))
         AND l.track_key = lower(trim(t.title))
        WHERE t.album_id = :album_id
        ORDER BY t.disc_number, t.track_number, t.id
        LIMIT 200
        "#
    );
    let mut tracks = catalog::query_tracks(
        &connection,
        &sql,
        named_params! { ":album_id": request.album_id },
        "rating album queue",
        Some(store),
    )?;
    if request.unrated_only {
        tracks.retain(|track| track.rating.is_none());
    }
    tracks.truncate(request.limit);
    Ok(tracks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    fn fixture() -> (Connection, StateStore, std::path::PathBuf) {
        let connection = Connection::open_in_memory().expect("open rating fixture");
        connection
            .execute_batch(
                r#"
                CREATE TABLE albums (
                  id TEXT PRIMARY KEY, album TEXT, album_artist_display TEXT, year INTEGER,
                  release_year INTEGER, canonical_genre TEXT, total_tracks INTEGER NOT NULL,
                  rated_tracks INTEGER NOT NULL, rating_completeness REAL NOT NULL,
                  loved_tracks INTEGER NOT NULL, total_seconds INTEGER NOT NULL,
                  album_rating INTEGER, calculated_album_rating INTEGER,
                  effective_album_rating INTEGER, album_score REAL
                );
                CREATE TABLE tracks (
                  id INTEGER PRIMARY KEY, album_id TEXT, title TEXT, album_artist_display TEXT,
                  album TEXT, release_year INTEGER, normalized_rating INTEGER, rating_raw TEXT,
                  love TEXT, time_seconds INTEGER, canonical_genre TEXT, file_path TEXT,
                  filename TEXT, import_run_id INTEGER NOT NULL, disc_number INTEGER, track_number INTEGER
                );
                CREATE TABLE lastfm_track_popularity (artist_key TEXT, track_key TEXT, play_count INTEGER);
                INSERT INTO albums VALUES
                  ('almost', 'Almost', 'Artist', 2000, 2000, 'Rock', 4, 3, .75, 1, 400, NULL, NULL, NULL, NULL),
                  ('complete', 'Complete', 'Artist', 2001, 2001, 'Rock', 2, 2, 1, 1, 240, NULL, 90, 90, 110);
                INSERT INTO tracks VALUES
                  (1, 'almost', 'One', 'Artist', 'Almost', 2000, 100, NULL, 'L', 100, 'Rock', 'D:\\Music', 'one.mp3', 1, 1, 1),
                  (2, 'almost', 'Two', 'Artist', 'Almost', 2000, 80, NULL, NULL, 100, 'Rock', 'D:\\Music', 'two.mp3', 1, 1, 2),
                  (3, 'almost', 'Three', 'Artist', 'Almost', 2000, 60, NULL, NULL, 100, 'Rock', 'D:\\Music', 'three.mp3', 1, 1, 3),
                  (4, 'almost', 'Four', 'Artist', 'Almost', 2000, NULL, NULL, NULL, 100, 'Rock', 'D:\\Music', 'four.mp3', 1, 1, 4),
                  (5, 'complete', 'Five', 'Artist', 'Complete', 2001, 100, NULL, 'L', 120, 'Rock', 'D:\\Music', 'five.mp3', 1, 1, 1),
                  (6, 'complete', 'Six', 'Artist', 'Complete', 2001, 80, NULL, NULL, 120, 'Rock', 'D:\\Music', 'six.mp3', 1, 1, 2);
                "#,
            )
            .expect("seed rating fixture");
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("aurora-ratings-{unique}.sqlite3"));
        let store = StateStore::new(path.clone()).expect("state store");
        (connection, store, path)
    }

    #[test]
    fn album_score_matches_music_library_formula() {
        let (connection, store, path) = fixture();
        let snapshot =
            query_album_snapshot(&connection, "complete", &HashMap::new()).expect("album snapshot");
        assert_eq!(snapshot.album.effective_rating, Some(4.5));
        let expected = ((90.0 * 0.5) + (0.5 * 100.0) + (2.0 * 0.3)) / 10.0 + 100.0;
        assert!((snapshot.album.album_score.expect("score") - expected).abs() < 0.001);
        drop(store);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn completion_states_are_mutually_exclusive() {
        assert_eq!(completion_kind(10, 9), Some(CompletionKind::AlmostComplete));
        assert_eq!(completion_kind(10, 4), Some(CompletionKind::PartiallyRated));
        assert_eq!(completion_kind(10, 0), Some(CompletionKind::Unrated));
        assert_eq!(completion_kind(10, 10), None);
    }

    #[test]
    fn unrated_candidates_use_year_instead_of_release_year() {
        let connection = Connection::open_in_memory().expect("unrated ordering fixture");
        connection
            .execute_batch(
                r#"
                CREATE TABLE albums (
                  id TEXT PRIMARY KEY, total_tracks INTEGER NOT NULL, rated_tracks INTEGER NOT NULL,
                  year INTEGER, release_year INTEGER
                );
                INSERT INTO albums VALUES
                  ('new-release', 10, 0, 1985, 2025),
                  ('new-year', 10, 0, 2000, 1990);
                "#,
            )
            .expect("seed unrated ordering fixture");

        let ids = query_candidate_ids(&connection, CompletionKind::Unrated, 10)
            .expect("unrated candidates");
        assert_eq!(ids, vec!["new-year", "new-release"]);
    }

    #[test]
    fn fixture_overview_keeps_track_and_album_ratings_distinct() {
        let (connection, store, path) = fixture();
        let overview = query_overview(&connection, &store).expect("ratings overview");
        assert_eq!(overview.completion.almost_complete, 1);
        assert_eq!(overview.rated_albums, 1);
        assert_eq!(overview.track_bands.last().expect("five stars").count, 2);
        drop(store);
        let _ = fs::remove_file(path);
    }

    #[test]
    #[ignore = "reads the user's million-track Music Library catalog"]
    fn live_catalog_ratings_are_exact_and_bounded() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("aurora-ratings-live-{unique}.sqlite3"));
        let store = StateStore::new(path.clone()).expect("temporary state store");
        let started = Instant::now();
        let overview = load_ratings_overview(&store).expect("live ratings overview");
        let elapsed = started.elapsed();

        assert_eq!(
            overview
                .track_bands
                .iter()
                .map(|band| band.count)
                .sum::<i64>(),
            1_096_288
        );
        assert_eq!(
            overview
                .album_bands
                .iter()
                .map(|band| band.count)
                .sum::<i64>(),
            72_012
        );
        assert_eq!(overview.rated_albums, 12_434);
        assert_eq!(overview.completion.almost_complete, 678);
        assert_eq!(overview.completion.partially_rated, 5_723);
        assert_eq!(overview.completion.unrated, 59_578);
        assert!(overview.initial_page.albums.len() <= MAX_ALBUMS);
        assert!(overview.five_star_albums.len() <= 8);
        assert!(
            elapsed < Duration::from_secs(20),
            "ratings overview took {elapsed:?}"
        );

        drop(store);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("sqlite3-wal"));
        let _ = fs::remove_file(path.with_extension("sqlite3-shm"));
        eprintln!("live ratings overview: {elapsed:?}");
    }
}
