use crate::{
    catalog::{self, TrackSummary},
    state_store::StateStore,
};
use rusqlite::{Connection, Row, named_params};
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, collections::HashMap};

const MAX_CHART_ITEMS: usize = 100;
const MAX_PERIOD_YEARS: i32 = 20;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ChartKind {
    Singles,
    Albums,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ChartSource {
    OfficialUk,
    VgLista,
    TiISkuddet,
    Norsktoppen,
    Billboard,
    AuroraScore,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ChartScope {
    Week,
    Period,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChartPeriod {
    pub(crate) from_year: i32,
    pub(crate) from_week: u8,
    pub(crate) to_year: i32,
    pub(crate) to_week: u8,
    pub(crate) label: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChartPageRequest {
    pub(crate) kind: ChartKind,
    pub(crate) source: ChartSource,
    pub(crate) scope: ChartScope,
    pub(crate) period: ChartPeriod,
    pub(crate) selected_year: i32,
    pub(crate) selected_week: u8,
    pub(crate) limit: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChartItemDetailRequest {
    pub(crate) page: ChartPageRequest,
    pub(crate) artist_key: String,
    pub(crate) title_key: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChartWeek {
    pub(crate) year: i32,
    pub(crate) week: u8,
    pub(crate) date: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChartEntry {
    pub(crate) position: usize,
    pub(crate) source_position: i64,
    pub(crate) previous_position: Option<i64>,
    pub(crate) movement: Option<i64>,
    pub(crate) peak_position: Option<i64>,
    pub(crate) appearances: u32,
    pub(crate) weeks_at_number_one: u32,
    pub(crate) total_points: i64,
    pub(crate) artist: String,
    pub(crate) title: String,
    pub(crate) artist_key: String,
    pub(crate) title_key: String,
    pub(crate) matched_track_id: Option<String>,
    pub(crate) matched_album_id: Option<String>,
    pub(crate) artwork_album_id: Option<String>,
    pub(crate) rating: Option<f64>,
    pub(crate) loved: bool,
    pub(crate) album_score: Option<f64>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AlbumScoreEntry {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) original_year: Option<i32>,
    pub(crate) release_year: Option<i32>,
    pub(crate) score: f64,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChartPage {
    pub(crate) request: ChartPageRequest,
    pub(crate) source_label: &'static str,
    pub(crate) chart_title: String,
    pub(crate) annual_only: bool,
    pub(crate) chart_date: Option<String>,
    pub(crate) weeks: Vec<ChartWeek>,
    pub(crate) entries: Vec<ChartEntry>,
    pub(crate) total_entries: usize,
    pub(crate) album_score_entries: Vec<AlbumScoreEntry>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChartSourceRank {
    pub(crate) source: ChartSource,
    pub(crate) label: &'static str,
    pub(crate) best_rank: Option<i64>,
    pub(crate) appearances: u32,
    pub(crate) weeks_at_number_one: u32,
    pub(crate) annual_only: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChartItemDetail {
    pub(crate) source_ranks: Vec<ChartSourceRank>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SourceShape {
    Weekly,
    Annual,
    Score,
}

#[derive(Clone, Debug)]
struct RawChartRow {
    _year: i32,
    _week: Option<u8>,
    rank: i64,
    artist: String,
    title: String,
    artist_key: String,
    title_key: String,
    previous_position: Option<i64>,
    peak_position: Option<i64>,
    appearances: Option<u32>,
    matched_track_id: Option<String>,
    matched_album_id: Option<String>,
    artwork_album_id: Option<String>,
    rating: Option<f64>,
    loved: bool,
    album_score: Option<f64>,
    chart_date: Option<String>,
}

#[derive(Debug)]
struct Aggregate {
    representative: RawChartRow,
    rank_counts: [u16; MAX_CHART_ITEMS],
    appearances: u32,
    total_points: i64,
}

fn source_label(source: ChartSource) -> &'static str {
    match source {
        ChartSource::OfficialUk => "Official UK",
        ChartSource::VgLista => "VG Lista",
        ChartSource::TiISkuddet => "Ti i Skuddet",
        ChartSource::Norsktoppen => "Norsktoppen",
        ChartSource::Billboard => "Billboard",
        ChartSource::AuroraScore => "Aurora Score",
    }
}

fn source_shape(source: ChartSource) -> SourceShape {
    match source {
        ChartSource::Billboard => SourceShape::Annual,
        ChartSource::AuroraScore => SourceShape::Score,
        _ => SourceShape::Weekly,
    }
}

fn valid_source(kind: ChartKind, source: ChartSource) -> bool {
    match kind {
        ChartKind::Singles => matches!(
            source,
            ChartSource::OfficialUk
                | ChartSource::VgLista
                | ChartSource::TiISkuddet
                | ChartSource::Norsktoppen
                | ChartSource::Billboard
        ),
        ChartKind::Albums => matches!(
            source,
            ChartSource::OfficialUk
                | ChartSource::VgLista
                | ChartSource::Billboard
                | ChartSource::AuroraScore
        ),
    }
}

fn validate_period(period: &ChartPeriod) -> Result<(), String> {
    if !(1890..=2200).contains(&period.from_year)
        || !(1890..=2200).contains(&period.to_year)
        || !(1..=53).contains(&period.from_week)
        || !(1..=53).contains(&period.to_week)
    {
        return Err("Chart periods require valid years and ISO weeks.".to_owned());
    }
    let start = period.from_year * 100 + i32::from(period.from_week);
    let end = period.to_year * 100 + i32::from(period.to_week);
    if start > end || period.to_year - period.from_year > MAX_PERIOD_YEARS {
        return Err("Chart periods must be chronological and no longer than 20 years.".to_owned());
    }
    if period.label.trim().is_empty() || period.label.chars().count() > 80 {
        return Err("Chart period labels must contain between 1 and 80 characters.".to_owned());
    }
    Ok(())
}

fn validate_request(request: &ChartPageRequest) -> Result<(), String> {
    validate_period(&request.period)?;
    if !valid_source(request.kind, request.source) {
        return Err("That chart source is not available for this chart type.".to_owned());
    }
    if !(1..=53).contains(&request.selected_week) {
        return Err("The selected chart week is invalid.".to_owned());
    }
    if request.limit == 0 || request.limit > MAX_CHART_ITEMS {
        return Err("Chart pages must request between 1 and 100 entries.".to_owned());
    }
    Ok(())
}

fn table_for(kind: ChartKind, source: ChartSource) -> Result<&'static str, String> {
    match (kind, source) {
        (ChartKind::Singles, ChartSource::OfficialUk) => Ok("official_uk_single_chart_entries"),
        (ChartKind::Singles, ChartSource::VgLista) => Ok("vg_lista_single_chart_entries"),
        (ChartKind::Singles, ChartSource::TiISkuddet) => Ok("ti_i_skuddet_chart_entries"),
        (ChartKind::Singles, ChartSource::Norsktoppen) => Ok("norsktoppen_chart_entries"),
        (ChartKind::Singles, ChartSource::Billboard) => Ok("billboard_single_chart_entries"),
        (ChartKind::Albums, ChartSource::OfficialUk) => Ok("official_uk_album_chart_entries"),
        (ChartKind::Albums, ChartSource::VgLista) => Ok("vg_lista_album_chart_entries"),
        (ChartKind::Albums, ChartSource::Billboard) => Ok("billboard_chart_entries"),
        (ChartKind::Albums, ChartSource::AuroraScore) => Ok("albums"),
        _ => Err("That chart source is not available for this chart type.".to_owned()),
    }
}

fn map_raw_row(row: &Row<'_>) -> rusqlite::Result<RawChartRow> {
    Ok(RawChartRow {
        _year: row.get(0)?,
        _week: row.get(1)?,
        rank: row.get(2)?,
        artist: row.get(3)?,
        title: row.get(4)?,
        artist_key: row.get(5)?,
        title_key: row.get(6)?,
        previous_position: row.get(7)?,
        peak_position: row.get(8)?,
        appearances: row.get(9)?,
        matched_track_id: row.get::<_, Option<i64>>(10)?.map(|id| id.to_string()),
        matched_album_id: row.get(11)?,
        artwork_album_id: row.get(12)?,
        rating: row.get(13)?,
        loved: row.get::<_, i64>(14)? > 0,
        album_score: row.get(15)?,
        chart_date: row.get(16)?,
    })
}

fn weekly_select(kind: ChartKind, source: ChartSource, table: &str) -> String {
    let official = source == ChartSource::OfficialUk;
    let date_column = match source {
        ChartSource::OfficialUk | ChartSource::TiISkuddet | ChartSource::Norsktoppen => {
            "e.chart_date"
        }
        ChartSource::VgLista => "e.week_date",
        _ => "NULL",
    };
    let previous = if official {
        "NULLIF(CAST(e.last_week AS INTEGER), 0)"
    } else {
        "NULL"
    };
    let peak = if official {
        "NULLIF(CAST(e.peak AS INTEGER), 0)"
    } else {
        "NULL"
    };
    let appearances = if official {
        "NULLIF(CAST(e.weeks_on_chart AS INTEGER), 0)"
    } else {
        "NULL"
    };
    match kind {
        ChartKind::Singles => format!(
            r#"
            SELECT e.year, e.week, e.rank, e.artist, e.title, e.artist_key, e.title_key,
                   {previous}, {peak}, {appearances}, e.matched_track_id, NULL,
                   t.album_id,
                   CAST(COALESCE(t.normalized_rating, CASE trim(t.rating_raw)
                     WHEN '0.5' THEN 10 WHEN '1' THEN 20 WHEN '1.0' THEN 20
                     WHEN '1.5' THEN 30 WHEN '2' THEN 40 WHEN '2.0' THEN 40
                     WHEN '2.5' THEN 50 WHEN '3' THEN 60 WHEN '3.0' THEN 60
                     WHEN '3.5' THEN 70 WHEN '4' THEN 80 WHEN '4.0' THEN 80
                     WHEN '4.5' THEN 90 WHEN '5' THEN 100 WHEN '5.0' THEN 100 END) AS REAL) / 20.0,
                   CASE WHEN t.love = 'L' THEN 1 ELSE 0 END, a.album_score, {date_column}
            FROM {table} AS e
            LEFT JOIN tracks AS t ON t.id = e.matched_track_id
            LEFT JOIN albums AS a ON a.id = t.album_id
            WHERE e.rank BETWEEN 1 AND 100
              AND ((:week_scope = 1 AND e.year = :selected_year AND e.week = :selected_week)
                OR (:week_scope = 0
                  AND (e.year > :from_year OR (e.year = :from_year AND e.week >= :from_week))
                  AND (e.year < :to_year OR (e.year = :to_year AND e.week <= :to_week))))
            ORDER BY e.year, e.week, e.rank, e.id
            "#,
        ),
        ChartKind::Albums => format!(
            r#"
            SELECT e.year, e.week, e.rank, e.artist, e.title, e.artist_key, e.title_key,
                   {previous}, {peak}, {appearances}, NULL, e.matched_album_id,
                   e.matched_album_id,
                   CAST(COALESCE(a.effective_album_rating, a.calculated_album_rating, a.album_rating) AS REAL) / 20.0,
                   CASE WHEN COALESCE(a.loved_tracks, 0) > 0 THEN 1 ELSE 0 END,
                   a.album_score, {date_column}
            FROM {table} AS e
            LEFT JOIN albums AS a ON a.id = e.matched_album_id
            WHERE e.rank BETWEEN 1 AND 100
              AND ((:week_scope = 1 AND e.year = :selected_year AND e.week = :selected_week)
                OR (:week_scope = 0
                  AND (e.year > :from_year OR (e.year = :from_year AND e.week >= :from_week))
                  AND (e.year < :to_year OR (e.year = :to_year AND e.week <= :to_week))))
            ORDER BY e.year, e.week, e.rank, e.id
            "#,
        ),
    }
}

fn annual_select(kind: ChartKind, table: &str) -> String {
    match kind {
        ChartKind::Singles => format!(
            r#"
            SELECT e.year, NULL, e.rank, COALESCE(NULLIF(e.display_artist, ''), e.artist), e.title,
                   e.artist_key, e.title_key, NULL, NULL, NULL, e.matched_track_id, NULL,
                   t.album_id,
                   CAST(COALESCE(t.normalized_rating, CASE trim(t.rating_raw)
                     WHEN '0.5' THEN 10 WHEN '1' THEN 20 WHEN '1.0' THEN 20
                     WHEN '1.5' THEN 30 WHEN '2' THEN 40 WHEN '2.0' THEN 40
                     WHEN '2.5' THEN 50 WHEN '3' THEN 60 WHEN '3.0' THEN 60
                     WHEN '3.5' THEN 70 WHEN '4' THEN 80 WHEN '4.0' THEN 80
                     WHEN '4.5' THEN 90 WHEN '5' THEN 100 WHEN '5.0' THEN 100 END) AS REAL) / 20.0,
                   CASE WHEN t.love = 'L' THEN 1 ELSE 0 END, a.album_score,
                   CAST(e.year AS TEXT)
            FROM {table} AS e
            LEFT JOIN tracks AS t ON t.id = e.matched_track_id
            LEFT JOIN albums AS a ON a.id = t.album_id
            WHERE e.rank BETWEEN 1 AND 100 AND e.year BETWEEN :from_year AND :to_year
            ORDER BY e.year, e.rank, e.id
            "#,
        ),
        ChartKind::Albums => format!(
            r#"
            SELECT e.year, NULL, e.rank, e.artist, e.album, e.artist_key, e.album_key,
                   NULL, NULL, NULL, NULL, e.matched_album_id, e.matched_album_id,
                   CAST(COALESCE(a.effective_album_rating, a.calculated_album_rating, a.album_rating) AS REAL) / 20.0,
                   CASE WHEN COALESCE(a.loved_tracks, 0) > 0 THEN 1 ELSE 0 END,
                   a.album_score, CAST(e.year AS TEXT)
            FROM {table} AS e
            LEFT JOIN albums AS a ON a.id = e.matched_album_id
            WHERE e.rank BETWEEN 1 AND 100 AND e.year BETWEEN :from_year AND :to_year
            ORDER BY e.year, e.rank, e.id
            "#,
        ),
    }
}

fn score_select() -> &'static str {
    r#"
    WITH ranked AS MATERIALIZED (
      SELECT a.*,
             ROW_NUMBER() OVER (
               ORDER BY a.album_score DESC, COALESCE(a.release_year, a.year) DESC,
                        a.album_artist_display COLLATE NOCASE, a.album COLLATE NOCASE, a.id
             ) AS score_rank
      FROM albums AS a
      WHERE a.album_score IS NOT NULL
        AND COALESCE(a.release_year, a.year) BETWEEN :from_year AND :to_year
    )
    SELECT COALESCE(a.release_year, a.year), NULL, a.score_rank,
           COALESCE(NULLIF(a.album_artist_display, ''), 'Unknown Artist'),
           COALESCE(NULLIF(a.album, ''), 'Unknown Album'),
           lower(trim(COALESCE(a.album_artist_display, ''))), lower(trim(COALESCE(a.album, ''))),
           NULL, NULL, NULL, NULL, a.id, a.id,
           CAST(COALESCE(a.effective_album_rating, a.calculated_album_rating, a.album_rating) AS REAL) / 20.0,
           CASE WHEN a.loved_tracks > 0 THEN 1 ELSE 0 END, a.album_score,
           CAST(COALESCE(a.release_year, a.year) AS TEXT)
    FROM ranked AS a
    ORDER BY a.score_rank
    LIMIT 100
    "#
}

fn query_rows(
    connection: &Connection,
    request: &ChartPageRequest,
) -> Result<Vec<RawChartRow>, String> {
    let table = table_for(request.kind, request.source)?;
    let shape = source_shape(request.source);
    let sql = match shape {
        SourceShape::Weekly => weekly_select(request.kind, request.source, table),
        SourceShape::Annual => annual_select(request.kind, table),
        SourceShape::Score => score_select().to_owned(),
    };
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("Could not prepare the chart page: {error}"))?;
    let rows = match shape {
        SourceShape::Weekly => statement
            .query_map(
                named_params! {
                    ":week_scope": i64::from(request.scope == ChartScope::Week),
                    ":selected_year": request.selected_year,
                    ":selected_week": request.selected_week,
                    ":from_year": request.period.from_year,
                    ":from_week": request.period.from_week,
                    ":to_year": request.period.to_year,
                    ":to_week": request.period.to_week,
                },
                map_raw_row,
            )
            .map_err(|error| format!("Could not read the chart page: {error}"))?
            .collect::<Result<Vec<_>, _>>(),
        SourceShape::Annual | SourceShape::Score => statement
            .query_map(
                named_params! {
                    ":from_year": request.period.from_year,
                    ":to_year": request.period.to_year,
                },
                map_raw_row,
            )
            .map_err(|error| format!("Could not read the chart page: {error}"))?
            .collect::<Result<Vec<_>, _>>(),
    };
    rows.map_err(|error| format!("Could not decode the chart page: {error}"))
}

fn query_weeks(
    connection: &Connection,
    request: &ChartPageRequest,
) -> Result<Vec<ChartWeek>, String> {
    if source_shape(request.source) != SourceShape::Weekly {
        return Ok(Vec::new());
    }
    let table = table_for(request.kind, request.source)?;
    let date_column = match request.source {
        ChartSource::OfficialUk | ChartSource::TiISkuddet | ChartSource::Norsktoppen => {
            "chart_date"
        }
        ChartSource::VgLista => "week_date",
        _ => "NULL",
    };
    let sql = format!(
        r#"
        SELECT year, week, MIN({date_column})
        FROM {table}
        WHERE (year > :from_year OR (year = :from_year AND week >= :from_week))
          AND (year < :to_year OR (year = :to_year AND week <= :to_week))
        GROUP BY year, week
        ORDER BY year, week
        "#,
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("Could not prepare the chart calendar: {error}"))?;
    statement
        .query_map(
            named_params! {
                ":from_year": request.period.from_year,
                ":from_week": request.period.from_week,
                ":to_year": request.period.to_year,
                ":to_week": request.period.to_week,
            },
            |row| {
                Ok(ChartWeek {
                    year: row.get(0)?,
                    week: row.get(1)?,
                    date: row.get(2)?,
                })
            },
        )
        .map_err(|error| format!("Could not read the chart calendar: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode the chart calendar: {error}"))
}

fn aggregate_order(left: &Aggregate, right: &Aggregate) -> Ordering {
    for index in 0..MAX_CHART_ITEMS {
        let ordering = right.rank_counts[index].cmp(&left.rank_counts[index]);
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    right
        .total_points
        .cmp(&left.total_points)
        .then_with(|| right.appearances.cmp(&left.appearances))
        .then_with(|| left.representative.artist.cmp(&right.representative.artist))
        .then_with(|| left.representative.title.cmp(&right.representative.title))
}

fn entries_from_rows(
    rows: Vec<RawChartRow>,
    scope: ChartScope,
    limit: usize,
) -> (Vec<ChartEntry>, usize) {
    if scope == ChartScope::Week {
        let mut rows = rows;
        rows.sort_by_key(|row| row.rank);
        let total = rows.len();
        let entries = rows
            .into_iter()
            .take(limit)
            .enumerate()
            .map(|(index, row)| {
                let previous_position = row.previous_position;
                ChartEntry {
                    position: index + 1,
                    source_position: row.rank,
                    previous_position,
                    movement: previous_position.map(|previous| previous - row.rank),
                    peak_position: row.peak_position.or(Some(row.rank)),
                    appearances: row.appearances.unwrap_or(1),
                    weeks_at_number_one: u32::from(row.rank == 1),
                    total_points: (101 - row.rank).max(1),
                    artist: row.artist,
                    title: row.title,
                    artist_key: row.artist_key,
                    title_key: row.title_key,
                    matched_track_id: row.matched_track_id,
                    matched_album_id: row.matched_album_id,
                    artwork_album_id: row.artwork_album_id,
                    rating: row.rating,
                    loved: row.loved,
                    album_score: row.album_score,
                }
            })
            .collect();
        return (entries, total);
    }

    let mut grouped = HashMap::<String, Aggregate>::new();
    for row in rows {
        let key = format!("{}\u{1f}{}", row.artist_key, row.title_key);
        let rank_index = usize::try_from(row.rank.saturating_sub(1)).unwrap_or(MAX_CHART_ITEMS);
        if rank_index >= MAX_CHART_ITEMS {
            continue;
        }
        let points = (101 - row.rank).max(1);
        let aggregate = grouped.entry(key).or_insert_with(|| Aggregate {
            representative: row.clone(),
            rank_counts: [0; MAX_CHART_ITEMS],
            appearances: 0,
            total_points: 0,
        });
        aggregate.rank_counts[rank_index] = aggregate.rank_counts[rank_index].saturating_add(1);
        aggregate.appearances = aggregate.appearances.saturating_add(1);
        aggregate.total_points = aggregate.total_points.saturating_add(points);
        if row.rank < aggregate.representative.rank {
            let existing_match = aggregate.representative.matched_track_id.clone();
            let existing_album = aggregate.representative.matched_album_id.clone();
            aggregate.representative = row.clone();
            if aggregate.representative.matched_track_id.is_none() {
                aggregate.representative.matched_track_id = existing_match;
            }
            if aggregate.representative.matched_album_id.is_none() {
                aggregate.representative.matched_album_id = existing_album;
            }
        } else {
            if aggregate.representative.matched_track_id.is_none() {
                aggregate.representative.matched_track_id = row.matched_track_id.clone();
            }
            if aggregate.representative.matched_album_id.is_none() {
                aggregate.representative.matched_album_id = row.matched_album_id.clone();
            }
            if aggregate.representative.artwork_album_id.is_none() {
                aggregate.representative.artwork_album_id = row.artwork_album_id.clone();
            }
            aggregate.representative.loved |= row.loved;
            aggregate.representative.rating = aggregate.representative.rating.or(row.rating);
            aggregate.representative.album_score =
                aggregate.representative.album_score.or(row.album_score);
        }
    }
    let mut aggregates = grouped.into_values().collect::<Vec<_>>();
    aggregates.sort_by(aggregate_order);
    let total = aggregates.len();
    let entries = aggregates
        .into_iter()
        .take(limit)
        .enumerate()
        .map(|(index, aggregate)| {
            let row = aggregate.representative;
            ChartEntry {
                position: index + 1,
                source_position: row.rank,
                previous_position: None,
                movement: None,
                peak_position: Some(row.rank),
                appearances: aggregate.appearances,
                weeks_at_number_one: u32::from(aggregate.rank_counts[0]),
                total_points: aggregate.total_points,
                artist: row.artist,
                title: row.title,
                artist_key: row.artist_key,
                title_key: row.title_key,
                matched_track_id: row.matched_track_id,
                matched_album_id: row.matched_album_id,
                artwork_album_id: row.artwork_album_id,
                rating: row.rating,
                loved: row.loved,
                album_score: row.album_score,
            }
        })
        .collect();
    (entries, total)
}

fn query_album_scores(
    connection: &Connection,
    period: &ChartPeriod,
) -> Result<Vec<AlbumScoreEntry>, String> {
    let mut statement = connection
        .prepare_cached(
            r#"
            SELECT id, COALESCE(NULLIF(album, ''), 'Unknown Album'),
                   COALESCE(NULLIF(album_artist_display, ''), 'Unknown Artist'),
                   year, release_year, album_score
            FROM albums
            WHERE album_score IS NOT NULL
              AND COALESCE(release_year, year) BETWEEN :from_year AND :to_year
            ORDER BY album_score DESC, album COLLATE NOCASE, id
            LIMIT 5
            "#,
        )
        .map_err(|error| format!("Could not prepare the Aurora Score shelf: {error}"))?;
    statement
        .query_map(
            named_params! { ":from_year": period.from_year, ":to_year": period.to_year },
            |row| {
                Ok(AlbumScoreEntry {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    artist: row.get(2)?,
                    original_year: row.get(3)?,
                    release_year: row.get(4)?,
                    score: row.get(5)?,
                })
            },
        )
        .map_err(|error| format!("Could not read the Aurora Score shelf: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not decode the Aurora Score shelf: {error}"))
}

fn chart_title(
    kind: ChartKind,
    source: ChartSource,
    scope: ChartScope,
    period: &ChartPeriod,
) -> String {
    let noun = match kind {
        ChartKind::Singles => "Singles",
        ChartKind::Albums => "Albums",
    };
    if source == ChartSource::AuroraScore {
        return format!("Aurora Album Score · {}", period.label);
    }
    if scope == ChartScope::Period || source_shape(source) != SourceShape::Weekly {
        format!("{} {} · {}", source_label(source), noun, period.label)
    } else {
        format!("{} {} Chart", source_label(source), noun)
    }
}

fn query_page(connection: &Connection, request: ChartPageRequest) -> Result<ChartPage, String> {
    validate_request(&request)?;
    let effective_scope = if source_shape(request.source) == SourceShape::Weekly {
        request.scope
    } else {
        ChartScope::Period
    };
    let rows = query_rows(connection, &request)?;
    let chart_date = rows.first().and_then(|row| row.chart_date.clone());
    let weeks = query_weeks(connection, &request)?;
    let (entries, total_entries) = entries_from_rows(rows, effective_scope, request.limit);
    let album_score_entries = query_album_scores(connection, &request.period)?;
    let mut response_request = request;
    response_request.scope = effective_scope;
    Ok(ChartPage {
        source_label: source_label(response_request.source),
        chart_title: chart_title(
            response_request.kind,
            response_request.source,
            effective_scope,
            &response_request.period,
        ),
        annual_only: source_shape(response_request.source) != SourceShape::Weekly,
        chart_date,
        weeks,
        entries,
        total_entries,
        album_score_entries,
        request: response_request,
    })
}

fn detail_sources(kind: ChartKind) -> &'static [ChartSource] {
    match kind {
        ChartKind::Singles => &[
            ChartSource::OfficialUk,
            ChartSource::VgLista,
            ChartSource::TiISkuddet,
            ChartSource::Norsktoppen,
            ChartSource::Billboard,
        ],
        ChartKind::Albums => &[
            ChartSource::OfficialUk,
            ChartSource::VgLista,
            ChartSource::Billboard,
            ChartSource::AuroraScore,
        ],
    }
}

fn query_matching_ranks(
    connection: &Connection,
    page: &ChartPageRequest,
    artist_key: &str,
    title_key: &str,
) -> Result<Vec<i64>, String> {
    let table = table_for(page.kind, page.source)?;
    let shape = source_shape(page.source);
    let title_column = match (page.kind, page.source) {
        (ChartKind::Albums, ChartSource::Billboard) => "album_key",
        _ => "title_key",
    };
    let sql = match shape {
        SourceShape::Weekly => format!(
            r#"
            SELECT rank
            FROM {table}
            WHERE rank BETWEEN 1 AND 100
              AND artist_key = :artist_key AND {title_column} = :title_key
              AND ((:week_scope = 1 AND year = :selected_year AND week = :selected_week)
                OR (:week_scope = 0
                  AND (year > :from_year OR (year = :from_year AND week >= :from_week))
                  AND (year < :to_year OR (year = :to_year AND week <= :to_week))))
            "#,
        ),
        SourceShape::Annual => format!(
            r#"
            SELECT rank
            FROM {table}
            WHERE rank BETWEEN 1 AND 100
              AND artist_key = :artist_key AND {title_column} = :title_key
              AND year BETWEEN :from_year AND :to_year
            "#,
        ),
        SourceShape::Score => r#"
            WITH ranked AS MATERIALIZED (
              SELECT lower(trim(COALESCE(album_artist_display, ''))) AS artist_key,
                     lower(trim(COALESCE(album, ''))) AS title_key,
                     ROW_NUMBER() OVER (
                       ORDER BY album_score DESC, COALESCE(release_year, year) DESC,
                                album_artist_display COLLATE NOCASE, album COLLATE NOCASE, id
                     ) AS rank
              FROM albums
              WHERE album_score IS NOT NULL
                AND COALESCE(release_year, year) BETWEEN :from_year AND :to_year
            )
            SELECT rank FROM ranked
            WHERE artist_key = :artist_key AND title_key = :title_key
            "#
        .to_owned(),
    };
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("Could not prepare the chart source history: {error}"))?;
    match shape {
        SourceShape::Weekly => statement
            .query_map(
                named_params! {
                    ":artist_key": artist_key,
                    ":title_key": title_key,
                    ":week_scope": i64::from(page.scope == ChartScope::Week),
                    ":selected_year": page.selected_year,
                    ":selected_week": page.selected_week,
                    ":from_year": page.period.from_year,
                    ":from_week": page.period.from_week,
                    ":to_year": page.period.to_year,
                    ":to_week": page.period.to_week,
                },
                |row| row.get(0),
            )
            .map_err(|error| format!("Could not read the chart source history: {error}"))?
            .collect::<Result<Vec<_>, _>>(),
        SourceShape::Annual | SourceShape::Score => statement
            .query_map(
                named_params! {
                    ":artist_key": artist_key,
                    ":title_key": title_key,
                    ":from_year": page.period.from_year,
                    ":to_year": page.period.to_year,
                },
                |row| row.get(0),
            )
            .map_err(|error| format!("Could not read the chart source history: {error}"))?
            .collect::<Result<Vec<_>, _>>(),
    }
    .map_err(|error| format!("Could not decode the chart source history: {error}"))
}

fn query_item_detail(
    connection: &Connection,
    request: ChartItemDetailRequest,
) -> Result<ChartItemDetail, String> {
    validate_request(&request.page)?;
    if request.artist_key.chars().count() > 512 || request.title_key.chars().count() > 512 {
        return Err("The selected chart identity is invalid.".to_owned());
    }
    let mut source_ranks = Vec::new();
    for source in detail_sources(request.page.kind) {
        let mut page = request.page.clone();
        page.source = *source;
        if source_shape(*source) != SourceShape::Weekly {
            page.scope = ChartScope::Period;
        }
        let ranks =
            query_matching_ranks(connection, &page, &request.artist_key, &request.title_key)?;
        source_ranks.push(ChartSourceRank {
            source: *source,
            label: source_label(*source),
            best_rank: ranks.iter().copied().min(),
            appearances: u32::try_from(ranks.len()).unwrap_or(u32::MAX),
            weeks_at_number_one: u32::try_from(ranks.iter().filter(|rank| **rank == 1).count())
                .unwrap_or(u32::MAX),
            annual_only: source_shape(*source) != SourceShape::Weekly,
        });
    }
    Ok(ChartItemDetail { source_ranks })
}

fn track_select_sql() -> &'static str {
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
    "#
}

fn load_track_by_id(
    connection: &Connection,
    track_id: i64,
    store: &StateStore,
) -> Result<TrackSummary, String> {
    let mut tracks = catalog::query_tracks(
        connection,
        track_select_sql(),
        named_params! { ":track_id": track_id },
        "chart track",
        Some(store),
    )?;
    tracks
        .pop()
        .ok_or_else(|| "That chart entry is no longer matched to a library track.".to_owned())
}

fn query_queue(
    connection: &Connection,
    request: ChartPageRequest,
    store: &StateStore,
) -> Result<Vec<TrackSummary>, String> {
    let page = query_page(connection, request.clone())?;
    let mut tracks = Vec::new();
    match request.kind {
        ChartKind::Singles => {
            for entry in page.entries {
                let Some(track_id) = entry.matched_track_id.and_then(|id| id.parse::<i64>().ok())
                else {
                    continue;
                };
                if let Ok(track) = load_track_by_id(connection, track_id, store) {
                    tracks.push(track);
                }
                if tracks.len() >= MAX_CHART_ITEMS {
                    break;
                }
            }
        }
        ChartKind::Albums => {
            let mut statement = connection
                .prepare_cached(&format!(
                    "{} ORDER BY t.disc_number, t.track_number, t.id",
                    track_select_sql()
                        .replace("WHERE t.id = :track_id", "WHERE t.album_id = :album_id")
                ))
                .map_err(|error| format!("Could not prepare chart-album playback: {error}"))?;
            for entry in page.entries {
                let Some(album_id) = entry.matched_album_id else {
                    continue;
                };
                let remaining = MAX_CHART_ITEMS.saturating_sub(tracks.len());
                if remaining == 0 {
                    break;
                }
                let mut album_tracks = statement
                    .query_map(
                        named_params! { ":album_id": album_id },
                        catalog::map_track_row,
                    )
                    .map_err(|error| format!("Could not read chart-album playback: {error}"))?
                    .take(remaining)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| format!("Could not decode chart-album playback: {error}"))?;
                catalog::apply_overlays(&mut album_tracks, Some(store))?;
                tracks.extend(album_tracks);
            }
        }
    }
    Ok(tracks)
}

pub(crate) fn load_chart_page(request: ChartPageRequest) -> Result<ChartPage, String> {
    let path = catalog::default_catalog_path()?;
    let connection = catalog::open_catalog(&path)?;
    query_page(&connection, request)
}

pub(crate) fn load_chart_item_detail(
    request: ChartItemDetailRequest,
) -> Result<ChartItemDetail, String> {
    let path = catalog::default_catalog_path()?;
    let connection = catalog::open_catalog(&path)?;
    query_item_detail(&connection, request)
}

pub(crate) fn load_chart_entry_track(
    track_id: String,
    store: &StateStore,
) -> Result<TrackSummary, String> {
    if track_id.is_empty()
        || track_id.len() > 24
        || !track_id.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err("The chart track identity is invalid.".to_owned());
    }
    let path = catalog::default_catalog_path()?;
    let connection = catalog::open_catalog(&path)?;
    load_track_by_id(
        &connection,
        track_id
            .parse::<i64>()
            .map_err(|_| "The chart track identity is invalid.".to_owned())?,
        store,
    )
}

pub(crate) fn load_chart_queue(
    request: ChartPageRequest,
    store: &StateStore,
) -> Result<Vec<TrackSummary>, String> {
    let path = catalog::default_catalog_path()?;
    let connection = catalog::open_catalog(&path)?;
    query_queue(&connection, request, store)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(title: &str, rank: i64, week: u8) -> RawChartRow {
        RawChartRow {
            _year: 1985,
            _week: Some(week),
            rank,
            artist: "Artist".to_owned(),
            title: title.to_owned(),
            artist_key: "artist".to_owned(),
            title_key: title.to_lowercase(),
            previous_position: None,
            peak_position: None,
            appearances: None,
            matched_track_id: None,
            matched_album_id: None,
            artwork_album_id: None,
            rating: None,
            loved: false,
            album_score: None,
            chart_date: None,
        }
    }

    #[test]
    fn period_charts_prioritize_number_one_weeks_before_points() {
        let rows = vec![
            row("One week champion", 1, 23),
            row("One week champion", 100, 24),
            row("Consistent runner-up", 2, 23),
            row("Consistent runner-up", 2, 24),
            row("Consistent runner-up", 2, 25),
        ];
        let (entries, _) = entries_from_rows(rows, ChartScope::Period, 10);
        assert_eq!(entries[0].title, "One week champion");
        assert_eq!(entries[0].weeks_at_number_one, 1);
        assert_eq!(entries[1].title, "Consistent runner-up");
    }

    #[test]
    fn period_charts_use_second_places_as_the_next_tie_break() {
        let rows = vec![
            row("More seconds", 1, 23),
            row("More seconds", 2, 24),
            row("More seconds", 2, 25),
            row("More points", 1, 23),
            row("More points", 3, 24),
            row("More points", 3, 25),
            row("More points", 3, 26),
        ];
        let (entries, _) = entries_from_rows(rows, ChartScope::Period, 10);
        assert_eq!(entries[0].title, "More seconds");
    }

    #[test]
    fn requests_reject_incompatible_sources_and_unbounded_periods() {
        let mut request = ChartPageRequest {
            kind: ChartKind::Singles,
            source: ChartSource::AuroraScore,
            scope: ChartScope::Period,
            period: ChartPeriod {
                from_year: 1985,
                from_week: 1,
                to_year: 1985,
                to_week: 53,
                label: "1985".to_owned(),
            },
            selected_year: 1985,
            selected_week: 23,
            limit: 100,
        };
        assert!(validate_request(&request).is_err());
        request.source = ChartSource::OfficialUk;
        request.period.to_year = 2026;
        assert!(validate_request(&request).is_err());
    }

    #[test]
    #[ignore = "reads the user's live read-only Music Library chart tables"]
    fn live_catalog_opens_week_period_and_aurora_score_charts() {
        let path = catalog::default_catalog_path().expect("catalog path");
        let connection = catalog::open_catalog(&path).expect("open catalog");
        let base = ChartPageRequest {
            kind: ChartKind::Singles,
            source: ChartSource::OfficialUk,
            scope: ChartScope::Week,
            period: ChartPeriod {
                from_year: 1985,
                from_week: 23,
                to_year: 1985,
                to_week: 35,
                label: "Summer 1985".to_owned(),
            },
            selected_year: 1985,
            selected_week: 23,
            limit: 100,
        };
        let weekly = query_page(&connection, base.clone()).expect("weekly chart");
        assert!(!weekly.entries.is_empty());
        assert_eq!(weekly.entries[0].position, 1);
        let weekly_detail = query_item_detail(
            &connection,
            ChartItemDetailRequest {
                page: base.clone(),
                artist_key: weekly.entries[0].artist_key.clone(),
                title_key: weekly.entries[0].title_key.clone(),
            },
        )
        .expect("weekly source history");
        assert_eq!(weekly_detail.source_ranks.len(), 5);
        assert!(weekly_detail.source_ranks[0].best_rank.is_some());
        let period = query_page(
            &connection,
            ChartPageRequest {
                scope: ChartScope::Period,
                ..base.clone()
            },
        )
        .expect("period chart");
        assert!(!period.entries.is_empty());
        assert!(period.entries[0].appearances >= period.entries[0].weeks_at_number_one);
        let score_request = ChartPageRequest {
            kind: ChartKind::Albums,
            source: ChartSource::AuroraScore,
            scope: ChartScope::Period,
            ..base
        };
        let scores = query_page(&connection, score_request.clone()).expect("Aurora Score chart");
        assert!(!scores.entries.is_empty());
        assert!(scores.entries[0].album_score.is_some());
        assert!(
            scores
                .entries
                .windows(2)
                .all(|pair| pair[0].album_score >= pair[1].album_score)
        );
        let score_detail = query_item_detail(
            &connection,
            ChartItemDetailRequest {
                page: score_request,
                artist_key: scores.entries[0].artist_key.clone(),
                title_key: scores.entries[0].title_key.clone(),
            },
        )
        .expect("album source history");
        assert_eq!(score_detail.source_ranks.len(), 4);
        assert!(score_detail.source_ranks[3].best_rank.is_some());
    }
}
