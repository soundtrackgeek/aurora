use rusqlite::{Connection, OpenFlags, Row, named_params};
use serde::{Deserialize, Serialize};
use std::{
    env,
    path::{Component, Path, PathBuf},
    time::Duration,
};

const CATALOG_RELATIVE_PATH: &str = "com.local.musiclibrary\\music-library.sqlite3";
pub(crate) const COVER_ROOT: &str = r"C:\_code\music_backup_v5\AlbumCovers";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LibrarySummary {
    pub(crate) songs: i64,
    pub(crate) albums: i64,
    pub(crate) artists: i64,
    pub(crate) genres: i64,
    pub(crate) loved: i64,
    pub(crate) rated: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArtistSummary {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) track_count: i64,
    pub(crate) album_count: i64,
    pub(crate) play_count: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrackSummary {
    pub(crate) id: String,
    pub(crate) album_id: Option<String>,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) album: String,
    pub(crate) release_year: Option<i64>,
    pub(crate) rating: Option<f64>,
    pub(crate) loved: bool,
    pub(crate) duration_seconds: Option<i64>,
    pub(crate) genre: Option<String>,
    pub(crate) play_count: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LibrarySnapshot {
    pub(crate) source_state: &'static str,
    pub(crate) source_label: &'static str,
    pub(crate) source_path: String,
    pub(crate) summary: LibrarySummary,
    pub(crate) artists: Vec<ArtistSummary>,
    pub(crate) tracks: Vec<TrackSummary>,
}

pub(crate) fn default_catalog_path() -> Result<PathBuf, String> {
    let app_data = env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| "Windows APPDATA is unavailable.".to_owned())?;
    Ok(app_data.join(CATALOG_RELATIVE_PATH))
}

pub(crate) fn open_catalog(path: &Path) -> Result<Connection, String> {
    if !path.is_file() {
        return Err(format!("Music catalog was not found at {}", path.display()));
    }

    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY
        | OpenFlags::SQLITE_OPEN_URI
        | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = Connection::open_with_flags(path, flags)
        .map_err(|error| format!("Could not open the music catalog read-only: {error}"))?;
    connection
        .busy_timeout(Duration::from_secs(2))
        .map_err(|error| format!("Could not configure the read-only catalog: {error}"))?;
    connection
        .pragma_update(None, "query_only", true)
        .map_err(|error| format!("Could not enforce read-only catalog access: {error}"))?;
    Ok(connection)
}

fn map_track_row(row: &Row<'_>) -> rusqlite::Result<TrackSummary> {
    let rating: Option<i64> = row.get(5)?;
    let love: Option<String> = row.get(6)?;
    let id: i64 = row.get(0)?;
    Ok(TrackSummary {
        id: id.to_string(),
        title: row
            .get::<_, Option<String>>(1)?
            .unwrap_or_else(|| "Untitled".to_owned()),
        artist: row
            .get::<_, Option<String>>(2)?
            .unwrap_or_else(|| "Unknown Artist".to_owned()),
        album: row
            .get::<_, Option<String>>(3)?
            .unwrap_or_else(|| "Unknown Album".to_owned()),
        release_year: row.get(4)?,
        rating: rating.map(|value| value as f64 / 20.0),
        loved: love.as_deref() == Some("L"),
        duration_seconds: row.get(7)?,
        genre: row.get(8)?,
        play_count: row.get(9)?,
        album_id: row.get(10)?,
    })
}

pub(crate) fn query_snapshot(
    connection: &Connection,
    source_path: String,
) -> Result<LibrarySnapshot, String> {
    let summary = connection
        .query_row(
            r#"
            SELECT
              (SELECT COUNT(*) FROM tracks),
              (SELECT COUNT(*) FROM albums),
              (SELECT COUNT(DISTINCT COALESCE(NULLIF(TRIM(album_artist_display), ''), 'unknown')) FROM albums),
              (SELECT COUNT(DISTINCT COALESCE(NULLIF(TRIM(canonical_genre), ''), 'unknown')) FROM albums),
              (SELECT COUNT(*) FROM tracks WHERE love = 'L'),
              (SELECT COUNT(*) FROM tracks WHERE normalized_rating IS NOT NULL)
            "#,
            [],
            |row| {
                Ok(LibrarySummary {
                    songs: row.get(0)?,
                    albums: row.get(1)?,
                    artists: row.get(2)?,
                    genres: row.get(3)?,
                    loved: row.get(4)?,
                    rated: row.get(5)?,
                })
            },
        )
        .map_err(|error| format!("Could not read the library overview: {error}"))?;

    let mut artist_statement = connection
        .prepare_cached(
            r#"
            WITH artist_rollup AS (
              SELECT
                COALESCE(NULLIF(TRIM(album_artist_display), ''), 'Unknown Artist') AS display_name,
                SUM(total_tracks) AS track_count,
                COUNT(*) AS album_count
              FROM albums
              WHERE NULLIF(TRIM(album_artist_display), '') IS NOT NULL
                AND album_artist_display <> 'Various Artists'
              GROUP BY COALESCE(NULLIF(TRIM(album_artist_display), ''), 'Unknown Artist')
            )
            SELECT display_name, CAST(track_count AS INTEGER), album_count
            FROM artist_rollup
            ORDER BY track_count DESC, display_name COLLATE NOCASE
            LIMIT 8
            "#,
        )
        .map_err(|error| format!("Could not prepare the artist universe: {error}"))?;
    let artists = artist_statement
        .query_map([], |row| {
            let name: String = row.get(0)?;
            Ok(ArtistSummary {
                id: name.clone(),
                name,
                track_count: row.get(1)?,
                album_count: row.get(2)?,
                play_count: None,
            })
        })
        .map_err(|error| format!("Could not read the artist universe: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode the artist universe: {error}"))?;

    let tracks = query_tracks(
        connection,
        r#"
        WITH page AS MATERIALIZED (
          SELECT id, title, album_artist_display, album, release_year,
                 normalized_rating, love, time_seconds, canonical_genre, album_id
          FROM tracks
          WHERE normalized_rating = 100
          ORDER BY id DESC
          LIMIT 50
        )
        SELECT p.id, p.title, p.album_artist_display, p.album, p.release_year,
               p.normalized_rating, p.love, p.time_seconds, p.canonical_genre,
               l.play_count, p.album_id
        FROM page AS p
        LEFT JOIN lastfm_track_popularity AS l
          ON l.artist_key = lower(trim(p.album_artist_display))
         AND l.track_key = lower(trim(p.title))
        ORDER BY p.id DESC
        "#,
        [],
        "first track page",
    )?;

    Ok(LibrarySnapshot {
        source_state: "connected",
        source_label: "Live catalog · read only",
        source_path,
        summary,
        artists,
        tracks,
    })
}

fn query_tracks<P: rusqlite::Params>(
    connection: &Connection,
    sql: &str,
    params: P,
    label: &str,
) -> Result<Vec<TrackSummary>, String> {
    let mut statement = connection
        .prepare(sql)
        .map_err(|error| format!("Could not prepare the {label}: {error}"))?;
    statement
        .query_map(params, map_track_row)
        .map_err(|error| format!("Could not read the {label}: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode the {label}: {error}"))
}

pub(crate) fn load_default_snapshot() -> Result<LibrarySnapshot, String> {
    let path = default_catalog_path()?;
    let connection = open_catalog(&path)?;
    query_snapshot(&connection, path.to_string_lossy().into_owned())
}

pub(crate) fn load_artist_tracks(artist: String) -> Result<Vec<TrackSummary>, String> {
    if artist.trim().is_empty() || artist.chars().count() > 256 {
        return Err("Artist selection is invalid.".to_owned());
    }
    let path = default_catalog_path()?;
    let connection = open_catalog(&path)?;
    query_tracks(
        &connection,
        r#"
        WITH page AS MATERIALIZED (
          SELECT id, title, album_artist_display, album, release_year,
                 normalized_rating, love, time_seconds, canonical_genre, album_id
          FROM tracks
          WHERE album_artist_display = :artist COLLATE NOCASE
          ORDER BY normalized_rating DESC, title COLLATE NOCASE ASC, id ASC
          LIMIT 50
        )
        SELECT p.id, p.title, p.album_artist_display, p.album, p.release_year,
               p.normalized_rating, p.love, p.time_seconds, p.canonical_genre,
               l.play_count, p.album_id
        FROM page AS p
        LEFT JOIN lastfm_track_popularity AS l
          ON l.artist_key = lower(trim(p.album_artist_display))
         AND l.track_key = lower(trim(p.title))
        ORDER BY p.normalized_rating DESC, p.title COLLATE NOCASE ASC, p.id ASC
        "#,
        named_params! { ":artist": artist },
        "artist track page",
    )
}

pub(crate) fn build_fts_prefix_query(input: &str) -> Option<String> {
    let terms: Vec<String> = input
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| !term.is_empty())
        .take(8)
        .map(|term| {
            let bounded: String = term.chars().take(64).collect();
            let escaped = bounded.replace('"', "\"\"");
            format!("\"{escaped}\"*")
        })
        .collect();
    (!terms.is_empty()).then(|| terms.join(" AND "))
}

pub(crate) fn load_search_tracks(query: String) -> Result<Vec<TrackSummary>, String> {
    if query.chars().count() > 512 {
        return Err("Search text is too long.".to_owned());
    }
    let Some(match_query) = build_fts_prefix_query(&query) else {
        return Ok(Vec::new());
    };
    let path = default_catalog_path()?;
    let connection = open_catalog(&path)?;
    query_tracks(
        &connection,
        r#"
        WITH hits AS MATERIALIZED (
          SELECT CAST(track_id AS INTEGER) AS id, bm25(track_search_fts) AS relevance
          FROM track_search_fts
          WHERE track_search_fts MATCH :match_query
          ORDER BY relevance
          LIMIT 50
        )
        SELECT t.id, t.title, t.album_artist_display, t.album, t.release_year,
               t.normalized_rating, t.love, t.time_seconds, t.canonical_genre,
               l.play_count, t.album_id
        FROM hits AS h
        JOIN tracks AS t ON t.id = h.id
        LEFT JOIN lastfm_track_popularity AS l
          ON l.artist_key = lower(trim(t.album_artist_display))
         AND l.track_key = lower(trim(t.title))
        ORDER BY h.relevance, t.id
        "#,
        named_params! { ":match_query": match_query },
        "catalog search",
    )
}

fn parse_track_id(track_id: &str) -> Result<i64, String> {
    if track_id.is_empty()
        || track_id.len() > 24
        || !track_id.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err("Track identity is invalid.".to_owned());
    }
    track_id
        .parse::<i64>()
        .map_err(|_| "Track identity is invalid.".to_owned())
}

pub(crate) fn load_tracks_by_ids(track_ids: &[String]) -> Result<Vec<TrackSummary>, String> {
    if track_ids.is_empty() || track_ids.len() > 200 {
        return Err("A playback queue must contain between 1 and 200 tracks.".to_owned());
    }
    let path = default_catalog_path()?;
    let connection = open_catalog(&path)?;
    let mut statement = connection
        .prepare_cached(
            r#"
            SELECT t.id, t.title, t.album_artist_display, t.album, t.release_year,
                   t.normalized_rating, t.love, t.time_seconds, t.canonical_genre,
                   l.play_count, t.album_id
            FROM tracks AS t
            LEFT JOIN lastfm_track_popularity AS l
              ON l.artist_key = lower(trim(t.album_artist_display))
             AND l.track_key = lower(trim(t.title))
            WHERE t.id = :track_id
            "#,
        )
        .map_err(|error| format!("Could not prepare the playback queue: {error}"))?;

    track_ids
        .iter()
        .map(|track_id| {
            let parsed = parse_track_id(track_id)?;
            statement
                .query_row(named_params! { ":track_id": parsed }, map_track_row)
                .map_err(|error| format!("Track {track_id} is no longer available: {error}"))
        })
        .collect()
}

pub(crate) fn resolve_audio_path(track_id: &str) -> Result<PathBuf, String> {
    let parsed = parse_track_id(track_id)?;
    let path = default_catalog_path()?;
    let connection = open_catalog(&path)?;
    let (directory, filename): (String, String) = connection
        .query_row(
            "SELECT file_path, filename FROM tracks WHERE id = :track_id",
            named_params! { ":track_id": parsed },
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| format!("Could not resolve this track from the catalog: {error}"))?;

    let filename_path = Path::new(&filename);
    if filename_path.is_absolute()
        || filename_path.components().count() != 1
        || !matches!(
            filename_path.components().next(),
            Some(Component::Normal(_))
        )
    {
        return Err("The catalog contains an unsafe audio filename.".to_owned());
    }
    let audio_path = PathBuf::from(directory).join(filename_path);
    let is_mp3 = audio_path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mp3"));
    if !is_mp3 || !audio_path.is_file() {
        return Err("The MP3 file is unavailable at its catalog location.".to_owned());
    }
    Ok(audio_path)
}

pub(crate) fn resolve_cover_path(album_id: &str) -> Result<PathBuf, String> {
    if album_id.trim().is_empty() || album_id.chars().count() > 512 {
        return Err("Album identity is invalid.".to_owned());
    }
    let path = default_catalog_path()?;
    let connection = open_catalog(&path)?;
    let cover_path: String = connection
        .query_row(
            "SELECT cache_path FROM album_covers WHERE album_id = :album_id AND file_size_bytes > 0",
            named_params! { ":album_id": album_id },
            |row| row.get(0),
        )
        .map_err(|_| "No album cover is available.".to_owned())?;

    let root = std::fs::canonicalize(COVER_ROOT)
        .map_err(|_| "The album-cover archive is unavailable.".to_owned())?;
    let candidate = std::fs::canonicalize(&cover_path)
        .map_err(|_| "The album cover is unavailable.".to_owned())?;
    if !candidate.starts_with(&root) || !candidate.is_file() {
        return Err("The album cover resolved outside the configured archive.".to_owned());
    }
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_maps_musicbee_values_without_writing() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(
                r#"
                CREATE TABLE tracks (
                  id INTEGER PRIMARY KEY, album_id TEXT, title TEXT, album_artist_display TEXT,
                  album TEXT, release_year INTEGER, normalized_rating INTEGER, love TEXT,
                  time_seconds INTEGER, canonical_genre TEXT
                );
                CREATE TABLE albums (
                  album_artist_display TEXT, canonical_genre TEXT, total_tracks INTEGER NOT NULL
                );
                CREATE TABLE lastfm_track_popularity (
                  artist_key TEXT, track_key TEXT, play_count INTEGER,
                  PRIMARY KEY (artist_key, track_key)
                );
                INSERT INTO albums VALUES ('Sigur Rós', 'Post-rock', 1);
                INSERT INTO tracks VALUES (7, 'album-7', 'Sæglópur', 'Sigur Rós', 'Takk...', 2005, 100, 'L', 473, 'Post-rock');
                INSERT INTO lastfm_track_popularity VALUES ('sigur rós', 'sæglópur', 42);
                "#,
            )
            .expect("fixture schema");

        let snapshot = query_snapshot(&connection, "fixture.sqlite3".to_owned()).expect("snapshot");

        assert_eq!(snapshot.summary.songs, 1);
        assert_eq!(snapshot.summary.loved, 1);
        assert_eq!(snapshot.artists[0].name, "Sigur Rós");
        assert_eq!(snapshot.tracks[0].album_id.as_deref(), Some("album-7"));
        assert_eq!(snapshot.tracks[0].rating, Some(5.0));
        assert!(snapshot.tracks[0].loved);
        assert_eq!(snapshot.tracks[0].play_count, Some(42));
    }

    #[test]
    fn fts_query_quotes_and_bounds_user_terms() {
        assert_eq!(
            build_fts_prefix_query("white ner OR star"),
            Some("\"white\"* AND \"ner\"* AND \"OR\"* AND \"star\"*".to_owned())
        );
        assert_eq!(build_fts_prefix_query("///"), None);
    }

    #[test]
    fn track_ids_cannot_smuggle_paths_or_sql() {
        assert_eq!(parse_track_id("42").expect("valid identity"), 42);
        assert!(parse_track_id("../42").is_err());
        assert!(parse_track_id("42 OR 1=1").is_err());
    }
}
