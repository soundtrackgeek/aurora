//! Every chart run of one artist, merged per song and per album, for the
//! artist page's chart overview.

use super::track_peaks::{
    credit_prefix, is_recording_variant, published_identity_keys, table_exists, table_has_column,
};
use super::*;
use crate::chart_identity::{artist_group_key, main_performers, text_key as catalog_chart_key};
use crate::chart_song_match::without_parentheses;
use std::collections::BTreeMap;

const MAX_ITEMS: usize = 500;

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArtistChartPeak {
    label: String,
    country: &'static str,
    peak: i64,
    /// Weeks on a weekly chart, or years on an annual chart.
    weeks: u32,
    annual: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArtistChartItem {
    title: String,
    best_peak: i64,
    first_year: Option<i64>,
    track_id: Option<String>,
    album_id: Option<String>,
    peaks: Vec<ArtistChartPeak>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArtistChartHistory {
    singles: Vec<ArtistChartItem>,
    albums: Vec<ArtistChartItem>,
}

struct Row {
    printed: Option<String>,
    title_key: String,
    rank: i64,
    year: Option<i64>,
    matched_id: Option<String>,
    library_title: Option<String>,
}

#[derive(Default)]
struct Group {
    printed: Vec<String>,
    library_title: Option<String>,
    matched_id: Option<String>,
    first_year: Option<i64>,
    peaks: BTreeMap<(usize, String), ArtistChartPeak>,
}

/// Charts print subtitles ("Silent Running (On Dangerous Ground)") that other
/// charts drop, so a subtitle joins the plain title. Other recordings
/// ("Song (Live)") stay separate, as on the track badges.
fn group_key(printed: Option<&str>, title_key: &str) -> String {
    if let Some(printed) = printed
        && let Some(base) = without_parentheses(printed)
        && !is_recording_variant(printed)
    {
        let base = catalog_chart_key(&base);
        if !base.is_empty() {
            return base;
        }
    }
    title_key.to_owned()
}

fn shouting(value: &str) -> bool {
    value.chars().any(char::is_alphabetic) && !value.chars().any(char::is_lowercase)
}

impl Group {
    fn add(&mut self, order: usize, label: &str, country: &'static str, annual: bool, row: Row) {
        if let Some(printed) = row.printed.filter(|title| !title.trim().is_empty()) {
            self.printed.push(printed.trim().to_owned());
        }
        if self.library_title.is_none() {
            self.library_title = row.library_title.filter(|title| !title.trim().is_empty());
        }
        if self.matched_id.is_none() {
            self.matched_id = row.matched_id;
        }
        if let Some(year) = row.year.filter(|year| *year > 0) {
            self.first_year = Some(self.first_year.map_or(year, |first| first.min(year)));
        }
        let peak = self
            .peaks
            .entry((order, label.to_owned()))
            .or_insert(ArtistChartPeak {
                label: label.to_owned(),
                country,
                peak: row.rank,
                weeks: 0,
                annual,
            });
        peak.peak = peak.peak.min(row.rank);
        peak.weeks = peak.weeks.saturating_add(1);
    }

    fn finish(self, key: &str, kind: ChartKind) -> ArtistChartItem {
        // The library spelling reads best; otherwise prefer the plain printed
        // title over a subtitled one, and normal case over all-caps charts.
        let library = self
            .library_title
            .filter(|title| catalog_chart_key(title) == key);
        let title = library
            .or_else(|| {
                let plain = |title: &&String| catalog_chart_key(title) == key;
                self.printed
                    .iter()
                    .filter(plain)
                    .find(|title| !shouting(title))
                    .or_else(|| self.printed.iter().find(|title| !shouting(title)))
                    .or_else(|| self.printed.iter().find(plain))
                    .or_else(|| self.printed.first())
                    .cloned()
            })
            .unwrap_or_else(|| key.to_owned());
        let peaks = self.peaks.into_values().collect::<Vec<_>>();
        let (track_id, album_id) = match kind {
            ChartKind::Singles => (self.matched_id, None),
            ChartKind::Albums => (None, self.matched_id),
        };
        ArtistChartItem {
            title,
            best_peak: peaks.iter().map(|peak| peak.peak).min().unwrap_or(0),
            first_year: self.first_year,
            track_id,
            album_id,
            peaks,
        }
    }
}

/// The artist's own credits, plus collaborations they lead ("Artist & Guest").
fn credit_matches(row_artist_key: &str, artist_key: &str) -> bool {
    row_artist_key == artist_key
        || main_performers(row_artist_key).first().map(String::as_str) == Some(artist_key)
}

fn table_rows(
    connection: &Connection,
    kind: ChartKind,
    source: ChartSource,
    artist_key: &str,
) -> Result<Vec<Row>, String> {
    let table = table_for(kind, source)?;
    if !table_exists(connection, table)? {
        return Ok(Vec::new());
    }
    let has = |column: &str| table_has_column(connection, table, column);
    let billboard_albums = kind == ChartKind::Albums && source == ChartSource::Billboard;
    let (title_column, key_column) = if billboard_albums {
        ("album", "album_key")
    } else {
        ("title", "title_key")
    };
    let printed = if has(title_column)? {
        format!("e.{title_column}")
    } else {
        "NULL".to_owned()
    };
    let year = if has("year")? { "e.year" } else { "NULL" };
    let (matched, library, join) = match kind {
        ChartKind::Singles if has("matched_track_id")? && table_exists(connection, "tracks")? => (
            "CAST(t.id AS TEXT)",
            "t.title",
            "LEFT JOIN tracks AS t ON t.id = e.matched_track_id",
        ),
        ChartKind::Albums if has("matched_album_id")? && table_exists(connection, "albums")? => (
            "a.id",
            "a.album",
            "LEFT JOIN albums AS a ON a.id = e.matched_album_id",
        ),
        _ => ("NULL", "NULL", ""),
    };
    let (from, to) = credit_prefix(artist_key);
    let mut statement = connection
        .prepare(&format!(
            "SELECT e.artist_key, {printed}, e.{key_column}, e.rank, {year}, {matched}, {library}
             FROM {table} AS e {join}
             WHERE (e.artist_key = ?1 OR (e.artist_key >= ?2 AND e.artist_key < ?3))
               AND e.rank > 0 AND e.{key_column} IS NOT NULL"
        ))
        .map_err(|e| format!("Could not read the artist's chart history: {e}"))?;
    let rows = statement
        .query_map([artist_key, &from, &to], |r| {
            Ok((
                r.get::<_, String>(0)?,
                Row {
                    printed: r.get(1)?,
                    title_key: r.get(2)?,
                    rank: r.get(3)?,
                    year: r.get(4)?,
                    matched_id: r.get(5)?,
                    library_title: r.get(6)?,
                },
            ))
        })
        .map_err(|e| format!("Could not read the artist's chart history: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Could not read the artist's chart history: {e}"))?;
    Ok(rows
        .into_iter()
        .filter(|(row_artist, _)| credit_matches(row_artist, artist_key))
        .map(|(_, row)| row)
        .collect())
}

/// Published US weekly books (Hot 100, rock and airplay charts), one peak per chart.
fn published_rows(connection: &Connection, artist: &str) -> Result<Vec<(String, Row)>, String> {
    if !table_exists(connection, "published_chart_books")?
        || !table_exists(connection, "published_chart_entries")?
    {
        return Ok(Vec::new());
    }
    let keyed = published_identity_keys(connection)?;
    let peak = "CASE WHEN CAST(e.peak_position AS INTEGER) > 0 THEN MIN(e.position, CAST(e.peak_position AS INTEGER)) ELSE e.position END";
    let week = if table_has_column(connection, "published_chart_entries", "week_ending")? {
        "CAST(substr(e.week_ending, 1, 4) AS INTEGER)"
    } else {
        "NULL"
    };
    let (artist_column, title_key, identity) = if keyed {
        (
            "e.artist_group_key",
            "e.title_key",
            "(e.artist_group_key = ?1 OR (e.artist_group_key >= ?2 AND e.artist_group_key < ?3))",
        )
    } else {
        ("e.artist", "NULL", "e.artist = ?1 COLLATE NOCASE")
    };
    let artist_key = if keyed {
        artist_group_key(artist)
    } else {
        artist.trim().to_owned()
    };
    let (from, to) = credit_prefix(&artist_key);
    let sql = format!(
        "SELECT b.chart, {artist_column}, e.title, {title_key}, {peak}, {week}
         FROM published_chart_entries AS e JOIN published_chart_books AS b ON b.id = e.book_id
         WHERE {identity} AND e.position > 0"
    );
    let mut statement = connection.prepare(&sql).map_err(|e| e.to_string())?;
    let map = |r: &rusqlite::Row<'_>| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, Option<i64>>(5)?,
        ))
    };
    let rows = if keyed {
        statement.query_map([&artist_key, &from, &to], map)
    } else {
        statement.query_map([&artist_key], map)
    }
    .map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .filter(|(_, row_artist, ..)| !keyed || credit_matches(row_artist, &artist_key))
        .filter_map(|(chart, _, printed, key, rank, year)| {
            let title_key = key
                .filter(|key| !key.is_empty())
                .or_else(|| printed.as_deref().map(catalog_chart_key))?;
            Some((
                chart,
                Row {
                    printed,
                    title_key,
                    rank,
                    year,
                    matched_id: None,
                    library_title: None,
                },
            ))
        })
        .collect())
}

fn source_country(source: ChartSource) -> &'static str {
    match source {
        ChartSource::OfficialUk => "UK",
        ChartSource::Billboard | ChartSource::PublishedUs => "US",
        _ => "NO",
    }
}

fn chart_label(source: ChartSource) -> &'static str {
    if source == ChartSource::Billboard {
        "Billboard annual"
    } else {
        source_label(source)
    }
}

fn finish(groups: BTreeMap<String, Group>, kind: ChartKind) -> Vec<ArtistChartItem> {
    let mut items = groups
        .into_iter()
        .map(|(key, group)| group.finish(&key, kind))
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        left.first_year
            .unwrap_or(i64::MAX)
            .cmp(&right.first_year.unwrap_or(i64::MAX))
            .then(left.best_peak.cmp(&right.best_peak))
            .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
    });
    items.truncate(MAX_ITEMS);
    items
}

fn query(connection: &Connection, artist: &str) -> Result<ArtistChartHistory, String> {
    if artist.trim().is_empty() || artist.len() > 2048 {
        return Err("The selected artist identity is invalid.".into());
    }
    let artist_key = catalog_chart_key(artist);
    let mut history = ArtistChartHistory::default();
    for kind in [ChartKind::Singles, ChartKind::Albums] {
        let mut groups = BTreeMap::<String, Group>::new();
        let sources = detail_sources(kind)
            .iter()
            .filter(|source| **source != ChartSource::AuroraScore);
        for (order, source) in sources.enumerate() {
            let annual = source_shape(*source) != SourceShape::Weekly;
            for row in table_rows(connection, kind, *source, &artist_key)? {
                groups
                    .entry(group_key(row.printed.as_deref(), &row.title_key))
                    .or_default()
                    .add(
                        order,
                        chart_label(*source),
                        source_country(*source),
                        annual,
                        row,
                    );
            }
        }
        if kind == ChartKind::Singles {
            for (chart, row) in published_rows(connection, artist)? {
                groups
                    .entry(group_key(row.printed.as_deref(), &row.title_key))
                    .or_default()
                    .add(usize::MAX, &chart, "US", false, row);
            }
        }
        let items = finish(groups, kind);
        match kind {
            ChartKind::Singles => history.singles = items,
            ChartKind::Albums => history.albums = items,
        }
    }
    Ok(history)
}

pub(crate) fn load(artist: String) -> Result<ArtistChartHistory, String> {
    let connection = catalog::open_catalog(&catalog::default_catalog_path()?)?;
    query(&connection, &artist)
}

#[cfg(test)]
mod tests {
    use super::*;

    type Summary = (String, i64, Vec<(String, i64, u32)>);

    fn summary(items: &[ArtistChartItem]) -> Vec<Summary> {
        items
            .iter()
            .map(|item| {
                (
                    item.title.clone(),
                    item.best_peak,
                    item.peaks
                        .iter()
                        .map(|peak| (peak.label.clone(), peak.peak, peak.weeks))
                        .collect(),
                )
            })
            .collect()
    }

    #[test]
    fn merges_every_chart_run_per_song_and_album() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("
            CREATE TABLE tracks (id INTEGER, title TEXT);
            INSERT INTO tracks VALUES (7, 'Love Cries');
            CREATE TABLE albums (id TEXT, album TEXT);
            INSERT INTO albums VALUES ('a1', 'Commandos');
            CREATE TABLE official_uk_single_chart_entries (year INTEGER, rank INTEGER, artist_key TEXT, title TEXT, title_key TEXT, matched_track_id INTEGER);
            INSERT INTO official_uk_single_chart_entries VALUES
              (1989,40,'stage dolls','LOVE CRIES','love cries',NULL),
              (1989,12,'stage dolls','LOVE CRIES','love cries',NULL),
              (1990,3,'stage dolls','LOVE CRIES (LIVE)','love cries live',NULL),
              (1988,1,'other','LOVE CRIES','love cries',NULL),
              (1991,9,'stage dolls and guest','DUET','duet',NULL),
              (1991,2,'stage dollsy','IMPOSTER','imposter',NULL);
            CREATE TABLE vg_lista_single_chart_entries (year INTEGER, rank INTEGER, artist_key TEXT, title TEXT, title_key TEXT, matched_track_id INTEGER);
            INSERT INTO vg_lista_single_chart_entries VALUES
              (1988,2,'stage dolls','Love Cries','love cries',7),
              (1988,5,'stage dolls','Wings of Steel (Hard Times)','wings of steel hard times',NULL);
            CREATE TABLE billboard_single_chart_entries (year INTEGER, rank INTEGER, artist_key TEXT, title TEXT, title_key TEXT, matched_track_id INTEGER);
            INSERT INTO billboard_single_chart_entries VALUES (1989,46,'stage dolls','Love Cries','love cries',NULL);
            CREATE TABLE vg_lista_album_chart_entries (year INTEGER, rank INTEGER, artist_key TEXT, title TEXT, title_key TEXT, matched_album_id TEXT);
            INSERT INTO vg_lista_album_chart_entries VALUES (1986,1,'stage dolls','Commandos','commandos','a1'),(1986,4,'stage dolls','Commandos','commandos',NULL);
            CREATE TABLE billboard_chart_entries (year INTEGER, rank INTEGER, artist_key TEXT, album TEXT, album_key TEXT, matched_album_id TEXT);
            INSERT INTO billboard_chart_entries VALUES (1989,118,'stage dolls','Stage Dolls','stage dolls',NULL);
            CREATE TABLE published_chart_books (id INTEGER, chart TEXT);
            INSERT INTO published_chart_books VALUES (1,'Billboard Hot 100'),(2,'Mainstream Rock Tracks');
            CREATE TABLE published_chart_identity (id INTEGER PRIMARY KEY, key_version INTEGER);
            INSERT INTO published_chart_identity VALUES (1, 1);
            CREATE TABLE published_chart_entries (book_id INTEGER, artist TEXT, title TEXT, week_ending TEXT, position INTEGER, peak_position TEXT, artist_group_key TEXT, title_key TEXT);
            INSERT INTO published_chart_entries VALUES
              (1,'Stage Dolls','Love Cries','1989-11-04',46,'46','stage dolls','love cries'),
              (1,'Stage Dolls','Love Cries','1989-11-11',50,'46','stage dolls','love cries'),
              (2,'Stage Dolls','Wings Of Steel','1989-03-01',30,'','stage dolls','wings of steel'),
              (1,'Someone Else','Love Cries','1989-11-11',1,'1','someone else','love cries');
        ").unwrap();
        let history = query(&connection, "Stage Dolls").unwrap();
        assert_eq!(
            summary(&history.singles),
            vec![
                (
                    "Love Cries".to_owned(),
                    2,
                    vec![
                        ("Official UK".to_owned(), 12, 2),
                        ("VG Lista".to_owned(), 2, 1),
                        ("Billboard annual".to_owned(), 46, 1),
                        ("Billboard Hot 100".to_owned(), 46, 2),
                    ]
                ),
                (
                    "Wings Of Steel".to_owned(),
                    5,
                    vec![
                        ("VG Lista".to_owned(), 5, 1),
                        ("Mainstream Rock Tracks".to_owned(), 30, 1),
                    ]
                ),
                (
                    "LOVE CRIES (LIVE)".to_owned(),
                    3,
                    vec![("Official UK".to_owned(), 3, 1)]
                ),
                ("DUET".to_owned(), 9, vec![("Official UK".to_owned(), 9, 1)]),
            ]
        );
        assert_eq!(history.singles[0].track_id.as_deref(), Some("7"));
        assert_eq!(history.singles[0].first_year, Some(1988));
        assert_eq!(
            summary(&history.albums),
            vec![
                (
                    "Commandos".to_owned(),
                    1,
                    vec![("VG Lista".to_owned(), 1, 2)]
                ),
                (
                    "Stage Dolls".to_owned(),
                    118,
                    vec![("Billboard annual".to_owned(), 118, 1)]
                ),
            ]
        );
        assert_eq!(history.albums[0].album_id.as_deref(), Some("a1"));
        assert!(history.albums[1].peaks[0].annual);
    }

    #[test]
    fn missing_archives_are_empty_and_invalid_artists_are_rejected() {
        let connection = Connection::open_in_memory().unwrap();
        let history = query(&connection, "Anyone").unwrap();
        assert!(history.singles.is_empty() && history.albums.is_empty());
        assert!(query(&connection, " ").is_err());
    }
}
