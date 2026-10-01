use super::*;
use crate::chart_identity::{artist_group_key, main_performers, text_key as catalog_chart_key};
use crate::chart_song_match::without_parentheses;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrackChartPeak {
    label: String,
    peak: i64,
    country: &'static str,
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool, String> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [table],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())
}

/// Music Library records a key version once every archive row has identity keys.
fn published_identity_keys(connection: &Connection) -> Result<bool, String> {
    if !table_exists(connection, "published_chart_identity")? {
        return Ok(false);
    }
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM published_chart_identity)",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())
}

fn query(
    connection: &Connection,
    artist: &str,
    title: &str,
) -> Result<Vec<TrackChartPeak>, String> {
    if artist.trim().is_empty()
        || title.trim().is_empty()
        || artist.len() > 2048
        || title.len() > 2048
    {
        return Err("The selected track identity is invalid.".into());
    }
    let artist_key = catalog_chart_key(artist);
    let title_key = catalog_chart_key(title);
    // Library titles carry version suffixes ("Colour of Love (Massive version)")
    // that charts do not print, and collaborations are credited by their lead.
    // Like the stored chart ranks, try the exact identity first, then fall back.
    let base_key = without_parentheses(title).map(|base| catalog_chart_key(&base));
    let titles = std::iter::once(title_key.clone())
        .chain(base_key.filter(|base| !base.is_empty() && *base != title_key))
        .collect::<Vec<_>>();
    let artists = std::iter::once(artist_key.clone())
        .chain(
            main_performers(artist)
                .into_iter()
                .filter(|performer| *performer != artist_key),
        )
        .collect::<Vec<_>>();
    let mut peaks = Vec::new();
    for source in detail_sources(ChartKind::Singles) {
        let table = table_for(ChartKind::Singles, *source)?;
        if !table_exists(connection, table)? {
            continue;
        }
        let mut peak: Option<i64> = None;
        'lookup: for artist_key in &artists {
            for title_key in &titles {
                peak = connection.query_row(
                    &format!("SELECT MIN(rank) FROM {table} WHERE artist_key = ?1 AND title_key = ?2 AND rank > 0"),
                    [artist_key, title_key], |r| r.get(0),
                ).map_err(|e| format!("Could not read track chart peaks: {e}"))?;
                if peak.is_some() {
                    break 'lookup;
                }
            }
        }
        if let Some(peak) = peak {
            peaks.push(TrackChartPeak {
                label: if *source == ChartSource::Billboard {
                    "Billboard annual".into()
                } else {
                    source_label(*source).into()
                },
                peak,
                country: match source {
                    ChartSource::OfficialUk => "UK",
                    ChartSource::Billboard => "US",
                    _ => "NO",
                },
            });
        }
    }
    if table_exists(connection, "published_chart_books")?
        && table_exists(connection, "published_chart_entries")?
    {
        // Keyed archives merge printed spellings (Hall & Oates vs Hall / Oates);
        // archives from older Music Library versions only match exact spellings.
        let keyed = published_identity_keys(connection)?;
        let identity = if keyed {
            "e.artist_group_key = ?1 AND e.title_key = ?2"
        } else {
            "e.artist = ?1 COLLATE NOCASE AND e.title = ?2 COLLATE NOCASE"
        };
        let mut attempts = Vec::new();
        if keyed {
            let group_artists =
                std::iter::once(artist_group_key(artist)).chain(main_performers(artist));
            for group_artist in group_artists {
                for title in &titles {
                    attempts.push((group_artist.clone(), title.clone()));
                }
            }
        } else {
            attempts.push((artist.trim().to_string(), title.trim().to_string()));
            if let Some(base) = without_parentheses(title.trim()) {
                attempts.push((artist.trim().to_string(), base));
            }
        }
        let mut statement = connection.prepare(&format!(
            "SELECT b.chart, MIN(CASE WHEN CAST(e.peak_position AS INTEGER) > 0 THEN MIN(e.position, CAST(e.peak_position AS INTEGER)) ELSE e.position END)
             FROM published_chart_entries e JOIN published_chart_books b ON b.id = e.book_id
             WHERE {identity} AND e.position > 0
             GROUP BY b.chart ORDER BY b.chart COLLATE NOCASE"
        )).map_err(|e| e.to_string())?;
        for (artist, title) in attempts {
            let rows = statement
                .query_map([artist, title], |r| {
                    Ok(TrackChartPeak {
                        label: r.get(0)?,
                        peak: r.get(1)?,
                        country: "US",
                    })
                })
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            if !rows.is_empty() {
                peaks.extend(rows);
                break;
            }
        }
    }
    Ok(peaks)
}

pub(crate) fn load(artist: String, title: String) -> Result<Vec<TrackChartPeak>, String> {
    let connection = catalog::open_catalog(&catalog::default_catalog_path()?)?;
    query(&connection, &artist, &title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combines_all_years_and_series_without_merging_other_recordings() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("CREATE TABLE official_uk_single_chart_entries (artist_key TEXT, title_key TEXT, rank INTEGER);
            INSERT INTO official_uk_single_chart_entries VALUES ('artist','song',14),('artist','song',3),('other','song',1),('artist','song',0);
            CREATE TABLE billboard_single_chart_entries AS SELECT 'artist' artist_key, 'song' title_key, 153 rank;
            CREATE TABLE published_chart_books (id INTEGER, chart TEXT);
            INSERT INTO published_chart_books VALUES (1,'Radio Songs'),(2,'Radio Songs'),(3,'Mainstream Rock Tracks');
            CREATE TABLE published_chart_entries (book_id INTEGER, artist TEXT, title TEXT, position INTEGER, peak_position TEXT);
            INSERT INTO published_chart_entries VALUES (1,'ARTIST','Song',8,'4'),(2,'Artist','Song',2,'1'),(3,'Artist','Song',53,''),(1,'Artist','Song (Live)',1,'1'),(3,'Other','Song',1,'1');").unwrap();
        let peaks = query(&connection, "Artist", "Song").unwrap();
        assert_eq!(
            peaks
                .iter()
                .map(|p| (p.label.as_str(), p.peak, p.country))
                .collect::<Vec<_>>(),
            vec![
                ("Official UK", 3, "UK"),
                ("Billboard annual", 153, "US"),
                ("Mainstream Rock Tracks", 53, "US"),
                ("Radio Songs", 1, "US")
            ]
        );
        assert!(query(&connection, "Artist", "Missing").unwrap().is_empty());
    }

    #[test]
    fn punctuation_matches_legacy_charts_without_changing_weekly_peaks() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("CREATE TABLE official_uk_single_chart_entries (artist_key TEXT, title_key TEXT, rank INTEGER);
            INSERT INTO official_uk_single_chart_entries VALUES ('breathe','how can i fall',48),('other','how can i fall',1),('breathe','how can i fall live',1);
            CREATE TABLE ti_i_skuddet_chart_entries AS SELECT 'breathe' artist_key, 'how can i fall' title_key, 14 rank;
            CREATE TABLE billboard_single_chart_entries AS SELECT 'breathe' artist_key, 'how can i fall' title_key, 97 rank;
            CREATE TABLE published_chart_books (id INTEGER, chart TEXT);
            INSERT INTO published_chart_books VALUES (1,'Billboard Hot 100');
            CREATE TABLE published_chart_entries (book_id INTEGER, artist TEXT, title TEXT, position INTEGER, peak_position TEXT);
            INSERT INTO published_chart_entries VALUES (1,'Breathe','How Can I Fall?',24,'3');").unwrap();
        let peaks = query(&connection, "Breathe", "How Can I Fall?").unwrap();
        let actual = peaks
            .iter()
            .map(|p| (p.label.as_str(), p.peak, p.country))
            .collect::<Vec<_>>();
        assert!(actual.contains(&("Official UK", 48, "UK")));
        assert!(actual.contains(&("Ti i Skuddet", 14, "NO")));
        assert!(actual.contains(&("Billboard annual", 97, "US")));
        assert!(actual.contains(&("Billboard Hot 100", 3, "US")));
        assert_eq!(actual.len(), 4);
    }

    #[test]
    fn catalog_keys_fold_punctuation_accents_and_artist_ampersands() {
        assert_eq!(
            catalog_chart_key("  BÉYONCÉ & JAY-Z  "),
            "beyonce and jay z"
        );
        assert_eq!(catalog_chart_key("Æ Œ Ø Ð Þ Ł ß"), "ae oe o d th l ss");
        assert_eq!(catalog_chart_key("Don't  Stop… (Live)"), "don t stop live");
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("CREATE TABLE official_uk_single_chart_entries (artist_key TEXT, title_key TEXT, rank INTEGER);
            INSERT INTO official_uk_single_chart_entries VALUES ('beyonce and jay z','deja vu',1);").unwrap();
        let peaks = query(&connection, "Beyoncé & Jay-Z", "Déjà Vu").unwrap();
        assert_eq!(peaks.len(), 1);
        assert_eq!(peaks[0].peak, 1);
        // A library version suffix falls back to the plain chart title, like the track badges.
        assert_eq!(
            query(&connection, "Beyoncé & Jay-Z", "Déjà Vu (Live)").unwrap()[0].peak,
            1
        );
        assert!(
            query(&connection, "Beyoncé & Jay-Z", "Déjà Vu Reprise")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn keyed_published_archives_merge_printed_artist_spellings() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("CREATE TABLE published_chart_books (id INTEGER, chart TEXT);
            INSERT INTO published_chart_books VALUES (1,'Billboard Hot 100'),(2,'Adult Contemporary');
            CREATE TABLE published_chart_identity (id INTEGER PRIMARY KEY, key_version INTEGER);
            CREATE TABLE published_chart_entries (book_id INTEGER, artist TEXT, title TEXT, position INTEGER, peak_position TEXT, artist_group_key TEXT, title_key TEXT);").unwrap();
        // Columns added by a migration but not yet keyed keep the exact-spelling lookup.
        connection
            .execute(
                "INSERT INTO published_chart_entries VALUES (1, 'Daryl Hall & John Oates', 'Kiss On My List', 2, '', NULL, NULL)",
                [],
            )
            .unwrap();
        let unkeyed = query(&connection, "Daryl Hall & John Oates", "Kiss On My List").unwrap();
        assert_eq!(unkeyed.len(), 1);
        assert_eq!(unkeyed[0].peak, 2);
        connection
            .execute_batch(
                "DELETE FROM published_chart_entries; INSERT INTO published_chart_identity VALUES (1, 1);",
            )
            .unwrap();
        for (book, artist, title, position) in [
            (1, "Daryl Hall John Oates", "Kiss on My List", 1),
            (2, "Daryl Hall / John Oates", "Kiss On My List", 4),
            (1, "Daryl Hall & John Oates", "Kiss On My List (Live)", 1),
            (2, "Electric Light Orchestra", "Kiss On My List", 1),
        ] {
            connection
                .execute(
                    "INSERT INTO published_chart_entries VALUES (?1, ?2, ?3, ?4, '', ?5, ?6)",
                    rusqlite::params![
                        book,
                        artist,
                        title,
                        position,
                        artist_group_key(artist),
                        catalog_chart_key(title)
                    ],
                )
                .unwrap();
        }
        let peaks = query(&connection, "Daryl Hall & John Oates", "Kiss On My List").unwrap();
        assert_eq!(
            peaks
                .iter()
                .map(|p| (p.label.as_str(), p.peak))
                .collect::<Vec<_>>(),
            vec![("Adult Contemporary", 4), ("Billboard Hot 100", 1)]
        );
    }

    #[test]
    fn version_suffixes_and_collaboration_leads_fall_back_after_exact_match() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("CREATE TABLE official_uk_single_chart_entries (artist_key TEXT, title_key TEXT, rank INTEGER);
            INSERT INTO official_uk_single_chart_entries VALUES ('snap','colour of love',54),('snap','colour of love massive version',9),('john lennon','imagine',1);").unwrap();
        let peaks = query(&connection, "Snap!", "Colour of Love (Massive version)").unwrap();
        assert_eq!(peaks.len(), 1);
        assert_eq!(peaks[0].peak, 9);
        let peaks = query(&connection, "Snap!", "Colour of Love (Radio Edit)").unwrap();
        assert_eq!(peaks[0].peak, 54);
        let peaks = query(&connection, "John Lennon & Yoko Ono", "Imagine").unwrap();
        assert_eq!(peaks[0].peak, 1);
    }

    #[test]
    fn missing_archives_are_empty_and_invalid_identity_is_rejected() {
        let connection = Connection::open_in_memory().unwrap();
        assert!(query(&connection, "Artist", "Song").unwrap().is_empty());
        assert!(query(&connection, "", "Song").is_err());
    }
}
