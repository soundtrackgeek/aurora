use crate::{
    catalog::{
        ArtistSummary, TrackSummary, apply_overlays, default_catalog_path, map_track_row,
        open_catalog, parse_catalog_search, push_album_search_predicates,
        push_track_search_predicates,
    },
    ratings,
    state_store::StateStore,
    tag_model::LoveState,
};
use rusqlite::{Connection, Row, params_from_iter, types::Value};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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
    t.album_id, t.file_path, t.filename, t.import_run_id, t.year AS original_year,
    t.publisher AS publisher, t.display_artist AS display_artist,
    t.track_number AS track_number"#;

const TRACK_RATING_EXPRESSION: &str = r#"COALESCE(t.normalized_rating, CASE trim(t.rating_raw)
      WHEN '0.5' THEN 10 WHEN '1' THEN 20 WHEN '1.0' THEN 20
      WHEN '1.5' THEN 30 WHEN '2' THEN 40 WHEN '2.0' THEN 40
      WHEN '2.5' THEN 50 WHEN '3' THEN 60 WHEN '3.0' THEN 60
      WHEN '3.5' THEN 70 WHEN '4' THEN 80 WHEN '4.0' THEN 80
      WHEN '4.5' THEN 90 WHEN '5' THEN 100 WHEN '5.0' THEN 100 END)"#;

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
    Oldest,
    TitleAsc,
    TitleDesc,
    ArtistAsc,
    ArtistDesc,
    AlbumAsc,
    AlbumDesc,
    YearAsc,
    YearDesc,
    ReleaseYearAsc,
    ReleaseYearDesc,
    RatingAsc,
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
    #[serde(default)]
    pub(crate) year_basis: YearBasis,
    #[serde(default)]
    pub(crate) missing_year: bool,
    pub(crate) genre: Option<String>,
    pub(crate) artist: Option<String>,
    pub(crate) sort: Option<TrackSort>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrackPage {
    pub(crate) items: Vec<TrackSummary>,
    pub(crate) next_cursor: Option<ExploreCursor>,
    pub(crate) total_count: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AlbumSort {
    Newest,
    Oldest,
    TitleAsc,
    TitleDesc,
    ArtistAsc,
    ArtistDesc,
    YearAsc,
    #[default]
    YearDesc,
    ReleaseYearAsc,
    ReleaseYearDesc,
    RatingAsc,
    RatingDesc,
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AlbumPageRequest {
    pub(crate) page_size: Option<u16>,
    pub(crate) cursor: Option<ExploreCursor>,
    pub(crate) search: Option<String>,
    pub(crate) rating: Option<f64>,
    #[serde(default)]
    pub(crate) unrated: bool,
    pub(crate) year_from: Option<i32>,
    pub(crate) year_to: Option<i32>,
    #[serde(default)]
    pub(crate) year_basis: YearBasis,
    #[serde(default)]
    pub(crate) missing_year: bool,
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
    pub(crate) original_year: Option<i64>,
    pub(crate) publisher: Option<String>,
    pub(crate) genre: Option<String>,
    pub(crate) total_tracks: i64,
    pub(crate) rated_tracks: i64,
    pub(crate) loved_tracks: i64,
    pub(crate) duration_seconds: i64,
    pub(crate) rating: Option<f64>,
    pub(crate) album_score: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AlbumPage {
    pub(crate) items: Vec<AlbumSummary>,
    pub(crate) next_cursor: Option<ExploreCursor>,
    pub(crate) total_count: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum YearBasis {
    #[default]
    Original,
    Release,
}

impl YearBasis {
    fn track_column(self) -> &'static str {
        match self {
            Self::Original => "t.year",
            Self::Release => "t.release_year",
        }
    }

    fn album_column(self) -> &'static str {
        match self {
            Self::Original => "a.year",
            Self::Release => "a.release_year",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ArtistSort {
    #[default]
    NameAsc,
    NameDesc,
    TrackCountAsc,
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
    pub(crate) total_count: u64,
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
        return Err("Year range is invalid.".to_owned());
    }
    Ok(())
}

fn validate_year_selection(
    year_from: Option<i32>,
    year_to: Option<i32>,
    missing_year: bool,
) -> Result<(), String> {
    validate_years(year_from, year_to)?;
    if missing_year && (year_from.is_some() || year_to.is_some()) {
        return Err("Missing year cannot be combined with a year range.".to_owned());
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

fn push_missing_year_filter(sql: &mut String, column: &str, missing_year: bool) {
    if missing_year {
        sql.push_str(" AND (");
        sql.push_str(column);
        sql.push_str(" IS NULL OR ");
        sql.push_str(column);
        sql.push_str(" NOT BETWEEN 1000 AND 2999)");
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

fn push_order_by(sql: &mut String, sort: SortDefinition, id_expression: &str) {
    let direction = if sort.descending { "DESC" } else { "ASC" };
    sql.push_str(" ORDER BY ");
    if matches!(sort.cursor_kind, CursorKind::NullableInteger) {
        sql.push('(');
        sql.push_str(sort.expression);
        sql.push_str(") IS NULL ASC, ");
    }
    sql.push('(');
    sql.push_str(sort.expression);
    sql.push_str(") ");
    sql.push_str(direction);
    sql.push_str(", ");
    sql.push_str(id_expression);
    sql.push(' ');
    sql.push_str(direction);
}

fn track_sort(sort: TrackSort) -> SortDefinition {
    match sort {
        TrackSort::Newest => SortDefinition {
            cursor_tag: "track-added-desc",
            expression: "t.id",
            descending: true,
            cursor_kind: CursorKind::Integer,
        },
        TrackSort::Oldest => SortDefinition {
            cursor_tag: "track-added-asc",
            expression: "t.id",
            descending: false,
            cursor_kind: CursorKind::Integer,
        },
        TrackSort::TitleAsc => SortDefinition {
            cursor_tag: "track-title-asc",
            expression: "COALESCE(t.title, '') COLLATE NOCASE",
            descending: false,
            cursor_kind: CursorKind::Text,
        },
        TrackSort::TitleDesc => SortDefinition {
            cursor_tag: "track-title-desc",
            expression: "COALESCE(t.title, '') COLLATE NOCASE",
            descending: true,
            cursor_kind: CursorKind::Text,
        },
        TrackSort::ArtistAsc => SortDefinition {
            cursor_tag: "track-artist-asc",
            expression: "(COALESCE(t.album_artist_display, '') || char(31) || COALESCE(t.title, '')) COLLATE NOCASE",
            descending: false,
            cursor_kind: CursorKind::Text,
        },
        TrackSort::ArtistDesc => SortDefinition {
            cursor_tag: "track-artist-desc",
            expression: "(COALESCE(t.album_artist_display, '') || char(31) || COALESCE(t.title, '')) COLLATE NOCASE",
            descending: true,
            cursor_kind: CursorKind::Text,
        },
        TrackSort::AlbumAsc => SortDefinition {
            cursor_tag: "track-album-asc",
            expression: "(COALESCE(t.album, '') || char(31) || COALESCE(t.title, '')) COLLATE NOCASE",
            descending: false,
            cursor_kind: CursorKind::Text,
        },
        TrackSort::AlbumDesc => SortDefinition {
            cursor_tag: "track-album-desc",
            expression: "(COALESCE(t.album, '') || char(31) || COALESCE(t.title, '')) COLLATE NOCASE",
            descending: true,
            cursor_kind: CursorKind::Text,
        },
        TrackSort::YearAsc => SortDefinition {
            cursor_tag: "track-year-asc",
            expression: "CASE WHEN t.year BETWEEN 1000 AND 2999 THEN t.year END",
            descending: false,
            cursor_kind: CursorKind::NullableInteger,
        },
        TrackSort::YearDesc => SortDefinition {
            cursor_tag: "track-year-desc",
            expression: "CASE WHEN t.year BETWEEN 1000 AND 2999 THEN t.year END",
            descending: true,
            cursor_kind: CursorKind::NullableInteger,
        },
        TrackSort::ReleaseYearAsc => SortDefinition {
            cursor_tag: "track-release-year-asc",
            expression: "CASE WHEN t.release_year BETWEEN 1000 AND 2999 THEN t.release_year END",
            descending: false,
            cursor_kind: CursorKind::NullableInteger,
        },
        TrackSort::ReleaseYearDesc => SortDefinition {
            cursor_tag: "track-release-year-desc",
            expression: "CASE WHEN t.release_year BETWEEN 1000 AND 2999 THEN t.release_year END",
            descending: true,
            cursor_kind: CursorKind::NullableInteger,
        },
        TrackSort::RatingAsc => SortDefinition {
            cursor_tag: "track-rating-asc",
            expression: TRACK_RATING_EXPRESSION,
            descending: false,
            cursor_kind: CursorKind::NullableInteger,
        },
        TrackSort::RatingDesc => SortDefinition {
            cursor_tag: "track-rating-desc",
            expression: TRACK_RATING_EXPRESSION,
            descending: true,
            cursor_kind: CursorKind::NullableInteger,
        },
    }
}

fn filtered_row_count(
    connection: &Connection,
    filtered_sql: &str,
    params: &[Value],
    explorer_name: &str,
) -> Result<u64, String> {
    let count_sql = format!("SELECT COUNT(*) FROM ({filtered_sql}) AS filtered_rows");
    let count = connection
        .query_row(&count_sql, params_from_iter(params.iter()), |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| format!("Could not count the {explorer_name} explorer: {error}"))?;
    u64::try_from(count)
        .map_err(|_| format!("The {explorer_name} explorer returned an invalid result count."))
}

fn track_page_from_connection(
    connection: &Connection,
    request: TrackPageRequest,
    store: Option<&StateStore>,
) -> Result<TrackPage, String> {
    let page_size = page_size(request.page_size)?;
    validate_year_selection(request.year_from, request.year_to, request.missing_year)?;
    let rating = rating_value(request.rating, request.unrated)?;
    let genre = bounded_optional_text(&request.genre, "Genre filter", MAX_FILTER_CHARS)?;
    let artist = bounded_optional_text(&request.artist, "Artist filter", MAX_FILTER_CHARS)?;
    let search = bounded_optional_text(&request.search, "Search text", MAX_SEARCH_CHARS)?;
    let parsed_search = search.as_deref().map(parse_catalog_search).transpose()?;
    if parsed_search
        .as_ref()
        .is_some_and(|search| search.is_empty())
    {
        return Ok(TrackPage {
            items: Vec::new(),
            next_cursor: None,
            total_count: 0,
        });
    }
    let sort = track_sort(request.sort.unwrap_or_default());
    let mut params = Vec::<Value>::new();
    let mut predicates = String::from(" WHERE 1 = 1");
    if let Some(search) = &parsed_search {
        push_track_search_predicates(&mut predicates, &mut params, search);
    }
    if let Some(rating) = rating {
        predicates.push_str(" AND t.normalized_rating = ?");
        params.push(Value::Integer(rating));
    } else if request.unrated {
        predicates.push_str(" AND (t.normalized_rating IS NULL OR t.normalized_rating <= 0)");
    }
    if let Some(love_state) = request.love_state {
        match love_state {
            LoveState::Loved => predicates.push_str(" AND t.love = 'L'"),
            LoveState::Banned => predicates.push_str(" AND t.love = 'B'"),
            LoveState::Neutral => {
                predicates.push_str(" AND COALESCE(NULLIF(trim(t.love), ''), '') NOT IN ('L', 'B')")
            }
        }
    }
    push_year_filters(
        &mut predicates,
        &mut params,
        request.year_basis.track_column(),
        request.year_from,
        request.year_to,
    );
    push_missing_year_filter(
        &mut predicates,
        request.year_basis.track_column(),
        request.missing_year,
    );
    if let Some(genre) = genre {
        push_exact_filter(&mut predicates, &mut params, "t.canonical_genre", genre);
    }
    if let Some(artist) = artist {
        push_exact_filter(
            &mut predicates,
            &mut params,
            "t.album_artist_display",
            artist,
        );
    }
    let total_count = filtered_row_count(
        connection,
        &format!("SELECT t.id FROM tracks AS t{predicates}"),
        &params,
        "track",
    )?;
    let mut sql = format!(
        "SELECT {TRACK_COLUMNS}, COALESCE(CAST(({}) AS TEXT), 'unrated') AS cursor_value FROM tracks AS t LEFT JOIN lastfm_track_popularity AS l ON l.artist_key = lower(trim(t.album_artist_display)) AND l.track_key = lower(trim(t.title)){predicates}",
        sort.expression
    );
    push_keyset(
        &mut sql,
        &mut params,
        request.cursor.as_ref(),
        sort,
        "t.id",
        true,
    )?;
    push_order_by(&mut sql, sort, "t.id");
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
            row.get("cursor_value")
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
    Ok(TrackPage {
        items,
        next_cursor,
        total_count,
    })
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
        original_year: row.get(11)?,
        publisher: match row.as_ref().column_index("publisher") {
            Ok(index) => row.get(index)?,
            Err(_) => None,
        },
        genre: row.get(4)?,
        total_tracks: row.get(5)?,
        rated_tracks: row.get(6)?,
        loved_tracks: row.get(7)?,
        duration_seconds: row.get(8)?,
        rating: effective_rating.map(|rating| rating as f64 / 20.0),
        album_score: row.get(9)?,
    })
}

fn album_sort(sort: AlbumSort) -> SortDefinition {
    match sort {
        AlbumSort::Newest => SortDefinition {
            cursor_tag: "album-added-desc",
            expression: "COALESCE((SELECT addition.added_at_ms FROM aurora_state.album_additions AS addition WHERE addition.album_id = a.id), (SELECT MAX(album_track.id) FROM tracks AS album_track WHERE album_track.album_id = a.id))",
            descending: true,
            cursor_kind: CursorKind::Integer,
        },
        AlbumSort::Oldest => SortDefinition {
            cursor_tag: "album-added-asc",
            expression: "COALESCE((SELECT addition.added_at_ms FROM aurora_state.album_additions AS addition WHERE addition.album_id = a.id), (SELECT MAX(album_track.id) FROM tracks AS album_track WHERE album_track.album_id = a.id))",
            descending: false,
            cursor_kind: CursorKind::Integer,
        },
        AlbumSort::TitleAsc => SortDefinition {
            cursor_tag: "album-title-asc",
            expression: "COALESCE(a.album, '') COLLATE NOCASE",
            descending: false,
            cursor_kind: CursorKind::Text,
        },
        AlbumSort::TitleDesc => SortDefinition {
            cursor_tag: "album-title-desc",
            expression: "COALESCE(a.album, '') COLLATE NOCASE",
            descending: true,
            cursor_kind: CursorKind::Text,
        },
        AlbumSort::ArtistAsc => SortDefinition {
            cursor_tag: "album-artist-asc",
            expression: "(COALESCE(a.album_artist_display, '') || char(31) || COALESCE(a.album, '')) COLLATE NOCASE",
            descending: false,
            cursor_kind: CursorKind::Text,
        },
        AlbumSort::ArtistDesc => SortDefinition {
            cursor_tag: "album-artist-desc",
            expression: "(COALESCE(a.album_artist_display, '') || char(31) || COALESCE(a.album, '')) COLLATE NOCASE",
            descending: true,
            cursor_kind: CursorKind::Text,
        },
        AlbumSort::YearAsc => SortDefinition {
            cursor_tag: "album-year-asc",
            expression: "CASE WHEN a.year BETWEEN 1000 AND 2999 THEN a.year END",
            descending: false,
            cursor_kind: CursorKind::NullableInteger,
        },
        AlbumSort::YearDesc => SortDefinition {
            cursor_tag: "album-year-desc",
            expression: "CASE WHEN a.year BETWEEN 1000 AND 2999 THEN a.year END",
            descending: true,
            cursor_kind: CursorKind::NullableInteger,
        },
        AlbumSort::ReleaseYearAsc => SortDefinition {
            cursor_tag: "album-release-year-asc",
            expression: "CASE WHEN a.release_year BETWEEN 1000 AND 2999 THEN a.release_year END",
            descending: false,
            cursor_kind: CursorKind::NullableInteger,
        },
        AlbumSort::ReleaseYearDesc => SortDefinition {
            cursor_tag: "album-release-year-desc",
            expression: "CASE WHEN a.release_year BETWEEN 1000 AND 2999 THEN a.release_year END",
            descending: true,
            cursor_kind: CursorKind::NullableInteger,
        },
        AlbumSort::RatingAsc => SortDefinition {
            cursor_tag: "album-rating-asc",
            expression: "a.effective_album_rating",
            descending: false,
            cursor_kind: CursorKind::NullableInteger,
        },
        AlbumSort::RatingDesc => SortDefinition {
            cursor_tag: "album-rating-desc",
            expression: "a.effective_album_rating",
            descending: true,
            cursor_kind: CursorKind::NullableInteger,
        },
    }
}

fn album_page_from_connection(
    connection: &Connection,
    request: AlbumPageRequest,
) -> Result<AlbumPage, String> {
    let page_size = page_size(request.page_size)?;
    validate_year_selection(request.year_from, request.year_to, request.missing_year)?;
    let rating = rating_value(request.rating, request.unrated)?;
    let genre = bounded_optional_text(&request.genre, "Genre filter", MAX_FILTER_CHARS)?;
    let artist = bounded_optional_text(&request.artist, "Artist filter", MAX_FILTER_CHARS)?;
    let search = bounded_optional_text(&request.search, "Search text", MAX_SEARCH_CHARS)?;
    let parsed_search = search.as_deref().map(parse_catalog_search).transpose()?;
    if parsed_search
        .as_ref()
        .is_some_and(|search| search.is_empty())
    {
        return Ok(AlbumPage {
            items: Vec::new(),
            next_cursor: None,
            total_count: 0,
        });
    }
    let plain_match_query = parsed_search
        .as_ref()
        .and_then(|search| search.plain_fts_query().map(str::to_owned));
    let sort = album_sort(request.sort.unwrap_or_default());
    let mut params = Vec::<Value>::new();
    let mut sql = format!(
        "SELECT a.id, a.album, a.album_artist_display, a.release_year, a.canonical_genre, a.total_tracks, a.rated_tracks, a.loved_tracks, a.total_seconds, a.album_score, a.effective_album_rating, a.year, a.publisher AS publisher, COALESCE(CAST(({}) AS TEXT), 'unrated') AS cursor_value FROM albums AS a",
        sort.expression
    );
    if plain_match_query.is_some() {
        sql.push_str(" JOIN album_search_fts ON album_search_fts.album_id = a.id");
    }
    sql.push_str(" WHERE 1 = 1");
    if let Some(match_query) = plain_match_query {
        sql.push_str(" AND album_search_fts MATCH ?");
        params.push(Value::Text(match_query));
    } else if let Some(search) = &parsed_search {
        push_album_search_predicates(&mut sql, &mut params, search);
    }
    if let Some(rating) = rating {
        sql.push_str(" AND CAST(ROUND(a.effective_album_rating / 10.0) AS INTEGER) * 10 = ?");
        params.push(Value::Integer(rating));
    } else if request.unrated {
        sql.push_str(" AND (a.effective_album_rating IS NULL OR a.effective_album_rating <= 0)");
    }
    push_year_filters(
        &mut sql,
        &mut params,
        request.year_basis.album_column(),
        request.year_from,
        request.year_to,
    );
    push_missing_year_filter(
        &mut sql,
        request.year_basis.album_column(),
        request.missing_year,
    );
    if let Some(genre) = genre {
        push_exact_filter(&mut sql, &mut params, "a.canonical_genre", genre);
    }
    if let Some(artist) = artist {
        sql.push_str(" AND a.album_artist_display = ?");
        params.push(Value::Text(artist));
    }
    let total_count = filtered_row_count(connection, &sql, &params, "album")?;
    push_keyset(
        &mut sql,
        &mut params,
        request.cursor.as_ref(),
        sort,
        "a.id",
        false,
    )?;
    push_order_by(&mut sql, sort, "a.id");
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
            row.get(13)
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
        total_count,
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
            cursor_tag: "artist-name-asc",
            expression: "r.name COLLATE NOCASE",
            descending: false,
            cursor_kind: CursorKind::Text,
        },
        ArtistSort::NameDesc => SortDefinition {
            cursor_tag: "artist-name-desc",
            expression: "r.name COLLATE NOCASE",
            descending: true,
            cursor_kind: CursorKind::Text,
        },
        ArtistSort::TrackCountAsc => SortDefinition {
            cursor_tag: "artist-count-asc",
            expression: "r.track_count",
            descending: false,
            cursor_kind: CursorKind::Integer,
        },
        ArtistSort::TrackCountDesc => SortDefinition {
            cursor_tag: "artist-count-desc",
            expression: "r.track_count",
            descending: true,
            cursor_kind: CursorKind::Integer,
        },
    };
    let mut params = Vec::<Value>::new();
    let mut rollup_where =
        String::from(" WHERE NULLIF(trim(a.album_artist_display), '') IS NOT NULL");
    if let Some(search) = search {
        let parsed_search = parse_catalog_search(&search)?;
        if parsed_search.is_empty() {
            return Ok(ArtistPage {
                items: Vec::new(),
                next_cursor: None,
                total_count: 0,
            });
        }
        if parsed_search.plain_fts_query().is_some() {
            rollup_where.push_str(" AND a.album_artist_display LIKE ? ESCAPE '\\' COLLATE NOCASE");
            let escaped = search
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_");
            params.push(Value::Text(format!("%{escaped}%")));
        } else {
            push_album_search_predicates(&mut rollup_where, &mut params, &parsed_search);
        }
    }
    if let Some(genre) = genre {
        push_exact_filter(&mut rollup_where, &mut params, "a.canonical_genre", genre);
    }
    let mut sql = format!(
        "WITH r AS (SELECT a.album_artist_display AS name, CAST(SUM(a.total_tracks) AS INTEGER) AS track_count, COUNT(*) AS album_count FROM albums AS a {rollup_where} GROUP BY a.album_artist_display COLLATE NOCASE) SELECT r.name, r.track_count, r.album_count, CAST(({}) AS TEXT) AS cursor_value FROM r WHERE 1 = 1",
        definition.expression
    );
    let total_count = filtered_row_count(connection, &sql, &params, "artist")?;
    push_keyset(
        &mut sql,
        &mut params,
        request.cursor.as_ref(),
        definition,
        "r.name COLLATE NOCASE",
        false,
    )?;
    push_order_by(&mut sql, definition, "r.name COLLATE NOCASE");
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
        total_count,
    })
}

pub(crate) fn load_track_page(
    request: TrackPageRequest,
    store: &StateStore,
) -> Result<TrackPage, String> {
    let connection = open_catalog(&default_catalog_path()?)?;
    track_page_from_connection(&connection, request, Some(store))
}

pub(crate) fn load_album_page(
    request: AlbumPageRequest,
    store: &StateStore,
) -> Result<AlbumPage, String> {
    let connection = open_catalog(&default_catalog_path()?)?;
    connection
        .execute(
            "ATTACH DATABASE ?1 AS aurora_state",
            [store.path().to_string_lossy().as_ref()],
        )
        .map_err(|error| format!("Could not attach Aurora's album additions: {error}"))?;
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
    let mut album = connection
        .query_row(
            "SELECT a.id, a.album, a.album_artist_display, a.release_year, a.canonical_genre, a.total_tracks, a.rated_tracks, a.loved_tracks, a.total_seconds, a.album_score, a.effective_album_rating, a.year, a.publisher AS publisher FROM albums AS a WHERE a.id = ?",
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
    apply_overlays(&mut tracks, store)?;
    let deleted_track_keys = if let Some(store) = store {
        ratings::pending_deleted_track_keys_for_album(connection, album_id, store)?
    } else {
        HashSet::new()
    };
    tracks.retain(|track| !deleted_track_keys.contains(&track.track_key));
    let tracks_truncated = tracks.len() > usize::from(MAX_PAGE_SIZE);
    tracks.truncate(usize::from(MAX_PAGE_SIZE));
    if let Some(store) = store {
        let live =
            ratings::live_album_from_connection(connection, album_id, store, &deleted_track_keys)?;
        album.total_tracks = live.total_tracks;
        album.rated_tracks = live.rated_tracks;
        album.loved_tracks = live.loved_tracks;
        album.duration_seconds = live.duration_seconds;
        album.rating = live.effective_rating;
        album.album_score = live.album_score;
    }
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
            sort: Some(AlbumSort::YearDesc),
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
                  canonical_genre TEXT, publisher TEXT, love TEXT, rating_raw TEXT, normalized_rating INTEGER,
                  release_year INTEGER, time_seconds INTEGER, file_path TEXT, filename TEXT,
                  disc_number INTEGER, track_number INTEGER, year INTEGER
                );
                CREATE TABLE albums (
                  id TEXT PRIMARY KEY, album TEXT, album_artist_display TEXT, canonical_genre TEXT,
                  release_year INTEGER, total_tracks INTEGER NOT NULL, rated_tracks INTEGER NOT NULL,
                  loved_tracks INTEGER NOT NULL, total_seconds INTEGER NOT NULL,
                  album_score REAL, effective_album_rating INTEGER, year INTEGER, publisher TEXT
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
                ATTACH DATABASE ':memory:' AS aurora_state;
                CREATE TABLE aurora_state.album_additions (
                  album_id TEXT PRIMARY KEY, added_at_ms INTEGER NOT NULL
                );
                INSERT INTO albums VALUES
                  ('a1', 'Takk...', 'Sigur Rós', 'Post-rock', 2005, 2, 1, 1, 741, 95, 90, 1999, 'EMI Records'),
                  ('a2', 'Ágætis byrjun', 'Sigur Rós', 'Post-rock', 1999, 1, 0, 0, 426, 80, NULL, 1999, 'FatCat'),
                  ('a3', 'Discovery', 'Daft Punk', 'House', 2001, 1, 1, 1, 301, 88, 80, 2001, 'Virgin');
                INSERT INTO tracks VALUES
                  (7, 1, 'a1', 'Sæglópur', 'Jónsi', 'Sigur Rós', 'Takk...', 'Post-rock', 'EMI Records', 'L', '5', 100, 2005, 473, 'H:\Music\Sigur Rós', '01.mp3', 1, 1, 1999),
                  (8, 1, 'a1', 'Hoppípolla', 'Sigur Rós', 'Sigur Rós', 'Takk...', 'Post-rock', 'EMI Records', NULL, '', NULL, 2005, 268, 'H:\Music\Sigur Rós', '02.mp3', 1, 2, 1999),
                  (9, 1, 'a2', 'Svefn-g-englar', 'Sigur Rós', 'Sigur Rós', 'Ágætis byrjun', 'Post-rock', 'FatCat', 'B', '4.5', NULL, 1999, 426, 'H:\Music\Sigur Rós', '03.mp3', 1, 1, 1999),
                  (10, 1, 'a3', 'Digital Love', 'Daft Punk', 'Daft Punk', 'Discovery', 'House', 'Virgin', 'L', '4', 80, 2001, 301, 'H:\Music\Daft Punk', '01.mp3', 1, 1, 2001);
                INSERT INTO track_search_fts
                  SELECT id, album_id, title, display_artist, album, album_artist_display,
                         canonical_genre, publisher, file_path, filename FROM tracks;
                INSERT INTO album_search_fts
                  SELECT id, album, album_artist_display, canonical_genre, publisher FROM albums;
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
        assert_eq!(YearBasis::default(), YearBasis::Original);
        assert_eq!(AlbumSort::default(), AlbumSort::YearDesc);
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
    fn explorer_sorts_reverse_chronological_and_alphabetical_order() {
        let connection = fixture();

        let track_year_asc = track_page_from_connection(
            &connection,
            TrackPageRequest {
                sort: Some(TrackSort::YearAsc),
                ..TrackPageRequest::default()
            },
            None,
        )
        .expect("ascending track years");
        let track_year_desc = track_page_from_connection(
            &connection,
            TrackPageRequest {
                sort: Some(TrackSort::YearDesc),
                ..TrackPageRequest::default()
            },
            None,
        )
        .expect("descending track years");
        let mut reversed_track_ids = track_year_desc
            .items
            .iter()
            .map(|track| track.id.as_str())
            .collect::<Vec<_>>();
        reversed_track_ids.reverse();
        assert_eq!(
            track_year_asc
                .items
                .iter()
                .map(|track| track.id.as_str())
                .collect::<Vec<_>>(),
            reversed_track_ids
        );

        let track_title_asc = track_page_from_connection(
            &connection,
            TrackPageRequest {
                sort: Some(TrackSort::TitleAsc),
                ..TrackPageRequest::default()
            },
            None,
        )
        .expect("ascending track titles");
        let track_title_desc = track_page_from_connection(
            &connection,
            TrackPageRequest {
                sort: Some(TrackSort::TitleDesc),
                ..TrackPageRequest::default()
            },
            None,
        )
        .expect("descending track titles");
        assert_eq!(
            track_title_asc
                .items
                .first()
                .map(|track| track.title.as_str()),
            track_title_desc
                .items
                .last()
                .map(|track| track.title.as_str())
        );

        let album_year_asc = album_page_from_connection(
            &connection,
            AlbumPageRequest {
                sort: Some(AlbumSort::YearAsc),
                ..AlbumPageRequest::default()
            },
        )
        .expect("ascending album years");
        let album_year_desc = album_page_from_connection(
            &connection,
            AlbumPageRequest {
                sort: Some(AlbumSort::YearDesc),
                ..AlbumPageRequest::default()
            },
        )
        .expect("descending album years");
        let mut reversed_album_ids = album_year_desc
            .items
            .iter()
            .map(|album| album.id.as_str())
            .collect::<Vec<_>>();
        reversed_album_ids.reverse();
        assert_eq!(
            album_year_asc
                .items
                .iter()
                .map(|album| album.id.as_str())
                .collect::<Vec<_>>(),
            reversed_album_ids
        );

        connection
            .execute(
                "INSERT INTO aurora_state.album_additions VALUES ('a1', 1800000000000)",
                [],
            )
            .expect("recorded album addition");
        let album_added_asc = album_page_from_connection(
            &connection,
            AlbumPageRequest {
                sort: Some(AlbumSort::Oldest),
                ..AlbumPageRequest::default()
            },
        )
        .expect("oldest added albums");
        let album_added_desc = album_page_from_connection(
            &connection,
            AlbumPageRequest {
                sort: Some(AlbumSort::Newest),
                ..AlbumPageRequest::default()
            },
        )
        .expect("newest added albums");
        let mut reversed_added_album_ids = album_added_desc
            .items
            .iter()
            .map(|album| album.id.as_str())
            .collect::<Vec<_>>();
        reversed_added_album_ids.reverse();
        assert_eq!(
            album_added_asc
                .items
                .iter()
                .map(|album| album.id.as_str())
                .collect::<Vec<_>>(),
            reversed_added_album_ids
        );
        assert_eq!(
            album_added_desc
                .items
                .first()
                .map(|album| album.id.as_str()),
            Some("a1")
        );

        let artist_name_asc = artist_page_from_connection(
            &connection,
            ArtistPageRequest {
                sort: Some(ArtistSort::NameAsc),
                ..ArtistPageRequest::default()
            },
        )
        .expect("ascending artist names");
        let artist_name_desc = artist_page_from_connection(
            &connection,
            ArtistPageRequest {
                sort: Some(ArtistSort::NameDesc),
                ..ArtistPageRequest::default()
            },
        )
        .expect("descending artist names");
        let mut reversed_artist_ids = artist_name_desc
            .items
            .iter()
            .map(|artist| artist.id.as_str())
            .collect::<Vec<_>>();
        reversed_artist_ids.reverse();
        assert_eq!(
            artist_name_asc
                .items
                .iter()
                .map(|artist| artist.id.as_str())
                .collect::<Vec<_>>(),
            reversed_artist_ids
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
        assert_eq!(first.total_count, 3);
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
        assert_eq!(second.total_count, 3);
        assert_ne!(second.items[0].id, first_id);

        let loved_five_star = track_page_from_connection(
            &connection,
            TrackPageRequest {
                rating: Some(5.0),
                love_state: Some(LoveState::Loved),
                year_from: Some(2005),
                year_to: Some(2005),
                year_basis: YearBasis::Release,
                genre: Some("Post-rock".to_owned()),
                ..TrackPageRequest::default()
            },
            None,
        )
        .expect("filtered tracks");
        assert_eq!(loved_five_star.items.len(), 1);
        assert_eq!(loved_five_star.items[0].title, "Sæglópur");

        let original_year = track_page_from_connection(
            &connection,
            TrackPageRequest {
                year_from: Some(1999),
                year_to: Some(1999),
                year_basis: YearBasis::Original,
                artist: Some("Sigur Rós".to_owned()),
                ..TrackPageRequest::default()
            },
            None,
        )
        .expect("original-year tracks");
        assert_eq!(original_year.items.len(), 3);
        assert!(
            original_year
                .items
                .iter()
                .all(|track| track.original_year == Some(1999))
        );
    }

    #[test]
    fn track_search_fields_map_to_distinct_catalog_columns_and_combine() {
        let connection = fixture();
        let page = track_page_from_connection(
            &connection,
            TrackPageRequest {
                search: Some("artist:jónsi,aartist:sigur rós,album:takk,genre:post rock,year:1999,ryear:2005,publisher:emi,title:sæglópur".to_owned()),
                ..TrackPageRequest::default()
            },
            None,
        )
        .expect("fielded track search");

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].title, "Sæglópur");
        assert_eq!(page.items[0].display_artist.as_deref(), Some("Jónsi"));
        assert_eq!(page.items[0].original_year, Some(1999));
        assert_eq!(page.items[0].release_year, Some(2005));

        let publisher_year = track_page_from_connection(
            &connection,
            TrackPageRequest {
                search: Some("publisher:emi,year:1999".to_owned()),
                ..TrackPageRequest::default()
            },
            None,
        )
        .expect("publisher search using Year");
        assert_eq!(publisher_year.items.len(), 2);
        assert!(
            publisher_year
                .items
                .iter()
                .all(|track| track.original_year == Some(1999))
        );

        let year_range = track_page_from_connection(
            &connection,
            TrackPageRequest {
                search: Some("year:1999..2000".to_owned()),
                ..TrackPageRequest::default()
            },
            None,
        )
        .expect("Year range search");
        assert_eq!(year_range.items.len(), 3);
        assert!(
            year_range
                .items
                .iter()
                .all(|track| track.original_year == Some(1999))
        );

        let release_year_range = track_page_from_connection(
            &connection,
            TrackPageRequest {
                search: Some("ryear:1999..2001".to_owned()),
                ..TrackPageRequest::default()
            },
            None,
        )
        .expect("Release Year range search");
        assert_eq!(release_year_range.items.len(), 2);
        assert!(release_year_range.items.iter().all(|track| {
            track
                .release_year
                .is_some_and(|year| (1999..=2001).contains(&year))
        }));

        let wrong_artist = track_page_from_connection(
            &connection,
            TrackPageRequest {
                search: Some("artist:sigur rós,title:sæglópur".to_owned()),
                ..TrackPageRequest::default()
            },
            None,
        )
        .expect("display artist search");
        assert!(wrong_artist.items.is_empty());

        let albums = album_page_from_connection(
            &connection,
            AlbumPageRequest {
                search: Some("artist:jónsi,year:1999".to_owned()),
                ..AlbumPageRequest::default()
            },
        )
        .expect("fielded album search");
        assert_eq!(
            albums
                .items
                .iter()
                .map(|album| album.id.as_str())
                .collect::<Vec<_>>(),
            vec!["a1"]
        );

        let range_albums = album_page_from_connection(
            &connection,
            AlbumPageRequest {
                search: Some("year:1999..2000".to_owned()),
                ..AlbumPageRequest::default()
            },
        )
        .expect("album Year range search");
        assert_eq!(
            range_albums
                .items
                .iter()
                .map(|album| album.id.as_str())
                .collect::<Vec<_>>(),
            vec!["a2", "a1"]
        );

        let artists = artist_page_from_connection(
            &connection,
            ArtistPageRequest {
                search: Some("title:sæglópur,publisher:emi".to_owned()),
                ..ArtistPageRequest::default()
            },
        )
        .expect("fielded artist search");
        assert_eq!(artists.items.len(), 1);
        assert_eq!(artists.items[0].name, "Sigur Rós");
    }

    #[test]
    fn track_search_supports_or_not_negative_prefix_and_exact_values() {
        let connection = fixture();
        let boolean = track_page_from_connection(
            &connection,
            TrackPageRequest {
                search: Some("genre:post rock OR house NOT aartist:daft punk".to_owned()),
                ..TrackPageRequest::default()
            },
            None,
        )
        .expect("boolean track search");
        assert_eq!(boolean.items.len(), 3);
        assert!(
            boolean
                .items
                .iter()
                .all(|track| track.artist == "Sigur Rós")
        );

        let boolean_albums = album_page_from_connection(
            &connection,
            AlbumPageRequest {
                search: Some("genre:post rock OR house NOT aartist:daft punk".to_owned()),
                ..AlbumPageRequest::default()
            },
        )
        .expect("boolean album search");
        assert_eq!(boolean_albums.items.len(), 2);
        assert!(
            boolean_albums
                .items
                .iter()
                .all(|album| album.artist == "Sigur Rós")
        );

        let boolean_artists = artist_page_from_connection(
            &connection,
            ArtistPageRequest {
                search: Some("genre:post rock OR house NOT aartist:daft punk".to_owned()),
                ..ArtistPageRequest::default()
            },
        )
        .expect("boolean artist search");
        assert_eq!(boolean_artists.items.len(), 1);
        assert_eq!(boolean_artists.items[0].name, "Sigur Rós");

        let excluded = track_page_from_connection(
            &connection,
            TrackPageRequest {
                search: Some("genre:post rock,-aartist:sigur rós".to_owned()),
                ..TrackPageRequest::default()
            },
            None,
        )
        .expect("negative prefix search");
        assert!(excluded.items.is_empty());

        connection
            .execute_batch(
                r#"
                INSERT INTO albums VALUES
                  ('a4', 'Kiss', 'Kiss', 'Rock', 1974, 1, 0, 0, 180, NULL, NULL, 1974, 'Casablanca'),
                  ('a5', 'Certain Things Are Likely', 'Kissing the Pink', 'Synth-pop', 1986, 1, 0, 0, 210, NULL, NULL, 1986, 'Magnet'),
                  ('a6', 'Film Music', 'Composer', 'Drama', 2020, 1, 0, 0, 180, NULL, NULL, 2020, 'Label'),
                  ('a7', 'Compilation', 'Various Artists', 'Soundtrack', 2020, 1, 0, 0, 180, NULL, NULL, 2020, 'Label');
                INSERT INTO tracks VALUES
                  (11, 1, 'a4', 'Strutter', 'Kiss', 'Kiss', 'Kiss', 'Rock', 'Casablanca', NULL, '', NULL, 1974, 180, 'H:\Music\Kiss', '01.mp3', 1, 1, 1974),
                  (12, 1, 'a5', 'Certain Things Are Likely', 'Kissing the Pink', 'Kissing the Pink', 'Certain Things Are Likely', 'Synth-pop', 'Magnet', NULL, '', NULL, 1986, 210, 'H:\Music\Kissing the Pink', '01.mp3', 1, 1, 1986),
                  (13, 1, 'a6', 'Main Theme', 'Composer', 'Composer', 'Film Music', 'Drama', 'Label', NULL, '', NULL, 2020, 180, 'H:\Music\Composer', '01.mp3', 1, 1, 2020),
                  (14, 1, 'a7', 'Pop Song', 'Singer', 'Various Artists', 'Compilation', 'Soundtrack', 'Label', NULL, '', NULL, 2020, 180, 'H:\Music\Various', '01.mp3', 1, 1, 2020);
                INSERT INTO track_search_fts
                  SELECT id, album_id, title, display_artist, album, album_artist_display,
                         canonical_genre, publisher, file_path, filename FROM tracks WHERE id IN (11, 12, 13, 14);
                "#,
            )
            .expect("exact-search fixture rows");

        let prefix = track_page_from_connection(
            &connection,
            TrackPageRequest {
                search: Some("aartist:kiss".to_owned()),
                ..TrackPageRequest::default()
            },
            None,
        )
        .expect("prefix artist search");
        assert_eq!(prefix.items.len(), 2);

        let exact = track_page_from_connection(
            &connection,
            TrackPageRequest {
                search: Some("aartist:\"kiss\"".to_owned()),
                ..TrackPageRequest::default()
            },
            None,
        )
        .expect("exact artist search");
        assert_eq!(exact.items.len(), 1);
        assert_eq!(exact.items[0].artist, "Kiss");

        let scores = track_page_from_connection(
            &connection,
            TrackPageRequest {
                search: Some("genre:scores".to_owned()),
                ..TrackPageRequest::default()
            },
            None,
        )
        .expect("scores genre search");
        assert_eq!(scores.items.len(), 1);
        assert_eq!(scores.items[0].genre.as_deref(), Some("Drama"));
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

        let four_and_a_half = album_page_from_connection(
            &connection,
            AlbumPageRequest {
                rating: Some(4.5),
                ..AlbumPageRequest::default()
            },
        )
        .expect("album rating band");
        assert_eq!(four_and_a_half.items.len(), 1);
        assert_eq!(four_and_a_half.items[0].id, "a1");

        let unrated = album_page_from_connection(
            &connection,
            AlbumPageRequest {
                unrated: true,
                ..AlbumPageRequest::default()
            },
        )
        .expect("unrated albums");
        assert_eq!(unrated.items.len(), 1);
        assert_eq!(unrated.items[0].id, "a2");

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
        assert_eq!(artists.total_count, 2);

        let detail = album_detail_from_connection(&connection, "a1", None).expect("album detail");
        assert_eq!(detail.tracks.len(), 2);
        assert_eq!(detail.tracks[0].title, "Sæglópur");
        assert_eq!(detail.tracks[0].display_artist.as_deref(), Some("Jónsi"));
        assert!(!detail.tracks_truncated);
    }

    #[test]
    fn album_detail_hides_a_missing_file_while_its_catalog_sync_is_pending() {
        let connection = fixture();
        connection
            .execute("ALTER TABLE albums ADD COLUMN album_rating REAL", [])
            .expect("rating detail fixture column");
        let album_directory = tempfile::TempDir::new().expect("temporary album");
        let directory = album_directory.path().to_string_lossy().into_owned();
        std::fs::write(album_directory.path().join("01.mp3"), b"fixture").expect("available track");
        connection
            .execute(
                "UPDATE tracks SET file_path = ?1 WHERE album_id = 'a1'",
                [&directory],
            )
            .expect("use temporary album paths");

        let state_path = album_directory.path().join("aurora-state.sqlite3");
        let store = StateStore::new(state_path).expect("state store");
        store
            .queue_library_file_syncs(&[(directory, "02.mp3".to_owned())])
            .expect("queue deleted file");

        let detail = album_detail_from_connection(&connection, "a1", Some(&store))
            .expect("album detail with pending deletion");

        assert_eq!(detail.tracks.len(), 1);
        assert_eq!(detail.tracks[0].title, "Sæglópur");
        assert_eq!(detail.album.total_tracks, 1);
        assert_eq!(detail.album.duration_seconds, 473);
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
            vec![Some(5.0), Some(4.5)]
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

        let ascending_first = track_page_from_connection(
            &connection,
            TrackPageRequest {
                page_size: Some(2),
                sort: Some(TrackSort::RatingAsc),
                ..TrackPageRequest::default()
            },
            None,
        )
        .expect("low ratings page");
        assert_eq!(
            ascending_first
                .items
                .iter()
                .map(|track| track.rating)
                .collect::<Vec<_>>(),
            vec![Some(4.0), Some(4.5)]
        );
        let ascending_second = track_page_from_connection(
            &connection,
            TrackPageRequest {
                page_size: Some(2),
                cursor: ascending_first.next_cursor,
                sort: Some(TrackSort::RatingAsc),
                ..TrackPageRequest::default()
            },
            None,
        )
        .expect("ascending unrated page");
        assert_eq!(ascending_second.items.len(), 2);
        assert_eq!(
            ascending_second
                .items
                .iter()
                .map(|track| track.rating)
                .collect::<Vec<_>>(),
            vec![Some(5.0), None]
        );
    }
}
