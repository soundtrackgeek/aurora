use super::*;

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
    let mut peaks = Vec::new();
    for source in detail_sources(ChartKind::Singles) {
        let table = table_for(ChartKind::Singles, *source)?;
        if !table_exists(connection, table)? {
            continue;
        }
        let peak: Option<i64> = connection.query_row(
            &format!("SELECT MIN(rank) FROM {table} WHERE artist_key = ?1 AND title_key = ?2 AND rank > 0"),
            [artist.trim().to_lowercase(), title.trim().to_lowercase()], |r| r.get(0),
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
    fn missing_archives_are_empty_and_invalid_identity_is_rejected() {
        let connection = Connection::open_in_memory().unwrap();
        assert!(query(&connection, "Artist", "Song").unwrap().is_empty());
        assert!(query(&connection, "", "Song").is_err());
    }
}
