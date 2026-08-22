use crate::{
    catalog::{self, TrackSummary},
    state_store::StateStore,
};
use rusqlite::{Connection, OptionalExtension, named_params};
use serde::{Deserialize, Serialize};

const MAX_ALBUMS: usize = 100;
const MAX_ALBUMS_PER_GROUP: usize = 10;
const MAX_QUEUE_BATCH: usize = 100;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum YearBasis {
    Original,
    Release,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct YearSelection {
    pub(crate) basis: YearBasis,
    pub(crate) year: Option<i32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct YearQueueRequest {
    pub(crate) selection: YearSelection,
    pub(crate) limit: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct YearBucket {
    pub(crate) year: i64,
    pub(crate) album_count: i64,
    pub(crate) track_count: i64,
    pub(crate) rated_tracks: i64,
    pub(crate) loved_tracks: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct YearStats {
    pub(crate) first_year: Option<i64>,
    pub(crate) last_year: Option<i64>,
    pub(crate) different_albums: i64,
    pub(crate) different_tracks: i64,
    pub(crate) missing_original_albums: i64,
    pub(crate) missing_original_tracks: i64,
    pub(crate) missing_release_albums: i64,
    pub(crate) missing_release_tracks: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct YearSummary {
    pub(crate) album_count: i64,
    pub(crate) track_count: i64,
    pub(crate) rated_tracks: i64,
    pub(crate) loved_tracks: i64,
    pub(crate) duration_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct YearFlow {
    pub(crate) year: Option<i64>,
    pub(crate) album_count: i64,
    pub(crate) track_count: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct YearAlbum {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) original_year: Option<i64>,
    pub(crate) release_year: Option<i64>,
    pub(crate) total_tracks: i64,
    pub(crate) rated_tracks: i64,
    pub(crate) loved_tracks: i64,
    pub(crate) duration_seconds: i64,
    pub(crate) genre: Option<String>,
    pub(crate) rating: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct YearDetail {
    pub(crate) selection: YearSelection,
    pub(crate) summary: YearSummary,
    pub(crate) flows: Vec<YearFlow>,
    pub(crate) albums: Vec<YearAlbum>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct YearOverview {
    pub(crate) original_years: Vec<YearBucket>,
    pub(crate) release_years: Vec<YearBucket>,
    pub(crate) stats: YearStats,
    pub(crate) initial_detail: YearDetail,
}

fn field_for(basis: YearBasis) -> &'static str {
    match basis {
        YearBasis::Original => "year",
        YearBasis::Release => "release_year",
    }
}

fn counterpart_field_for(basis: YearBasis) -> &'static str {
    match basis {
        YearBasis::Original => "release_year",
        YearBasis::Release => "year",
    }
}

fn validate_selection(selection: &YearSelection) -> Result<(), String> {
    if selection
        .year
        .is_some_and(|year| !(1000..=2999).contains(&year))
    {
        return Err("Year selection is invalid.".to_owned());
    }
    Ok(())
}

fn selection_predicate(selection: &YearSelection, alias: &str) -> String {
    let field = field_for(selection.basis);
    format!(
        "((:selected_year IS NULL AND ({alias}.{field} IS NULL OR {alias}.{field} NOT BETWEEN 1000 AND 2999)) OR (:selected_year IS NOT NULL AND {alias}.{field} = :selected_year))"
    )
}

fn query_timeline(connection: &Connection, field: &str) -> Result<Vec<YearBucket>, String> {
    let sql = format!(
        r#"
        SELECT {field}, COUNT(*), CAST(SUM(total_tracks) AS INTEGER),
               CAST(SUM(rated_tracks) AS INTEGER), CAST(SUM(loved_tracks) AS INTEGER)
        FROM albums
        WHERE {field} BETWEEN 1000 AND 2999
        GROUP BY {field}
        ORDER BY {field}
        "#,
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("Could not prepare the {field} timeline: {error}"))?;
    statement
        .query_map([], |row| {
            Ok(YearBucket {
                year: row.get(0)?,
                album_count: row.get(1)?,
                track_count: row.get(2)?,
                rated_tracks: row.get(3)?,
                loved_tracks: row.get(4)?,
            })
        })
        .map_err(|error| format!("Could not read the {field} timeline: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode the {field} timeline: {error}"))
}

fn query_stats(connection: &Connection) -> Result<YearStats, String> {
    connection
        .query_row(
            r#"
            SELECT MIN(CASE WHEN year BETWEEN 1000 AND 2999 THEN year END),
                   MAX(CASE WHEN year BETWEEN 1000 AND 2999 THEN year END),
                   SUM(CASE WHEN year BETWEEN 1000 AND 2999
                                  AND release_year BETWEEN 1000 AND 2999
                                  AND year <> release_year THEN 1 ELSE 0 END),
                   SUM(CASE WHEN year BETWEEN 1000 AND 2999
                                  AND release_year BETWEEN 1000 AND 2999
                                  AND year <> release_year THEN total_tracks ELSE 0 END),
                   SUM(CASE WHEN year IS NULL OR year NOT BETWEEN 1000 AND 2999 THEN 1 ELSE 0 END),
                   SUM(CASE WHEN year IS NULL OR year NOT BETWEEN 1000 AND 2999 THEN total_tracks ELSE 0 END),
                   SUM(CASE WHEN release_year IS NULL OR release_year NOT BETWEEN 1000 AND 2999 THEN 1 ELSE 0 END),
                   SUM(CASE WHEN release_year IS NULL OR release_year NOT BETWEEN 1000 AND 2999 THEN total_tracks ELSE 0 END)
            FROM albums
            "#,
            [],
            |row| {
                Ok(YearStats {
                    first_year: row.get(0)?,
                    last_year: row.get(1)?,
                    different_albums: row.get(2)?,
                    different_tracks: row.get(3)?,
                    missing_original_albums: row.get(4)?,
                    missing_original_tracks: row.get(5)?,
                    missing_release_albums: row.get(6)?,
                    missing_release_tracks: row.get(7)?,
                })
            },
        )
        .map_err(|error| format!("Could not read the Years summary: {error}"))
}

fn query_summary(
    connection: &Connection,
    selection: &YearSelection,
) -> Result<YearSummary, String> {
    let predicate = selection_predicate(selection, "a");
    let sql = format!(
        r#"
        SELECT COUNT(*), COALESCE(CAST(SUM(a.total_tracks) AS INTEGER), 0),
               COALESCE(CAST(SUM(a.rated_tracks) AS INTEGER), 0),
               COALESCE(CAST(SUM(a.loved_tracks) AS INTEGER), 0),
               COALESCE(CAST(SUM(a.total_seconds) AS INTEGER), 0)
        FROM albums AS a
        WHERE {predicate}
        "#,
    );
    connection
        .query_row(
            &sql,
            named_params! { ":selected_year": selection.year },
            |row| {
                Ok(YearSummary {
                    album_count: row.get(0)?,
                    track_count: row.get(1)?,
                    rated_tracks: row.get(2)?,
                    loved_tracks: row.get(3)?,
                    duration_seconds: row.get(4)?,
                })
            },
        )
        .map_err(|error| format!("Could not read this year summary: {error}"))
}

fn query_flows(
    connection: &Connection,
    selection: &YearSelection,
) -> Result<Vec<YearFlow>, String> {
    let counterpart = counterpart_field_for(selection.basis);
    let predicate = selection_predicate(selection, "a");
    let sql = format!(
        r#"
        SELECT CASE WHEN a.{counterpart} BETWEEN 1000 AND 2999 THEN a.{counterpart} END AS counterpart_year,
               COUNT(*), CAST(SUM(a.total_tracks) AS INTEGER)
        FROM albums AS a
        WHERE {predicate}
        GROUP BY counterpart_year
        ORDER BY (counterpart_year IS NULL), counterpart_year
        "#,
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("Could not prepare the two-clock flows: {error}"))?;
    statement
        .query_map(named_params! { ":selected_year": selection.year }, |row| {
            Ok(YearFlow {
                year: row.get(0)?,
                album_count: row.get(1)?,
                track_count: row.get(2)?,
            })
        })
        .map_err(|error| format!("Could not read the two-clock flows: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode the two-clock flows: {error}"))
}

fn query_albums(
    connection: &Connection,
    selection: &YearSelection,
) -> Result<Vec<YearAlbum>, String> {
    let counterpart = counterpart_field_for(selection.basis);
    let predicate = selection_predicate(selection, "a");
    let sql = format!(
        r#"
        WITH candidates AS MATERIALIZED (
          SELECT a.id,
                 COALESCE(NULLIF(TRIM(a.album), ''), 'Unknown Album') AS title,
                 COALESCE(NULLIF(TRIM(a.album_artist_display), ''), 'Unknown Artist') AS artist,
                 a.year, a.release_year, a.total_tracks, a.rated_tracks, a.loved_tracks,
                 a.total_seconds, a.canonical_genre,
                 COALESCE(a.effective_album_rating, a.calculated_album_rating, a.album_rating) / 20.0 AS rating,
                 CASE
                   WHEN :selected_year IS NOT NULL AND a.{counterpart} = :selected_year THEN -2
                   WHEN a.{counterpart} BETWEEN 1000 AND 2999 THEN (a.{counterpart} / 10) * 10
                   ELSE -1
                 END AS edition_group,
                 ROW_NUMBER() OVER (
                   PARTITION BY CASE
                     WHEN :selected_year IS NOT NULL AND a.{counterpart} = :selected_year THEN -2
                     WHEN a.{counterpart} BETWEEN 1000 AND 2999 THEN (a.{counterpart} / 10) * 10
                     ELSE -1
                   END
                   ORDER BY (a.loved_tracks > 0) DESC, a.loved_tracks DESC,
                            COALESCE(a.effective_album_rating, a.calculated_album_rating, a.album_rating, -1) DESC,
                            COALESCE(a.album_score, -1) DESC, a.id
                 ) AS group_rank
          FROM albums AS a
          WHERE {predicate}
        )
        SELECT id, title, artist, year, release_year, total_tracks, rated_tracks,
               loved_tracks, total_seconds, canonical_genre, rating
        FROM candidates
        WHERE group_rank <= :per_group
        ORDER BY CASE edition_group WHEN -2 THEN -10000 WHEN -1 THEN 10000 ELSE edition_group END,
                 group_rank, id
        LIMIT :album_limit
        "#,
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("Could not prepare the year editions: {error}"))?;
    statement
        .query_map(
            named_params! {
                ":selected_year": selection.year,
                ":per_group": MAX_ALBUMS_PER_GROUP as i64,
                ":album_limit": MAX_ALBUMS as i64,
            },
            |row| {
                Ok(YearAlbum {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    artist: row.get(2)?,
                    original_year: row.get(3)?,
                    release_year: row.get(4)?,
                    total_tracks: row.get(5)?,
                    rated_tracks: row.get(6)?,
                    loved_tracks: row.get(7)?,
                    duration_seconds: row.get(8)?,
                    genre: row.get(9)?,
                    rating: row.get(10)?,
                })
            },
        )
        .map_err(|error| format!("Could not read the year editions: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode the year editions: {error}"))
}

fn query_year_detail(
    connection: &Connection,
    selection: YearSelection,
) -> Result<YearDetail, String> {
    validate_selection(&selection)?;
    let summary = query_summary(connection, &selection)?;
    let flows = query_flows(connection, &selection)?;
    let albums = query_albums(connection, &selection)?;
    Ok(YearDetail {
        selection,
        summary,
        flows,
        albums,
    })
}

fn suggested_release_year(connection: &Connection) -> Result<i32, String> {
    connection
        .query_row(
            r#"
            SELECT MAX(release_year)
            FROM albums
            WHERE release_year BETWEEN 1000 AND 2999
              AND release_year < CAST(strftime('%Y', 'now') AS INTEGER)
            "#,
            [],
            |row| row.get::<_, Option<i32>>(0),
        )
        .optional()
        .map_err(|error| format!("Could not choose the opening release year: {error}"))?
        .flatten()
        .or_else(|| {
            connection
                .query_row(
                    "SELECT MAX(release_year) FROM albums WHERE release_year BETWEEN 1000 AND 2999",
                    [],
                    |row| row.get::<_, Option<i32>>(0),
                )
                .ok()
                .flatten()
        })
        .ok_or_else(|| "No valid release years are available in the catalog.".to_owned())
}

fn query_year_overview(connection: &Connection) -> Result<YearOverview, String> {
    let original_years = query_timeline(connection, "year")?;
    let release_years = query_timeline(connection, "release_year")?;
    let stats = query_stats(connection)?;
    let initial_detail = query_year_detail(
        connection,
        YearSelection {
            basis: YearBasis::Release,
            year: Some(suggested_release_year(connection)?),
        },
    )?;
    Ok(YearOverview {
        original_years,
        release_years,
        stats,
        initial_detail,
    })
}

fn query_year_queue(
    connection: &Connection,
    request: YearQueueRequest,
    store: Option<&StateStore>,
) -> Result<Vec<TrackSummary>, String> {
    validate_selection(&request.selection)?;
    if request.limit == 0 || request.limit > MAX_QUEUE_BATCH {
        return Err("Year playback must request between 1 and 100 tracks.".to_owned());
    }
    let predicate = selection_predicate(&request.selection, "a");
    let sql = format!(
        r#"
        WITH chosen_albums AS MATERIALIZED (
          SELECT a.id
          FROM albums AS a
          WHERE {predicate}
          ORDER BY (a.loved_tracks > 0) DESC, a.loved_tracks DESC,
                   COALESCE(a.effective_album_rating, a.calculated_album_rating, a.album_rating, -1) DESC,
                   COALESCE(a.album_score, -1) DESC, a.id
          LIMIT 100
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
          FROM tracks AS t
          JOIN chosen_albums AS chosen ON chosen.id = t.album_id
          ORDER BY (t.love = 'L') DESC, rating_value DESC, t.album_id, t.disc_number, t.track_number, t.id
          LIMIT :queue_limit
        )
        SELECT p.id, p.title, p.album_artist_display, p.album, p.release_year,
               p.rating_value, p.love, p.time_seconds, p.canonical_genre,
               l.play_count, p.album_id, p.file_path, p.filename, p.import_run_id
        FROM page AS p
        LEFT JOIN lastfm_track_popularity AS l
          ON l.artist_key = lower(trim(p.album_artist_display))
         AND l.track_key = lower(trim(p.title))
        ORDER BY (p.love = 'L') DESC, p.rating_value DESC, p.album_id, p.id
        "#,
    );
    catalog::query_tracks(
        connection,
        &sql,
        named_params! {
            ":selected_year": request.selection.year,
            ":queue_limit": request.limit as i64,
        },
        "year playback",
        store,
    )
}

pub(crate) fn load_year_overview() -> Result<YearOverview, String> {
    let path = catalog::default_catalog_path()?;
    let connection = catalog::open_catalog(&path)?;
    query_year_overview(&connection)
}

pub(crate) fn load_year_detail(selection: YearSelection) -> Result<YearDetail, String> {
    let path = catalog::default_catalog_path()?;
    let connection = catalog::open_catalog(&path)?;
    query_year_detail(&connection, selection)
}

pub(crate) fn load_year_queue(
    request: YearQueueRequest,
    store: &StateStore,
) -> Result<Vec<TrackSummary>, String> {
    let path = catalog::default_catalog_path()?;
    let connection = catalog::open_catalog(&path)?;
    query_year_queue(&connection, request, Some(store))
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
                  year INTEGER, release_year INTEGER, total_tracks INTEGER NOT NULL,
                  rated_tracks INTEGER NOT NULL, loved_tracks INTEGER NOT NULL,
                  total_seconds INTEGER NOT NULL, canonical_genre TEXT,
                  effective_album_rating INTEGER, calculated_album_rating INTEGER,
                  album_rating INTEGER, album_score REAL
                );
                CREATE TABLE tracks (
                  id INTEGER PRIMARY KEY, album_id TEXT NOT NULL, title TEXT,
                  album_artist_display TEXT, album TEXT, year INTEGER, release_year INTEGER,
                  normalized_rating INTEGER, rating_raw TEXT, love TEXT, time_seconds INTEGER,
                  canonical_genre TEXT, file_path TEXT, filename TEXT, import_run_id INTEGER NOT NULL,
                  disc_number INTEGER, track_number INTEGER
                );
                CREATE TABLE lastfm_track_popularity (
                  artist_key TEXT, track_key TEXT, play_count INTEGER
                );
                INSERT INTO albums VALUES
                  ('original', 'Original Edition', 'Artist', 1982, 1982, 2, 2, 1, 400, 'Electronic', 100, NULL, NULL, 9),
                  ('reissue', 'Later Edition', 'Artist', 1982, 2025, 2, 2, 0, 400, 'Electronic', 80, NULL, NULL, 7),
                  ('archive', 'Archive Edition', 'Artist', 1969, 2025, 1, 0, 0, 200, 'Rock', NULL, NULL, NULL, 2),
                  ('missing-original', 'Unknown Origin', 'Artist', NULL, 2025, 1, 0, 0, 200, 'Ambient', NULL, NULL, NULL, 1),
                  ('missing-release', 'Unknown Release', 'Artist', 1982, NULL, 1, 0, 0, 200, 'Ambient', NULL, NULL, NULL, 1);
                INSERT INTO tracks VALUES
                  (1, 'original', 'One', 'Artist', 'Original Edition', 1982, 1982, 100, NULL, 'L', 200, 'Electronic', 'D:\\Music', 'one.mp3', 1, 1, 1),
                  (2, 'original', 'Two', 'Artist', 'Original Edition', 1982, 1982, 100, NULL, NULL, 200, 'Electronic', 'D:\\Music', 'two.mp3', 1, 1, 2),
                  (3, 'reissue', 'Three', 'Artist', 'Later Edition', 1982, 2025, 80, NULL, NULL, 200, 'Electronic', 'D:\\Music', 'three.mp3', 1, 1, 1),
                  (4, 'reissue', 'Four', 'Artist', 'Later Edition', 1982, 2025, 80, NULL, NULL, 200, 'Electronic', 'D:\\Music', 'four.mp3', 1, 1, 2),
                  (5, 'archive', 'Five', 'Artist', 'Archive Edition', 1969, 2025, NULL, NULL, NULL, 200, 'Rock', 'D:\\Music', 'five.mp3', 1, 1, 1),
                  (6, 'missing-original', 'Six', 'Artist', 'Unknown Origin', NULL, 2025, NULL, NULL, NULL, 200, 'Ambient', 'D:\\Music', 'six.mp3', 1, 1, 1),
                  (7, 'missing-release', 'Seven', 'Artist', 'Unknown Release', 1982, NULL, NULL, NULL, NULL, 200, 'Ambient', 'D:\\Music', 'seven.mp3', 1, 1, 1);
                "#,
            )
            .expect("seed fixture");
        connection
    }

    #[test]
    fn preserves_distinct_original_and_release_timelines() {
        let connection = fixture();
        let original = query_timeline(&connection, "year").expect("original timeline");
        let release = query_timeline(&connection, "release_year").expect("release timeline");
        assert_eq!(
            original
                .iter()
                .find(|bucket| bucket.year == 1982)
                .unwrap()
                .album_count,
            3
        );
        assert_eq!(
            release
                .iter()
                .find(|bucket| bucket.year == 2025)
                .unwrap()
                .album_count,
            3
        );
        let stats = query_stats(&connection).expect("stats");
        assert_eq!(stats.different_albums, 2);
        assert_eq!(stats.different_tracks, 3);
        assert_eq!(stats.missing_original_albums, 1);
        assert_eq!(stats.missing_release_albums, 1);
    }

    #[test]
    fn either_clock_becomes_the_authoritative_detail_lens() {
        let connection = fixture();
        let release = query_year_detail(
            &connection,
            YearSelection {
                basis: YearBasis::Release,
                year: Some(2025),
            },
        )
        .expect("release detail");
        assert_eq!(release.summary.album_count, 3);
        assert!(
            release
                .flows
                .iter()
                .any(|flow| flow.year == Some(1982) && flow.album_count == 1)
        );
        assert!(
            release
                .flows
                .iter()
                .any(|flow| flow.year.is_none() && flow.album_count == 1)
        );

        let original = query_year_detail(
            &connection,
            YearSelection {
                basis: YearBasis::Original,
                year: Some(1982),
            },
        )
        .expect("original detail");
        assert_eq!(original.summary.album_count, 3);
        assert!(original.flows.iter().any(|flow| flow.year == Some(2025)));
        assert!(original.flows.iter().any(|flow| flow.year.is_none()));
        assert!(
            original
                .albums
                .iter()
                .all(|album| album.original_year == Some(1982))
        );
    }

    #[test]
    fn missing_years_are_separate_bounded_selections() {
        let connection = fixture();
        let missing_original = query_year_detail(
            &connection,
            YearSelection {
                basis: YearBasis::Original,
                year: None,
            },
        )
        .expect("missing original");
        let missing_release = query_year_detail(
            &connection,
            YearSelection {
                basis: YearBasis::Release,
                year: None,
            },
        )
        .expect("missing release");
        assert_eq!(missing_original.albums[0].id, "missing-original");
        assert_eq!(missing_release.albums[0].id, "missing-release");
    }

    #[test]
    fn year_queue_is_validated_and_capped() {
        let connection = fixture();
        let tracks = query_year_queue(
            &connection,
            YearQueueRequest {
                selection: YearSelection {
                    basis: YearBasis::Original,
                    year: Some(1982),
                },
                limit: 2,
            },
            None,
        )
        .expect("year queue");
        assert_eq!(tracks.len(), 2);
        assert!(
            query_year_queue(
                &connection,
                YearQueueRequest {
                    selection: YearSelection {
                        basis: YearBasis::Release,
                        year: Some(2025)
                    },
                    limit: 101,
                },
                None,
            )
            .is_err()
        );
    }

    #[test]
    #[ignore = "requires the live read-only Music Library catalog"]
    fn live_catalog_years_are_distinct_and_bounded() {
        let overview = load_year_overview().expect("load live Years overview");
        assert!(!overview.original_years.is_empty());
        assert!(!overview.release_years.is_empty());
        assert!(overview.original_years.len() < 2_000);
        assert!(overview.release_years.len() < 2_000);
        assert!(overview.stats.different_albums > 0);
        assert!(overview.stats.different_tracks > 0);
        assert!(overview.initial_detail.albums.len() <= MAX_ALBUMS);
        assert!(overview.initial_detail.albums.iter().all(
            |album| album.release_year == overview.initial_detail.selection.year.map(i64::from)
        ));
    }
}
