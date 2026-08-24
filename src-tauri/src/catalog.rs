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
    pub(crate) display_artist: Option<String>,
    pub(crate) album: String,
    pub(crate) release_year: Option<i64>,
    pub(crate) original_year: Option<i64>,
    pub(crate) publisher: Option<String>,
    pub(crate) rating: Option<f64>,
    pub(crate) loved: bool,
    pub(crate) love_state: LoveState,
    pub(crate) tag_sync_state: Option<TagSyncState>,
    pub(crate) can_undo_tag_edit: bool,
    pub(crate) duration_seconds: Option<i64>,
    pub(crate) genre: Option<String>,
    pub(crate) play_count: Option<i64>,
    pub(crate) track_number: Option<u32>,
    pub(crate) track_total: Option<u32>,
    pub(crate) disc_number: Option<u32>,
    pub(crate) disc_total: Option<u32>,
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

    pub(crate) fn apply_tag_projection(&mut self, updated: &Self) {
        self.title = updated.title.clone();
        self.artist = updated.artist.clone();
        self.display_artist = updated.display_artist.clone();
        self.album = updated.album.clone();
        self.rating = updated.rating;
        self.loved = updated.loved;
        self.love_state = updated.love_state;
        self.release_year = updated.release_year;
        self.original_year = updated.original_year;
        self.publisher = updated.publisher.clone();
        self.genre = updated.genre.clone();
        self.track_number = updated.track_number;
        self.track_total = updated.track_total;
        self.disc_number = updated.disc_number;
        self.disc_total = updated.disc_total;
        self.tag_sync_state = updated.tag_sync_state;
        self.can_undo_tag_edit = updated.can_undo_tag_edit;
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
    pub(crate) catalog_revision: i64,
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

fn completed_import_revision_for_connection(connection: &Connection) -> Result<i64, String> {
    connection
        .query_row(
            "SELECT COALESCE(MAX(id), 0) FROM import_runs WHERE status = 'completed'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not identify the current library import: {error}"))
}

pub(crate) fn completed_import_revision() -> Result<i64, String> {
    let path = default_catalog_path()?;
    let connection = open_catalog(&path)?;
    completed_import_revision_for_connection(&connection)
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
        display_artist: match row.as_ref().column_index("display_artist") {
            Ok(index) => row.get(index)?,
            Err(_) => None,
        },
        album: row
            .get::<_, Option<String>>(3)?
            .unwrap_or_else(|| "Unknown Album".to_owned()),
        release_year: row.get(4)?,
        original_year: match row.as_ref().column_index("original_year") {
            Ok(index) => row.get(index)?,
            Err(_) => None,
        },
        publisher: match row.as_ref().column_index("publisher") {
            Ok(index) => row.get(index)?,
            Err(_) => None,
        },
        rating: rating.map(|value| value as f64 / 20.0),
        loved: love_state == LoveState::Loved,
        love_state,
        tag_sync_state: None,
        can_undo_tag_edit: false,
        duration_seconds: row.get(7)?,
        genre: row.get(8)?,
        play_count: row.get(9)?,
        track_number: optional_u32(row, "track_number")?,
        track_total: optional_u32(row, "track_total")?,
        disc_number: optional_u32(row, "disc_number")?,
        disc_total: optional_u32(row, "disc_total")?,
        album_id: row.get(10)?,
        directory,
        filename,
        catalog_import_run_id: row.get(13)?,
    })
}

fn optional_u32(row: &Row<'_>, name: &str) -> rusqlite::Result<Option<u32>> {
    let Ok(index) = row.as_ref().column_index(name) else {
        return Ok(None);
    };
    row.get::<_, Option<i64>>(index)?
        .map(u32::try_from)
        .transpose()
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                index,
                rusqlite::types::Type::Integer,
                Box::new(error),
            )
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
    let transaction = connection
        .unchecked_transaction()
        .map_err(|error| format!("Could not start a consistent catalog snapshot: {error}"))?;
    let connection = &*transaction;
    let current_import_run_id = completed_import_revision_for_connection(connection)?;
    if let Some(store) = store {
        reconcile_all_overlays(connection, store)?;
    }
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
          SELECT id, title, album_artist_display, display_artist, album, release_year,
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
               l.play_count, p.album_id, p.file_path, p.filename, p.import_run_id,
               p.display_artist AS display_artist
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

    let snapshot = LibrarySnapshot {
        source_state: "connected",
        source_label: "Live catalog · file-tag overlay",
        source_path,
        catalog_revision: current_import_run_id,
        summary,
        artists,
        tracks,
    };
    drop(artist_statement);
    transaction
        .commit()
        .map_err(|error| format!("Could not finish the consistent catalog snapshot: {error}"))?;
    Ok(snapshot)
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
          SELECT id, title, album_artist_display, display_artist, album, release_year,
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
               l.play_count, p.album_id, p.file_path, p.filename, p.import_run_id,
               p.display_artist AS display_artist
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
const MAX_SEARCH_ALTERNATIVES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq)]
enum CatalogSearchField {
    Any,
    Artist,
    AlbumArtist,
    Album,
    Genre,
    Year,
    ReleaseYear,
    Publisher,
    Title,
}

impl CatalogSearchField {
    fn fts_column(self) -> Option<&'static str> {
        match self {
            Self::Any | Self::Year | Self::ReleaseYear => None,
            Self::Artist => Some("display_artist"),
            Self::AlbumArtist => Some("album_artist_display"),
            Self::Album => Some("album"),
            Self::Genre => Some("canonical_genre"),
            Self::Publisher => Some("publisher"),
            Self::Title => Some("title"),
        }
    }

    fn sql_column(self) -> Option<&'static str> {
        match self {
            Self::Any | Self::Year | Self::ReleaseYear => None,
            Self::Artist => Some("display_artist"),
            Self::AlbumArtist => Some("album_artist_display"),
            Self::Album => Some("album"),
            Self::Genre => Some("canonical_genre"),
            Self::Publisher => Some("publisher"),
            Self::Title => Some("title"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum CatalogSearchMatch {
    Prefix(String),
    Exact(String),
    ScoreGenreGroup,
    YearRange { from: Option<i32>, to: Option<i32> },
}

#[derive(Clone, Debug, PartialEq)]
struct CatalogSearchAlternative {
    field: CatalogSearchField,
    matcher: CatalogSearchMatch,
}

#[derive(Clone, Debug, PartialEq)]
struct CatalogSearchGroup {
    negated: bool,
    alternatives: Vec<CatalogSearchAlternative>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CatalogSearch {
    groups: Vec<CatalogSearchGroup>,
}

impl CatalogSearch {
    pub(crate) fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    pub(crate) fn plain_fts_query(&self) -> Option<&str> {
        let group = self.groups.first()?;
        if self.groups.len() != 1 || group.negated || group.alternatives.len() != 1 {
            return None;
        }
        let alternative = group.alternatives.first()?;
        if alternative.field != CatalogSearchField::Any {
            return None;
        }
        match &alternative.matcher {
            CatalogSearchMatch::Prefix(query) => Some(query),
            CatalogSearchMatch::Exact(_)
            | CatalogSearchMatch::ScoreGenreGroup
            | CatalogSearchMatch::YearRange { .. } => None,
        }
    }

    fn fts_only_query(&self) -> Option<String> {
        let group = self.groups.first()?;
        if self.groups.len() != 1
            || group.negated
            || group
                .alternatives
                .iter()
                .any(|alternative| !matches!(alternative.matcher, CatalogSearchMatch::Prefix(_)))
        {
            return None;
        }
        group_fts_query(group)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum SearchTokenKind {
    And,
    Or,
    Not,
}

#[derive(Clone, Debug, PartialEq)]
enum SearchToken {
    Text(String),
    Operator(SearchTokenKind),
}

fn push_search_text(tokens: &mut Vec<SearchToken>, value: &str) {
    let value = value.trim();
    if !value.is_empty() {
        tokens.push(SearchToken::Text(value.to_owned()));
    }
}

fn tokenize_catalog_search(input: &str) -> Result<Vec<SearchToken>, String> {
    let characters = input.chars().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut value = String::new();
    let mut quoted = false;
    let mut index = 0;

    while index < characters.len() {
        let character = characters[index];
        if character == '"' {
            quoted = !quoted;
            value.push(character);
            index += 1;
            continue;
        }
        if !quoted && character == ',' {
            push_search_text(&mut tokens, &value);
            value.clear();
            tokens.push(SearchToken::Operator(SearchTokenKind::And));
            index += 1;
            continue;
        }
        let at_word_boundary =
            index == 0 || characters[index - 1].is_whitespace() || characters[index - 1] == ',';
        if !quoted && at_word_boundary && character.is_ascii_alphabetic() {
            let mut end = index;
            while end < characters.len() && characters[end].is_ascii_alphabetic() {
                end += 1;
            }
            let boundary_after = end == characters.len()
                || characters[end].is_whitespace()
                || characters[end] == ',';
            if boundary_after {
                let word = characters[index..end].iter().collect::<String>();
                let operator = match word.as_str() {
                    "AND" => Some(SearchTokenKind::And),
                    "OR" => Some(SearchTokenKind::Or),
                    "NOT" => Some(SearchTokenKind::Not),
                    _ => None,
                };
                if let Some(operator) = operator {
                    push_search_text(&mut tokens, &value);
                    value.clear();
                    tokens.push(SearchToken::Operator(operator));
                    index = end;
                    continue;
                }
            }
        }
        value.push(character);
        index += 1;
    }

    if quoted {
        return Err("Search quotes are not closed.".to_owned());
    }
    push_search_text(&mut tokens, &value);
    Ok(tokens)
}

fn parse_search_field(value: &str) -> Option<CatalogSearchField> {
    match value.trim().to_ascii_lowercase().as_str() {
        "artist" => Some(CatalogSearchField::Artist),
        "aartist" => Some(CatalogSearchField::AlbumArtist),
        "album" => Some(CatalogSearchField::Album),
        "genre" => Some(CatalogSearchField::Genre),
        "year" => Some(CatalogSearchField::Year),
        "ryear" => Some(CatalogSearchField::ReleaseYear),
        "publisher" => Some(CatalogSearchField::Publisher),
        "title" => Some(CatalogSearchField::Title),
        _ => None,
    }
}

fn build_fts_prefix_query(
    input: &str,
    column: Option<&str>,
    term_count: &mut usize,
) -> Result<String, String> {
    let mut terms = Vec::new();
    for term in input
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| !term.is_empty())
    {
        if *term_count >= MAX_FTS_SEARCH_TERMS {
            return Err(format!(
                "Search can contain at most {MAX_FTS_SEARCH_TERMS} words."
            ));
        }
        *term_count += 1;
        let bounded: String = term.chars().take(64).collect();
        let escaped = bounded.replace('"', "\"\"");
        let prefix = format!("\"{escaped}\"*");
        terms.push(match column {
            Some(column) => format!("{column} : {prefix}"),
            None => prefix,
        });
    }
    if terms.is_empty() {
        return Err("Search needs a word or an exact quoted value.".to_owned());
    }
    Ok(terms.join(" AND "))
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

fn parse_search_year_range(value: &str, field: &str) -> Result<(Option<i32>, Option<i32>), String> {
    let value = value.trim();
    let Some((raw_from, raw_to)) = value.split_once("..") else {
        let year = parse_search_year(value, field)?;
        return Ok((Some(year), Some(year)));
    };
    if raw_to.contains("..") {
        return Err(format!(
            "{field} range must use one '..', for example {field}:1985..1987."
        ));
    }
    let from = (!raw_from.trim().is_empty())
        .then(|| parse_search_year(raw_from, field))
        .transpose()?;
    let to = (!raw_to.trim().is_empty())
        .then(|| parse_search_year(raw_to, field))
        .transpose()?;
    if from.is_none() && to.is_none() {
        return Err(format!("{field} range needs a starting or ending year."));
    }
    if from.zip(to).is_some_and(|(from, to)| from > to) {
        return Err(format!(
            "{field} range must start at or before its ending year."
        ));
    }
    Ok((from, to))
}

fn exact_search_value(value: &str) -> Result<Option<String>, String> {
    let value = value.trim();
    let starts = value.starts_with('"');
    let ends = value.ends_with('"');
    if starts != ends || (!starts && value.contains('"')) {
        return Err("Quotes must wrap one complete search value.".to_owned());
    }
    if !starts {
        return Ok(None);
    }
    let exact = value[1..value.len() - 1].trim();
    if exact.is_empty() {
        return Err("Exact search quotes cannot be empty.".to_owned());
    }
    Ok(Some(exact.to_owned()))
}

pub(crate) fn parse_catalog_search(input: &str) -> Result<CatalogSearch, String> {
    let tokens = tokenize_catalog_search(input)?;
    let mut groups = Vec::new();
    let mut current: Option<CatalogSearchGroup> = None;
    let mut inherited_field = None;
    let mut pending_not = false;
    let mut after_or = false;
    let mut term_count = 0;
    let mut alternative_count = 0;

    for token in tokens {
        match token {
            SearchToken::Text(raw) => {
                let mut raw = raw.trim();
                let negative_prefix = raw.starts_with('-');
                if negative_prefix {
                    raw = raw[1..].trim();
                    if raw.is_empty() {
                        return Err("Negative search needs a value after '-'.".to_owned());
                    }
                    if current.is_some() {
                        return Err(
                            "Use a comma, AND, or NOT before a negative '-' clause.".to_owned()
                        );
                    }
                }
                if current.is_none() {
                    current = Some(CatalogSearchGroup {
                        negated: pending_not || negative_prefix,
                        alternatives: Vec::new(),
                    });
                    pending_not = false;
                }

                let (field, value, explicit_field) = raw
                    .split_once(':')
                    .and_then(|(field, value)| {
                        parse_search_field(field).map(|field| (field, value.trim(), true))
                    })
                    .unwrap_or((
                        inherited_field.unwrap_or(CatalogSearchField::Any),
                        raw,
                        false,
                    ));
                if explicit_field {
                    inherited_field = Some(field);
                }
                if value.is_empty() {
                    return Err("Search field needs a value.".to_owned());
                }
                let exact = exact_search_value(value)?;
                let matcher = match field {
                    CatalogSearchField::Year | CatalogSearchField::ReleaseYear => {
                        let field_name = if field == CatalogSearchField::Year {
                            "year"
                        } else {
                            "ryear"
                        };
                        let (from, to) =
                            parse_search_year_range(exact.as_deref().unwrap_or(value), field_name)?;
                        CatalogSearchMatch::YearRange { from, to }
                    }
                    CatalogSearchField::Genre
                        if exact.is_none()
                            && matches!(
                                value.to_ascii_lowercase().as_str(),
                                "score" | "scores"
                            ) =>
                    {
                        CatalogSearchMatch::ScoreGenreGroup
                    }
                    _ => match exact {
                        Some(value) => CatalogSearchMatch::Exact(value),
                        None => CatalogSearchMatch::Prefix(build_fts_prefix_query(
                            value,
                            field.fts_column(),
                            &mut term_count,
                        )?),
                    },
                };
                alternative_count += 1;
                if alternative_count > MAX_SEARCH_ALTERNATIVES {
                    return Err(format!(
                        "Search can contain at most {MAX_SEARCH_ALTERNATIVES} alternatives."
                    ));
                }
                current
                    .as_mut()
                    .expect("a search value creates a group")
                    .alternatives
                    .push(CatalogSearchAlternative { field, matcher });
                after_or = false;
            }
            SearchToken::Operator(SearchTokenKind::Or) => {
                if current
                    .as_ref()
                    .is_none_or(|group| group.alternatives.is_empty())
                    || after_or
                {
                    return Err("OR needs a search value on both sides.".to_owned());
                }
                after_or = true;
            }
            SearchToken::Operator(SearchTokenKind::And) => {
                if after_or {
                    return Err("OR needs a search value on both sides.".to_owned());
                }
                if let Some(group) = current.take() {
                    groups.push(group);
                }
                inherited_field = None;
                pending_not = false;
            }
            SearchToken::Operator(SearchTokenKind::Not) => {
                if after_or {
                    return Err("NOT cannot replace a value after OR.".to_owned());
                }
                if let Some(group) = current.take() {
                    groups.push(group);
                }
                inherited_field = None;
                if pending_not {
                    return Err("NOT needs one search clause.".to_owned());
                }
                pending_not = true;
            }
        }
    }
    if after_or {
        return Err("OR needs a search value on both sides.".to_owned());
    }
    if pending_not {
        return Err("NOT needs one search clause.".to_owned());
    }
    if let Some(group) = current {
        groups.push(group);
    }
    Ok(CatalogSearch { groups })
}

const EXACT_TEXT_COLUMNS: [&str; 6] = [
    "display_artist",
    "album_artist_display",
    "album",
    "canonical_genre",
    "publisher",
    "title",
];

const SCORE_GENRE_GROUP: [&str; 13] = [
    "action",
    "animation",
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
    "anime",
];

fn exact_text_predicate(
    alias: &str,
    field: CatalogSearchField,
    value: &str,
    params: &mut Vec<Value>,
) -> String {
    if field == CatalogSearchField::Any {
        return format!(
            "({})",
            EXACT_TEXT_COLUMNS
                .iter()
                .map(|column| {
                    params.push(Value::Text(value.to_owned()));
                    format!("TRIM(COALESCE({alias}.{column}, '')) = ? COLLATE NOCASE")
                })
                .collect::<Vec<_>>()
                .join(" OR ")
        );
    }
    let column = field
        .sql_column()
        .expect("exact text search uses a text field");
    params.push(Value::Text(value.to_owned()));
    format!("TRIM(COALESCE({alias}.{column}, '')) = ? COLLATE NOCASE")
}

fn non_prefix_predicate(
    alias: &str,
    alternative: &CatalogSearchAlternative,
    params: &mut Vec<Value>,
) -> Option<String> {
    match &alternative.matcher {
        CatalogSearchMatch::Prefix(_) => None,
        CatalogSearchMatch::Exact(value) => Some(exact_text_predicate(
            alias,
            alternative.field,
            value,
            params,
        )),
        CatalogSearchMatch::ScoreGenreGroup => Some(format!(
            "LOWER(TRIM(COALESCE({alias}.canonical_genre, ''))) IN ({})",
            SCORE_GENRE_GROUP
                .iter()
                .map(|genre| {
                    params.push(Value::Text((*genre).to_owned()));
                    "?"
                })
                .collect::<Vec<_>>()
                .join(", ")
        )),
        CatalogSearchMatch::YearRange { from, to } => {
            let column = match alternative.field {
                CatalogSearchField::Year => "year",
                CatalogSearchField::ReleaseYear => "release_year",
                _ => unreachable!("numeric search uses a year field"),
            };
            if from == to {
                let year = from.expect("an equal bounded range contains one year");
                params.push(Value::Integer(i64::from(year)));
                return Some(format!("{alias}.{column} = ?"));
            }
            let mut predicates = Vec::new();
            if let Some(from) = from {
                params.push(Value::Integer(i64::from(*from)));
                predicates.push(format!("{alias}.{column} >= ?"));
            }
            if let Some(to) = to {
                params.push(Value::Integer(i64::from(*to)));
                predicates.push(format!("{alias}.{column} <= ?"));
            }
            Some(format!("({})", predicates.join(" AND ")))
        }
    }
}

fn group_fts_query(group: &CatalogSearchGroup) -> Option<String> {
    let queries = group
        .alternatives
        .iter()
        .filter_map(|alternative| match &alternative.matcher {
            CatalogSearchMatch::Prefix(query) => Some(format!("({query})")),
            CatalogSearchMatch::Exact(_)
            | CatalogSearchMatch::ScoreGenreGroup
            | CatalogSearchMatch::YearRange { .. } => None,
        })
        .collect::<Vec<_>>();
    (!queries.is_empty()).then(|| queries.join(" OR "))
}

pub(crate) fn push_track_search_predicates(
    sql: &mut String,
    params: &mut Vec<Value>,
    search: &CatalogSearch,
) {
    for group in &search.groups {
        let mut alternatives = Vec::new();
        if let Some(match_query) = group_fts_query(group) {
            alternatives.push("t.id IN (SELECT CAST(track_id AS INTEGER) FROM track_search_fts WHERE track_search_fts MATCH ?)".to_owned());
            params.push(Value::Text(match_query));
        }
        alternatives.extend(
            group
                .alternatives
                .iter()
                .filter_map(|alternative| non_prefix_predicate("t", alternative, params)),
        );
        sql.push_str(if group.negated {
            " AND NOT ("
        } else {
            " AND ("
        });
        sql.push_str(&alternatives.join(" OR "));
        sql.push(')');
    }
}

pub(crate) fn push_album_search_predicates(
    sql: &mut String,
    params: &mut Vec<Value>,
    search: &CatalogSearch,
) {
    for group in &search.groups {
        let mut alternatives = Vec::new();
        if let Some(match_query) = group_fts_query(group) {
            alternatives.push(
                "a.id IN (SELECT album_id FROM track_search_fts WHERE track_search_fts MATCH ?)"
                    .to_owned(),
            );
            params.push(Value::Text(match_query));
        }
        let track_predicates = group
            .alternatives
            .iter()
            .filter_map(|alternative| non_prefix_predicate("search_track", alternative, params))
            .collect::<Vec<_>>();
        if !track_predicates.is_empty() {
            alternatives.push(format!(
                "a.id IN (SELECT search_track.album_id FROM tracks AS search_track WHERE {})",
                track_predicates.join(" OR ")
            ));
        }
        sql.push_str(if group.negated {
            " AND NOT ("
        } else {
            " AND ("
        });
        sql.push_str(&alternatives.join(" OR "));
        sql.push(')');
    }
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
    let ranked_query = search.fts_only_query();
    let mut params = Vec::<Value>::new();
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
               l.play_count, t.album_id, t.file_path, t.filename, t.import_run_id,
               t.display_artist AS display_artist
        FROM tracks AS t
        "#,
    );
    if ranked_query.is_some() {
        sql.push_str(" JOIN track_search_fts ON CAST(track_search_fts.track_id AS INTEGER) = t.id");
    }
    sql.push_str(
        " LEFT JOIN lastfm_track_popularity AS l ON l.artist_key = lower(trim(t.album_artist_display)) AND l.track_key = lower(trim(t.title)) WHERE 1 = 1",
    );
    if let Some(match_query) = ranked_query {
        sql.push_str(" AND track_search_fts MATCH ?");
        params.push(Value::Text(match_query));
        sql.push_str(" ORDER BY bm25(track_search_fts), t.id LIMIT 50");
    } else {
        push_track_search_predicates(&mut sql, &mut params, &search);
        sql.push_str(" ORDER BY t.id DESC LIMIT 50");
    }
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
                   l.play_count, t.album_id, t.file_path, t.filename, t.import_run_id,
                   t.display_artist AS display_artist
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
            let by_id = parse_track_id(&reference.id)
                .and_then(|parsed| {
                    statement
                        .query_row(named_params! { ":track_id": parsed }, map_track_row)
                        .map_err(|error| {
                            format!("Track {} is no longer available: {error}", reference.id)
                        })
                })
                .and_then(|track| verify_track_identity(track, &reference.track_key));
            by_id.or_else(|_| load_track_by_stable_key(&connection, &reference.track_key))
        })
        .collect::<Result<Vec<_>, _>>()?;
    apply_overlays(&mut tracks, Some(store))?;
    Ok(tracks)
}

pub(crate) fn load_tracks_by_references(
    references: &[StoredQueueEntry],
    store: &StateStore,
) -> Result<(Vec<TrackSummary>, usize, i64), String> {
    if references.is_empty() || references.len() > 200 {
        return Err("A playback queue must contain between 1 and 200 tracks.".to_owned());
    }
    let path = default_catalog_path()?;
    let connection = open_catalog(&path)?;
    let transaction = connection
        .unchecked_transaction()
        .map_err(|error| format!("Could not start a consistent queue rebind: {error}"))?;
    let catalog_revision = completed_import_revision_for_connection(&transaction)?;
    let mut by_path = transaction
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
                   l.play_count, t.album_id, t.file_path, t.filename, t.import_run_id,
                   t.display_artist AS display_artist
            FROM tracks AS t
            LEFT JOIN lastfm_track_popularity AS l
              ON l.artist_key = lower(trim(t.album_artist_display))
             AND l.track_key = lower(trim(t.title))
            WHERE t.file_path = :directory AND t.filename = :filename
            "#,
        )
        .map_err(|error| format!("Could not prepare stable queue restore: {error}"))?;
    let mut by_id = transaction
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
                   l.play_count, t.album_id, t.file_path, t.filename, t.import_run_id,
                   t.display_artist AS display_artist
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
        let track = match (&reference.directory, &reference.filename) {
            (Some(directory), Some(filename)) => {
                let exact = by_path.query_row(
                    named_params! { ":directory": directory, ":filename": filename },
                    map_track_row,
                );
                match exact {
                    Ok(track) => Some(track),
                    Err(rusqlite::Error::QueryReturnedNoRows) => {
                        match canonical_catalog_identity(directory, filename) {
                            Some((canonical_directory, canonical_filename)) => {
                                match by_path.query_row(
                                    named_params! {
                                        ":directory": canonical_directory,
                                        ":filename": canonical_filename,
                                    },
                                    map_track_row,
                                ) {
                                    Ok(track) => Some(track),
                                    Err(rusqlite::Error::QueryReturnedNoRows) => None,
                                    Err(error) => {
                                        return Err(format!(
                                            "Could not resolve the playback queue from its canonical paths: {error}"
                                        ));
                                    }
                                }
                            }
                            None => None,
                        }
                    }
                    Err(error) => {
                        return Err(format!(
                            "Could not resolve the playback queue from its stored paths: {error}"
                        ));
                    }
                }
            }
            _ => match parse_track_id(&reference.track_id) {
                Ok(parsed) => {
                    match by_id.query_row(named_params! { ":track_id": parsed }, map_track_row) {
                        Ok(track) => Some(track),
                        Err(rusqlite::Error::QueryReturnedNoRows) => None,
                        Err(error) => {
                            return Err(format!(
                                "Could not resolve the playback queue from its legacy IDs: {error}"
                            ));
                        }
                    }
                }
                Err(_) => None,
            },
        };
        let verified = track.and_then(|track| match reference.track_key.as_deref() {
            Some(track_key) => verify_track_identity(track, track_key).ok(),
            None => Some(track),
        });
        let track = match (verified, reference.track_key.as_deref()) {
            (Some(track), _) => track,
            (None, Some(track_key)) => match lookup_track_by_stable_key(&transaction, track_key) {
                Ok(track) => track,
                Err(StableTrackLookupError::Invalid | StableTrackLookupError::Missing) => {
                    missing += 1;
                    continue;
                }
                Err(StableTrackLookupError::Failure(error)) => return Err(error),
            },
            (None, None) => {
                missing += 1;
                continue;
            }
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
    drop(by_id);
    drop(by_path);
    transaction
        .commit()
        .map_err(|error| format!("Could not finish the consistent queue rebind: {error}"))?;
    if !tracks.is_empty() {
        apply_overlays(&mut tracks, Some(store))?;
    }
    Ok((tracks, missing, catalog_revision))
}

fn canonical_catalog_identity(directory: &str, filename: &str) -> Option<(String, String)> {
    let canonical = Path::new(directory).join(filename).canonicalize().ok()?;
    let canonical_directory = canonical.parent()?.to_string_lossy();
    let canonical_filename = canonical.file_name()?.to_str()?.to_owned();
    let canonical_directory = if let Some(rest) = canonical_directory.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else {
        canonical_directory
            .strip_prefix(r"\\?\")
            .unwrap_or(&canonical_directory)
            .to_owned()
    };
    Some((canonical_directory, canonical_filename))
}

fn load_catalog_track_by_id(track_id: &str, track_key: &str) -> Result<TrackSummary, String> {
    let path = default_catalog_path()?;
    let connection = open_catalog(&path)?;
    let by_id = parse_track_id(track_id)
        .and_then(|parsed| {
            connection
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
                   l.play_count, t.album_id, t.file_path, t.filename, t.import_run_id,
                   t.display_artist AS display_artist
            FROM tracks AS t
            LEFT JOIN lastfm_track_popularity AS l
              ON l.artist_key = lower(trim(t.album_artist_display))
             AND l.track_key = lower(trim(t.title))
            WHERE t.id = :track_id
            "#,
                    named_params! { ":track_id": parsed },
                    map_track_row,
                )
                .map_err(|error| format!("Could not resolve this track from the catalog: {error}"))
        })
        .and_then(|track| verify_track_identity(track, track_key));
    by_id.or_else(|_| load_track_by_stable_key(&connection, track_key))
}

enum StableTrackLookupError {
    Invalid,
    Missing,
    Failure(String),
}

impl StableTrackLookupError {
    fn into_message(self) -> String {
        match self {
            Self::Invalid => "Stable track identity is invalid.".to_owned(),
            Self::Missing => {
                "This track is no longer present in the Music Library catalog.".to_owned()
            }
            Self::Failure(error) => error,
        }
    }
}

fn load_track_by_stable_key(
    connection: &Connection,
    track_key: &str,
) -> Result<TrackSummary, String> {
    lookup_track_by_stable_key(connection, track_key).map_err(StableTrackLookupError::into_message)
}

fn lookup_track_by_stable_key(
    connection: &Connection,
    track_key: &str,
) -> Result<TrackSummary, StableTrackLookupError> {
    if track_key.is_empty() || track_key.chars().count() > 1024 {
        return Err(StableTrackLookupError::Invalid);
    }
    let (directory, filename) = track_key
        .rsplit_once('\\')
        .filter(|(directory, filename)| !directory.is_empty() && !filename.is_empty())
        .ok_or(StableTrackLookupError::Invalid)?;
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
                   l.play_count, t.album_id, t.file_path, t.filename, t.import_run_id,
                   t.display_artist AS display_artist
            FROM tracks AS t
            LEFT JOIN lastfm_track_popularity AS l
              ON l.artist_key = lower(trim(t.album_artist_display))
             AND l.track_key = lower(trim(t.title))
            WHERE t.file_path = :directory AND t.filename = :filename
            "#,
        )
        .map_err(|error| {
            StableTrackLookupError::Failure(format!(
                "Could not prepare stable track resolution: {error}"
            ))
        })?;
    if let Some((canonical_directory, canonical_filename)) =
        canonical_catalog_identity(directory, filename)
    {
        match statement.query_row(
            named_params! {
                ":directory": canonical_directory,
                ":filename": canonical_filename,
            },
            map_track_row,
        ) {
            Ok(track) => {
                return verify_track_identity(track, track_key)
                    .map_err(|_| StableTrackLookupError::Missing);
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {}
            Err(error) => {
                return Err(StableTrackLookupError::Failure(format!(
                    "Could not resolve the canonical track path: {error}"
                )));
            }
        }
    }

    let mut case_insensitive = connection
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
                   l.play_count, t.album_id, t.file_path, t.filename, t.import_run_id,
                   t.display_artist AS display_artist
            FROM tracks AS t
            LEFT JOIN lastfm_track_popularity AS l
              ON l.artist_key = lower(trim(t.album_artist_display))
             AND l.track_key = lower(trim(t.title))
            WHERE replace(t.file_path, '/', char(92)) = :directory COLLATE NOCASE
              AND t.filename = :filename COLLATE NOCASE
            LIMIT 1
            "#,
        )
        .map_err(|error| {
            StableTrackLookupError::Failure(format!(
                "Could not prepare normalized track resolution: {error}"
            ))
        })?;
    let track = match case_insensitive.query_row(
        named_params! { ":directory": directory, ":filename": filename },
        map_track_row,
    ) {
        Ok(track) => track,
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            return Err(StableTrackLookupError::Missing);
        }
        Err(error) => {
            return Err(StableTrackLookupError::Failure(format!(
                "Could not resolve the normalized track path: {error}"
            )));
        }
    };
    verify_track_identity(track, track_key).map_err(|_| StableTrackLookupError::Missing)
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

pub(crate) fn resolve_album_tracks(
    album_id: &str,
    store: &StateStore,
) -> Result<Vec<ResolvedTrack>, String> {
    let album_id = album_id.trim();
    if album_id.is_empty() || album_id.chars().count() > 512 {
        return Err("Album identity is invalid.".to_owned());
    }
    let connection = open_catalog(&default_catalog_path()?)?;
    let mut summaries = album_tag_tracks_from_connection(&connection, album_id)?;
    let catalog_values = summaries
        .iter()
        .map(TrackSummary::catalog_tag_values)
        .collect::<Vec<_>>();
    apply_overlays(&mut summaries, Some(store))?;
    summaries
        .into_iter()
        .zip(catalog_values)
        .map(|(summary, catalog_values)| {
            let audio_path = validated_audio_path(&summary.directory, &summary.filename)?;
            Ok(ResolvedTrack {
                summary,
                audio_path,
                catalog_values,
            })
        })
        .collect()
}

fn album_tag_tracks_from_connection(
    connection: &Connection,
    album_id: &str,
) -> Result<Vec<TrackSummary>, String> {
    const MAX_TAG_EDITOR_ALBUM_TRACKS: usize = 500;
    let mut statement = connection
        .prepare(
            r#"
            SELECT t.id, t.title, t.album_artist_display, t.album, t.release_year,
                   COALESCE(t.normalized_rating, CASE trim(t.rating_raw)
                     WHEN '0.5' THEN 10 WHEN '1' THEN 20 WHEN '1.0' THEN 20
                     WHEN '1.5' THEN 30 WHEN '2' THEN 40 WHEN '2.0' THEN 40
                     WHEN '2.5' THEN 50 WHEN '3' THEN 60 WHEN '3.0' THEN 60
                     WHEN '3.5' THEN 70 WHEN '4' THEN 80 WHEN '4.0' THEN 80
                     WHEN '4.5' THEN 90 WHEN '5' THEN 100 WHEN '5.0' THEN 100 END),
                   t.love, t.time_seconds, t.canonical_genre, l.play_count,
                   t.album_id, t.file_path, t.filename, t.import_run_id,
                   t.year AS original_year, t.publisher AS publisher,
                   t.display_artist AS display_artist,
                   t.track_number AS track_number, NULL AS track_total,
                   t.disc_number AS disc_number, NULL AS disc_total
            FROM tracks AS t
            LEFT JOIN lastfm_track_popularity AS l
              ON l.artist_key = lower(trim(t.album_artist_display))
             AND l.track_key = lower(trim(t.title))
            WHERE t.album_id = ?1
            ORDER BY COALESCE(t.disc_number, 0), COALESCE(t.track_number, 0), t.id
            LIMIT 501
            "#,
        )
        .map_err(|error| format!("Could not prepare the album tag selection: {error}"))?;
    let summaries = statement
        .query_map([album_id], map_track_row)
        .map_err(|error| format!("Could not read the album tag selection: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode the album tag selection: {error}"))?;
    if summaries.is_empty() {
        return Err("This album is no longer available in the catalog.".to_owned());
    }
    if summaries.len() > MAX_TAG_EDITOR_ALBUM_TRACKS {
        return Err(format!(
            "This album contains more than {MAX_TAG_EDITOR_ALBUM_TRACKS} tracks; Aurora refused an unsafe batch edit."
        ));
    }
    Ok(summaries)
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
                  display_artist TEXT, album TEXT, release_year INTEGER, normalized_rating INTEGER, rating_raw TEXT,
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
                INSERT INTO tracks VALUES (7, 'album-7', 'Sæglópur', 'Sigur Rós', 'Jónsi', 'Takk...', 2005, 100, '5', 'L', 473, 'Post-rock', 'H:\Music\Sigur Rós', '01 Sæglópur.mp3', 52);
                INSERT INTO tracks VALUES (8, 'album-7', 'Hoppípolla', 'Sigur Rós', 'Sigur Rós', 'Takk...', 2005, NULL, '4.5', NULL, 268, 'Post-rock', 'H:\Music\Sigur Rós', '02 Hoppípolla.mp3', 52);
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
        assert_eq!(snapshot.tracks[0].display_artist.as_deref(), Some("Jónsi"));
        assert_eq!(snapshot.tracks[0].rating, Some(5.0));
        assert!(snapshot.tracks[0].loved);
        assert_eq!(snapshot.tracks[0].play_count, Some(42));
        assert_eq!(
            snapshot.tracks[0].track_key,
            "h:\\music\\sigur rós\\01 sæglópur.mp3"
        );
        assert!(verify_track_identity(snapshot.tracks[0].clone(), "wrong-track-key").is_err());
        assert_eq!(snapshot.catalog_revision, 52);
        let stable_track = load_track_by_stable_key(&connection, &snapshot.tracks[0].track_key)
            .expect("stable-key lookup");
        assert_eq!(stable_track.id, "7");
        assert_eq!(stable_track.display_artist.as_deref(), Some("Jónsi"));
        connection
            .execute(
                "UPDATE tracks SET file_path = 'H:/MUSIC/Sigur Rós' WHERE id = 7",
                [],
            )
            .expect("alternate path spelling");
        assert_eq!(
            load_track_by_stable_key(&connection, &snapshot.tracks[0].track_key)
                .expect("slash-normalized stable-key lookup")
                .id,
            "7"
        );

        let mut rebound = snapshot.tracks[0].clone();
        rebound.id = "fresh-id".to_owned();
        let mut stale_tag_update = snapshot.tracks[0].clone();
        stale_tag_update.id = "old-id".to_owned();
        stale_tag_update.rating = Some(3.5);
        stale_tag_update.love_state = LoveState::Banned;
        stale_tag_update.loved = false;
        rebound.apply_tag_projection(&stale_tag_update);
        assert_eq!(rebound.id, "fresh-id");
        assert_eq!(rebound.rating, Some(3.5));
        assert_eq!(rebound.love_state, LoveState::Banned);
    }

    #[test]
    fn completed_import_revision_ignores_unfinished_runs() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(
                r#"
                CREATE TABLE import_runs (id INTEGER PRIMARY KEY, status TEXT NOT NULL);
                INSERT INTO import_runs VALUES (51, 'failed');
                INSERT INTO import_runs VALUES (52, 'completed');
                INSERT INTO import_runs VALUES (53, 'running');
                "#,
            )
            .expect("fixture schema");

        assert_eq!(
            completed_import_revision_for_connection(&connection).expect("completed revision"),
            52
        );
    }

    #[test]
    fn album_tag_resolver_is_not_limited_to_the_ui_page_size() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(
                r#"
                CREATE TABLE tracks (
                  id INTEGER PRIMARY KEY, album_id TEXT, title TEXT,
                  album_artist_display TEXT, display_artist TEXT, album TEXT,
                  release_year INTEGER, normalized_rating INTEGER, rating_raw TEXT,
                  love TEXT, time_seconds INTEGER, canonical_genre TEXT,
                  file_path TEXT, filename TEXT, import_run_id INTEGER NOT NULL,
                  year INTEGER, publisher TEXT, track_number INTEGER, disc_number INTEGER
                );
                CREATE TABLE lastfm_track_popularity (
                  artist_key TEXT, track_key TEXT, play_count INTEGER,
                  PRIMARY KEY (artist_key, track_key)
                );
                "#,
            )
            .expect("fixture schema");
        for index in 1..=125_i64 {
            connection
                .execute(
                    r#"
                    INSERT INTO tracks(
                      id, album_id, title, album_artist_display, display_artist, album,
                      file_path, filename, import_run_id, track_number, disc_number
                    ) VALUES (?1, 'album-large', ?2, 'Composer', 'Performer', 'Large Album',
                              'D:\Music\Large Album', ?3, 52, ?1, 1)
                    "#,
                    params![index, format!("Track {index}"), format!("{index:03}.mp3")],
                )
                .expect("insert track");
        }

        let tracks = album_tag_tracks_from_connection(&connection, "album-large")
            .expect("resolve complete album selection");

        assert_eq!(tracks.len(), 125);
        assert_eq!(tracks[0].track_number, Some(1));
        assert_eq!(tracks[124].track_number, Some(125));
    }

    #[test]
    fn stable_track_lookup_does_not_treat_catalog_errors_as_missing_tracks() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute("CREATE TABLE tracks (id INTEGER PRIMARY KEY)", [])
            .expect("incomplete fixture schema");

        assert!(matches!(
            lookup_track_by_stable_key(&connection, r"h:\music\artist\track.mp3"),
            Err(StableTrackLookupError::Failure(_))
        ));
    }

    #[test]
    fn fts_query_quotes_boolean_operators_and_bounds_user_terms() {
        assert_eq!(
            parse_catalog_search("white ner star")
                .expect("plain search")
                .plain_fts_query(),
            Some("\"white\"* AND \"ner\"* AND \"star\"*")
        );
        let alternatives = parse_catalog_search("white ner OR star").expect("OR search");
        assert_eq!(alternatives.groups.len(), 1);
        assert_eq!(alternatives.groups[0].alternatives.len(), 2);
        assert_eq!(
            alternatives.fts_only_query(),
            Some("(\"white\"* AND \"ner\"*) OR (\"star\"*)".to_owned())
        );
        assert!(parse_catalog_search("///").is_err());
        assert!(parse_catalog_search(&vec!["term"; 40].join(" ")).is_err());
    }

    #[test]
    fn catalog_search_maps_fields_and_exact_years() {
        let search = parse_catalog_search(
            "artist:kiss,aartist:def leppard,album:love gun,genre:hard rock,year:1985,ryear:2025,publisher:la-la land records,title:easy tonight",
        )
        .expect("fielded search");

        assert_eq!(search.groups.len(), 8);
        assert_eq!(
            search.groups[0].alternatives[0].field,
            CatalogSearchField::Artist
        );
        assert_eq!(
            search.groups[1].alternatives[0].field,
            CatalogSearchField::AlbumArtist
        );
        assert_eq!(
            search.groups[4].alternatives[0].matcher,
            CatalogSearchMatch::YearRange {
                from: Some(1985),
                to: Some(1985)
            }
        );
        assert_eq!(
            search.groups[5].alternatives[0].matcher,
            CatalogSearchMatch::YearRange {
                from: Some(2025),
                to: Some(2025)
            }
        );
        assert!(parse_catalog_search("year:not-a-year").is_err());
        assert!(parse_catalog_search("artist:").is_err());
    }

    #[test]
    fn catalog_search_parses_closed_open_and_inherited_year_ranges() {
        let search = parse_catalog_search("year:1985..1987 OR 1990..1992,ryear:..1987 OR 1995..")
            .expect("year range search");

        assert_eq!(search.groups.len(), 2);
        assert_eq!(
            search.groups[0].alternatives[0].matcher,
            CatalogSearchMatch::YearRange {
                from: Some(1985),
                to: Some(1987)
            }
        );
        assert_eq!(
            search.groups[0].alternatives[1].matcher,
            CatalogSearchMatch::YearRange {
                from: Some(1990),
                to: Some(1992)
            }
        );
        assert_eq!(
            search.groups[1].alternatives[0].matcher,
            CatalogSearchMatch::YearRange {
                from: None,
                to: Some(1987)
            }
        );
        assert_eq!(
            search.groups[1].alternatives[1].matcher,
            CatalogSearchMatch::YearRange {
                from: Some(1995),
                to: None
            }
        );

        assert!(parse_catalog_search("year:1987..1985").is_err());
        assert!(parse_catalog_search("ryear:..").is_err());
        assert!(parse_catalog_search("year:1985..1987..1989").is_err());
    }

    #[test]
    fn catalog_search_inherits_or_fields_and_negates_groups() {
        let search = parse_catalog_search(
            "genre:hard rock OR heavy metal OR AOR NOT aartist:bon jovi OR def leppard OR \"Kiss\"",
        )
        .expect("boolean search");

        assert_eq!(search.groups.len(), 2);
        assert!(!search.groups[0].negated);
        assert!(search.groups[1].negated);
        assert!(
            search.groups[0]
                .alternatives
                .iter()
                .all(|alternative| alternative.field == CatalogSearchField::Genre)
        );
        assert!(
            search.groups[1]
                .alternatives
                .iter()
                .all(|alternative| alternative.field == CatalogSearchField::AlbumArtist)
        );
        assert_eq!(
            search.groups[1].alternatives[2].matcher,
            CatalogSearchMatch::Exact("Kiss".to_owned())
        );

        let negative = parse_catalog_search("genre:synthpop,-aartist:madonna")
            .expect("negative prefix search");
        assert_eq!(negative.groups.len(), 2);
        assert!(negative.groups[1].negated);
        assert!(parse_catalog_search("genre:rock OR").is_err());
        assert!(parse_catalog_search("aartist:\"Kiss").is_err());
    }

    #[test]
    fn catalog_search_expands_the_music_library_scores_group() {
        let search = parse_catalog_search("genre:scores OR synthpop").expect("scores search");
        assert_eq!(
            search.groups[0].alternatives[0].matcher,
            CatalogSearchMatch::ScoreGenreGroup
        );
        assert!(matches!(
            search.groups[0].alternatives[1].matcher,
            CatalogSearchMatch::Prefix(_)
        ));

        let exact = parse_catalog_search("genre:\"scores\"").expect("exact scores search");
        assert_eq!(
            exact.groups[0].alternatives[0].matcher,
            CatalogSearchMatch::Exact("scores".to_owned())
        );
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
