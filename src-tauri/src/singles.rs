//! Official singles from Music Library's shared catalog.
//!
//! Music Library (0.210.0 or newer) fills `track_singles` from MusicBrainz and
//! keys each row like `lastfm_track_popularity`: `lower(trim(album_artist))`
//! and `lower(trim(title))`. Aurora only reads it, shows `confirmed` rows, and
//! treats a missing table (older Music Library) as "no singles known".

use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AlbumSingle {
    pub(crate) track_key: String,
    pub(crate) title: String,
    pub(crate) year: Option<i32>,
}

/// SQLite's `lower(trim(x))`: `lower` folds ASCII only and `trim` removes spaces
/// only, so the key matches rows Music Library derived inside SQLite.
fn sqlite_key(value: &str) -> String {
    value.trim_matches(' ').to_ascii_lowercase()
}

fn singles_from_connection(
    connection: &Connection,
    artist: &str,
    tracks: &[crate::catalog::TrackSummary],
) -> Vec<AlbumSingle> {
    let Ok(mut statement) = connection.prepare(
        "SELECT track_key, single_title, single_year FROM track_singles
         WHERE artist_key = lower(trim(?)) AND confidence = 'confirmed'",
    ) else {
        return Vec::new();
    };
    let Ok(rows) = statement.query_map([artist], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<i32>>(2)?,
        ))
    }) else {
        return Vec::new();
    };
    let known = rows
        .filter_map(Result::ok)
        .map(|(key, title, year)| (key, (title, year)))
        .collect::<HashMap<_, _>>();
    tracks
        .iter()
        .filter_map(|track| {
            let (title, year) = known.get(&sqlite_key(&track.title))?;
            Some(AlbumSingle {
                track_key: track.track_key.clone(),
                title: title.clone(),
                year: *year,
            })
        })
        .collect()
}

pub(crate) fn album_singles(
    artist: &str,
    tracks: &[crate::catalog::TrackSummary],
) -> Vec<AlbumSingle> {
    if tracks.is_empty() {
        return Vec::new();
    }
    let Ok(path) = crate::catalog::default_catalog_path() else {
        return Vec::new();
    };
    let Ok(connection) = crate::catalog::open_catalog(&path) else {
        return Vec::new();
    };
    singles_from_connection(&connection, artist, tracks)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(key: &str, title: &str) -> crate::catalog::TrackSummary {
        crate::catalog::TrackSummary {
            artist_id: None,
            id: key.to_owned(),
            track_key: key.to_owned(),
            album_id: Some("fire-inside".to_owned()),
            title: title.to_owned(),
            artist: "Bob Seger".to_owned(),
            display_artist: None,
            album: "The Fire Inside".to_owned(),
            release_year: Some(1991),
            original_year: Some(1991),
            publisher: None,
            rating: None,
            loved: false,
            love_state: crate::tag_model::LoveState::Neutral,
            tag_sync_state: None,
            can_undo_tag_edit: false,
            duration_seconds: Some(220),
            genre: None,
            play_count: None,
            track_number: None,
            track_total: None,
            disc_number: None,
            disc_total: None,
            directory: String::new(),
            filename: String::new(),
            catalog_import_run_id: 1,
            registered_plays: None,
        }
    }

    fn catalog() -> Connection {
        let connection = Connection::open_in_memory().expect("memory database");
        connection
            .execute_batch(
                "CREATE TABLE track_singles (
                    artist_key TEXT NOT NULL, track_key TEXT NOT NULL, single_title TEXT NOT NULL,
                    single_year INTEGER, confidence TEXT NOT NULL);
                 INSERT INTO track_singles VALUES
                    ('bob seger', 'take a chance', 'Take a Chance', 1991, 'confirmed'),
                    ('bob seger', 'the real love', 'The Real Love', 1991, 'possible');",
            )
            .expect("schema");
        connection
    }

    #[test]
    fn only_confirmed_singles_reach_the_album() {
        let tracks = [
            track("a", "  Take A Chance"),
            track("b", "The Real Love"),
            track("c", "Sightseeing"),
        ];
        assert_eq!(
            singles_from_connection(&catalog(), " Bob Seger ", &tracks),
            vec![AlbumSingle {
                track_key: "a".into(),
                title: "Take a Chance".into(),
                year: Some(1991)
            }]
        );
    }

    #[test]
    fn an_older_catalog_without_the_table_means_no_singles() {
        let connection = Connection::open_in_memory().expect("memory database");
        assert!(
            singles_from_connection(&connection, "Bob Seger", &[track("a", "Take A Chance")])
                .is_empty()
        );
    }

    #[test]
    fn another_artists_single_never_matches() {
        assert!(
            singles_from_connection(&catalog(), "Someone Else", &[track("a", "Take A Chance")])
                .is_empty()
        );
    }
}
