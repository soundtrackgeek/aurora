//! Read the Music Library weekly archive without modifying its catalog or matches.
use super::*;
use crate::chart_identity::{artist_group_key as artist_key, main_performers, text_key as key};
use rusqlite::{OptionalExtension, params};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PublishedSeries {
    chart: String,
    years: Vec<i32>,
}

fn require_archive(conn: &Connection) -> Result<(), String> {
    let ready = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('published_chart_books', 'published_chart_entries')",
        [], |row| row.get::<_, i64>(0),
    ).map_err(|e| e.to_string())?;
    if ready != 2 {
        return Err("Open Published Charts in Music Library to prepare the bundled US weekly archive, then refresh Aurora Charts. Aurora reads the same catalog; the annual Singles CSV import is separate.".into());
    }
    Ok(())
}

pub(super) fn series(conn: &Connection) -> Result<Vec<PublishedSeries>, String> {
    require_archive(conn)?;
    let mut statement = conn.prepare("SELECT chart, CAST(substr(book, 1, 4) AS INTEGER) FROM published_chart_books WHERE row_count > 0 ORDER BY chart COLLATE NOCASE, book").map_err(|e| e.to_string())?;
    let mut grouped = BTreeMap::<String, Vec<i32>>::new();
    for row in statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
        })
        .map_err(|e| e.to_string())?
    {
        let (chart, year) = row.map_err(|e| e.to_string())?;
        grouped.entry(chart).or_default().push(year);
    }
    Ok(grouped
        .into_iter()
        .map(|(chart, years)| PublishedSeries { chart, years })
        .collect())
}

pub(super) fn weeks(conn: &Connection, chart: &str, year: i32) -> Result<Vec<String>, String> {
    require_archive(conn)?;
    let mut statement = conn.prepare(
        "SELECT DISTINCT e.week_ending FROM published_chart_books b JOIN published_chart_entries e ON e.book_id = b.id
         WHERE b.chart = ?1 AND b.book = ?2 ORDER BY e.week_ending",
    ).map_err(|e| e.to_string())?;
    statement
        .query_map(params![chart, format!("{year}_us_singles")], |row| {
            row.get(0)
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[derive(Clone)]
struct WeeklyRow {
    artist: String,
    title: String,
    date: String,
    position: i64,
    previous: Option<i64>,
    appearances: u32,
    debut: Option<String>,
    peak: Option<i64>,
}

fn rows(conn: &Connection, request: &ChartPageRequest) -> Result<Vec<WeeklyRow>, String> {
    require_archive(conn)?;
    let chart = request
        .published_chart
        .as_deref()
        .filter(|v| !v.trim().is_empty() && v.len() <= 300)
        .ok_or("Choose a US weekly chart.")?;
    let date = request.published_week.as_deref().unwrap_or("");
    if request.scope == ChartScope::Week {
        let valid = conn
            .query_row(
                "SELECT date(?1) = ?1 AND substr(?1, 1, 4) = ?2",
                params![date, request.selected_year.to_string()],
                |r| r.get::<_, Option<bool>>(0),
            )
            .map_err(|e| e.to_string())?;
        if valid != Some(true) {
            return Err("Choose a published week-ending date in the selected year.".into());
        }
    }
    let mut statement = conn.prepare(
        "SELECT e.artist, e.title, e.week_ending, e.position, NULLIF(CAST(e.last_week AS INTEGER), 0),
                CAST(e.weeks_on_chart AS INTEGER), NULLIF(e.entry_date, ''), NULLIF(CAST(e.peak_position AS INTEGER), 0)
         FROM published_chart_books b JOIN published_chart_entries e ON e.book_id = b.id
         WHERE b.chart = ?1 AND b.book = ?2 AND (?3 = 0 OR e.week_ending = ?4)
         ORDER BY e.week_ending, e.position, e.source_row",
    ).map_err(|e| format!("Could not read US weekly charts: {e}"))?;
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for row in statement
        .query_map(
            params![
                chart,
                format!("{}_us_singles", request.selected_year),
                request.scope == ChartScope::Week,
                date
            ],
            |r| {
                Ok(WeeklyRow {
                    artist: r.get(0)?,
                    title: r.get(1)?,
                    date: r.get(2)?,
                    position: r.get(3)?,
                    previous: r.get(4)?,
                    appearances: r.get::<_, u32>(5)?.max(1),
                    debut: r.get(6)?,
                    peak: r.get(7)?,
                })
            },
        )
        .map_err(|e| e.to_string())?
    {
        let row = row.map_err(|e| e.to_string())?;
        if row.position > 0
            && seen.insert((row.artist.clone(), row.title.clone(), row.date.clone()))
        {
            result.push(row);
        }
    }
    Ok(result)
}

// Full titles win; parenthetical fallbacks are resolved separately by SongIndex.
fn title_key(value: &str) -> String {
    let normalized = key(value);
    for suffix in [" album version", " lp version", " lp"] {
        if let Some(base) = normalized.strip_suffix(suffix).filter(|s| !s.is_empty()) {
            return base.into();
        }
    }
    normalized
}

fn parenthetical_key(title: &str) -> Option<String> {
    crate::chart_song_match::without_parentheses(title)
        .map(|base| key(&base))
        .filter(|base| !base.is_empty())
}

#[derive(Clone)]
struct Match {
    track_id: String,
    album_id: Option<String>,
    album_title: Option<String>,
    rating: Option<f64>,
    loved: bool,
    score: Option<f64>,
    preference: (bool, bool, i64),
}

pub(super) fn match_entries(conn: &Connection, entries: &mut [ChartEntry]) -> Result<(), String> {
    let wanted = entries
        .iter()
        .filter(|e| e.matched_track_id.is_none())
        .flat_map(|e| {
            let mut titles = vec![key(&e.title), title_key(&e.title)];
            if let Some(base) = parenthetical_key(&e.title) {
                titles.push(base);
            }
            titles
                .into_iter()
                .map(|title| (artist_key(&e.artist), title))
                .collect::<Vec<_>>()
        })
        .collect::<HashSet<_>>();
    if wanted.is_empty() {
        return Ok(());
    }
    // A chart lead can reach a library collaboration ("John Lennon" vs "John
    // Lennon & Yoko Ono"); SongIndex only uses it after full-credit matches fail.
    let wanted_performers = entries
        .iter()
        .filter(|e| e.matched_track_id.is_none())
        .flat_map(|e| {
            let titles = [key(&e.title), title_key(&e.title)];
            main_performers(&e.artist)
                .into_iter()
                .take(1)
                .flat_map(move |performer| titles.clone().map(|title| (performer.clone(), title)))
        })
        .collect::<HashSet<_>>();
    let candidate_performers = wanted_performers
        .iter()
        .map(|(performer, _)| performer.clone())
        .collect::<HashSet<_>>();
    // Honor existing annual catalog matches when they resolve to a single live track.
    let annual_exists = conn.query_row("SELECT 1 FROM sqlite_master WHERE type='table' AND name='billboard_single_chart_entries'", [], |_| Ok(())).optional().map_err(|e| e.to_string())?.is_some();
    let mut preferred = HashMap::<(String, String), HashSet<i64>>::new();
    if annual_exists {
        let mut statement = conn.prepare("SELECT DISTINCT e.artist, e.title, t.id FROM billboard_single_chart_entries e JOIN tracks t ON t.id=e.matched_track_id").map_err(|e| e.to_string())?;
        for row in statement
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?
        {
            let (artist, title, id) = row.map_err(|e| e.to_string())?;
            let identity = (artist_key(&artist), title_key(&title));
            if wanted.contains(&identity) {
                preferred.entry(identity).or_default().insert(id);
            }
        }
    }
    let mut song_index = crate::chart_song_match::SongIndex::default();
    let mut matches = Vec::<Match>::new();
    // Normalize each distinct artist once, then seek their tracks through the
    // covering artist/title indexes. Normalizing every title in a large catalog
    // on every chart load is prohibitively expensive.
    let candidate_artists = wanted
        .iter()
        .map(|(artist, _)| artist.clone())
        .collect::<HashSet<_>>();
    conn.create_scalar_function(
        "aurora_chart_artist",
        1,
        rusqlite::functions::FunctionFlags::SQLITE_UTF8
            | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
        move |context| {
            let artist = context.get::<Option<String>>(0)?.unwrap_or_default();
            Ok(candidate_artists.contains(&artist_key(&artist))
                || main_performers(&artist)
                    .iter()
                    .any(|performer| candidate_performers.contains(performer)))
        },
    )
    .map_err(|e| e.to_string())?;
    let candidate_identities = wanted.clone();
    conn.create_scalar_function(
        "aurora_chart_identity",
        2,
        rusqlite::functions::FunctionFlags::SQLITE_UTF8
            | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
        move |context| {
            let artist = context.get::<Option<String>>(0)?.unwrap_or_default();
            let title = context.get::<Option<String>>(1)?.unwrap_or_default();
            let performers = main_performers(&artist);
            let artist = artist_key(&artist);
            Ok(performers.iter().any(|performer| {
                wanted_performers.contains(&(performer.clone(), key(&title)))
                    || wanted_performers.contains(&(performer.clone(), title_key(&title)))
            }) || candidate_identities.contains(&(artist.clone(), key(&title)))
                || candidate_identities.contains(&(artist.clone(), title_key(&title)))
                || parenthetical_key(&title)
                    .is_some_and(|base| candidate_identities.contains(&(artist, base))))
        },
    )
    .map_err(|e| e.to_string())?;
    let mut statement = conn
        .prepare(
            "WITH display_artists AS MATERIALIZED (
           SELECT DISTINCT display_artist COLLATE NOCASE AS artist FROM tracks
         ), album_artists AS MATERIALIZED (
           SELECT DISTINCT album_artist_display COLLATE NOCASE AS artist FROM tracks
         )
         SELECT t.id, t.title, COALESCE(NULLIF(t.display_artist,''), t.album_artist_display,''),
                COALESCE(t.album_artist_display,''), t.album_id, a.album,
                COALESCE(t.normalized_rating / 20.0, CAST(NULLIF(t.rating_raw,'') AS REAL)),
                t.love = 'L', a.album_score
         FROM tracks t LEFT JOIN albums a ON a.id=t.album_id WHERE t.id IN (
           SELECT id FROM tracks WHERE display_artist COLLATE NOCASE IN (
             SELECT artist FROM display_artists
             WHERE aurora_chart_artist(artist)
           ) AND aurora_chart_identity(display_artist, title)
           UNION SELECT id FROM tracks WHERE album_artist_display COLLATE NOCASE IN (
             SELECT artist FROM album_artists
             WHERE aurora_chart_artist(artist)
           ) AND aurora_chart_identity(album_artist_display, title)
         ) ORDER BY t.id",
        )
        .map_err(|e| format!("Could not match chart songs to the catalog: {e}"))?;
    let mut cursor = statement.query([]).map_err(|e| e.to_string())?;
    while let Some(row) = cursor.next().map_err(|e| e.to_string())? {
        let raw_title = row
            .get::<_, Option<String>>(1)
            .map_err(|e| e.to_string())?
            .unwrap_or_default();
        let title = key(&raw_title);
        let display_artist = row.get::<_, String>(2).map_err(|e| e.to_string())?;
        let artist = artist_key(&display_artist);
        let identity = (artist.clone(), title_key(&raw_title));
        let id: i64 = row.get(0).map_err(|e| e.to_string())?;
        let album_artist = artist_key(&row.get::<_, String>(3).map_err(|e| e.to_string())?);
        let saved = preferred
            .get(&identity)
            .is_some_and(|ids| ids.len() == 1 && ids.contains(&id));
        let candidate = Match {
            track_id: id.to_string(),
            album_id: row.get(4).map_err(|e| e.to_string())?,
            album_title: row.get(5).map_err(|e| e.to_string())?,
            rating: row
                .get::<_, Option<f64>>(6)
                .map_err(|e| e.to_string())?
                .filter(|r| (0.5..=5.0).contains(r)),
            loved: row
                .get::<_, Option<bool>>(7)
                .map_err(|e| e.to_string())?
                .unwrap_or(false),
            score: row.get(8).map_err(|e| e.to_string())?,
            preference: (!saved, album_artist != identity.0, id),
        };
        song_index.insert(
            matches.len(),
            &[artist],
            &main_performers(&display_artist),
            &title,
            &[title_key(&raw_title)],
            parenthetical_key(&raw_title).as_deref(),
        );
        matches.push(candidate);
    }
    for entry in entries.iter_mut().filter(|e| e.matched_track_id.is_none()) {
        let candidates = song_index.resolve(
            &[artist_key(&entry.artist)],
            &main_performers(&entry.artist),
            &key(&entry.title),
            &[title_key(&entry.title)],
            parenthetical_key(&entry.title).as_deref(),
        );
        if let Some(found) = candidates
            .iter()
            .map(|id| &matches[*id])
            .min_by_key(|found| found.preference)
        {
            entry.matched_track_id = Some(found.track_id.clone());
            entry.matched_album_id = found.album_id.clone();
            entry.matched_album_title = found.album_title.clone();
            entry.artwork_album_id = found.album_id.clone();
            entry.rating = found.rating;
            entry.loved = found.loved;
            entry.album_score = found.score;
        }
    }
    Ok(())
}

pub(super) fn match_albums(conn: &Connection, entries: &mut [ChartEntry]) -> Result<(), String> {
    let wanted = entries
        .iter()
        .filter(|e| e.matched_album_id.is_none())
        .map(|e| (key(&e.artist), key(&e.title)))
        .collect::<HashSet<_>>();
    if wanted.is_empty() {
        return Ok(());
    }
    conn.create_scalar_function(
        "aurora_chart_album_identity",
        2,
        rusqlite::functions::FunctionFlags::SQLITE_UTF8
            | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
        move |context| {
            let artist = context.get::<Option<String>>(0)?.unwrap_or_default();
            let title = context.get::<Option<String>>(1)?.unwrap_or_default();
            Ok(wanted.contains(&(key(&artist), key(&title))))
        },
    )
    .map_err(|e| e.to_string())?;
    let mut statement = conn.prepare("SELECT id, album, album_artist_display,
        CAST(COALESCE(effective_album_rating, calculated_album_rating, album_rating) AS REAL) / 20.0,
        COALESCE(loved_tracks, 0) > 0, album_score FROM albums
        WHERE aurora_chart_album_identity(album_artist_display, album) ORDER BY id")
        .map_err(|e| e.to_string())?;
    let mut rows = statement.query([]).map_err(|e| e.to_string())?;
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let id: String = row.get(0).map_err(|e| e.to_string())?;
        let title: String = row.get(1).map_err(|e| e.to_string())?;
        let artist: String = row.get(2).map_err(|e| e.to_string())?;
        for entry in entries.iter_mut().filter(|e| {
            e.matched_album_id.is_none()
                && key(&e.artist) == key(&artist)
                && key(&e.title) == key(&title)
        }) {
            entry.matched_album_id = Some(id.clone());
            entry.matched_album_title = Some(title.clone());
            entry.artwork_album_id = Some(id.clone());
            entry.rating = row.get(3).map_err(|e| e.to_string())?;
            entry.loved = row.get(4).map_err(|e| e.to_string())?;
            entry.album_score = row.get(5).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub(super) fn page(conn: &Connection, request: ChartPageRequest) -> Result<ChartPage, String> {
    let mut rows = rows(conn, &request)?;
    let artist_keys = filtered_artist_keys(conn, &request.filters)?;
    if let Some(keys) = artist_keys {
        rows.retain(|row| keys.contains(&row.artist.trim().to_lowercase()));
    }
    let mut grouped = BTreeMap::<(String, String), Vec<WeeklyRow>>::new();
    for row in rows {
        grouped
            .entry((row.artist.clone(), row.title.clone()))
            .or_default()
            .push(row);
    }
    let mut entries = grouped
        .into_values()
        .map(|rows| {
            let first = &rows[0];
            let peak = rows.iter().map(|r| r.position).min();
            ChartEntry {
                position: first.position as usize,
                source_position: first.position,
                previous_position: first.previous,
                movement: first.previous.map(|p| p - first.position),
                peak_position: if request.scope == ChartScope::Week {
                    first.peak
                } else {
                    peak
                },
                appearances: if request.scope == ChartScope::Week {
                    first.appearances
                } else {
                    rows.len() as u32
                },
                weeks_at_number_one: rows.iter().filter(|r| r.position == 1).count() as u32,
                total_points: rows.iter().map(|r| (101 - r.position).max(1)).sum(),
                artist: first.artist.clone(),
                title: first.title.clone(),
                artist_key: first.artist.clone(),
                title_key: first.title.clone(),
                matched_track_id: None,
                matched_album_id: None,
                matched_album_title: None,
                artwork_album_id: None,
                rating: None,
                loved: false,
                album_score: None,
                entry_date: first.debut.clone(),
            }
        })
        .collect::<Vec<_>>();
    if request.scope == ChartScope::Week {
        entries.sort_by(|a, b| {
            a.source_position
                .cmp(&b.source_position)
                .then(a.artist.cmp(&b.artist))
                .then(a.title.cmp(&b.title))
        });
    } else {
        entries.sort_by(|a, b| {
            b.weeks_at_number_one
                .cmp(&a.weeks_at_number_one)
                .then(b.appearances.cmp(&a.appearances))
                .then(a.peak_position.cmp(&b.peak_position))
                .then(a.artist.cmp(&b.artist))
                .then(a.title.cmp(&b.title))
        });
        for (i, entry) in entries.iter_mut().enumerate() {
            entry.position = i + 1;
            entry.previous_position = None;
            entry.movement = None;
        }
    }
    entries.retain(|entry| request.matches_search(entry));
    let total_entries = entries.len();
    if request.library_status.is_empty() {
        entries.truncate(request.entry_limit());
    }
    match_entries(conn, &mut entries)?;
    let total_entries = if request.library_status.is_empty() {
        total_entries
    } else {
        entries.retain(|entry| {
            (entry.matched_track_id.is_some() || entry.matched_album_id.is_some())
                == (request.library_status == "inLibrary")
        });
        entries.len()
    };
    entries.truncate(request.entry_limit());
    let chart = request.published_chart.as_deref().unwrap_or("US weekly");
    Ok(ChartPage {
        chart_title: if request.scope == ChartScope::Week {
            chart.into()
        } else {
            format!("{chart} · {} year chart", request.selected_year)
        },
        source_label: "US weekly",
        annual_only: false,
        chart_date: if request.scope == ChartScope::Week {
            request.published_week.clone()
        } else {
            None
        },
        weeks: Vec::new(),
        entries,
        total_entries,
        album_score_entries: Vec::new(),
        request,
    })
}

pub(super) fn detail(
    conn: &Connection,
    request: ChartItemDetailRequest,
) -> Result<ChartItemDetail, String> {
    let rows = rows(conn, &request.page)?
        .into_iter()
        .filter(|r| r.artist == request.artist_key && r.title == request.title_key)
        .collect::<Vec<_>>();
    Ok(ChartItemDetail {
        source_ranks: vec![ChartSourceRank {
            source: ChartSource::PublishedUs,
            label: "US weekly",
            best_rank: rows.iter().map(|r| r.position).min(),
            appearances: rows.len() as u32,
            weeks_at_number_one: rows.iter().filter(|r| r.position == 1).count() as u32,
            annual_only: false,
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ChartPageRequest {
        ChartPageRequest {
            search: String::new(),
            library_status: String::new(),
            filters: ChartArtistFilters::default(),
            kind: ChartKind::Singles,
            source: ChartSource::PublishedUs,
            scope: ChartScope::Week,
            selected_year: 1993,
            selected_week: 1,
            published_chart: Some("Mainstream Rock Tracks".into()),
            published_week: Some("1993-01-02".into()),
            period: ChartPeriod {
                from_year: 1993,
                from_week: 1,
                to_year: 1993,
                to_week: 53,
                label: "1993 year chart".into(),
            },
            year_basis: ChartYearBasis::Year,
            limit: 1000,
        }
    }

    fn fixture() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE published_chart_books (id INTEGER, chart TEXT, book TEXT, row_count INTEGER);
          CREATE TABLE published_chart_entries (book_id INTEGER, artist TEXT, title TEXT, week_ending TEXT, position INTEGER, last_week TEXT, weeks_on_chart TEXT, entry_date TEXT, source_row INTEGER, peak_position TEXT);
          CREATE TABLE albums (id TEXT, album TEXT, album_score REAL);
          CREATE TABLE tracks (id INTEGER, title TEXT, display_artist TEXT, album_artist_display TEXT, album_id TEXT, normalized_rating INTEGER, rating_raw TEXT, love TEXT);
          CREATE TABLE billboard_single_chart_entries (artist TEXT, title TEXT, matched_track_id INTEGER);
          INSERT INTO published_chart_books VALUES (1,'Mainstream Rock Tracks','1993_us_singles',5),(2,'Country Singles Chart','1993_us_singles',1);
          INSERT INTO published_chart_entries VALUES
            (1,'Artist','Song (LP Version)','1993-01-02',2,'5','8','1992-11-14',1,'1'),
            (1,'Artist','Other Song','1993-01-02',150,'','1','1993-01-02',2,''),
            (1,'Unmatched','Song','1993-01-02',3,'','1','',3,''),
            (1,'Artist','Song (LP Version)','1993-01-09',1,'2','9','1992-11-14',4,'1'),
            (1,'Artist','Song (LP Version)','1993-01-09',1,'2','9','1992-11-14',5,'1'),
            (2,'Artist','Song (LP Version)','1993-01-02',9,'','1','',1,'9');
          INSERT INTO albums VALUES ('artist-album','Artist Album',100),('compilation','Hits',90),('remix','Remixes',80);
          INSERT INTO tracks VALUES (1,'Song','Artist','Various Artists','compilation',80,'',''),
            (2,'Song','Artist','Artist','artist-album',90,'','L'),
            (3,'Song (Live)','Artist','Artist','artist-album',80,'',''),
            (4,'Song','Other Artist','Other Artist','remix',80,'','');").unwrap();
        conn
    }

    #[test]
    fn full_weekly_and_year_charts_and_search_reach_beyond_one_thousand() {
        let conn = fixture();
        for index in 0..1105 {
            conn.execute("INSERT INTO published_chart_entries VALUES (1, 'Needle Artist', ?1, '1993-01-02', ?2, '', '1', '', ?2, '')", params![format!("Extra Song {index:04}"), index + 200]).unwrap();
        }
        conn.execute("INSERT INTO tracks VALUES (5,'Extra Song 1104','Needle Artist','Needle Artist','artist-album',80,'','')", []).unwrap();
        for scope in [ChartScope::Week, ChartScope::Period] {
            let mut request = request();
            request.scope = scope;
            request.limit = 100;
            let limited = page(&conn, request.clone()).unwrap();
            assert_eq!(limited.entries.len(), 100);
            request.limit = 0;
            let full = page(&conn, request.clone()).unwrap();
            assert_eq!(full.entries.len(), full.total_entries);
            assert_eq!(full.total_entries, 1108);
            assert_eq!(
                limited
                    .entries
                    .iter()
                    .map(|entry| &entry.title)
                    .collect::<Vec<_>>(),
                full.entries
                    .iter()
                    .take(100)
                    .map(|entry| &entry.title)
                    .collect::<Vec<_>>()
            );
            request.limit = 1;
            request.search = " eXtRa SoNg 1104 ".into();
            request.library_status = "inLibrary".into();
            let searched = page(&conn, request.clone()).unwrap();
            assert_eq!(searched.total_entries, 1);
            assert_eq!(searched.entries[0].title, "Extra Song 1104");
            assert!(searched.entries[0].position > 1000);
            request.library_status.clear();
            request.search = "needle artist".into();
            assert_eq!(page(&conn, request).unwrap().total_entries, 1105);
        }
    }

    #[test]
    fn charted_lead_artist_matches_library_collaboration() {
        let conn = fixture();
        conn.execute_batch(
            "INSERT INTO published_chart_entries VALUES
               (1,'JOHN LENNON','WOMAN','1993-01-02',20,'','1','',20,''),
               (1,'DURAN DURAN','WOMAN','1993-01-02',21,'','1','',21,''),
               (1,'DAVE STEWART WITH BARBARA GASKIN','IT''S MY PARTY','1993-01-02',22,'','1','',22,'');
             INSERT INTO tracks VALUES
               (6,'Woman','John Lennon & Yoko Ono','John Lennon & Yoko Ono','artist-album',80,'',''),
               (7,'It''s My Party','Dave Stewart & Barbara Gaskin','Dave Stewart','artist-album',80,'','');",
        )
        .unwrap();
        let chart = page(&conn, request()).unwrap();
        let matched = |artist: &str| {
            chart
                .entries
                .iter()
                .find(|entry| entry.artist == artist)
                .unwrap()
                .matched_track_id
                .clone()
        };
        assert_eq!(matched("JOHN LENNON"), Some("6".into()));
        assert_eq!(
            matched("DAVE STEWART WITH BARBARA GASKIN"),
            Some("7".into())
        );
        assert_eq!(matched("DURAN DURAN"), None);
    }

    #[test]
    fn parenthetical_examples_match_both_directions_and_exact_title_wins() {
        for (artist, printed, bare) in [
            (
                "'Til Tuesday",
                "Looking Over My Shoulder (Single Mix)",
                "Looking Over My Shoulder",
            ),
            (
                "Bon Jovi",
                "In And Out Of Love (Edit)",
                "In And Out Of Love",
            ),
            (
                "Jesse Johnson's Revue",
                "I Want My Girl (Specially Remixed Version)",
                "I Want My Girl",
            ),
            (
                "Y&T",
                "Summertime Girls (Studio Version)",
                "Summertime Girls",
            ),
            (
                "Kim Carnes",
                "Crazy In The Night (Barking At Airplanes)",
                "Crazy In The Night",
            ),
            (
                "Tina Turner",
                "We Don't Need Another Hero (Thunderdome)",
                "We Don't Need Another Hero",
            ),
        ] {
            for (chart, library) in [(printed, bare), (bare, printed)] {
                let conn = fixture();
                conn.execute("DELETE FROM tracks", []).unwrap();
                conn.execute(
                    "UPDATE published_chart_entries SET artist=?1, title=?2",
                    params![artist, chart],
                )
                .unwrap();
                conn.execute(
                    "INSERT INTO tracks VALUES (10,?1,?2,?2,'artist-album',80,'','')",
                    params![library, artist],
                )
                .unwrap();
                let page = super::page(&conn, request()).unwrap();
                assert_eq!(page.entries[0].title, chart);
                assert_eq!(
                    page.entries[0].matched_track_id.as_deref(),
                    Some("10"),
                    "{artist}: {chart}"
                );
                // An exact title must outrank the older fallback candidate.
                conn.execute(
                    "INSERT INTO tracks VALUES (11,?1,?2,?2,'compilation',80,'','')",
                    params![chart, artist],
                )
                .unwrap();
                let page = super::page(&conn, request()).unwrap();
                assert_eq!(page.entries[0].matched_track_id.as_deref(), Some("11"));
            }
        }
    }

    #[test]
    fn parenthetical_fallback_leaves_different_versions_ambiguous() {
        let conn = fixture();
        conn.execute("DELETE FROM tracks", []).unwrap();
        conn.execute(
            "UPDATE published_chart_entries SET title='Song' WHERE artist='Artist'",
            [],
        )
        .unwrap();
        conn.execute_batch(
            "INSERT INTO tracks VALUES
            (10,'Song (Live)','Artist','Artist','artist-album',80,'',''),
            (11,'Song (Remix)','Artist','Artist','artist-album',80,'','');",
        )
        .unwrap();
        let mut request = request();
        request.library_status = "inLibrary".into();
        assert_eq!(query_page(&conn, request.clone()).unwrap().total_entries, 0);
        conn.execute("DELETE FROM tracks WHERE id=11", []).unwrap();
        assert_eq!(query_page(&conn, request).unwrap().total_entries, 1);
    }

    #[test]
    fn library_filter_precedes_limit_and_preserves_printed_ranks() {
        let conn = fixture();
        let mut request = request();
        request.limit = 1;
        request.library_status = "notInLibrary".into();
        let page = query_page(&conn, request.clone()).unwrap();
        assert_eq!(page.total_entries, 2);
        assert_eq!(page.entries[0].position, 3);
        assert!(page.entries[0].matched_track_id.is_none());
        request.library_status = "inLibrary".into();
        let page = query_page(&conn, request).unwrap();
        assert_eq!(page.total_entries, 1);
        assert_eq!(page.entries[0].position, 2);
        assert_eq!(page.entries[0].matched_track_id.as_deref(), Some("2"));
    }

    #[test]
    fn reads_exact_calendar_dates_and_matches_without_conflating_versions() {
        let conn = fixture();
        assert_eq!(series(&conn).unwrap().len(), 2);
        assert_eq!(
            weeks(&conn, "Mainstream Rock Tracks", 1993).unwrap(),
            ["1993-01-02", "1993-01-09"]
        );
        let page = super::page(&conn, request()).unwrap();
        assert_eq!(page.total_entries, 3);
        assert_eq!(page.entries[0].position, 2); // retain printed positions, including gaps
        assert_eq!(page.entries[2].position, 150); // no top-100 truncation
        assert_eq!(page.entries[0].matched_track_id.as_deref(), Some("2"));
        assert_eq!(
            page.entries[0].matched_album_title.as_deref(),
            Some("Artist Album")
        );
        assert_eq!(page.entries[0].entry_date.as_deref(), Some("1992-11-14"));
        assert_eq!(page.entries[0].movement, Some(3));
        assert_eq!(page.entries[0].peak_position, Some(1));
        assert!(page.entries[1].matched_track_id.is_none()); // same title, different artist
        assert_ne!(title_key("Song (Live)"), title_key("Song"));
        assert_ne!(title_key("Song (Remix)"), title_key("Song"));
        let mut annual = request();
        annual.scope = ChartScope::Period;
        let annual = super::page(&conn, annual).unwrap();
        assert_eq!(annual.entries[0].appearances, 2); // duplicate source row counted once
        assert_eq!(annual.entries[0].weeks_at_number_one, 1);
        assert_eq!(annual.total_entries, 3); // two songs by same artist remain separate
    }

    #[test]
    fn all_singles_sources_resolve_corrected_tags_without_reimporting_matches() {
        let conn = fixture();
        conn.execute_batch("ALTER TABLE albums ADD COLUMN year INTEGER;
            ALTER TABLE albums ADD COLUMN release_year INTEGER;
            ALTER TABLE albums ADD COLUMN album_artist_display TEXT;
            CREATE TABLE vg_lista_single_chart_entries (id INTEGER, year INTEGER, week INTEGER, rank INTEGER, artist TEXT, title TEXT, artist_key TEXT, title_key TEXT, matched_track_id INTEGER, week_date TEXT);
            INSERT INTO vg_lista_single_chart_entries VALUES
              (1,1992,10,6,'Shakespears Sister','Stay','shakespears sister','stay',NULL,'1992-03-02'),
              (2,1992,11,7,'Shakespears Sister','Stay','shakespears sister','stay',NULL,'1992-03-09');
            INSERT INTO tracks VALUES (10,'Stay','Shakespear''s Sister','Shakespear''s Sister','artist-album',90,'','L');
            CREATE TABLE official_uk_single_chart_entries AS SELECT *, week_date AS chart_date, NULL AS last_week, NULL AS peak, NULL AS weeks_on_chart FROM vg_lista_single_chart_entries;
            CREATE TABLE ti_i_skuddet_chart_entries AS SELECT * FROM official_uk_single_chart_entries;
            CREATE TABLE norsktoppen_chart_entries AS SELECT * FROM official_uk_single_chart_entries;
            DROP TABLE billboard_single_chart_entries;
            CREATE TABLE billboard_single_chart_entries AS SELECT *, artist AS display_artist FROM vg_lista_single_chart_entries;").unwrap();
        let mut request = request();
        request.source = ChartSource::VgLista;
        request.limit = 100;
        request.selected_year = 1992;
        request.selected_week = 10;
        request.period.from_year = 1992;
        request.period.to_year = 1992;
        request.scope = ChartScope::Period;
        assert!(
            query_page(&conn, request.clone()).unwrap().entries[0]
                .matched_track_id
                .is_none()
        );
        conn.execute("UPDATE tracks SET display_artist='Shakespears Sister', album_artist_display='Shakespears Sister' WHERE id=10", []).unwrap();
        for source in [
            ChartSource::VgLista,
            ChartSource::OfficialUk,
            ChartSource::TiISkuddet,
            ChartSource::Norsktoppen,
            ChartSource::Billboard,
        ] {
            request.source = source;
            for scope in [ChartScope::Period, ChartScope::Week] {
                request.scope = scope;
                let page = query_page(&conn, request.clone()).unwrap();
                assert_eq!(page.total_entries, 1);
                let entry = &page.entries[0];
                assert_eq!(entry.matched_track_id.as_deref(), Some("10"));
                assert_eq!(entry.matched_album_title.as_deref(), Some("Artist Album"));
                assert_eq!(entry.rating, Some(4.5));
                assert!(entry.loved);
                request.library_status = "notInLibrary".into();
                let missing = query_page(&conn, request.clone()).unwrap();
                assert_eq!(missing.total_entries, 0);
                assert!(missing.entries.is_empty());
                request.library_status = "inLibrary".into();
                assert_eq!(query_page(&conn, request.clone()).unwrap().total_entries, 1);
                request.library_status.clear();
            }
        }
        assert_eq!(conn.query_row("SELECT count(*) FROM vg_lista_single_chart_entries WHERE matched_track_id IS NOT NULL", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
        // An existing archive match remains authoritative, even when another
        // catalog candidate would otherwise be preferred.
        conn.execute(
            "UPDATE vg_lista_single_chart_entries SET matched_track_id=1",
            [],
        )
        .unwrap();
        request.source = ChartSource::VgLista;
        request.limit = 100;
        assert_eq!(
            query_page(&conn, request.clone()).unwrap().entries[0]
                .matched_track_id
                .as_deref(),
            Some("1")
        );
        conn.execute("DELETE FROM tracks WHERE id=1", []).unwrap();
        assert_eq!(
            query_page(&conn, request).unwrap().entries[0]
                .matched_track_id
                .as_deref(),
            Some("10")
        );
    }

    #[test]
    #[ignore = "read-only audit against an explicitly supplied local catalog"]
    fn live_vg_lista_1992_stay_audit() {
        let path = std::env::var("AURORA_CHART_AUDIT_CATALOG").unwrap();
        let conn = catalog::open_catalog(std::path::Path::new(&path)).unwrap();
        let mut request = request();
        request.source = ChartSource::VgLista;
        request.limit = 100;
        request.scope = ChartScope::Period;
        request.selected_year = 1992;
        request.period.from_year = 1992;
        request.period.to_year = 1992;
        let started = std::time::Instant::now();
        let page = query_page(&conn, request).unwrap();
        let entry = page
            .entries
            .iter()
            .find(|e| e.artist == "Shakespears Sister" && e.title == "Stay")
            .unwrap();
        eprintln!(
            "VG Lista 1992: {} entries in {:?}; Stay track={:?}, album={:?}",
            page.total_entries,
            started.elapsed(),
            entry.matched_track_id,
            entry.matched_album_title
        );
        assert!(entry.matched_track_id.is_some());
    }

    #[test]
    fn respects_catalog_match_and_reports_missing_archive() {
        let conn = fixture();
        conn.execute(
            "INSERT INTO billboard_single_chart_entries VALUES ('Artist','Song (LP)',1)",
            [],
        )
        .unwrap();
        assert_eq!(
            super::page(&conn, request()).unwrap().entries[0]
                .matched_track_id
                .as_deref(),
            Some("1")
        );
        assert!(
            series(&Connection::open_in_memory().unwrap())
                .unwrap_err()
                .contains("Open Published Charts in Music Library")
        );
        let mut wrong = request();
        wrong.published_week = Some("1992-12-26".into());
        assert!(super::page(&conn, wrong).is_err());
    }

    #[test]
    #[ignore = "read-only audit against an explicitly supplied local catalog"]
    fn live_catalog_weekly_audit() {
        let path = std::env::var("AURORA_CHART_AUDIT_CATALOG")
            .expect("Supply a catalog path for this read-only audit");
        let conn = catalog::open_catalog(std::path::Path::new(&path)).unwrap();
        let started = std::time::Instant::now();
        let series = series(&conn).unwrap();
        eprintln!("{} chart series in {:?}", series.len(), started.elapsed());
        for chart in [
            "Mainstream Rock Tracks",
            "Country Singles Chart",
            "Billboard Hot 100",
        ] {
            let mut request = request();
            request.published_chart = Some(chart.into());
            let dates = weeks(&conn, chart, 1993).unwrap();
            request.published_week = Some(dates[0].clone());
            let started = std::time::Instant::now();
            let page = super::page(&conn, request).unwrap();
            eprintln!(
                "{chart}: {} entries, {} matched, {:?}",
                page.total_entries,
                page.entries
                    .iter()
                    .filter(|e| e.matched_track_id.is_some())
                    .count(),
                started.elapsed()
            );
            assert!(!page.entries.is_empty());
        }
    }
}
