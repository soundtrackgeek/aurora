use rusqlite::{Connection, OpenFlags, Row, named_params};
use serde::Serialize;
use std::{
    env,
    path::{Path, PathBuf},
    time::Duration,
};

const CATALOG_RELATIVE_PATH: &str = "com.local.musiclibrary\\music-library.sqlite3";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LibrarySummary {
    songs: i64,
    albums: i64,
    artists: i64,
    genres: i64,
    loved: i64,
    rated: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtistSummary {
    id: String,
    name: String,
    track_count: i64,
    album_count: i64,
    play_count: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrackSummary {
    id: String,
    title: String,
    artist: String,
    album: String,
    release_year: Option<i64>,
    rating: Option<f64>,
    loved: bool,
    duration_seconds: Option<i64>,
    genre: Option<String>,
    play_count: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LibrarySnapshot {
    source_state: &'static str,
    source_label: &'static str,
    source_path: String,
    summary: LibrarySummary,
    artists: Vec<ArtistSummary>,
    tracks: Vec<TrackSummary>,
}

fn default_catalog_path() -> Result<PathBuf, String> {
    let app_data = env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| "Windows APPDATA is unavailable.".to_owned())?;
    Ok(app_data.join(CATALOG_RELATIVE_PATH))
}

fn open_catalog(path: &Path) -> Result<Connection, String> {
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
    })
}

fn query_snapshot(connection: &Connection, source_path: String) -> Result<LibrarySnapshot, String> {
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

    let mut track_statement = connection
        .prepare_cached(
            r#"
            WITH page AS MATERIALIZED (
              SELECT id, title, album_artist_display, album, release_year,
                     normalized_rating, love, time_seconds, canonical_genre
              FROM tracks
              WHERE normalized_rating = 100
              ORDER BY id DESC
              LIMIT 50
            )
            SELECT p.id, p.title, p.album_artist_display, p.album, p.release_year,
                   p.normalized_rating, p.love, p.time_seconds, p.canonical_genre,
                   l.play_count
            FROM page AS p
            LEFT JOIN lastfm_track_popularity AS l
              ON l.artist_key = lower(trim(p.album_artist_display))
             AND l.track_key = lower(trim(p.title))
            ORDER BY p.id DESC
            "#,
        )
        .map_err(|error| format!("Could not prepare the first track page: {error}"))?;
    let tracks = track_statement
        .query_map([], map_track_row)
        .map_err(|error| format!("Could not read the first track page: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode the first track page: {error}"))?;

    Ok(LibrarySnapshot {
        source_state: "connected",
        source_label: "Live catalog · read only",
        source_path,
        summary,
        artists,
        tracks,
    })
}

fn load_default_snapshot() -> Result<LibrarySnapshot, String> {
    let path = default_catalog_path()?;
    let connection = open_catalog(&path)?;
    query_snapshot(&connection, path.to_string_lossy().into_owned())
}

fn load_artist_tracks(artist: String) -> Result<Vec<TrackSummary>, String> {
    if artist.trim().is_empty() || artist.chars().count() > 256 {
        return Err("Artist selection is invalid.".to_owned());
    }
    let path = default_catalog_path()?;
    let connection = open_catalog(&path)?;
    let mut statement = connection
        .prepare_cached(
            r#"
            WITH page AS MATERIALIZED (
              SELECT id, title, album_artist_display, album, release_year,
                     normalized_rating, love, time_seconds, canonical_genre
              FROM tracks
              WHERE album_artist_display = :artist COLLATE NOCASE
              ORDER BY normalized_rating DESC, title COLLATE NOCASE ASC, id ASC
              LIMIT 50
            )
            SELECT p.id, p.title, p.album_artist_display, p.album, p.release_year,
                   p.normalized_rating, p.love, p.time_seconds, p.canonical_genre,
                   l.play_count
            FROM page AS p
            LEFT JOIN lastfm_track_popularity AS l
              ON l.artist_key = lower(trim(p.album_artist_display))
             AND l.track_key = lower(trim(p.title))
            ORDER BY p.normalized_rating DESC, p.title COLLATE NOCASE ASC, p.id ASC
            "#,
        )
        .map_err(|error| format!("Could not prepare the artist track page: {error}"))?;
    statement
        .query_map(named_params! { ":artist": artist }, map_track_row)
        .map_err(|error| format!("Could not read tracks for this artist: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode tracks for this artist: {error}"))
}

fn build_fts_prefix_query(input: &str) -> Option<String> {
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

fn load_search_tracks(query: String) -> Result<Vec<TrackSummary>, String> {
    if query.chars().count() > 512 {
        return Err("Search text is too long.".to_owned());
    }
    let Some(match_query) = build_fts_prefix_query(&query) else {
        return Ok(Vec::new());
    };
    let path = default_catalog_path()?;
    let connection = open_catalog(&path)?;
    let mut statement = connection
        .prepare_cached(
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
                   l.play_count
            FROM hits AS h
            JOIN tracks AS t ON t.id = h.id
            LEFT JOIN lastfm_track_popularity AS l
              ON l.artist_key = lower(trim(t.album_artist_display))
             AND l.track_key = lower(trim(t.title))
            ORDER BY h.relevance, t.id
            "#,
        )
        .map_err(|error| format!("Could not prepare the catalog search: {error}"))?;
    statement
        .query_map(named_params! { ":match_query": match_query }, map_track_row)
        .map_err(|error| format!("Could not search the music catalog: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode catalog search results: {error}"))
}

#[tauri::command]
async fn library_snapshot() -> Result<LibrarySnapshot, String> {
    tauri::async_runtime::spawn_blocking(load_default_snapshot)
        .await
        .map_err(|error| format!("The catalog worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn artist_tracks(artist: String) -> Result<Vec<TrackSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || load_artist_tracks(artist))
        .await
        .map_err(|error| format!("The artist worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn search_tracks(query: String) -> Result<Vec<TrackSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || load_search_tracks(query))
        .await
        .map_err(|error| format!("The search worker stopped unexpectedly: {error}"))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            library_snapshot,
            artist_tracks,
            search_tracks
        ])
        .run(tauri::generate_context!())
        .expect("error while running Aurora");
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
                  id INTEGER PRIMARY KEY, title TEXT, album_artist_display TEXT, album TEXT,
                  release_year INTEGER, normalized_rating INTEGER, love TEXT,
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
                INSERT INTO tracks VALUES (7, 'Sæglópur', 'Sigur Rós', 'Takk...', 2005, 100, 'L', 473, 'Post-rock');
                INSERT INTO lastfm_track_popularity VALUES ('sigur rós', 'sæglópur', 42);
                "#,
            )
            .expect("fixture schema");

        let snapshot = query_snapshot(&connection, "fixture.sqlite3".to_owned()).expect("snapshot");

        assert_eq!(snapshot.summary.songs, 1);
        assert_eq!(snapshot.summary.loved, 1);
        assert_eq!(snapshot.artists[0].name, "Sigur Rós");
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
}
