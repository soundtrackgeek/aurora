use crate::{
    device_mode,
    state_store::{StateStore, StoredQueueEntry},
    tag_model::{LoveState, TagSyncState, TagValues},
};
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, Row, named_params, params, params_from_iter,
    types::Value,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
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
    pub(crate) track_key: String,
    pub(crate) album_id: Option<String>,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) album: String,
    pub(crate) release_year: Option<i64>,
    pub(crate) rating: Option<f64>,
    pub(crate) loved: bool,
    pub(crate) love_state: LoveState,
    pub(crate) tag_sync_state: Option<TagSyncState>,
    pub(crate) can_undo_tag_edit: bool,
    pub(crate) duration_seconds: Option<i64>,
    pub(crate) genre: Option<String>,
    pub(crate) play_count: Option<i64>,
    #[serde(skip)]
    pub(crate) directory: String,
    #[serde(skip)]
    pub(crate) filename: String,
    #[serde(skip)]
    pub(crate) catalog_import_run_id: i64,
}

impl TrackSummary {
    pub(crate) fn catalog_tag_values(&self) -> TagValues {
        TagValues {
            rating: self.rating,
            love_state: self.love_state,
            release_year: self.release_year.and_then(|year| i32::try_from(year).ok()),
        }
    }

    pub(crate) fn apply_tag_values(&mut self, values: &TagValues, pending_import: bool) {
        self.rating = values.rating;
        self.love_state = values.love_state;
        self.loved = values.love_state == LoveState::Loved;
        self.release_year = values.release_year.map(i64::from);
        self.tag_sync_state = pending_import.then_some(TagSyncState::PendingImport);
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedTrack {
    pub(crate) summary: TrackSummary,
    pub(crate) audio_path: PathBuf,
    pub(crate) catalog_values: TagValues,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrackReference {
    pub(crate) id: String,
    pub(crate) track_key: String,
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

pub(crate) fn map_track_row(row: &Row<'_>) -> rusqlite::Result<TrackSummary> {
    let rating: Option<i64> = row.get(5)?;
    let love: Option<String> = row.get(6)?;
    let id: i64 = row.get(0)?;
    let directory: String = row.get(11)?;
    let filename: String = row.get(12)?;
    let love_state = LoveState::from_catalog(love.as_deref());
    Ok(TrackSummary {
        id: id.to_string(),
        track_key: normalize_track_key(&directory, &filename),
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
        loved: love_state == LoveState::Loved,
        love_state,
        tag_sync_state: None,
        can_undo_tag_edit: false,
        duration_seconds: row.get(7)?,
        genre: row.get(8)?,
        play_count: row.get(9)?,
        album_id: row.get(10)?,
        directory,
        filename,
        catalog_import_run_id: row.get(13)?,
    })
}

pub(crate) fn normalize_track_key(directory: &str, filename: &str) -> String {
    let directory = directory.trim().replace('/', "\\");
    let directory = directory.strip_prefix("\\\\?\\").unwrap_or(&directory);
    format!("{}\\{}", directory.trim_end_matches('\\'), filename.trim()).to_lowercase()
}

pub(crate) fn apply_overlays(
    tracks: &mut [TrackSummary],
    store: Option<&StateStore>,
) -> Result<(), String> {
    let Some(store) = store else {
        return Ok(());
    };
    let keys = tracks
        .iter()
        .map(|track| track.track_key.clone())
        .collect::<Vec<_>>();
    let overlays = store
        .overlays_for_keys(&keys)?
        .into_iter()
        .map(|overlay| (overlay.track_key.clone(), overlay))
        .collect::<HashMap<_, _>>();
    let undoable = store.undoable_keys(&keys)?;
    for track in tracks {
        let catalog_values = track.catalog_tag_values();
        track.can_undo_tag_edit = undoable.contains(&track.track_key);
        if let Some(overlay) = overlays.get(&track.track_key) {
            store.upsert_overlay(
                &track.track_key,
                &track.directory,
                &track.filename,
                &catalog_values,
                &overlay.values,
                track.catalog_import_run_id,
                overlay.last_operation_id,
            )?;
            if overlay.values != catalog_values {
                track.apply_tag_values(&overlay.values, true);
            }
        }
    }
    Ok(())
}

fn reconcile_all_overlays(connection: &Connection, store: &StateStore) -> Result<(), String> {
    let mut statement = connection
        .prepare_cached(
            r#"
            SELECT COALESCE(normalized_rating, CASE trim(rating_raw)
                     WHEN '0.5' THEN 10 WHEN '1' THEN 20 WHEN '1.0' THEN 20
                     WHEN '1.5' THEN 30 WHEN '2' THEN 40 WHEN '2.0' THEN 40
                     WHEN '2.5' THEN 50 WHEN '3' THEN 60 WHEN '3.0' THEN 60
                     WHEN '3.5' THEN 70 WHEN '4' THEN 80 WHEN '4.0' THEN 80
                     WHEN '4.5' THEN 90 WHEN '5' THEN 100 WHEN '5.0' THEN 100 END),
                   love, release_year, import_run_id
            FROM tracks WHERE file_path = ?1 AND filename = ?2
            "#,
        )
        .map_err(|error| format!("Could not prepare tag-overlay reconciliation: {error}"))?;
    for overlay in store.all_overlays()? {
        let catalog = statement
            .query_row(params![overlay.directory, overlay.filename], |row| {
                let rating: Option<i64> = row.get(0)?;
                let love: Option<String> = row.get(1)?;
                Ok((
                    TagValues {
                        rating: rating.map(|value| value as f64 / 20.0),
                        love_state: LoveState::from_catalog(love.as_deref()),
                        release_year: row.get(2)?,
                    },
                    row.get::<_, i64>(3)?,
                ))
            })
            .optional()
            .map_err(|error| format!("Could not reconcile a pending MP3 edit: {error}"))?;
        if let Some((catalog_values, import_run_id)) = catalog {
            store.upsert_overlay(
                &overlay.track_key,
                &overlay.directory,
                &overlay.filename,
                &catalog_values,
                &overlay.values,
                import_run_id,
                overlay.last_operation_id,
            )?;
        }
    }
    Ok(())
}

pub(crate) fn query_snapshot(
    connection: &Connection,
    source_path: String,
    store: Option<&StateStore>,
) -> Result<LibrarySnapshot, String> {
    if let Some(store) = store {
        reconcile_all_overlays(connection, store)?;
    }
    let current_import_run_id = connection
        .query_row(
            "SELECT COALESCE(MAX(id), 0) FROM import_runs WHERE status = 'completed'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("Could not identify the current library import: {error}"))?;
    let mut summary = connection
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
                 COALESCE(normalized_rating, CASE trim(rating_raw)
                   WHEN '0.5' THEN 10 WHEN '1' THEN 20 WHEN '1.0' THEN 20
                   WHEN '1.5' THEN 30 WHEN '2' THEN 40 WHEN '2.0' THEN 40
                   WHEN '2.5' THEN 50 WHEN '3' THEN 60 WHEN '3.0' THEN 60
                   WHEN '3.5' THEN 70 WHEN '4' THEN 80 WHEN '4.0' THEN 80
                   WHEN '4.5' THEN 90 WHEN '5' THEN 100 WHEN '5.0' THEN 100 END) AS rating_value,
                 love, time_seconds, canonical_genre, album_id,
                 file_path, filename, import_run_id
          FROM tracks
          WHERE normalized_rating = 100
          ORDER BY id DESC
          LIMIT 50
        )
        SELECT p.id, p.title, p.album_artist_display, p.album, p.release_year,
               p.rating_value, p.love, p.time_seconds, p.canonical_genre,
               l.play_count, p.album_id, p.file_path, p.filename, p.import_run_id
        FROM page AS p
        LEFT JOIN lastfm_track_popularity AS l
          ON l.artist_key = lower(trim(p.album_artist_display))
         AND l.track_key = lower(trim(p.title))
        ORDER BY p.id DESC
        "#,
        [],
        "first track page",
        store,
    )?;

    if let Some(store) = store {
        let (loved_delta, rated_delta) = store.overlay_summary_deltas(current_import_run_id)?;
        summary.loved = (summary.loved + loved_delta).max(0);
        summary.rated = (summary.rated + rated_delta).max(0);
    }

    Ok(LibrarySnapshot {
        source_state: "connected",
        source_label: "Live catalog · file-tag overlay",
        source_path,
        summary,
        artists,
        tracks,
    })
}

pub(crate) fn query_tracks<P: rusqlite::Params>(
    connection: &Connection,
    sql: &str,
    params: P,
    label: &str,
    store: Option<&StateStore>,
) -> Result<Vec<TrackSummary>, String> {
    let mut statement = connection
        .prepare(sql)
        .map_err(|error| format!("Could not prepare the {label}: {error}"))?;
    let mut tracks = statement
        .query_map(params, map_track_row)
        .map_err(|error| format!("Could not read the {label}: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode the {label}: {error}"))?;
    apply_overlays(&mut tracks, store)?;
    Ok(tracks)
}

pub(crate) fn load_default_snapshot(store: &StateStore) -> Result<LibrarySnapshot, String> {
    let path = default_catalog_path()?;
    let connection = open_catalog(&path)?;
    query_snapshot(
        &connection,
        path.to_string_lossy().into_owned(),
        Some(store),
    )
}

pub(crate) fn load_artist_tracks(
    artist: String,
    store: &StateStore,
) -> Result<Vec<TrackSummary>, String> {
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
                 COALESCE(normalized_rating, CASE trim(rating_raw)
                   WHEN '0.5' THEN 10 WHEN '1' THEN 20 WHEN '1.0' THEN 20
                   WHEN '1.5' THEN 30 WHEN '2' THEN 40 WHEN '2.0' THEN 40
                   WHEN '2.5' THEN 50 WHEN '3' THEN 60 WHEN '3.0' THEN 60
                   WHEN '3.5' THEN 70 WHEN '4' THEN 80 WHEN '4.0' THEN 80
                   WHEN '4.5' THEN 90 WHEN '5' THEN 100 WHEN '5.0' THEN 100 END) AS rating_value,
                 love, time_seconds, canonical_genre, album_id,
                 file_path, filename, import_run_id
          FROM tracks
          WHERE album_artist_display = :artist COLLATE NOCASE
          ORDER BY rating_value DESC, title COLLATE NOCASE ASC, id ASC
          LIMIT 50
        )
        SELECT p.id, p.title, p.album_artist_display, p.album, p.release_year,
               p.rating_value, p.love, p.time_seconds, p.canonical_genre,
               l.play_count, p.album_id, p.file_path, p.filename, p.import_run_id
        FROM page AS p
        LEFT JOIN lastfm_track_popularity AS l
          ON l.artist_key = lower(trim(p.album_artist_display))
         AND l.track_key = lower(trim(p.title))
        ORDER BY p.rating_value DESC, p.title COLLATE NOCASE ASC, p.id ASC
        "#,
        named_params! { ":artist": artist },
        "artist track page",
        Some(store),
    )
}

const MAX_FTS_SEARCH_TERMS: usize = 32;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CatalogSearch {
    pub(crate) fts_query: Option<String>,
    pub(crate) year: Option<i32>,
    pub(crate) release_year: Option<i32>,
    pub(crate) has_fields: bool,
}

impl CatalogSearch {
    pub(crate) fn is_empty(&self) -> bool {
        self.fts_query.is_none() && self.year.is_none() && self.release_year.is_none()
    }
}

fn push_fts_prefix_terms(terms: &mut Vec<String>, input: &str, column: Option<&str>) {
    for term in input
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| !term.is_empty())
    {
        if terms.len() >= MAX_FTS_SEARCH_TERMS {
            break;
        }
        let bounded: String = term.chars().take(64).collect();
        let escaped = bounded.replace('"', "\"\"");
        let prefix = format!("\"{escaped}\"*");
        terms.push(match column {
            Some(column) => format!("{column} : {prefix}"),
            None => prefix,
        });
    }
}

fn parse_search_year(value: &str, field: &str) -> Result<i32, String> {
    let year = value
        .trim()
        .parse::<i32>()
        .map_err(|_| format!("{field} must be a year between 1000 and 2999."))?;
    if !(1000..=2999).contains(&year) {
        return Err(format!("{field} must be a year between 1000 and 2999."));
    }
    Ok(year)
}

fn set_search_year(target: &mut Option<i32>, year: i32, field: &str) -> Result<(), String> {
    if target.is_some_and(|current| current != year) {
        return Err(format!("{field} cannot contain two different years."));
    }
    *target = Some(year);
    Ok(())
}

pub(crate) fn parse_catalog_search(input: &str) -> Result<CatalogSearch, String> {
    let mut terms = Vec::new();
    let mut search = CatalogSearch::default();

    for clause in input
        .split(',')
        .map(str::trim)
        .filter(|clause| !clause.is_empty())
    {
        let Some((raw_field, raw_value)) = clause.split_once(':') else {
            push_fts_prefix_terms(&mut terms, clause, None);
            continue;
        };
        let field = raw_field.trim().to_ascii_lowercase();
        let column = match field.as_str() {
            "artist" => Some("display_artist"),
            "aartist" => Some("album_artist_display"),
            "album" => Some("album"),
            "genre" => Some("canonical_genre"),
            "publisher" => Some("publisher"),
            "title" => Some("title"),
            _ => None,
        };
        if let Some(column) = column {
            search.has_fields = true;
            if raw_value.trim().is_empty() {
                return Err(format!("{field} needs a search value."));
            }
            push_fts_prefix_terms(&mut terms, raw_value, Some(column));
            continue;
        }
        match field.as_str() {
            "year" => {
                search.has_fields = true;
                let year = parse_search_year(raw_value, "year")?;
                set_search_year(&mut search.year, year, "year")?;
            }
            "ryear" => {
                search.has_fields = true;
                let year = parse_search_year(raw_value, "ryear")?;
                set_search_year(&mut search.release_year, year, "ryear")?;
            }
            _ => push_fts_prefix_terms(&mut terms, clause, None),
        }
    }

    search.fts_query = (!terms.is_empty()).then(|| terms.join(" AND "));
    Ok(search)
}

pub(crate) fn load_search_tracks(
    query: String,
    store: &StateStore,
) -> Result<Vec<TrackSummary>, String> {
    if query.chars().count() > 512 {
        return Err("Search text is too long.".to_owned());
    }
    let search = parse_catalog_search(&query)?;
    if search.is_empty() {
        return Ok(Vec::new());
    }
    let path = default_catalog_path()?;
    let connection = open_catalog(&path)?;
    let mut params = Vec::<Value>::new();
    let has_fts = search.fts_query.is_some();
    let mut sql = String::from(
        r#"
        SELECT t.id, t.title, t.album_artist_display, t.album, t.release_year,
               COALESCE(t.normalized_rating, CASE trim(t.rating_raw)
                 WHEN '0.5' THEN 10 WHEN '1' THEN 20 WHEN '1.0' THEN 20
                 WHEN '1.5' THEN 30 WHEN '2' THEN 40 WHEN '2.0' THEN 40
                 WHEN '2.5' THEN 50 WHEN '3' THEN 60 WHEN '3.0' THEN 60
                 WHEN '3.5' THEN 70 WHEN '4' THEN 80 WHEN '4.0' THEN 80
                 WHEN '4.5' THEN 90 WHEN '5' THEN 100 WHEN '5.0' THEN 100 END),
               t.love, t.time_seconds, t.canonical_genre,
               l.play_count, t.album_id, t.file_path, t.filename, t.import_run_id
        FROM tracks AS t
        "#,
    );
    if has_fts {
        sql.push_str(" JOIN track_search_fts ON CAST(track_search_fts.track_id AS INTEGER) = t.id");
    }
    sql.push_str(
        " LEFT JOIN lastfm_track_popularity AS l ON l.artist_key = lower(trim(t.album_artist_display)) AND l.track_key = lower(trim(t.title)) WHERE 1 = 1",
    );
    if let Some(match_query) = search.fts_query {
        sql.push_str(" AND track_search_fts MATCH ?");
        params.push(Value::Text(match_query));
    }
    if let Some(year) = search.year {
        sql.push_str(" AND t.year = ?");
        params.push(Value::Integer(i64::from(year)));
    }
    if let Some(year) = search.release_year {
        sql.push_str(" AND t.release_year = ?");
        params.push(Value::Integer(i64::from(year)));
    }
    sql.push_str(if has_fts {
        " ORDER BY bm25(track_search_fts), t.id LIMIT 50"
    } else {
        " ORDER BY t.id DESC LIMIT 50"
    });
    query_tracks(
        &connection,
        &sql,
        params_from_iter(params.iter()),
        "catalog search",
        Some(store),
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

pub(crate) fn load_tracks_by_ids(
    track_references: &[TrackReference],
    store: &StateStore,
) -> Result<Vec<TrackSummary>, String> {
    if track_references.is_empty() || track_references.len() > 200 {
        return Err("A playback queue must contain between 1 and 200 tracks.".to_owned());
    }
    let path = default_catalog_path()?;
    let connection = open_catalog(&path)?;
    let mut statement = connection
        .prepare_cached(
            r#"
            SELECT t.id, t.title, t.album_artist_display, t.album, t.release_year,
                   COALESCE(t.normalized_rating, CASE trim(t.rating_raw)
                     WHEN '0.5' THEN 10 WHEN '1' THEN 20 WHEN '1.0' THEN 20
                     WHEN '1.5' THEN 30 WHEN '2' THEN 40 WHEN '2.0' THEN 40
                     WHEN '2.5' THEN 50 WHEN '3' THEN 60 WHEN '3.0' THEN 60
                     WHEN '3.5' THEN 70 WHEN '4' THEN 80 WHEN '4.0' THEN 80
                     WHEN '4.5' THEN 90 WHEN '5' THEN 100 WHEN '5.0' THEN 100 END),
                   t.love, t.time_seconds, t.canonical_genre,
                   l.play_count, t.album_id, t.file_path, t.filename, t.import_run_id
            FROM tracks AS t
            LEFT JOIN lastfm_track_popularity AS l
              ON l.artist_key = lower(trim(t.album_artist_display))
             AND l.track_key = lower(trim(t.title))
            WHERE t.id = :track_id
            "#,
        )
        .map_err(|error| format!("Could not prepare the playback queue: {error}"))?;

    let mut tracks = track_references
        .iter()
        .map(|reference| {
            let parsed = parse_track_id(&reference.id)?;
            let track = statement
                .query_row(named_params! { ":track_id": parsed }, map_track_row)
                .map_err(|error| {
                    format!("Track {} is no longer available: {error}", reference.id)
                })?;
            verify_track_identity(track, &reference.track_key)
        })
        .collect::<Result<Vec<_>, _>>()?;
    apply_overlays(&mut tracks, Some(store))?;
    Ok(tracks)
}

pub(crate) fn load_tracks_by_references(
    references: &[StoredQueueEntry],
    store: &StateStore,
) -> Result<(Vec<TrackSummary>, usize), String> {
    if references.is_empty() || references.len() > 200 {
        return Err("A playback queue must contain between 1 and 200 tracks.".to_owned());
    }
    let path = default_catalog_path()?;
    let connection = open_catalog(&path)?;
    let mut by_path = connection
        .prepare_cached(
            r#"
            SELECT t.id, t.title, t.album_artist_display, t.album, t.release_year,
                   COALESCE(t.normalized_rating, CASE trim(t.rating_raw)
                     WHEN '0.5' THEN 10 WHEN '1' THEN 20 WHEN '1.0' THEN 20
                     WHEN '1.5' THEN 30 WHEN '2' THEN 40 WHEN '2.0' THEN 40
                     WHEN '2.5' THEN 50 WHEN '3' THEN 60 WHEN '3.0' THEN 60
                     WHEN '3.5' THEN 70 WHEN '4' THEN 80 WHEN '4.0' THEN 80
                     WHEN '4.5' THEN 90 WHEN '5' THEN 100 WHEN '5.0' THEN 100 END),
                   t.love, t.time_seconds, t.canonical_genre,
                   l.play_count, t.album_id, t.file_path, t.filename, t.import_run_id
            FROM tracks AS t
            LEFT JOIN lastfm_track_popularity AS l
              ON l.artist_key = lower(trim(t.album_artist_display))
             AND l.track_key = lower(trim(t.title))
            WHERE t.file_path = :directory AND t.filename = :filename
            "#,
        )
        .map_err(|error| format!("Could not prepare stable queue restore: {error}"))?;
    let mut by_id = connection
        .prepare_cached(
            r#"
            SELECT t.id, t.title, t.album_artist_display, t.album, t.release_year,
                   COALESCE(t.normalized_rating, CASE trim(t.rating_raw)
                     WHEN '0.5' THEN 10 WHEN '1' THEN 20 WHEN '1.0' THEN 20
                     WHEN '1.5' THEN 30 WHEN '2' THEN 40 WHEN '2.0' THEN 40
                     WHEN '2.5' THEN 50 WHEN '3' THEN 60 WHEN '3.0' THEN 60
                     WHEN '3.5' THEN 70 WHEN '4' THEN 80 WHEN '4.0' THEN 80
                     WHEN '4.5' THEN 90 WHEN '5' THEN 100 WHEN '5.0' THEN 100 END),
                   t.love, t.time_seconds, t.canonical_genre,
                   l.play_count, t.album_id, t.file_path, t.filename, t.import_run_id
            FROM tracks AS t
            LEFT JOIN lastfm_track_popularity AS l
              ON l.artist_key = lower(trim(t.album_artist_display))
             AND l.track_key = lower(trim(t.title))
            WHERE t.id = :track_id
            "#,
        )
        .map_err(|error| format!("Could not prepare legacy queue restore: {error}"))?;

    let mut tracks = Vec::with_capacity(references.len());
    let mut missing = 0;
    for reference in references {
        let track_result = match (&reference.directory, &reference.filename) {
            (Some(directory), Some(filename)) => by_path.query_row(
                named_params! { ":directory": directory, ":filename": filename },
                map_track_row,
            ),
            _ => match parse_track_id(&reference.track_id) {
                Ok(parsed) => by_id.query_row(named_params! { ":track_id": parsed }, map_track_row),
                Err(_) => {
                    missing += 1;
                    continue;
                }
            },
        };
        let Ok(track) = track_result else {
            missing += 1;
            continue;
        };
        if reference
            .track_key
            .as_ref()
            .is_some_and(|expected_key| &track.track_key != expected_key)
        {
            missing += 1;
            continue;
        }
        tracks.push(track);
    }
    if !tracks.is_empty() {
        apply_overlays(&mut tracks, Some(store))?;
    }
    Ok((tracks, missing))
}

fn load_catalog_track_by_id(track_id: &str, track_key: &str) -> Result<TrackSummary, String> {
    let parsed = parse_track_id(track_id)?;
    let path = default_catalog_path()?;
    let connection = open_catalog(&path)?;
    let track = connection
        .query_row(
            r#"
            SELECT t.id, t.title, t.album_artist_display, t.album, t.release_year,
                   COALESCE(t.normalized_rating, CASE trim(t.rating_raw)
                     WHEN '0.5' THEN 10 WHEN '1' THEN 20 WHEN '1.0' THEN 20
                     WHEN '1.5' THEN 30 WHEN '2' THEN 40 WHEN '2.0' THEN 40
                     WHEN '2.5' THEN 50 WHEN '3' THEN 60 WHEN '3.0' THEN 60
                     WHEN '3.5' THEN 70 WHEN '4' THEN 80 WHEN '4.0' THEN 80
                     WHEN '4.5' THEN 90 WHEN '5' THEN 100 WHEN '5.0' THEN 100 END),
                   t.love, t.time_seconds, t.canonical_genre,
                   l.play_count, t.album_id, t.file_path, t.filename, t.import_run_id
            FROM tracks AS t
            LEFT JOIN lastfm_track_popularity AS l
              ON l.artist_key = lower(trim(t.album_artist_display))
             AND l.track_key = lower(trim(t.title))
            WHERE t.id = :track_id
            "#,
            named_params! { ":track_id": parsed },
            map_track_row,
        )
        .map_err(|error| format!("Could not resolve this track from the catalog: {error}"))?;
    verify_track_identity(track, track_key)
}

fn verify_track_identity(track: TrackSummary, expected_key: &str) -> Result<TrackSummary, String> {
    if expected_key.is_empty() || expected_key.chars().count() > 1024 {
        return Err("Stable track identity is invalid.".to_owned());
    }
    if track.track_key != expected_key {
        return Err(
            "This track changed identity during a Music Library import. Refresh before continuing."
                .to_owned(),
        );
    }
    Ok(track)
}

pub(crate) fn resolve_track(
    track_id: &str,
    track_key: &str,
    store: &StateStore,
) -> Result<ResolvedTrack, String> {
    let mut summary = load_catalog_track_by_id(track_id, track_key)?;
    let catalog_values = summary.catalog_tag_values();
    apply_overlays(std::slice::from_mut(&mut summary), Some(store))?;
    let audio_path = validated_audio_path(&summary.directory, &summary.filename)?;
    Ok(ResolvedTrack {
        summary,
        audio_path,
        catalog_values,
    })
}

pub(crate) fn resolve_audio_path(
    track_id: &str,
    track_key: &str,
    store: &StateStore,
) -> Result<PathBuf, String> {
    Ok(resolve_track(track_id, track_key, store)?.audio_path)
}

pub(crate) fn catalog_tag_values_by_path(
    directory: &str,
    filename: &str,
) -> Result<Option<(TagValues, i64)>, String> {
    let path = default_catalog_path()?;
    let connection = open_catalog(&path)?;
    connection
        .query_row(
            r#"
            SELECT COALESCE(normalized_rating, CASE trim(rating_raw)
                     WHEN '0.5' THEN 10 WHEN '1' THEN 20 WHEN '1.0' THEN 20
                     WHEN '1.5' THEN 30 WHEN '2' THEN 40 WHEN '2.0' THEN 40
                     WHEN '2.5' THEN 50 WHEN '3' THEN 60 WHEN '3.0' THEN 60
                     WHEN '3.5' THEN 70 WHEN '4' THEN 80 WHEN '4.0' THEN 80
                     WHEN '4.5' THEN 90 WHEN '5' THEN 100 WHEN '5.0' THEN 100 END),
                   love, release_year, import_run_id
            FROM tracks WHERE file_path = ?1 AND filename = ?2
            "#,
            params![directory, filename],
            |row| {
                let rating: Option<i64> = row.get(0)?;
                let love: Option<String> = row.get(1)?;
                Ok((
                    TagValues {
                        rating: rating.map(|value| value as f64 / 20.0),
                        love_state: LoveState::from_catalog(love.as_deref()),
                        release_year: row.get(2)?,
                    },
                    row.get(3)?,
                ))
            },
        )
        .optional()
        .map_err(|error| format!("Could not read recovered track values: {error}"))
}

fn validated_audio_path(directory: &str, filename: &str) -> Result<PathBuf, String> {
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
    let audio_path = device_mode::resolve_device_path(Path::new(directory)).join(filename_path);
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
                  album TEXT, release_year INTEGER, normalized_rating INTEGER, rating_raw TEXT,
                  love TEXT,
                  time_seconds INTEGER, canonical_genre TEXT, file_path TEXT, filename TEXT,
                  import_run_id INTEGER NOT NULL
                );
                CREATE TABLE albums (
                  album_artist_display TEXT, canonical_genre TEXT, total_tracks INTEGER NOT NULL
                );
                CREATE TABLE import_runs (id INTEGER PRIMARY KEY, status TEXT NOT NULL);
                CREATE TABLE lastfm_track_popularity (
                  artist_key TEXT, track_key TEXT, play_count INTEGER,
                  PRIMARY KEY (artist_key, track_key)
                );
                INSERT INTO albums VALUES ('Sigur Rós', 'Post-rock', 2);
                INSERT INTO import_runs VALUES (52, 'completed');
                INSERT INTO tracks VALUES (7, 'album-7', 'Sæglópur', 'Sigur Rós', 'Takk...', 2005, 100, '5', 'L', 473, 'Post-rock', 'H:\Music\Sigur Rós', '01 Sæglópur.mp3', 52);
                INSERT INTO tracks VALUES (8, 'album-7', 'Hoppípolla', 'Sigur Rós', 'Takk...', 2005, NULL, '4.5', NULL, 268, 'Post-rock', 'H:\Music\Sigur Rós', '02 Hoppípolla.mp3', 52);
                INSERT INTO lastfm_track_popularity VALUES ('sigur rós', 'sæglópur', 42);
                "#,
            )
            .expect("fixture schema");

        let snapshot =
            query_snapshot(&connection, "fixture.sqlite3".to_owned(), None).expect("snapshot");

        assert_eq!(snapshot.summary.songs, 2);
        assert_eq!(snapshot.summary.loved, 1);
        assert_eq!(snapshot.summary.rated, 1);
        assert_eq!(snapshot.artists[0].name, "Sigur Rós");
        assert_eq!(snapshot.tracks[0].album_id.as_deref(), Some("album-7"));
        assert_eq!(snapshot.tracks[0].rating, Some(5.0));
        assert!(snapshot.tracks[0].loved);
        assert_eq!(snapshot.tracks[0].play_count, Some(42));
        assert_eq!(
            snapshot.tracks[0].track_key,
            "h:\\music\\sigur rós\\01 sæglópur.mp3"
        );
        assert!(verify_track_identity(snapshot.tracks[0].clone(), "wrong-track-key").is_err());
    }

    #[test]
    fn fts_query_quotes_and_bounds_user_terms() {
        assert_eq!(
            parse_catalog_search("white ner OR star")
                .expect("plain search")
                .fts_query,
            Some("\"white\"* AND \"ner\"* AND \"OR\"* AND \"star\"*".to_owned())
        );
        assert!(parse_catalog_search("///").expect("punctuation").is_empty());
        let bounded = parse_catalog_search(&vec!["term"; 40].join(" "))
            .expect("bounded search")
            .fts_query
            .expect("bounded terms");
        assert_eq!(bounded.split(" AND ").count(), MAX_FTS_SEARCH_TERMS);
    }

    #[test]
    fn catalog_search_maps_fields_and_exact_years() {
        let search = parse_catalog_search(
            "artist:kiss,aartist:def leppard,album:love gun,genre:hard rock,year:1985,ryear:2025,publisher:la-la land records,title:easy tonight",
        )
        .expect("fielded search");

        assert!(search.has_fields);
        assert_eq!(search.year, Some(1985));
        assert_eq!(search.release_year, Some(2025));
        let fts = search.fts_query.expect("text fields");
        assert!(fts.contains("display_artist : \"kiss\"*"));
        assert!(fts.contains("album_artist_display : \"def\"*"));
        assert!(fts.contains("album : \"love\"*"));
        assert!(fts.contains("canonical_genre : \"hard\"*"));
        assert!(fts.contains("publisher : \"la\"*"));
        assert!(fts.contains("title : \"easy\"*"));
        assert_eq!(fts.split(" AND ").count(), 13);
        assert!(parse_catalog_search("year:not-a-year").is_err());
        assert!(parse_catalog_search("artist:").is_err());
    }

    #[test]
    fn track_ids_cannot_smuggle_paths_or_sql() {
        assert_eq!(parse_track_id("42").expect("valid identity"), 42);
        assert!(parse_track_id("../42").is_err());
        assert!(parse_track_id("42 OR 1=1").is_err());
    }

    #[test]
    fn stable_track_keys_normalize_windows_case_slashes_and_long_path_prefix() {
        assert_eq!(
            normalize_track_key(r"\\?\H:/MUSIC/KoЯn", " 01 Song.MP3 "),
            "h:\\music\\koяn\\01 song.mp3"
        );
    }
}
