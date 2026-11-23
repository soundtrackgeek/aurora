//! An album's Rank: its place by Album Score among the fully rated albums of its
//! original year. An album still being rated is placed among those finished albums,
//! so its Rank moves as its score does.
//!
//! Scores-umbrella albums are ranked only against other scores, Soundtrack albums only
//! against other soundtracks, and everything else against the rest of the library.

use std::collections::{BTreeSet, HashMap};

use rusqlite::{Connection, OptionalExtension, params_from_iter};

use crate::explorer::AlbumSummary;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum RankPool {
    Scores,
    Soundtrack,
    General,
}

pub(crate) fn pool(genre: Option<&str>) -> RankPool {
    let genre = genre.unwrap_or_default().trim().to_lowercase();
    if crate::catalog::SCORE_GENRE_GROUP.contains(&genre.as_str()) {
        RankPool::Scores
    } else if genre == "soundtrack" {
        RankPool::Soundtrack
    } else {
        RankPool::General
    }
}

/// Fully rated albums' IDs and scores by original year and genre pool.
type RankPools = HashMap<(i64, RankPool), Vec<(String, f64)>>;

fn rank_year(album: &AlbumSummary) -> Option<i64> {
    album
        .original_year
        .filter(|year| (1000..=2999).contains(year))
}

fn has_temp_table(connection: &Connection, name: &str) -> bool {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_temp_master WHERE type = 'table' AND name = ?1",
            [name],
            |_| Ok(()),
        )
        .optional()
        .ok()
        .flatten()
        .is_some()
}

/// The live score and track-count joins, falling back to the catalog when this
/// connection has not projected saved rating edits.
struct LiveSql {
    joins: String,
    score: &'static str,
    complete: &'static str,
}

fn live_sql(connection: &Connection) -> LiveSql {
    let mut joins = String::new();
    let score = if has_temp_table(connection, "aurora_live_album_metrics") {
        joins.push_str(" LEFT JOIN temp.aurora_live_album_metrics AS live ON live.album_id = a.id");
        "CASE WHEN live.album_id IS NOT NULL THEN live.album_score ELSE a.album_score END"
    } else {
        "a.album_score"
    };
    let complete = if has_temp_table(connection, "aurora_live_album_rating_state") {
        joins.push_str(
            " LEFT JOIN temp.aurora_live_album_rating_state AS state ON state.album_id = a.id",
        );
        "CASE WHEN state.album_id IS NOT NULL \
           THEN state.total_tracks > 0 AND state.rated_tracks >= state.total_tracks \
           ELSE a.total_tracks > 0 AND a.rated_tracks >= a.total_tracks END"
    } else {
        "a.total_tracks > 0 AND a.rated_tracks >= a.total_tracks"
    };
    LiveSql {
        joins,
        score,
        complete,
    }
}

/// Every fully rated, scored album of the given years, with live scores, track
/// counts and genres when this connection has projected them.
fn scored_albums(connection: &Connection, years: &BTreeSet<i64>) -> Result<RankPools, String> {
    let mut pools = RankPools::new();
    if years.is_empty() {
        return Ok(pools);
    }
    let live = live_sql(connection);
    let placeholders = vec!["?"; years.len()].join(", ");
    let sql = format!(
        "SELECT a.id, a.year, {genre}, {score} FROM albums AS a{joins} \
         WHERE a.year IN ({placeholders}) AND ({complete})",
        genre = crate::live_genres::album_sql("a"),
        score = live.score,
        joins = live.joins,
        complete = live.complete,
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("Could not prepare the album ranks: {error}"))?;
    let mut rows = statement
        .query(params_from_iter(years.iter()))
        .map_err(|error| format!("Could not read the album ranks: {error}"))?;
    while let Some(row) = rows
        .next()
        .map_err(|error| format!("Could not read the album ranks: {error}"))?
    {
        let decode = |error: rusqlite::Error| format!("Could not decode the album ranks: {error}");
        let Some(score) = row.get::<_, Option<f64>>(3).map_err(decode)? else {
            continue;
        };
        let genre: Option<String> = row.get(2).map_err(decode)?;
        pools
            .entry((row.get(1).map_err(decode)?, pool(genre.as_deref())))
            .or_default()
            .push((row.get(0).map_err(decode)?, score));
    }
    Ok(pools)
}

/// Sets `year_rank` on every album that has an Album Score and an original year:
/// one more than the fully rated albums of its year and pool that score higher.
pub(crate) fn attach(connection: &Connection, albums: &mut [AlbumSummary]) -> Result<(), String> {
    let years = albums
        .iter()
        .filter(|album| album.album_score.is_some())
        .filter_map(rank_year)
        .collect::<BTreeSet<_>>();
    let pools = scored_albums(connection, &years)?;
    for album in albums {
        album.year_rank = match (album.album_score, rank_year(album)) {
            (Some(score), Some(year)) => {
                let higher = pools
                    .get(&(year, pool(album.genre.as_deref())))
                    .map(|entries| {
                        entries
                            .iter()
                            .filter(|(id, other)| *id != album.id && *other > score + 1e-9)
                            .count()
                    })
                    .unwrap_or(0);
                Some(higher as u32 + 1)
            }
            _ => None,
        };
    }
    Ok(())
}

/// Ranks for the given albums, read through this connection's live state.
pub(crate) fn ranks_for_ids(
    connection: &Connection,
    album_ids: &[String],
) -> Result<HashMap<String, Option<u32>>, String> {
    if album_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let live = live_sql(connection);
    let mut albums = Vec::with_capacity(album_ids.len());
    for chunk in album_ids.chunks(500) {
        let placeholders = vec!["?"; chunk.len()].join(", ");
        let sql = format!(
            "SELECT a.id, a.year, {genre}, {score} FROM albums AS a{joins} WHERE a.id IN ({placeholders})",
            genre = crate::live_genres::album_sql("a"),
            score = live.score,
            joins = live.joins,
        );
        let mut statement = connection
            .prepare(&sql)
            .map_err(|error| format!("Could not prepare the album ranks: {error}"))?;
        let rows = statement
            .query_map(params_from_iter(chunk.iter()), |row| {
                Ok(AlbumSummary {
                    id: row.get(0)?,
                    original_year: row.get(1)?,
                    genre: row.get(2)?,
                    album_score: row.get(3)?,
                    ..AlbumSummary::default()
                })
            })
            .map_err(|error| format!("Could not read the album ranks: {error}"))?;
        for row in rows {
            albums.push(row.map_err(|error| format!("Could not decode the album ranks: {error}"))?);
        }
    }
    attach(connection, &mut albums)?;
    let mut ranks = album_ids
        .iter()
        .map(|id| (id.clone(), None))
        .collect::<HashMap<_, _>>();
    for album in albums {
        ranks.insert(album.id, album.year_rank);
    }
    Ok(ranks)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE albums (id TEXT PRIMARY KEY, year INTEGER, canonical_genre TEXT,
                                      album_score REAL, total_tracks INTEGER, rated_tracks INTEGER);
                 INSERT INTO albums VALUES
                   ('pop-a', 2026, 'Synthpop', 8.0, 10, 10),
                   ('pop-b', 2026, 'Rock', 9.5, 10, 10),
                   ('pop-c', 2026, 'Rock', 9.5, 10, 10),
                   ('pop-partial', 2026, 'Rock', 400.0, 10, 1),
                   ('pop-old', 2025, 'Rock', 50.0, 10, 10),
                   ('score-a', 2026, 'TV', 3.0, 10, 10),
                   ('score-b', 2026, 'Video Game', 600.0, 10, 10),
                   ('ost-a', 2026, 'Soundtrack', 1.0, 10, 10),
                   ('unrated', 2026, 'Rock', NULL, 10, 0);",
            )
            .unwrap();
        crate::live_genres::create_empty_overlay(&connection).unwrap();
        connection
    }

    fn ids(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn ranks_within_the_year_and_genre_pool() {
        let connection = catalog();
        let ranks = ranks_for_ids(
            &connection,
            &ids(&[
                "pop-a",
                "pop-b",
                "pop-c",
                "pop-partial",
                "pop-old",
                "score-a",
                "score-b",
                "ost-a",
                "unrated",
            ]),
        )
        .unwrap();
        // Scores and soundtracks never outrank the general pool, and the partly rated
        // album is placed among the finished ones without pushing them down.
        assert_eq!(ranks["pop-partial"], Some(1));
        assert_eq!(ranks["pop-b"], Some(1));
        assert_eq!(ranks["pop-c"], Some(1));
        assert_eq!(ranks["pop-a"], Some(3));
        assert_eq!(ranks["pop-old"], Some(1));
        assert_eq!(ranks["score-b"], Some(1));
        assert_eq!(ranks["score-a"], Some(2));
        assert_eq!(ranks["ost-a"], Some(1));
        assert_eq!(ranks["unrated"], None);
    }

    #[test]
    fn live_scores_change_the_rank() {
        let connection = catalog();
        connection
            .execute_batch(
                "CREATE TEMP TABLE aurora_live_album_metrics (album_id TEXT, album_score REAL);
                 INSERT INTO temp.aurora_live_album_metrics VALUES ('pop-a', 10.0);",
            )
            .unwrap();
        let ranks = ranks_for_ids(&connection, &ids(&["pop-a", "pop-b"])).unwrap();
        assert_eq!(ranks["pop-a"], Some(1));
        assert_eq!(ranks["pop-b"], Some(2));
    }

    #[test]
    fn finishing_an_album_lets_it_outrank_others() {
        let connection = catalog();
        let before = ranks_for_ids(&connection, &ids(&["pop-b"])).unwrap();
        assert_eq!(before["pop-b"], Some(1));
        connection
            .execute_batch(
                "CREATE TEMP TABLE aurora_live_album_rating_state (album_id TEXT, total_tracks INTEGER, rated_tracks INTEGER);
                 INSERT INTO temp.aurora_live_album_rating_state VALUES ('pop-partial', 10, 10);",
            )
            .unwrap();
        let after = ranks_for_ids(&connection, &ids(&["pop-b", "pop-partial"])).unwrap();
        assert_eq!(after["pop-partial"], Some(1));
        assert_eq!(after["pop-b"], Some(2));
    }

    #[test]
    fn umbrella_genres_share_one_pool() {
        assert_eq!(pool(Some("Drama")), RankPool::Scores);
        assert_eq!(pool(Some(" video game ")), RankPool::Scores);
        assert_eq!(pool(Some("Soundtrack")), RankPool::Soundtrack);
        assert_eq!(pool(Some("Synthwave")), RankPool::General);
        assert_eq!(pool(None), RankPool::General);
    }
}
