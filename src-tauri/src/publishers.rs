use crate::{
    catalog::{self, TrackSummary},
    state_store::StateStore,
};
use rusqlite::{Connection, Row, named_params};
use serde::{Deserialize, Serialize};

const MAX_PUBLISHERS: usize = 6;
const MAX_ALBUMS: usize = 64;
const MAX_QUEUE: usize = 100;
const MAX_SEARCH_CHARS: usize = 128;
const MAX_PUBLISHER_CHARS: usize = 256;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PublisherActivityBucket {
    pub(crate) year: i64,
    pub(crate) album_count: i64,
    pub(crate) track_count: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PublisherSummary {
    pub(crate) name: String,
    pub(crate) album_count: i64,
    pub(crate) track_count: i64,
    pub(crate) first_year: Option<i64>,
    pub(crate) last_year: Option<i64>,
    pub(crate) release_activity: Vec<PublisherActivityBucket>,
    pub(crate) original_activity: Vec<PublisherActivityBucket>,
    pub(crate) logo_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PublisherAlbum {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) original_year: Option<i64>,
    pub(crate) release_year: Option<i64>,
    pub(crate) publisher: String,
    pub(crate) total_tracks: i64,
    pub(crate) rated_tracks: i64,
    pub(crate) loved_tracks: i64,
    pub(crate) duration_seconds: i64,
    pub(crate) genre: Option<String>,
    pub(crate) rating: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PublisherDetail {
    pub(crate) publisher: PublisherSummary,
    pub(crate) albums: Vec<PublisherAlbum>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PublisherOverview {
    pub(crate) publishers: Vec<PublisherSummary>,
    pub(crate) initial_detail: PublisherDetail,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PublisherQueueRequest {
    pub(crate) publisher: String,
    pub(crate) limit: usize,
}

fn validate_search(search: Option<String>) -> Result<Option<String>, String> {
    let search = search
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if search
        .as_ref()
        .is_some_and(|value| value.chars().count() > MAX_SEARCH_CHARS)
    {
        return Err("Publisher search is too long.".to_owned());
    }
    Ok(search)
}

fn validate_publisher(publisher: String) -> Result<String, String> {
    let publisher = publisher.trim().to_owned();
    if publisher.is_empty() || publisher.chars().count() > MAX_PUBLISHER_CHARS {
        return Err("Publisher selection is invalid.".to_owned());
    }
    Ok(publisher)
}

fn query_activity(
    connection: &Connection,
    publisher: &str,
    field: &str,
) -> Result<Vec<PublisherActivityBucket>, String> {
    let sql = format!(
        r#"
        SELECT a.{field}, COUNT(*), CAST(SUM(a.total_tracks) AS INTEGER)
        FROM albums AS a
        WHERE lower(trim(a.publisher)) = lower(trim(:publisher))
          AND a.{field} BETWEEN 1000 AND 2999
        GROUP BY a.{field}
        ORDER BY a.{field}
        "#,
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("Could not prepare publisher activity: {error}"))?;
    statement
        .query_map(named_params! { ":publisher": publisher }, |row| {
            Ok(PublisherActivityBucket {
                year: row.get(0)?,
                album_count: row.get(1)?,
                track_count: row.get(2)?,
            })
        })
        .map_err(|error| format!("Could not read publisher activity: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode publisher activity: {error}"))
}

fn summary_rollup_sql() -> &'static str {
    r#"
    WITH normalized AS MATERIALIZED (
      SELECT lower(trim(a.publisher)) AS publisher_key,
             trim(a.publisher) AS display_name,
             a.total_tracks,
             CASE WHEN a.release_year BETWEEN 1000 AND 2999 THEN a.release_year END AS release_year
      FROM albums AS a
      WHERE NULLIF(trim(a.publisher), '') IS NOT NULL
    ), variants AS MATERIALIZED (
      SELECT publisher_key, display_name, COUNT(*) AS album_count,
             CAST(SUM(total_tracks) AS INTEGER) AS track_count,
             MIN(release_year) AS first_year, MAX(release_year) AS last_year
      FROM normalized
      GROUP BY publisher_key, display_name
    ), rollup AS (
      SELECT v.publisher_key,
             (SELECT v2.display_name FROM variants AS v2
              WHERE v2.publisher_key = v.publisher_key
              ORDER BY v2.album_count DESC, v2.display_name COLLATE NOCASE
              LIMIT 1) AS display_name,
             CAST(SUM(v.album_count) AS INTEGER) AS album_count,
             CAST(SUM(v.track_count) AS INTEGER) AS track_count,
             MIN(v.first_year) AS first_year,
             MAX(v.last_year) AS last_year
      FROM variants AS v
      GROUP BY v.publisher_key
    )
    SELECT display_name, album_count, track_count, first_year, last_year
    FROM rollup
    WHERE (:search IS NULL OR publisher_key LIKE '%' || lower(trim(:search)) || '%')
      AND (:exact IS NULL OR publisher_key = lower(trim(:exact)))
    ORDER BY album_count DESC, track_count DESC, display_name COLLATE NOCASE
    LIMIT :publisher_limit
    "#
}

fn query_summaries(
    connection: &Connection,
    search: Option<&str>,
    exact: Option<&str>,
    limit: usize,
) -> Result<Vec<PublisherSummary>, String> {
    let mut statement = connection
        .prepare(summary_rollup_sql())
        .map_err(|error| format!("Could not prepare publisher summaries: {error}"))?;
    let base = statement
        .query_map(
            named_params! {
                ":search": search,
                ":exact": exact,
                ":publisher_limit": limit as i64,
            },
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                ))
            },
        )
        .map_err(|error| format!("Could not read publisher summaries: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode publisher summaries: {error}"))?;
    base.into_iter()
        .map(|(name, album_count, track_count, first_year, last_year)| {
            Ok(PublisherSummary {
                release_activity: query_activity(connection, &name, "release_year")?,
                original_activity: query_activity(connection, &name, "year")?,
                name,
                album_count,
                track_count,
                first_year,
                last_year,
                logo_url: None,
            })
        })
        .collect()
}

fn map_album_row(row: &Row<'_>) -> rusqlite::Result<PublisherAlbum> {
    let effective_rating: Option<i64> = row.get(12)?;
    Ok(PublisherAlbum {
        id: row.get(0)?,
        title: row
            .get::<_, Option<String>>(1)?
            .unwrap_or_else(|| "Unknown Album".to_owned()),
        artist: row
            .get::<_, Option<String>>(2)?
            .unwrap_or_else(|| "Unknown Artist".to_owned()),
        original_year: row.get(3)?,
        release_year: row.get(4)?,
        publisher: row.get(5)?,
        total_tracks: row.get(6)?,
        rated_tracks: row.get(7)?,
        loved_tracks: row.get(8)?,
        duration_seconds: row.get(9)?,
        genre: row.get(10)?,
        rating: effective_rating.map(|value| value as f64 / 20.0),
    })
}

fn query_albums(connection: &Connection, publisher: &str) -> Result<Vec<PublisherAlbum>, String> {
    let mut statement = connection
        .prepare(
            r#"
            SELECT a.id, a.album, a.album_artist_display, a.year, a.release_year,
                   trim(a.publisher), a.total_tracks, a.rated_tracks, a.loved_tracks,
                   a.total_seconds, a.canonical_genre, a.album_score, a.effective_album_rating
            FROM albums AS a
            WHERE lower(trim(a.publisher)) = lower(trim(:publisher))
            ORDER BY CASE WHEN a.release_year BETWEEN 1000 AND 2999 THEN a.release_year END DESC,
                     (a.loved_tracks > 0) DESC, a.loved_tracks DESC,
                     COALESCE(a.effective_album_rating, -1) DESC,
                     COALESCE(a.album_score, -1) DESC, a.id
            LIMIT :album_limit
            "#,
        )
        .map_err(|error| format!("Could not prepare publisher releases: {error}"))?;
    statement
        .query_map(
            named_params! {
                ":publisher": publisher,
                ":album_limit": MAX_ALBUMS as i64,
            },
            map_album_row,
        )
        .map_err(|error| format!("Could not read publisher releases: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode publisher releases: {error}"))
}

fn query_detail(connection: &Connection, publisher: String) -> Result<PublisherDetail, String> {
    let publisher = validate_publisher(publisher)?;
    let summary = query_summaries(connection, None, Some(&publisher), 1)?
        .into_iter()
        .next()
        .ok_or_else(|| "Publisher is no longer available in the catalog.".to_owned())?;
    let albums = query_albums(connection, &summary.name)?;
    Ok(PublisherDetail {
        publisher: summary,
        albums,
    })
}

fn query_overview(
    connection: &Connection,
    search: Option<String>,
) -> Result<PublisherOverview, String> {
    let search = validate_search(search)?;
    let publishers = query_summaries(connection, search.as_deref(), None, MAX_PUBLISHERS)?;
    let initial = publishers.first().ok_or_else(|| {
        if search.is_some() {
            "No publishers matched this search.".to_owned()
        } else {
            "No publisher metadata is available in the catalog.".to_owned()
        }
    })?;
    let initial_detail = PublisherDetail {
        publisher: initial.clone(),
        albums: query_albums(connection, &initial.name)?,
    };
    Ok(PublisherOverview {
        publishers,
        initial_detail,
    })
}

fn query_queue(
    connection: &Connection,
    request: PublisherQueueRequest,
    store: Option<&StateStore>,
) -> Result<Vec<TrackSummary>, String> {
    let publisher = validate_publisher(request.publisher)?;
    if request.limit == 0 || request.limit > MAX_QUEUE {
        return Err("Publisher playback must request between 1 and 100 tracks.".to_owned());
    }
    catalog::query_tracks(
        connection,
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
                 t.file_path, t.filename, t.import_run_id, t.year AS original_year,
                 t.publisher AS publisher
          FROM tracks AS t
          WHERE lower(trim(t.publisher)) = lower(trim(:publisher))
          ORDER BY (t.love = 'L') DESC, rating_value DESC,
                   CASE WHEN t.release_year BETWEEN 1000 AND 2999 THEN t.release_year END DESC,
                   t.album_id, t.disc_number, t.track_number, t.id
          LIMIT :queue_limit
        )
        SELECT p.id, p.title, p.album_artist_display, p.album, p.release_year,
               p.rating_value, p.love, p.time_seconds, p.canonical_genre,
               l.play_count, p.album_id, p.file_path, p.filename, p.import_run_id,
               p.original_year, p.publisher
        FROM page AS p
        LEFT JOIN lastfm_track_popularity AS l
          ON l.artist_key = lower(trim(p.album_artist_display))
         AND l.track_key = lower(trim(p.title))
        ORDER BY (p.love = 'L') DESC, p.rating_value DESC, p.release_year DESC, p.album_id, p.id
        "#,
        named_params! {
            ":publisher": publisher,
            ":queue_limit": request.limit as i64,
        },
        "publisher playback",
        store,
    )
}

pub(crate) fn load_publisher_overview(search: Option<String>) -> Result<PublisherOverview, String> {
    let connection = catalog::open_catalog(&catalog::default_catalog_path()?)?;
    query_overview(&connection, search)
}

pub(crate) fn load_publisher_detail(publisher: String) -> Result<PublisherDetail, String> {
    let connection = catalog::open_catalog(&catalog::default_catalog_path()?)?;
    query_detail(&connection, publisher)
}

pub(crate) fn load_publisher_queue(
    request: PublisherQueueRequest,
    store: &StateStore,
) -> Result<Vec<TrackSummary>, String> {
    let connection = catalog::open_catalog(&catalog::default_catalog_path()?)?;
    query_queue(&connection, request, Some(store))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Connection {
        let connection = Connection::open_in_memory().expect("open publisher fixture");
        connection.execute_batch(
            r#"
            CREATE TABLE albums (
              id TEXT PRIMARY KEY, album TEXT, album_artist_display TEXT, year INTEGER,
              release_year INTEGER, publisher TEXT, total_tracks INTEGER, rated_tracks INTEGER,
              loved_tracks INTEGER, total_seconds INTEGER, canonical_genre TEXT,
              album_score REAL, effective_album_rating INTEGER
            );
            CREATE TABLE tracks (
              id INTEGER PRIMARY KEY, title TEXT, album_artist_display TEXT, album TEXT,
              year INTEGER, release_year INTEGER, publisher TEXT, normalized_rating INTEGER,
              rating_raw TEXT, love TEXT, time_seconds INTEGER, canonical_genre TEXT,
              album_id TEXT, file_path TEXT, filename TEXT, import_run_id INTEGER,
              disc_number INTEGER, track_number INTEGER
            );
            CREATE TABLE lastfm_track_popularity (artist_key TEXT, track_key TEXT, play_count INTEGER);
            INSERT INTO albums VALUES
              ('a1','Revolver','The Beatles',1966,1966,'Parlophone',14,14,8,2107,'Rock',920,100),
              ('a2','OK Computer','Radiohead',1997,1997,'PARLOPHONE',12,12,7,3213,'Alternative Rock',870,100),
              ('a3','Blue Train','John Coltrane',1957,1957,'Blue Note',5,5,3,2570,'Jazz',780,90);
            INSERT INTO tracks VALUES
              (1,'Taxman','The Beatles','Revolver',1966,1966,'Parlophone',100,NULL,'L',159,'Rock','a1','C:\\Music','01.mp3',1,1,1),
              (2,'Airbag','Radiohead','OK Computer',1997,1997,'PARLOPHONE',100,NULL,'L',284,'Alternative Rock','a2','C:\\Music','02.mp3',1,1,1),
              (3,'Blue Train','John Coltrane','Blue Train',1957,1957,'Blue Note',90,NULL,'L',640,'Jazz','a3','C:\\Music','03.mp3',1,1,1);
            "#,
        ).expect("publisher fixture schema");
        connection
    }

    #[test]
    fn overview_merges_case_only_publisher_variants() {
        let overview = query_overview(&fixture(), None).expect("publisher overview");
        let parlophone = overview
            .publishers
            .iter()
            .find(|publisher| publisher.name.eq_ignore_ascii_case("Parlophone"))
            .expect("Parlophone");
        assert_eq!(parlophone.album_count, 2);
        assert_eq!(parlophone.track_count, 26);
        assert_eq!(parlophone.release_activity.len(), 2);
    }

    #[test]
    fn overview_search_is_bounded_and_case_insensitive() {
        let overview =
            query_overview(&fixture(), Some("blue".to_owned())).expect("publisher search");
        assert_eq!(overview.publishers.len(), 1);
        assert_eq!(overview.initial_detail.publisher.name, "Blue Note");
        assert_eq!(overview.initial_detail.albums[0].publisher, "Blue Note");
    }

    #[test]
    fn queue_keeps_publisher_metadata() {
        let tracks = query_queue(
            &fixture(),
            PublisherQueueRequest {
                publisher: "parlophone".to_owned(),
                limit: 10,
            },
            None,
        )
        .expect("publisher queue");
        assert_eq!(tracks.len(), 2);
        assert!(tracks.iter().all(|track| {
            track
                .publisher
                .as_deref()
                .is_some_and(|publisher| publisher.eq_ignore_ascii_case("Parlophone"))
        }));
    }
}
