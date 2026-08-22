use crate::{
    catalog::{
        ArtistSummary, TrackSummary, apply_overlays, build_fts_prefix_query, default_catalog_path,
        map_track_row, open_catalog,
    },
    state_store::StateStore,
    tag_model::LoveState,
};
use rusqlite::{Connection, Row, params_from_iter, types::Value};
use serde::{Deserialize, Serialize};

const DEFAULT_PAGE_SIZE: u16 = 50;
const MAX_PAGE_SIZE: u16 = 100;
const MAX_SEARCH_CHARS: usize = 256;
const MAX_FILTER_CHARS: usize = 256;
const MAX_CURSOR_CHARS: usize = 1024;

const TRACK_COLUMNS: &str = r#"t.id, t.title, t.album_artist_display, t.album, t.release_year,
    COALESCE(t.normalized_rating, CASE trim(t.rating_raw)
      WHEN '0.5' THEN 10 WHEN '1' THEN 20 WHEN '1.0' THEN 20
      WHEN '1.5' THEN 30 WHEN '2' THEN 40 WHEN '2.0' THEN 40
      WHEN '2.5' THEN 50 WHEN '3' THEN 60 WHEN '3.0' THEN 60
      WHEN '3.5' THEN 70 WHEN '4' THEN 80 WHEN '4.0' THEN 80
      WHEN '4.5' THEN 90 WHEN '5' THEN 100 WHEN '5.0' THEN 100 END),
    t.love, t.time_seconds, t.canonical_genre, l.play_count,
    t.album_id, t.file_path, t.filename, t.import_run_id"#;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ExploreCursor {
    pub(crate) value: String,
    pub(crate) id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TrackSort {
    #[default]
    Newest,
    TitleAsc,
    ArtistAsc,
    AlbumAsc,
    ReleaseYearDesc,
    RatingDesc,
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TrackPageRequest {
    pub(crate) page_size: Option<u16>,
    pub(crate) cursor: Option<ExploreCursor>,
    pub(crate) search: Option<String>,
    pub(crate) rating: Option<f64>,
    #[serde(default)]
    pub(crate) unrated: bool,
    pub(crate) love_state: Option<LoveState>,
    pub(crate) year_from: Option<i32>,
    pub(crate) year_to: Option<i32>,
    pub(crate) genre: Option<String>,
    pub(crate) artist: Option<String>,
    pub(crate) sort: Option<TrackSort>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrackPage {
    pub(crate) items: Vec<TrackSummary>,
    pub(crate) next_cursor: Option<ExploreCursor>,
}

#[derive(Clone, Copy, Debug, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AlbumSort {
    TitleAsc,
    ArtistAsc,
    #[default]
    ReleaseYearDesc,
    RatingDesc,
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AlbumPageRequest {
    pub(crate) page_size: Option<u16>,
    pub(crate) cursor: Option<ExploreCursor>,
    pub(crate) search: Option<String>,
    pub(crate) year_from: Option<i32>,
    pub(crate) year_to: Option<i32>,
    pub(crate) genre: Option<String>,
    pub(crate) artist: Option<String>,
    pub(crate) sort: Option<AlbumSort>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AlbumSummary {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) release_year: Option<i64>,
    pub(crate) genre: Option<String>,
    pub(crate) total_tracks: i64,
    pub(crate) rated_tracks: i64,
    pub(crate) loved_tracks: i64,
    pub(crate) duration_seconds: i64,
    pub(crate) rating: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AlbumPage {
    pub(crate) items: Vec<AlbumSummary>,
    pub(crate) next_cursor: Option<ExploreCursor>,
}

#[derive(Clone, Copy, Debug, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ArtistSort {
    #[default]
    NameAsc,
    TrackCountDesc,
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ArtistPageRequest {
    pub(crate) page_size: Option<u16>,
    pub(crate) cursor: Option<ExploreCursor>,
    pub(crate) search: Option<String>,
    pub(crate) genre: Option<String>,
    pub(crate) sort: Option<ArtistSort>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArtistPage {
    pub(crate) items: Vec<ArtistSummary>,
    pub(crate) next_cursor: Option<ExploreCursor>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AlbumDetail {
    pub(crate) album: AlbumSummary,
    pub(crate) tracks: Vec<TrackSummary>,
    pub(crate) tracks_truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArtistDetail {
    pub(crate) artist: ArtistSummary,
    pub(crate) albums: Vec<AlbumSummary>,
    pub(crate) albums_truncated: bool,
}

#[derive(Clone, Copy)]
enum CursorKind {
    Integer,
    NullableInteger,
    Text,
}

#[derive(Clone, Copy)]
struct SortDefinition {
    cursor_tag: &'static str,
    expression: &'static str,
    descending: bool,
    cursor_kind: CursorKind,
}

fn page_size(value: Option<u16>) -> Result<usize, String> {
    let value = value.unwrap_or(DEFAULT_PAGE_SIZE);
    if !(1..=MAX_PAGE_SIZE).contains(&value) {
        return Err(format!("Page size must be between 1 and {MAX_PAGE_SIZE}."));
    }
    Ok(usize::from(value))
}

fn bounded_optional_text(
    value: &Option<String>,
    label: &str,
    max_chars: usize,
) -> Result<Option<String>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max_chars {
        return Err(format!("{label} is invalid."));
    }
    Ok(Some(value.to_owned()))
}

fn validate_years(year_from: Option<i32>, year_to: Option<i32>) -> Result<(), String> {
    if year_from.is_some_and(|year| !(1000..=2999).contains(&year))
        || year_to.is_some_and(|year| !(1000..=2999).contains(&year))
        || matches!((year_from, year_to), (Some(from), Some(to)) if from > to)
    {
        return Err("Release-year range is invalid.".to_owned());
    }
    Ok(())
}

fn rating_value(rating: Option<f64>, unrated: bool) -> Result<Option<i64>, String> {
    if rating.is_some() && unrated {
        return Err("Rating and unrated filters cannot be combined.".to_owned());
    }
    let Some(rating) = rating else {
        return Ok(None);
    };
    let doubled = rating * 2.0;
    if !rating.is_finite()
        || !(0.5..=5.0).contains(&rating)
        || (doubled - doubled.round()).abs() > f64::EPSILON
    {
        return Err("Rating filter must be a half-star value from 0.5 to 5.".to_owned());
    }
    Ok(Some((rating * 20.0).round() as i64))
}

fn validate_cursor(cursor: &ExploreCursor) -> Result<(), String> {
    if cursor.value.is_empty()
        || cursor.value.chars().count() > MAX_CURSOR_CHARS
        || cursor.id.is_empty()
        || cursor.id.chars().count() > MAX_CURSOR_CHARS
    {
        return Err("Page cursor is invalid.".to_owned());
    }
    Ok(())
}

fn cursor_parameter(value: &str, kind: CursorKind) -> Result<Value, String> {
    match kind {
        CursorKind::Integer | CursorKind::NullableInteger => value
            .parse::<i64>()
            .map(Value::Integer)
            .map_err(|_| "Page cursor is invalid.".to_owned()),
        CursorKind::Text => Ok(Value::Text(value.to_owned())),
    }
}

fn numeric_cursor_id(cursor: &ExploreCursor) -> Result<i64, String> {
    cursor
        .id
        .parse::<i64>()
        .map_err(|_| "Page cursor is invalid.".to_owned())
}

fn push_exact_filter(sql: &mut String, params: &mut Vec<Value>, column: &str, value: String) {
    sql.push_str(" AND ");
    sql.push_str(column);
    sql.push_str(" = ? COLLATE NOCASE");
    params.push(Value::Text(value));
}

fn push_year_filters(
    sql: &mut String,
    params: &mut Vec<Value>,
    column: &str,
    year_from: Option<i32>,
    year_to: Option<i32>,
) {
    if let Some(year) = year_from {
        sql.push_str(" AND ");
        sql.push_str(column);
        sql.push_str(" >= ?");
        params.push(Value::Integer(i64::from(year)));
    }
    if let Some(year) = year_to {
        sql.push_str(" AND ");
        sql.push_str(column);
        sql.push_str(" <= ?");
        params.push(Value::Integer(i64::from(year)));
    }
}

fn push_keyset(
    sql: &mut String,
    params: &mut Vec<Value>,
    cursor: Option<&ExploreCursor>,
    sort: SortDefinition,
    id_expression: &str,
    numeric_id: bool,
) -> Result<(), String> {
    let Some(cursor) = cursor else {
        return Ok(());
    };
    validate_cursor(cursor)?;
    let prefix = format!("{}:", sort.cursor_tag);
    let cursor_value_text = cursor
        .value
        .strip_prefix(&prefix)
        .ok_or_else(|| "Page cursor does not match the current sort.".to_owned())?;
    let operator = if sort.descending { "<" } else { ">" };
    if matches!(sort.cursor_kind, CursorKind::NullableInteger) {
        sql.push_str(" AND (");
        if cursor_value_text == "unrated" {
            sql.push_str(sort.expression);
            sql.push_str(" IS NULL AND ");
            sql.push_str(id_expression);
            sql.push(' ');
            sql.push_str(operator);
            sql.push_str(" ?)");
            params.push(if numeric_id {
                Value::Integer(numeric_cursor_id(cursor)?)
            } else {
                Value::Text(cursor.id.clone())
            });
        } else {
            let cursor_value = cursor_parameter(cursor_value_text, sort.cursor_kind)?;
            sql.push('(');
            sql.push_str(sort.expression);
            sql.push_str(") ");
            sql.push_str(operator);
            sql.push_str(" ? OR (");
            sql.push_str(sort.expression);
            sql.push_str(") IS NULL OR ((");
            sql.push_str(sort.expression);
            sql.push_str(") = ? AND ");
            sql.push_str(id_expression);
            sql.push(' ');
            sql.push_str(operator);
            sql.push_str(" ?))");
            params.push(cursor_value.clone());
            params.push(cursor_value);
            params.push(if numeric_id {
                Value::Integer(numeric_cursor_id(cursor)?)
            } else {
                Value::Text(cursor.id.clone())
            });
        }
        return Ok(());
    }
    sql.push_str(" AND ((");
    sql.push_str(sort.expression);
    sql.push_str(") ");
    sql.push_str(operator);
    sql.push_str(" ? OR ((");
    sql.push_str(sort.expression);
    sql.push_str(") = ? AND ");
    sql.push_str(id_expression);
    sql.push(' ');
    sql.push_str(operator);
    sql.push_str(" ?))");
    let cursor_value = cursor_parameter(cursor_value_text, sort.cursor_kind)?;
    params.push(cursor_value.clone());
    params.push(cursor_value);
    params.push(if numeric_id {
        Value::Integer(numeric_cursor_id(cursor)?)
    } else {
        Value::Text(cursor.id.clone())
    });
    Ok(())
}

fn track_sort(sort: TrackSort) -> SortDefinition {
    match sort {
        TrackSort::Newest => SortDefinition {
            cursor_tag: "track-newest",
            expression: "t.id",
            descending: true,
            cursor_kind: CursorKind::Integer,
        },
        TrackSort::TitleAsc => SortDefinition {
            cursor_tag: "track-title",
            expression: "COALESCE(t.title, '') COLLATE NOCASE",
            descending: false,
            cursor_kind: CursorKind::Text,
        },
        TrackSort::ArtistAsc => SortDefinition {
            cursor_tag: "track-artist",
            expression: "(COALESCE(t.album_artist_display, '') || char(31) || COALESCE(t.title, '')) COLLATE NOCASE",
            descending: false,
            cursor_kind: CursorKind::Text,
        },
        TrackSort::AlbumAsc => SortDefinition {
            cursor_tag: "track-album",
            expression: "(COALESCE(t.album, '') || char(31) || COALESCE(t.title, '')) COLLATE NOCASE",
            descending: false,
            cursor_kind: CursorKind::Text,
        },
        TrackSort::ReleaseYearDesc => SortDefinition {
            cursor_tag: "track-year",
            expression: "COALESCE(t.release_year, -1)",
            descending: true,
            cursor_kind: CursorKind::Integer,
        },
        TrackSort::RatingDesc => SortDefinition {
            cursor_tag: "track-rating",
            expression: "t.normalized_rating",
            descending: true,
            cursor_kind: CursorKind::NullableInteger,
        },
    }
}

fn track_page_from_connection(
    connection: &Connection,
    request: TrackPageRequest,
    store: Option<&StateStore>,
) -> Result<TrackPage, String> {
    let page_size = page_size(request.page_size)?;
    validate_years(request.year_from, request.year_to)?;
    let rating = rating_value(request.rating, request.unrated)?;
    let genre = bounded_optional_text(&request.genre, "Genre filter", MAX_FILTER_CHARS)?;
    let artist = bounded_optional_text(&request.artist, "Artist filter", MAX_FILTER_CHARS)?;
    let search = bounded_optional_text(&request.search, "Search text", MAX_SEARCH_CHARS)?;
    let match_query = search.as_deref().and_then(build_fts_prefix_query);
    if search.is_some() && match_query.is_none() {
        return Ok(TrackPage {
            items: Vec::new(),
            next_cursor: None,
        });
    }
    let sort = track_sort(request.sort.unwrap_or_default());
    let direction = if sort.descending { "DESC" } else { "ASC" };
    let mut params = Vec::<Value>::new();
    let mut sql = format!(
        "SELECT {TRACK_COLUMNS}, COALESCE(CAST(({}) AS TEXT), 'unrated') AS cursor_value FROM tracks AS t",
        sort.expression
    );
    if match_query.is_some() {
        sql.push_str(" JOIN track_search_fts ON CAST(track_search_fts.track_id AS INTEGER) = t.id");
    }
    sql.push_str(
        " LEFT JOIN lastfm_track_popularity AS l ON l.artist_key = lower(trim(t.album_artist_display)) AND l.track_key = lower(trim(t.title)) WHERE 1 = 1",
    );
    if let Some(match_query) = match_query {
        sql.push_str(" AND track_search_fts MATCH ?");
        params.push(Value::Text(match_query));
    }
    if let Some(rating) = rating {
        sql.push_str(" AND t.normalized_rating = ?");
        params.push(Value::Integer(rating));
    } else if request.unrated {
        sql.push_str(" AND t.normalized_rating IS NULL");
    }
    if let Some(love_state) = request.love_state {
        match love_state {
            LoveState::Loved => sql.push_str(" AND t.love = 'L'"),
            LoveState::Banned => sql.push_str(" AND t.love = 'B'"),
            LoveState::Neutral => {
                sql.push_str(" AND COALESCE(NULLIF(trim(t.love), ''), '') NOT IN ('L', 'B')")
            }
        }
    }
    push_year_filters(
        &mut sql,
        &mut params,
        "t.release_year",
        request.year_from,
        request.year_to,
    );
    if let Some(genre) = genre {
        push_exact_filter(&mut sql, &mut params, "t.canonical_genre", genre);
    }
    if let Some(artist) = artist {
        push_exact_filter(&mut sql, &mut params, "t.album_artist_display", artist);
    }
    push_keyset(
        &mut sql,
        &mut params,
        request.cursor.as_ref(),
        sort,
        "t.id",
        true,
    )?;
    sql.push_str(" ORDER BY (");
    sql.push_str(sort.expression);
    sql.push_str(") ");
    sql.push_str(direction);
    sql.push_str(", t.id ");
    sql.push_str(direction);
    sql.push_str(" LIMIT ?");
    params.push(Value::Integer((page_size + 1) as i64));

    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("Could not prepare the track explorer: {error}"))?;
    let mut rows = statement
        .query(params_from_iter(params.iter()))
        .map_err(|error| format!("Could not read the track explorer: {error}"))?;
    let mut mapped = Vec::<(TrackSummary, String)>::with_capacity(page_size + 1);
    while let Some(row) = rows
        .next()
        .map_err(|error| format!("Could not read the track explorer: {error}"))?
    {
        mapped.push((
            map_track_row(row)
                .map_err(|error| format!("Could not decode the track explorer: {error}"))?,
            row.get(14)
                .map_err(|error| format!("Could not decode the track cursor: {error}"))?,
        ));
    }
    let has_more = mapped.len() > page_size;
    mapped.truncate(page_size);
    let next_cursor = has_more.then(|| {
        let (track, value) = mapped.last().expect("a page with more rows has an item");
        ExploreCursor {
            value: format!("{}:{value}", sort.cursor_tag),
            id: track.id.clone(),
        }
    });
    let mut items = mapped
        .into_iter()
        .map(|(track, _)| track)
        .collect::<Vec<_>>();
    apply_overlays(&mut items, store)?;
    Ok(TrackPage { items, next_cursor })
}

fn map_album_row(row: &Row<'_>) -> rusqlite::Result<AlbumSummary> {
    let effective_rating: Option<i64> = row.get(10)?;
    Ok(AlbumSummary {
        id: row.get(0)?,
        title: row
            .get::<_, Option<String>>(1)?
            .unwrap_or_else(|| "Unknown Album".to_owned()),
        artist: row
            .get::<_, Option<String>>(2)?
            .unwrap_or_else(|| "Unknown Artist".to_owned()),
        release_year: row.get(3)?,
        genre: row.get(4)?,
        total_tracks: row.get(5)?,
        rated_tracks: row.get(6)?,
        loved_tracks: row.get(7)?,
        duration_seconds: row.get(8)?,
        rating: effective_rating.map(|rating| rating as f64 / 20.0),
    })
}

fn album_sort(sort: AlbumSort) -> SortDefinition {
    match sort {
        AlbumSort::TitleAsc => SortDefinition {
            cursor_tag: "album-title",
            expression: "COALESCE(a.album, '') COLLATE NOCASE",
            descending: false,
            cursor_kind: CursorKind::Text,
        },
        AlbumSort::ArtistAsc => SortDefinition {
            cursor_tag: "album-artist",
            expression: "(COALESCE(a.album_artist_display, '') || char(31) || COALESCE(a.album, '')) COLLATE NOCASE",
            descending: false,
            cursor_kind: CursorKind::Text,
        },
        AlbumSort::ReleaseYearDesc => SortDefinition {
            cursor_tag: "album-year",
            expression: "COALESCE(a.release_year, -1)",
            descending: true,
            cursor_kind: CursorKind::Integer,
        },
        AlbumSort::RatingDesc => SortDefinition {
            cursor_tag: "album-rating",
            expression: "COALESCE(a.effective_album_rating, -1)",
            descending: true,
            cursor_kind: CursorKind::Integer,
        },
    }
}

fn album_page_from_connection(
    connection: &Connection,
    request: AlbumPageRequest,
) -> Result<AlbumPage, String> {
    let page_size = page_size(request.page_size)?;
    validate_years(request.year_from, request.year_to)?;
    let genre = bounded_optional_text(&request.genre, "Genre filter", MAX_FILTER_CHARS)?;
    let artist = bounded_optional_text(&request.artist, "Artist filter", MAX_FILTER_CHARS)?;
    let search = bounded_optional_text(&request.search, "Search text", MAX_SEARCH_CHARS)?;
    let match_query = search.as_deref().and_then(build_fts_prefix_query);
    if search.is_some() && match_query.is_none() {
        return Ok(AlbumPage {
            items: Vec::new(),
            next_cursor: None,
        });
    }
    let sort = album_sort(request.sort.unwrap_or_default());
    let direction = if sort.descending { "DESC" } else { "ASC" };
    let mut params = Vec::<Value>::new();
    let mut sql = format!(
        "SELECT a.id, a.album, a.album_artist_display, a.release_year, a.canonical_genre, a.total_tracks, a.rated_tracks, a.loved_tracks, a.total_seconds, a.album_score, a.effective_album_rating, CAST(({}) AS TEXT) AS cursor_value FROM albums AS a",
        sort.expression
    );
    if match_query.is_some() {
        sql.push_str(" JOIN album_search_fts ON album_search_fts.album_id = a.id");
    }
    sql.push_str(" WHERE 1 = 1");
    if let Some(match_query) = match_query {
        sql.push_str(" AND album_search_fts MATCH ?");
        params.push(Value::Text(match_query));
    }
    push_year_filters(
        &mut sql,
        &mut params,
        "a.release_year",
        request.year_from,
        request.year_to,
    );
    if let Some(genre) = genre {
        push_exact_filter(&mut sql, &mut params, "a.canonical_genre", genre);
    }
    if let Some(artist) = artist {
        sql.push_str(" AND a.album_artist_display = ?");
        params.push(Value::Text(artist));
    }
    push_keyset(
        &mut sql,
        &mut params,
        request.cursor.as_ref(),
        sort,
        "a.id",
        false,
    )?;
    sql.push_str(" ORDER BY (");
    sql.push_str(sort.expression);
    sql.push_str(") ");
    sql.push_str(direction);
    sql.push_str(", a.id ");
    sql.push_str(direction);
    sql.push_str(" LIMIT ?");
    params.push(Value::Integer((page_size + 1) as i64));

    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("Could not prepare the album explorer: {error}"))?;
    let mut rows = statement
        .query(params_from_iter(params.iter()))
        .map_err(|error| format!("Could not read the album explorer: {error}"))?;
    let mut mapped = Vec::<(AlbumSummary, String)>::with_capacity(page_size + 1);
    while let Some(row) = rows
        .next()
        .map_err(|error| format!("Could not read the album explorer: {error}"))?
    {
        mapped.push((
            map_album_row(row)
                .map_err(|error| format!("Could not decode the album explorer: {error}"))?,
            row.get(11)
                .map_err(|error| format!("Could not decode the album cursor: {error}"))?,
        ));
    }
    let has_more = mapped.len() > page_size;
    mapped.truncate(page_size);
    let next_cursor = has_more.then(|| {
        let (album, value) = mapped.last().expect("a page with more rows has an item");
        ExploreCursor {
            value: format!("{}:{value}", sort.cursor_tag),
            id: album.id.clone(),
        }
    });
    Ok(AlbumPage {
        items: mapped.into_iter().map(|(album, _)| album).collect(),
        next_cursor,
    })
}

fn artist_page_from_connection(
    connection: &Connection,
    request: ArtistPageRequest,
) -> Result<ArtistPage, String> {
    let page_size = page_size(request.page_size)?;
    let search = bounded_optional_text(&request.search, "Search text", MAX_SEARCH_CHARS)?;
    let genre = bounded_optional_text(&request.genre, "Genre filter", MAX_FILTER_CHARS)?;
    let sort = request.sort.unwrap_or_default();
    let definition = match sort {
        ArtistSort::NameAsc => SortDefinition {
            cursor_tag: "artist-name",
            expression: "r.name COLLATE NOCASE",
            descending: false,
            cursor_kind: CursorKind::Text,
        },
        ArtistSort::TrackCountDesc => SortDefinition {
            cursor_tag: "artist-count",
            expression: "r.track_count",
            descending: true,
            cursor_kind: CursorKind::Integer,
        },
    };
    let direction = if definition.descending { "DESC" } else { "ASC" };
    let mut params = Vec::<Value>::new();
    let mut rollup_where =
        String::from(" WHERE NULLIF(trim(a.album_artist_display), '') IS NOT NULL");
    if let Some(search) = search {
        rollup_where.push_str(" AND a.album_artist_display LIKE ? ESCAPE '\\' COLLATE NOCASE");
        let escaped = search
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        params.push(Value::Text(format!("%{escaped}%")));
    }
    if let Some(genre) = genre {
        push_exact_filter(&mut rollup_where, &mut params, "a.canonical_genre", genre);
    }
    let mut sql = format!(
        "WITH r AS (SELECT a.album_artist_display AS name, CAST(SUM(a.total_tracks) AS INTEGER) AS track_count, COUNT(*) AS album_count FROM albums AS a {rollup_where} GROUP BY a.album_artist_display COLLATE NOCASE) SELECT r.name, r.track_count, r.album_count, CAST(({}) AS TEXT) AS cursor_value FROM r WHERE 1 = 1",
        definition.expression
    );
    push_keyset(
        &mut sql,
        &mut params,
        request.cursor.as_ref(),
        definition,
        "r.name COLLATE NOCASE",
        false,
    )?;
    sql.push_str(" ORDER BY (");
    sql.push_str(definition.expression);
    sql.push_str(") ");
    sql.push_str(direction);
    sql.push_str(", r.name COLLATE NOCASE ");
    sql.push_str(direction);
    sql.push_str(" LIMIT ?");
    params.push(Value::Integer((page_size + 1) as i64));
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("Could not prepare the artist explorer: {error}"))?;
    let mut rows = statement
        .query(params_from_iter(params.iter()))
        .map_err(|error| format!("Could not read the artist explorer: {error}"))?;
    let mut mapped = Vec::<(ArtistSummary, String)>::with_capacity(page_size + 1);
    while let Some(row) = rows
        .next()
        .map_err(|error| format!("Could not read the artist explorer: {error}"))?
    {
        let name: String = row
            .get(0)
            .map_err(|error| format!("Could not decode the artist explorer: {error}"))?;
        mapped.push((
            ArtistSummary {
                id: name.clone(),
                name,
                track_count: row
                    .get(1)
                    .map_err(|error| format!("Could not decode the artist explorer: {error}"))?,
                album_count: row
                    .get(2)
                    .map_err(|error| format!("Could not decode the artist explorer: {error}"))?,
                play_count: None,
            },
            row.get(3)
                .map_err(|error| format!("Could not decode the artist cursor: {error}"))?,
        ));
    }
    let has_more = mapped.len() > page_size;
    mapped.truncate(page_size);
    let next_cursor = has_more.then(|| {
        let (artist, value) = mapped.last().expect("a page with more rows has an item");
        ExploreCursor {
            value: format!("{}:{value}", definition.cursor_tag),
            id: artist.name.clone(),
        }
    });
    Ok(ArtistPage {
        items: mapped.into_iter().map(|(artist, _)| artist).collect(),
        next_cursor,
    })
}

pub(crate) fn load_track_page(
    request: TrackPageRequest,
    store: &StateStore,
) -> Result<TrackPage, String> {
    let connection = open_catalog(&default_catalog_path()?)?;
    track_page_from_connection(&connection, request, Some(store))
}

pub(crate) fn load_album_page(request: AlbumPageRequest) -> Result<AlbumPage, String> {
    let connection = open_catalog(&default_catalog_path()?)?;
    album_page_from_connection(&connection, request)
}

pub(crate) fn load_artist_page(request: ArtistPageRequest) -> Result<ArtistPage, String> {
    let connection = open_catalog(&default_catalog_path()?)?;
    artist_page_from_connection(&connection, request)
}

fn validate_identity(value: &str, label: &str, max_chars: usize) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max_chars {
        return Err(format!("{label} is invalid."));
    }
    Ok(value.to_owned())
}

pub(crate) fn load_album_detail(
    album_id: String,
    store: &StateStore,
) -> Result<AlbumDetail, String> {
    let album_id = validate_identity(&album_id, "Album identity", 512)?;
    let connection = open_catalog(&default_catalog_path()?)?;
    album_detail_from_connection(&connection, &album_id, Some(store))
}

fn album_detail_from_connection(
    connection: &Connection,
    album_id: &str,
    store: Option<&StateStore>,
) -> Result<AlbumDetail, String> {
    let album = connection
        .query_row(
            "SELECT a.id, a.album, a.album_artist_display, a.release_year, a.canonical_genre, a.total_tracks, a.rated_tracks, a.loved_tracks, a.total_seconds, a.album_score, a.effective_album_rating FROM albums AS a WHERE a.id = ?",
            [album_id],
            map_album_row,
        )
        .map_err(|_| "Album is no longer available in the catalog.".to_owned())?;
    let mut statement = connection
        .prepare(&format!(
            "SELECT {TRACK_COLUMNS} FROM tracks AS t LEFT JOIN lastfm_track_popularity AS l ON l.artist_key = lower(trim(t.album_artist_display)) AND l.track_key = lower(trim(t.title)) WHERE t.album_id = ? ORDER BY COALESCE(t.disc_number, 0), COALESCE(t.track_number, 0), t.id LIMIT 101"
        ))
        .map_err(|error| format!("Could not prepare the album tracks: {error}"))?;
    let mut tracks = statement
        .query_map([album_id], map_track_row)
        .map_err(|error| format!("Could not read the album tracks: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode the album tracks: {error}"))?;
    let tracks_truncated = tracks.len() > usize::from(MAX_PAGE_SIZE);
    tracks.truncate(usize::from(MAX_PAGE_SIZE));
    apply_overlays(&mut tracks, store)?;
    Ok(AlbumDetail {
        album,
        tracks,
        tracks_truncated,
    })
}

pub(crate) fn load_artist_detail(artist: String) -> Result<ArtistDetail, String> {
    let artist = validate_identity(&artist, "Artist identity", MAX_FILTER_CHARS)?;
    let connection = open_catalog(&default_catalog_path()?)?;
    artist_detail_from_connection(&connection, &artist)
}

fn artist_detail_from_connection(
    connection: &Connection,
    artist: &str,
) -> Result<ArtistDetail, String> {
    let artist_summary = connection
        .query_row(
            "SELECT ?1, CAST(SUM(total_tracks) AS INTEGER), COUNT(*) FROM albums WHERE album_artist_display = ?1 HAVING COUNT(*) > 0",
            [artist],
            |row| {
                let name: String = row.get(0)?;
                Ok(ArtistSummary {
                    id: name.clone(),
                    name,
                    track_count: row.get(1)?,
                    album_count: row.get(2)?,
                    play_count: None,
                })
            },
        )
        .map_err(|_| "Artist is no longer available in the catalog.".to_owned())?;
    let page = album_page_from_connection(
        connection,
        AlbumPageRequest {
            page_size: Some(MAX_PAGE_SIZE),
            artist: Some(artist.to_owned()),
            sort: Some(AlbumSort::ReleaseYearDesc),
            ..AlbumPageRequest::default()
        },
    )?;
    Ok(ArtistDetail {
        artist: artist_summary,
        albums_truncated: page.next_cursor.is_some(),
        albums: page.items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Connection {
        let connection = Connection::open_in_memory().expect("in-memory explorer database");
        connection
            .execute_batch(
                r#"
                CREATE TABLE tracks (
                  id INTEGER PRIMARY KEY, import_run_id INTEGER NOT NULL, album_id TEXT NOT NULL,
                  title TEXT, display_artist TEXT, album_artist_display TEXT, album TEXT,
                  canonical_genre TEXT, love TEXT, rating_raw TEXT, normalized_rating INTEGER,
                  release_year INTEGER, time_seconds INTEGER, file_path TEXT, filename TEXT,
                  disc_number INTEGER, track_number INTEGER
                );
                CREATE TABLE albums (
                  id TEXT PRIMARY KEY, album TEXT, album_artist_display TEXT, canonical_genre TEXT,
                  release_year INTEGER, total_tracks INTEGER NOT NULL, rated_tracks INTEGER NOT NULL,
                  loved_tracks INTEGER NOT NULL, total_seconds INTEGER NOT NULL,
                  album_score REAL, effective_album_rating INTEGER
                );
                CREATE TABLE lastfm_track_popularity (
                  artist_key TEXT, track_key TEXT, play_count INTEGER,
                  PRIMARY KEY (artist_key, track_key)
                );
                CREATE VIRTUAL TABLE track_search_fts USING fts5(
                  track_id UNINDEXED, album_id UNINDEXED, title, display_artist, album,
                  album_artist_display, canonical_genre, publisher, file_path, filename
                );
                CREATE VIRTUAL TABLE album_search_fts USING fts5(
                  album_id UNINDEXED, album, album_artist_display, canonical_genre, publisher
                );
                INSERT INTO albums VALUES
                  ('a1', 'Takk...', 'Sigur Rós', 'Post-rock', 2005, 2, 1, 1, 741, 95, 90),
                  ('a2', 'Ágætis byrjun', 'Sigur Rós', 'Post-rock', 1999, 1, 0, 0, 426, 80, NULL),
                  ('a3', 'Discovery', 'Daft Punk', 'House', 2001, 1, 1, 1, 301, 88, 80);
                INSERT INTO tracks VALUES
                  (7, 1, 'a1', 'Sæglópur', 'Sigur Rós', 'Sigur Rós', 'Takk...', 'Post-rock', 'L', '5', 100, 2005, 473, 'H:\Music\Sigur Rós', '01.mp3', 1, 1),
                  (8, 1, 'a1', 'Hoppípolla', 'Sigur Rós', 'Sigur Rós', 'Takk...', 'Post-rock', NULL, '', NULL, 2005, 268, 'H:\Music\Sigur Rós', '02.mp3', 1, 2),
                  (9, 1, 'a2', 'Svefn-g-englar', 'Sigur Rós', 'Sigur Rós', 'Ágætis byrjun', 'Post-rock', 'B', '4.5', NULL, 1999, 426, 'H:\Music\Sigur Rós', '03.mp3', 1, 1),
                  (10, 1, 'a3', 'Digital Love', 'Daft Punk', 'Daft Punk', 'Discovery', 'House', 'L', '4', 80, 2001, 301, 'H:\Music\Daft Punk', '01.mp3', 1, 1);
                INSERT INTO track_search_fts
                  SELECT id, album_id, title, display_artist, album, album_artist_display,
                         canonical_genre, '', file_path, filename FROM tracks;
                INSERT INTO album_search_fts
                  SELECT id, album, album_artist_display, canonical_genre, '' FROM albums;
                "#,
            )
            .expect("explorer fixture schema");
        connection
    }

    #[test]
    fn validates_page_rating_year_and_cursor_inputs() {
        assert!(page_size(Some(0)).is_err());
        assert!(page_size(Some(101)).is_err());
        assert_eq!(page_size(None).expect("default page"), 50);
        assert!(rating_value(Some(3.25), false).is_err());
        assert!(rating_value(Some(4.5), false).is_ok());
        assert!(rating_value(Some(4.5), true).is_err());
        assert!(validate_years(Some(2020), Some(1999)).is_err());
        assert!(
            numeric_cursor_id(&ExploreCursor {
                value: "100".to_owned(),
                id: "1 OR 1=1".to_owned(),
            })
            .is_err()
        );
    }

    #[test]
    fn track_pages_filter_and_continue_without_overlap() {
        let connection = fixture();
        let request = TrackPageRequest {
            page_size: Some(1),
            artist: Some("Sigur Rós".to_owned()),
            sort: Some(TrackSort::TitleAsc),
            ..TrackPageRequest::default()
        };
        let first = track_page_from_connection(&connection, request, None).expect("first page");
        assert_eq!(first.items.len(), 1);
        let first_id = first.items[0].id.clone();
        assert!(
            track_page_from_connection(
                &connection,
                TrackPageRequest {
                    page_size: Some(1),
                    cursor: first.next_cursor.clone(),
                    sort: Some(TrackSort::Newest),
                    ..TrackPageRequest::default()
                },
                None,
            )
            .is_err()
        );
        let second = track_page_from_connection(
            &connection,
            TrackPageRequest {
                page_size: Some(1),
                cursor: first.next_cursor,
                artist: Some("Sigur Rós".to_owned()),
                sort: Some(TrackSort::TitleAsc),
                ..TrackPageRequest::default()
            },
            None,
        )
        .expect("second page");
        assert_eq!(second.items.len(), 1);
        assert_ne!(second.items[0].id, first_id);

        let loved_five_star = track_page_from_connection(
            &connection,
            TrackPageRequest {
                rating: Some(5.0),
                love_state: Some(LoveState::Loved),
                year_from: Some(2005),
                year_to: Some(2005),
                genre: Some("Post-rock".to_owned()),
                ..TrackPageRequest::default()
            },
            None,
        )
        .expect("filtered tracks");
        assert_eq!(loved_five_star.items.len(), 1);
        assert_eq!(loved_five_star.items[0].title, "Sæglópur");
    }

    #[test]
    fn fts_album_artist_and_album_detail_queries_are_bounded() {
        let connection = fixture();
        let albums = album_page_from_connection(
            &connection,
            AlbumPageRequest {
                search: Some("Takk".to_owned()),
                ..AlbumPageRequest::default()
            },
        )
        .expect("album search");
        assert_eq!(albums.items.len(), 1);
        assert_eq!(albums.items[0].id, "a1");

        let artists = artist_page_from_connection(
            &connection,
            ArtistPageRequest {
                page_size: Some(1),
                sort: Some(ArtistSort::TrackCountDesc),
                ..ArtistPageRequest::default()
            },
        )
        .expect("artist page");
        assert_eq!(artists.items[0].name, "Sigur Rós");
        assert!(artists.next_cursor.is_some());

        let detail = album_detail_from_connection(&connection, "a1", None).expect("album detail");
        assert_eq!(detail.tracks.len(), 2);
        assert_eq!(detail.tracks[0].title, "Sæglópur");
        assert!(!detail.tracks_truncated);
    }

    #[test]
    fn rating_sort_keyset_crosses_from_rated_into_unrated_rows() {
        let connection = fixture();
        let first = track_page_from_connection(
            &connection,
            TrackPageRequest {
                page_size: Some(2),
                sort: Some(TrackSort::RatingDesc),
                ..TrackPageRequest::default()
            },
            None,
        )
        .expect("rated page");
        assert_eq!(
            first
                .items
                .iter()
                .map(|track| track.rating)
                .collect::<Vec<_>>(),
            vec![Some(5.0), Some(4.0)]
        );

        let second = track_page_from_connection(
            &connection,
            TrackPageRequest {
                page_size: Some(2),
                cursor: first.next_cursor,
                sort: Some(TrackSort::RatingDesc),
                ..TrackPageRequest::default()
            },
            None,
        )
        .expect("unrated page");
        assert_eq!(second.items.len(), 2);
        assert!(second.next_cursor.is_none());
        assert!(second.items.iter().any(|track| track.rating.is_none()));
    }
}
