use crate::{
    catalog::{self, TrackSummary},
    history::{self, GenreHistoryInsight, HistoryStore},
    state_store::StateStore,
};
use rusqlite::{Connection, Row, named_params, params};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const MAX_GENRES: usize = 1_000;
const MAX_QUEUE_BATCH: usize = 100;
const MAX_QUEUE_EXCLUSIONS: usize = 200;
const MAX_GENRE_TEXT: usize = 256;

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GenreSummary {
    pub(crate) name: String,
    pub(crate) track_count: i64,
    pub(crate) album_count: i64,
    pub(crate) artist_count: i64,
    pub(crate) rated_tracks: i64,
    pub(crate) loved_tracks: i64,
    pub(crate) duration_seconds: i64,
    pub(crate) average_rating: Option<f64>,
    pub(crate) first_year: Option<i64>,
    pub(crate) last_year: Option<i64>,
    pub(crate) representative_album_id: Option<String>,
    pub(crate) sessions: i64,
    pub(crate) plays: i64,
    pub(crate) listened_seconds: f64,
    pub(crate) last_listened_at_ms: Option<i64>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GenreDecade {
    pub(crate) decade: i64,
    pub(crate) track_count: i64,
    pub(crate) album_count: i64,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GenreAlbum {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) year: Option<i64>,
    pub(crate) publisher: Option<String>,
    pub(crate) total_tracks: i64,
    pub(crate) rated_tracks: i64,
    pub(crate) loved_tracks: i64,
    pub(crate) duration_seconds: i64,
    pub(crate) rating: Option<f64>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GenreArtist {
    pub(crate) name: String,
    pub(crate) track_count: i64,
    pub(crate) album_count: i64,
    pub(crate) loved_tracks: i64,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RelatedGenre {
    pub(crate) name: String,
    pub(crate) shared_artists: i64,
    pub(crate) shared_albums: i64,
    pub(crate) shared_tracks: i64,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GenreDetail {
    pub(crate) summary: GenreSummary,
    pub(crate) decades: Vec<GenreDecade>,
    pub(crate) albums: Vec<GenreAlbum>,
    pub(crate) artists: Vec<GenreArtist>,
    pub(crate) related_genres: Vec<RelatedGenre>,
    pub(crate) highlights: Vec<TrackSummary>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum GenreQueueMode {
    Radio,
    Shuffle,
    Loved,
    HighestRated,
    Rediscover,
    Unrated,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GenreQueueRequest {
    genre: String,
    mode: GenreQueueMode,
    limit: usize,
    exclude_track_keys: Vec<String>,
}

fn validate_genre(genre: &str) -> Result<&str, String> {
    let genre = genre.trim();
    if genre.is_empty() || genre.chars().count() > MAX_GENRE_TEXT {
        return Err("Genre selection is invalid.".to_owned());
    }
    Ok(genre)
}

fn history_for<'a>(
    insights: &'a HashMap<String, GenreHistoryInsight>,
    genre: &str,
) -> Option<&'a GenreHistoryInsight> {
    insights.get(&history::genre_identity(genre))
}

fn decode_genre_summary(
    row: &Row<'_>,
    history: &HashMap<String, GenreHistoryInsight>,
) -> rusqlite::Result<GenreSummary> {
    let name: String = row.get(0)?;
    let personal = history_for(history, &name).cloned().unwrap_or_default();
    Ok(GenreSummary {
        name,
        track_count: row.get(1)?,
        album_count: row.get(2)?,
        artist_count: row.get(3)?,
        rated_tracks: row.get(4)?,
        loved_tracks: row.get(5)?,
        duration_seconds: row.get(6)?,
        average_rating: row.get(7)?,
        first_year: row.get(8)?,
        last_year: row.get(9)?,
        representative_album_id: row.get(10)?,
        sessions: personal.sessions,
        plays: personal.plays,
        listened_seconds: personal.listened_seconds,
        last_listened_at_ms: personal.last_listened_at_ms,
    })
}

fn query_genre_index(
    connection: &Connection,
    history: &HashMap<String, GenreHistoryInsight>,
    store: Option<&StateStore>,
) -> Result<Vec<GenreSummary>, String> {
    let mut statement = connection
        .prepare(
            r#"
            WITH ranked AS MATERIALIZED (
              SELECT id, canonical_genre, album_artist_display, total_tracks,
                     rated_tracks, loved_tracks, total_seconds, year,
                     COALESCE(effective_album_rating, calculated_album_rating, album_rating) AS rating_value,
                     ROW_NUMBER() OVER (
                       PARTITION BY canonical_genre
                       ORDER BY (loved_tracks > 0) DESC, loved_tracks DESC,
                                COALESCE(effective_album_rating, calculated_album_rating, album_rating, -1) DESC,
                                COALESCE(album_score, -1) DESC, COALESCE(year, 0) DESC, id
                     ) AS cover_rank
              FROM albums
              WHERE NULLIF(TRIM(canonical_genre), '') IS NOT NULL
            )
            SELECT canonical_genre,
                   CAST(SUM(total_tracks) AS INTEGER), COUNT(*),
                   COUNT(DISTINCT COALESCE(NULLIF(TRIM(album_artist_display), ''), 'Unknown Artist')),
                   CAST(SUM(rated_tracks) AS INTEGER), CAST(SUM(loved_tracks) AS INTEGER),
                   CAST(SUM(total_seconds) AS INTEGER),
                   CASE WHEN SUM(CASE WHEN rating_value IS NOT NULL THEN rated_tracks ELSE 0 END) > 0
                     THEN SUM(CASE WHEN rating_value IS NOT NULL THEN rating_value * rated_tracks ELSE 0 END)
                          / SUM(CASE WHEN rating_value IS NOT NULL THEN rated_tracks ELSE 0 END) / 20.0
                   END,
                   MIN(NULLIF(year, 0)), MAX(NULLIF(year, 0)),
                   MAX(CASE WHEN cover_rank = 1 THEN id END)
            FROM ranked
            GROUP BY canonical_genre
            ORDER BY SUM(total_tracks) DESC, canonical_genre COLLATE NOCASE
            LIMIT ?1
            "#,
        )
        .map_err(|error| format!("Could not prepare the genre atlas: {error}"))?;
    let mut summaries = statement
        .query_map([MAX_GENRES as i64], |row| {
            decode_genre_summary(row, history)
        })
        .map_err(|error| format!("Could not read the genre atlas: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode the genre atlas: {error}"))?;
    apply_overlay_deltas(connection, &mut summaries, store)?;
    Ok(summaries)
}

fn query_genre_summary(
    connection: &Connection,
    genre: &str,
    history: &HashMap<String, GenreHistoryInsight>,
    store: Option<&StateStore>,
) -> Result<GenreSummary, String> {
    let mut statement = connection
        .prepare(
            r#"
            WITH ranked AS MATERIALIZED (
              SELECT id, canonical_genre, album_artist_display, total_tracks,
                     rated_tracks, loved_tracks, total_seconds, year,
                     COALESCE(effective_album_rating, calculated_album_rating, album_rating) AS rating_value,
                     ROW_NUMBER() OVER (
                       ORDER BY (loved_tracks > 0) DESC, loved_tracks DESC,
                                COALESCE(effective_album_rating, calculated_album_rating, album_rating, -1) DESC,
                                COALESCE(album_score, -1) DESC, COALESCE(year, 0) DESC, id
                     ) AS cover_rank
              FROM albums
              WHERE canonical_genre = ?1
            )
            SELECT canonical_genre,
                   CAST(SUM(total_tracks) AS INTEGER), COUNT(*),
                   COUNT(DISTINCT COALESCE(NULLIF(TRIM(album_artist_display), ''), 'Unknown Artist')),
                   CAST(SUM(rated_tracks) AS INTEGER), CAST(SUM(loved_tracks) AS INTEGER),
                   CAST(SUM(total_seconds) AS INTEGER),
                   CASE WHEN SUM(CASE WHEN rating_value IS NOT NULL THEN rated_tracks ELSE 0 END) > 0
                     THEN SUM(CASE WHEN rating_value IS NOT NULL THEN rating_value * rated_tracks ELSE 0 END)
                          / SUM(CASE WHEN rating_value IS NOT NULL THEN rated_tracks ELSE 0 END) / 20.0
                   END,
                   MIN(NULLIF(year, 0)), MAX(NULLIF(year, 0)),
                   MAX(CASE WHEN cover_rank = 1 THEN id END)
            FROM ranked
            GROUP BY canonical_genre
            "#,
        )
        .map_err(|error| format!("Could not prepare this genre summary: {error}"))?;
    let mut summaries = statement
        .query_map([genre], |row| decode_genre_summary(row, history))
        .map_err(|error| format!("Could not read this genre summary: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode this genre summary: {error}"))?;
    apply_overlay_deltas(connection, &mut summaries, store)?;
    summaries
        .pop()
        .ok_or_else(|| "That genre is no longer available in the catalog.".to_owned())
}

fn apply_overlay_deltas(
    connection: &Connection,
    summaries: &mut [GenreSummary],
    store: Option<&StateStore>,
) -> Result<(), String> {
    let Some(store) = store else {
        return Ok(());
    };
    let mut by_genre = summaries
        .iter_mut()
        .map(|summary| (summary.name.clone(), summary))
        .collect::<HashMap<_, _>>();
    let mut statement = connection
        .prepare_cached(
            "SELECT canonical_genre, import_run_id FROM tracks WHERE file_path = ?1 AND filename = ?2",
        )
        .map_err(|error| format!("Could not prepare genre tag-overlay reconciliation: {error}"))?;
    for overlay in store.all_overlays()? {
        let mut rows = statement
            .query(params![overlay.directory, overlay.filename])
            .map_err(|error| format!("Could not read a genre tag overlay: {error}"))?;
        let Some(row) = rows
            .next()
            .map_err(|error| format!("Could not decode a genre tag overlay: {error}"))?
        else {
            continue;
        };
        let genre: Option<String> = row
            .get(0)
            .map_err(|error| format!("Could not decode an overlay genre: {error}"))?;
        let import_run_id: i64 = row
            .get(1)
            .map_err(|error| format!("Could not decode an overlay import: {error}"))?;
        if import_run_id != overlay.catalog_import_run_id {
            continue;
        }
        let Some(summary) = genre.as_ref().and_then(|genre| by_genre.get_mut(genre)) else {
            continue;
        };
        let before_rated = i64::from(overlay.catalog_values.rating.is_some());
        let after_rated = i64::from(overlay.values.rating.is_some());
        let old_rated_tracks = summary.rated_tracks;
        let old_rating_sum = summary.average_rating.unwrap_or_default() * old_rated_tracks as f64;
        let next_rated_tracks = (old_rated_tracks + after_rated - before_rated).max(0);
        let next_rating_sum = old_rating_sum - overlay.catalog_values.rating.unwrap_or_default()
            + overlay.values.rating.unwrap_or_default();
        summary.rated_tracks = next_rated_tracks;
        summary.average_rating = (next_rated_tracks > 0)
            .then(|| (next_rating_sum / next_rated_tracks as f64).clamp(0.0, 5.0));
        let before_loved =
            i64::from(overlay.catalog_values.love_state == crate::tag_model::LoveState::Loved);
        let after_loved =
            i64::from(overlay.values.love_state == crate::tag_model::LoveState::Loved);
        summary.loved_tracks = (summary.loved_tracks + after_loved - before_loved).max(0);
    }
    Ok(())
}

pub(crate) fn load_genre_index(
    history: &HistoryStore,
    store: &StateStore,
) -> Result<Vec<GenreSummary>, String> {
    let path = catalog::default_catalog_path()?;
    let connection = catalog::open_catalog(&path)?;
    let history = history.genre_insights().unwrap_or_default();
    query_genre_index(&connection, &history, Some(store))
}

fn query_decades(connection: &Connection, genre: &str) -> Result<Vec<GenreDecade>, String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT (year / 10) * 10 AS decade,
                   CAST(SUM(total_tracks) AS INTEGER), COUNT(*)
            FROM albums
            WHERE canonical_genre = ?1
              AND year BETWEEN 1000 AND 9999
            GROUP BY decade
            ORDER BY decade
            "#,
        )
        .map_err(|error| format!("Could not prepare the genre timeline: {error}"))?;
    statement
        .query_map([genre], |row| {
            Ok(GenreDecade {
                decade: row.get(0)?,
                track_count: row.get(1)?,
                album_count: row.get(2)?,
            })
        })
        .map_err(|error| format!("Could not read the genre timeline: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode the genre timeline: {error}"))
}

fn query_albums(connection: &Connection, genre: &str) -> Result<Vec<GenreAlbum>, String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT id, COALESCE(NULLIF(TRIM(album), ''), 'Unknown Album'),
                   COALESCE(NULLIF(TRIM(album_artist_display), ''), 'Unknown Artist'),
                   year, publisher, total_tracks, rated_tracks, loved_tracks, total_seconds,
                   COALESCE(effective_album_rating, calculated_album_rating, album_rating) / 20.0
            FROM albums
            WHERE canonical_genre = ?1
            ORDER BY (loved_tracks > 0) DESC, loved_tracks DESC,
                     COALESCE(effective_album_rating, calculated_album_rating, album_rating, -1) DESC,
                     COALESCE(album_score, -1) DESC, COALESCE(year, 0) DESC, album COLLATE NOCASE
            LIMIT 12
            "#,
        )
        .map_err(|error| format!("Could not prepare the genre albums: {error}"))?;
    statement
        .query_map([genre], |row| {
            Ok(GenreAlbum {
                id: row.get(0)?,
                title: row.get(1)?,
                artist: row.get(2)?,
                year: row.get(3)?,
                publisher: row.get(4)?,
                total_tracks: row.get(5)?,
                rated_tracks: row.get(6)?,
                loved_tracks: row.get(7)?,
                duration_seconds: row.get(8)?,
                rating: row.get(9)?,
            })
        })
        .map_err(|error| format!("Could not read the genre albums: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode the genre albums: {error}"))
}

fn query_artists(connection: &Connection, genre: &str) -> Result<Vec<GenreArtist>, String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT COALESCE(NULLIF(TRIM(album_artist_display), ''), 'Unknown Artist'),
                   CAST(SUM(total_tracks) AS INTEGER), COUNT(*), CAST(SUM(loved_tracks) AS INTEGER)
            FROM albums
            WHERE canonical_genre = ?1
              AND COALESCE(NULLIF(TRIM(album_artist_display), ''), 'Unknown Artist') <> 'Various Artists'
            GROUP BY COALESCE(NULLIF(TRIM(album_artist_display), ''), 'Unknown Artist')
            ORDER BY SUM(total_tracks) DESC, SUM(loved_tracks) DESC, album_artist_display COLLATE NOCASE
            LIMIT 10
            "#,
        )
        .map_err(|error| format!("Could not prepare the genre artists: {error}"))?;
    statement
        .query_map([genre], |row| {
            Ok(GenreArtist {
                name: row.get(0)?,
                track_count: row.get(1)?,
                album_count: row.get(2)?,
                loved_tracks: row.get(3)?,
            })
        })
        .map_err(|error| format!("Could not read the genre artists: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode the genre artists: {error}"))
}

fn query_related(connection: &Connection, genre: &str) -> Result<Vec<RelatedGenre>, String> {
    let mut statement = connection
        .prepare(
            r#"
            WITH selected_artists AS MATERIALIZED (
              SELECT DISTINCT album_artist_display
              FROM albums
              WHERE canonical_genre = ?1
                AND NULLIF(TRIM(album_artist_display), '') IS NOT NULL
                AND album_artist_display <> 'Various Artists'
            )
            SELECT a.canonical_genre, COUNT(DISTINCT a.album_artist_display),
                   COUNT(*), CAST(SUM(a.total_tracks) AS INTEGER)
            FROM albums AS a
            JOIN selected_artists AS selected
              ON selected.album_artist_display = a.album_artist_display
            WHERE a.canonical_genre <> ?1
              AND NULLIF(TRIM(a.canonical_genre), '') IS NOT NULL
            GROUP BY a.canonical_genre
            ORDER BY COUNT(DISTINCT a.album_artist_display) DESC,
                     SUM(a.total_tracks) DESC, a.canonical_genre COLLATE NOCASE
            LIMIT 8
            "#,
        )
        .map_err(|error| format!("Could not prepare connected genres: {error}"))?;
    statement
        .query_map([genre], |row| {
            Ok(RelatedGenre {
                name: row.get(0)?,
                shared_artists: row.get(1)?,
                shared_albums: row.get(2)?,
                shared_tracks: row.get(3)?,
            })
        })
        .map_err(|error| format!("Could not read connected genres: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode connected genres: {error}"))
}

fn query_highlights(
    connection: &Connection,
    genre: &str,
    fallback_album_id: Option<&str>,
    store: Option<&StateStore>,
) -> Result<Vec<TrackSummary>, String> {
    let highlights = catalog::query_tracks(
        connection,
        r#"
        WITH page AS MATERIALIZED (
          SELECT id, title, album_artist_display, album, release_year,
                 COALESCE(normalized_rating, CASE trim(rating_raw)
                   WHEN '0.5' THEN 10 WHEN '1' THEN 20 WHEN '1.0' THEN 20
                   WHEN '1.5' THEN 30 WHEN '2' THEN 40 WHEN '2.0' THEN 40
                   WHEN '2.5' THEN 50 WHEN '3' THEN 60 WHEN '3.0' THEN 60
                   WHEN '3.5' THEN 70 WHEN '4' THEN 80 WHEN '4.0' THEN 80
                   WHEN '4.5' THEN 90 WHEN '5' THEN 100 WHEN '5.0' THEN 100 END) AS rating_value,
                 love, time_seconds, canonical_genre, album_id, file_path, filename, import_run_id,
                 year AS original_year, publisher
          FROM tracks
          WHERE canonical_genre = :genre
            AND (normalized_rating IS NOT NULL OR NULLIF(TRIM(rating_raw), '') IS NOT NULL OR love = 'L')
          ORDER BY rating_value DESC, (love = 'L') DESC, id DESC
          LIMIT 12
        )
        SELECT p.id, p.title, p.album_artist_display, p.album, p.release_year,
               p.rating_value, p.love, p.time_seconds, p.canonical_genre,
               l.play_count, p.album_id, p.file_path, p.filename, p.import_run_id,
               p.original_year, p.publisher
        FROM page AS p
        LEFT JOIN lastfm_track_popularity AS l
          ON l.artist_key = lower(trim(p.album_artist_display))
         AND l.track_key = lower(trim(p.title))
        ORDER BY p.rating_value DESC, (p.love = 'L') DESC, p.id DESC
        "#,
        named_params! { ":genre": genre },
        "genre highlights",
        store,
    )?;
    if !highlights.is_empty() {
        return Ok(highlights);
    }
    let Some(album_id) = fallback_album_id else {
        return Ok(Vec::new());
    };
    catalog::query_tracks(
        connection,
        r#"
        SELECT t.id, t.title, t.album_artist_display, t.album, t.release_year,
               COALESCE(t.normalized_rating, CASE trim(t.rating_raw)
                 WHEN '0.5' THEN 10 WHEN '1' THEN 20 WHEN '1.0' THEN 20
                 WHEN '1.5' THEN 30 WHEN '2' THEN 40 WHEN '2.0' THEN 40
                 WHEN '2.5' THEN 50 WHEN '3' THEN 60 WHEN '3.0' THEN 60
                 WHEN '3.5' THEN 70 WHEN '4' THEN 80 WHEN '4.0' THEN 80
                 WHEN '4.5' THEN 90 WHEN '5' THEN 100 WHEN '5.0' THEN 100 END),
               t.love, t.time_seconds, t.canonical_genre,
               l.play_count, t.album_id, t.file_path, t.filename, t.import_run_id,
               t.year AS original_year, t.publisher AS publisher
        FROM tracks AS t
        LEFT JOIN lastfm_track_popularity AS l
          ON l.artist_key = lower(trim(t.album_artist_display))
         AND l.track_key = lower(trim(t.title))
        WHERE t.album_id = ?1
        ORDER BY t.disc_number, t.track_number, t.id
        LIMIT 12
        "#,
        [album_id],
        "genre fallback highlights",
        store,
    )
}

fn query_genre_detail(
    connection: &Connection,
    genre: &str,
    history: &HashMap<String, GenreHistoryInsight>,
    store: Option<&StateStore>,
) -> Result<GenreDetail, String> {
    let genre = validate_genre(genre)?;
    let summary = query_genre_summary(connection, genre, history, store)?;
    let decades = query_decades(connection, genre)?;
    let albums = query_albums(connection, genre)?;
    let artists = query_artists(connection, genre)?;
    let related_genres = query_related(connection, genre)?;
    let highlights = query_highlights(
        connection,
        genre,
        albums.first().map(|album| album.id.as_str()),
        store,
    )?;
    Ok(GenreDetail {
        summary,
        decades,
        albums,
        artists,
        related_genres,
        highlights,
    })
}

pub(crate) fn load_genre_detail(
    genre: String,
    history: &HistoryStore,
    store: &StateStore,
) -> Result<GenreDetail, String> {
    let path = catalog::default_catalog_path()?;
    let connection = catalog::open_catalog(&path)?;
    let history = history.genre_insights().unwrap_or_default();
    query_genre_detail(&connection, &genre, &history, Some(store))
}

fn queue_sql(mode: GenreQueueMode) -> &'static str {
    match mode {
        GenreQueueMode::Radio | GenreQueueMode::Shuffle | GenreQueueMode::Unrated => {
            r#"
            WITH chosen_albums AS MATERIALIZED (
              SELECT id FROM albums
              WHERE canonical_genre = :genre
              ORDER BY RANDOM()
              LIMIT 80
            ), page AS MATERIALIZED (
              SELECT t.id, t.title, t.album_artist_display, t.album, t.release_year,
                     COALESCE(t.normalized_rating, CASE trim(t.rating_raw)
                       WHEN '0.5' THEN 10 WHEN '1' THEN 20 WHEN '1.0' THEN 20
                       WHEN '1.5' THEN 30 WHEN '2' THEN 40 WHEN '2.0' THEN 40
                       WHEN '2.5' THEN 50 WHEN '3' THEN 60 WHEN '3.0' THEN 60
                       WHEN '3.5' THEN 70 WHEN '4' THEN 80 WHEN '4.0' THEN 80
                       WHEN '4.5' THEN 90 WHEN '5' THEN 100 WHEN '5.0' THEN 100 END) AS rating_value,
                     t.love, t.time_seconds, t.canonical_genre, t.album_id,
                     t.file_path, t.filename, t.import_run_id
              FROM chosen_albums AS chosen
              JOIN tracks AS t ON t.album_id = chosen.id
              WHERE (:unrated = 0 OR (t.normalized_rating IS NULL AND NULLIF(TRIM(t.rating_raw), '') IS NULL))
              ORDER BY
                CASE WHEN :radio = 1 THEN
                  CASE WHEN t.love = 'L' THEN 0
                       WHEN COALESCE(t.normalized_rating, 0) >= 80 THEN 1
                       WHEN t.normalized_rating IS NOT NULL THEN 2 ELSE 3 END
                  ELSE 0 END,
                RANDOM()
              LIMIT :candidate_limit
            )
            SELECT p.id, p.title, p.album_artist_display, p.album, p.release_year,
                   p.rating_value, p.love, p.time_seconds, p.canonical_genre,
                   l.play_count, p.album_id, p.file_path, p.filename, p.import_run_id
            FROM page AS p
            LEFT JOIN lastfm_track_popularity AS l
              ON l.artist_key = lower(trim(p.album_artist_display))
             AND l.track_key = lower(trim(p.title))
            "#
        }
        GenreQueueMode::Loved => {
            r#"
            WITH page AS MATERIALIZED (
              SELECT t.id, t.title, t.album_artist_display, t.album, t.release_year,
                     COALESCE(t.normalized_rating, CASE trim(t.rating_raw)
                       WHEN '0.5' THEN 10 WHEN '1' THEN 20 WHEN '1.0' THEN 20
                       WHEN '1.5' THEN 30 WHEN '2' THEN 40 WHEN '2.0' THEN 40
                       WHEN '2.5' THEN 50 WHEN '3' THEN 60 WHEN '3.0' THEN 60
                       WHEN '3.5' THEN 70 WHEN '4' THEN 80 WHEN '4.0' THEN 80
                       WHEN '4.5' THEN 90 WHEN '5' THEN 100 WHEN '5.0' THEN 100 END) AS rating_value,
                     t.love, t.time_seconds, t.canonical_genre, t.album_id,
                     t.file_path, t.filename, t.import_run_id
              FROM tracks AS t
              WHERE t.canonical_genre = :genre AND t.love = 'L'
                AND :radio IN (0, 1) AND :unrated IN (0, 1)
              ORDER BY RANDOM()
              LIMIT :candidate_limit
            )
            SELECT p.id, p.title, p.album_artist_display, p.album, p.release_year,
                   p.rating_value, p.love, p.time_seconds, p.canonical_genre,
                   l.play_count, p.album_id, p.file_path, p.filename, p.import_run_id
            FROM page AS p
            LEFT JOIN lastfm_track_popularity AS l
              ON l.artist_key = lower(trim(p.album_artist_display))
             AND l.track_key = lower(trim(p.title))
            "#
        }
        GenreQueueMode::HighestRated | GenreQueueMode::Rediscover => {
            r#"
            WITH page AS MATERIALIZED (
              SELECT t.id, t.title, t.album_artist_display, t.album, t.release_year,
                     COALESCE(t.normalized_rating, CASE trim(t.rating_raw)
                       WHEN '0.5' THEN 10 WHEN '1' THEN 20 WHEN '1.0' THEN 20
                       WHEN '1.5' THEN 30 WHEN '2' THEN 40 WHEN '2.0' THEN 40
                       WHEN '2.5' THEN 50 WHEN '3' THEN 60 WHEN '3.0' THEN 60
                       WHEN '3.5' THEN 70 WHEN '4' THEN 80 WHEN '4.0' THEN 80
                       WHEN '4.5' THEN 90 WHEN '5' THEN 100 WHEN '5.0' THEN 100 END) AS rating_value,
                     t.love, t.time_seconds, t.canonical_genre, t.album_id,
                     t.file_path, t.filename, t.import_run_id
              FROM tracks AS t
              WHERE t.canonical_genre = :genre
                AND (t.normalized_rating IS NOT NULL OR NULLIF(TRIM(t.rating_raw), '') IS NOT NULL)
                AND :radio IN (0, 1) AND :unrated IN (0, 1)
              ORDER BY rating_value DESC, (t.love = 'L') DESC, RANDOM()
              LIMIT :candidate_limit
            )
            SELECT p.id, p.title, p.album_artist_display, p.album, p.release_year,
                   p.rating_value, p.love, p.time_seconds, p.canonical_genre,
                   l.play_count, p.album_id, p.file_path, p.filename, p.import_run_id
            FROM page AS p
            LEFT JOIN lastfm_track_popularity AS l
              ON l.artist_key = lower(trim(p.album_artist_display))
             AND l.track_key = lower(trim(p.title))
            ORDER BY p.rating_value DESC, (p.love = 'L') DESC
            "#
        }
    }
}

fn query_genre_queue(
    connection: &Connection,
    request: &GenreQueueRequest,
    played_track_keys: &HashSet<String>,
    store: Option<&StateStore>,
) -> Result<Vec<TrackSummary>, String> {
    let genre = validate_genre(&request.genre)?;
    if request.limit == 0 || request.limit > MAX_QUEUE_BATCH {
        return Err(format!(
            "Genre queues must contain between 1 and {MAX_QUEUE_BATCH} tracks per batch."
        ));
    }
    if request.exclude_track_keys.len() > MAX_QUEUE_EXCLUSIONS
        || request
            .exclude_track_keys
            .iter()
            .any(|key| key.trim().is_empty() || key.len() > 2_048)
    {
        return Err("Genre queue exclusions are invalid.".to_owned());
    }
    let candidate_limit = request.limit.saturating_mul(4).min(400) as i64;
    let candidates = catalog::query_tracks(
        connection,
        queue_sql(request.mode),
        named_params! {
            ":genre": genre,
            ":candidate_limit": candidate_limit,
            ":radio": i64::from(request.mode == GenreQueueMode::Radio),
            ":unrated": i64::from(request.mode == GenreQueueMode::Unrated),
        },
        "genre queue",
        store,
    )?;
    let excluded = request
        .exclude_track_keys
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    let mut preferred = Vec::new();
    let mut fallback = Vec::new();
    for track in candidates {
        if excluded.contains(&track.track_key) || !seen.insert(track.track_key.clone()) {
            continue;
        }
        if request.mode == GenreQueueMode::Rediscover
            && played_track_keys.contains(&track.track_key)
        {
            fallback.push(track);
        } else {
            preferred.push(track);
        }
    }
    if request.mode == GenreQueueMode::Rediscover && preferred.len() < request.limit {
        preferred.extend(fallback);
    }
    preferred.truncate(request.limit);
    Ok(preferred)
}

pub(crate) fn load_genre_queue(
    request: GenreQueueRequest,
    history: &HistoryStore,
    store: &StateStore,
) -> Result<Vec<TrackSummary>, String> {
    validate_genre(&request.genre)?;
    let played = if request.mode == GenreQueueMode::Rediscover {
        history
            .played_track_keys_for_genre(&request.genre)
            .unwrap_or_default()
    } else {
        HashSet::new()
    };
    let path = catalog::default_catalog_path()?;
    let connection = catalog::open_catalog(&path)?;
    query_genre_queue(&connection, &request, &played, Some(store))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Connection {
        let connection = Connection::open_in_memory().expect("open fixture");
        connection
            .execute_batch(
                r#"
                CREATE TABLE albums (
                  id TEXT PRIMARY KEY, album TEXT, album_artist_display TEXT,
                  canonical_genre TEXT, total_tracks INTEGER, rated_tracks INTEGER,
                  loved_tracks INTEGER, total_seconds INTEGER, release_year INTEGER, year INTEGER,
                  effective_album_rating INTEGER, calculated_album_rating INTEGER,
                  album_rating INTEGER, album_score REAL, publisher TEXT
                );
                CREATE TABLE tracks (
                  id INTEGER PRIMARY KEY, title TEXT, album_artist_display TEXT, album TEXT,
                  release_year INTEGER, normalized_rating INTEGER, rating_raw TEXT, love TEXT,
                  time_seconds INTEGER, canonical_genre TEXT, album_id TEXT, file_path TEXT,
                  filename TEXT, import_run_id INTEGER, disc_number INTEGER, track_number INTEGER,
                  year INTEGER, publisher TEXT
                );
                CREATE TABLE lastfm_track_popularity (
                  artist_key TEXT, track_key TEXT, play_count INTEGER
                );
                INSERT INTO albums VALUES
                  ('s1', 'Neon Nights', 'College', 'Synthwave', 2, 2, 1, 480, 2012, 1985, 90, 90, 90, 95, 'Valerie Records'),
                  ('s2', 'After Dark', 'M83', 'Synthwave', 1, 1, 0, 240, 2015, 1999, 80, 80, 80, 85, 'Mute Records'),
                  ('e1', 'Electric Sky', 'M83', 'Electronic', 1, 1, 0, 200, 2011, 2011, 100, 100, 100, 99, 'Mute Records');
                INSERT INTO tracks VALUES
                  (1, 'A Real Hero', 'College', 'Neon Nights', 2012, 100, '5', 'L', 240, 'Synthwave', 's1', 'D:\MUSIC\College', 'hero.mp3', 1, 1, 1, 1985, 'Valerie Records'),
                  (2, 'Night Drive', 'College', 'Neon Nights', 2012, 80, '4', NULL, 240, 'Synthwave', 's1', 'D:\MUSIC\College', 'drive.mp3', 1, 1, 2, 1985, 'Valerie Records'),
                  (3, 'Midnight', 'M83', 'After Dark', 2015, 80, '4', NULL, 240, 'Synthwave', 's2', 'D:\MUSIC\M83', 'midnight.mp3', 1, 1, 1, 1999, 'Mute Records'),
                  (4, 'Electric', 'M83', 'Electric Sky', 2011, 100, '5', NULL, 200, 'Electronic', 'e1', 'D:\MUSIC\M83', 'electric.mp3', 1, 1, 1, 2011, 'Mute Records');
                "#,
            )
            .expect("seed fixture");
        connection
    }

    #[test]
    fn index_rolls_up_catalog_and_history_without_losing_display_genre() {
        let connection = fixture();
        let mut history = HashMap::new();
        history.insert(
            history::genre_identity("Synthwave"),
            GenreHistoryInsight {
                sessions: 4,
                plays: 3,
                listened_seconds: 720.0,
                last_listened_at_ms: Some(42),
            },
        );
        let genres = query_genre_index(&connection, &history, None).expect("query genre index");
        assert_eq!(genres.len(), 2);
        assert_eq!(genres[0].name, "Synthwave");
        assert_eq!(genres[0].track_count, 3);
        assert_eq!(genres[0].artist_count, 2);
        assert_eq!(genres[0].plays, 3);
        assert_eq!(genres[0].average_rating, Some(4.3));
        assert_eq!(genres[0].first_year, Some(1985));
        assert_eq!(genres[0].last_year, Some(1999));
    }

    #[test]
    fn detail_bounds_sections_and_explains_connections_through_shared_artists() {
        let connection = fixture();
        let detail = query_genre_detail(&connection, "Synthwave", &HashMap::new(), None)
            .expect("query genre detail");
        assert_eq!(detail.summary.track_count, 3);
        assert_eq!(
            detail
                .decades
                .iter()
                .map(|bucket| bucket.decade)
                .collect::<Vec<_>>(),
            vec![1980, 1990]
        );
        assert_eq!(detail.albums.len(), 2);
        assert!(detail.albums.iter().any(|album| album.year == Some(1985)));
        assert_eq!(detail.artists[0].name, "College");
        assert_eq!(detail.related_genres[0].name, "Electronic");
        assert_eq!(detail.related_genres[0].shared_artists, 1);
        assert_eq!(detail.highlights.len(), 3);
    }

    #[test]
    fn queue_is_bounded_to_the_selected_genre_and_honors_exclusions() {
        let connection = fixture();
        let request = GenreQueueRequest {
            genre: "Synthwave".to_owned(),
            mode: GenreQueueMode::HighestRated,
            limit: 2,
            exclude_track_keys: vec!["d:\\music\\college\\hero.mp3".to_owned()],
        };
        let queue = query_genre_queue(&connection, &request, &HashSet::new(), None)
            .expect("query genre queue");
        assert_eq!(queue.len(), 2);
        assert!(
            queue
                .iter()
                .all(|track| track.genre.as_deref() == Some("Synthwave"))
        );
        assert!(queue.iter().all(|track| track.title != "A Real Hero"));
    }

    #[test]
    fn rejects_unbounded_queue_requests() {
        let connection = fixture();
        let request = GenreQueueRequest {
            genre: "Synthwave".to_owned(),
            mode: GenreQueueMode::Radio,
            limit: 101,
            exclude_track_keys: Vec::new(),
        };
        assert!(query_genre_queue(&connection, &request, &HashSet::new(), None).is_err());
    }

    #[test]
    #[ignore = "requires the live read-only Music Library catalog"]
    fn live_catalog_genre_atlas_is_bounded() {
        let path = catalog::default_catalog_path().expect("catalog path");
        let connection = catalog::open_catalog(&path).expect("open catalog");
        let genres = query_genre_index(&connection, &HashMap::new(), None).expect("genre index");
        assert!(!genres.is_empty());
        assert!(genres.len() <= MAX_GENRES);
        assert!(genres.iter().any(|genre| genre.name == "Synthwave"));
        let detail = query_genre_detail(&connection, "Synthwave", &HashMap::new(), None)
            .expect("genre detail");
        assert_eq!(detail.summary.name, "Synthwave");
        assert!(!detail.albums.is_empty());
        let queue = query_genre_queue(
            &connection,
            &GenreQueueRequest {
                genre: "Synthwave".to_owned(),
                mode: GenreQueueMode::Radio,
                limit: MAX_QUEUE_BATCH,
                exclude_track_keys: Vec::new(),
            },
            &HashSet::new(),
            None,
        )
        .expect("genre radio");
        assert!(!queue.is_empty());
        assert!(queue.len() <= MAX_QUEUE_BATCH);
        assert!(
            queue
                .iter()
                .all(|track| track.genre.as_deref() == Some("Synthwave"))
        );
    }
}
