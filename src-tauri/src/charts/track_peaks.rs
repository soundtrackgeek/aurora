use super::*;
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

// Keep this key contract aligned with Music Library db::billboard_text_key.
fn catalog_chart_key(value: &str) -> String {
    let lowercased = value.replace('&', " and ").to_lowercase();
    let folded = lowercased
        .nfd()
        .filter(|character| !is_combining_mark(*character))
        .fold(String::new(), |mut normalized, character| {
            match character {
                'æ' => normalized.push_str("ae"),
                'œ' => normalized.push_str("oe"),
                'ø' => normalized.push('o'),
                'ð' => normalized.push('d'),
                'þ' => normalized.push_str("th"),
                'ł' => normalized.push('l'),
                'ß' => normalized.push_str("ss"),
                _ => normalized.push(character),
            }
            normalized
        });

    folded
        .split(|character: char| !character.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

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
    let mut peaks = Vec::new();
    for source in detail_sources(ChartKind::Singles) {
        let table = table_for(ChartKind::Singles, *source)?;
        if !table_exists(connection, table)? {
            continue;
        }
        let peak: Option<i64> = connection.query_row(
            &format!("SELECT MIN(rank) FROM {table} WHERE artist_key = ?1 AND title_key = ?2 AND rank > 0"),
            [&artist_key, &title_key], |r| r.get(0),
        ).map_err(|e| format!("Could not read track chart peaks: {e}"))?;
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
        let mut statement = connection.prepare(
            "SELECT b.chart, MIN(CASE WHEN CAST(e.peak_position AS INTEGER) > 0 THEN MIN(e.position, CAST(e.peak_position AS INTEGER)) ELSE e.position END)
             FROM published_chart_entries e JOIN published_chart_books b ON b.id = e.book_id
             WHERE e.artist = ?1 COLLATE NOCASE AND e.title = ?2 COLLATE NOCASE AND e.position > 0
             GROUP BY b.chart ORDER BY b.chart COLLATE NOCASE"
        ).map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([artist.trim(), title.trim()], |r| {
                Ok(TrackChartPeak {
                    label: r.get(0)?,
                    peak: r.get(1)?,
                    country: "US",
                })
            })
            .map_err(|e| e.to_string())?;
        peaks.extend(
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?,
        );
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
        assert!(
            query(&connection, "Beyoncé & Jay-Z", "Déjà Vu (Live)")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn missing_archives_are_empty_and_invalid_identity_is_rejected() {
        let connection = Connection::open_in_memory().unwrap();
        assert!(query(&connection, "Artist", "Song").unwrap().is_empty());
        assert!(query(&connection, "", "Song").is_err());
    }
}
