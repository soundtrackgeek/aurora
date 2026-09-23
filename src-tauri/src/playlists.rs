use crate::{
    catalog::{self, TrackSummary},
    state_store::StateStore,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SavedPlaylistSummary {
    id: i64,
    name: String,
    description: String,
    track_count: i64,
    updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SavedPlaylistDetail {
    id: i64,
    name: String,
    description: String,
    track_count: i64,
    missing_count: i64,
    tracks: Vec<TrackSummary>,
}

fn has_playlists(connection: &Connection) -> Result<bool, String> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'saved_playlists'",
            [],
            |_| Ok(()),
        )
        .optional()
        .map(|row| row.is_some())
        .map_err(|error| format!("Could not inspect Music Library playlists: {error}"))
}

fn list_on(connection: &Connection) -> Result<Vec<SavedPlaylistSummary>, String> {
    if !has_playlists(connection)? {
        return Ok(Vec::new());
    }
    let mut statement = connection
        .prepare(
            "SELECT id, name, COALESCE(json_extract(playlist_json, '$.description'), ''),
                COALESCE(json_array_length(playlist_json, '$.tracks'), 0), updated_at
         FROM saved_playlists ORDER BY updated_at DESC, id DESC",
        )
        .map_err(|error| format!("Could not prepare Music Library playlists: {error}"))?;
    statement
        .query_map([], |row| {
            Ok(SavedPlaylistSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                track_count: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })
        .map_err(|error| format!("Could not read Music Library playlists: {error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("Could not decode Music Library playlists: {error}"))
}

pub(crate) fn list() -> Result<Vec<SavedPlaylistSummary>, String> {
    let connection = catalog::open_catalog(&catalog::default_catalog_path()?)?;
    list_on(&connection)
}

fn detail_on(
    connection: &Connection,
    id: i64,
    store: Option<&StateStore>,
) -> Result<SavedPlaylistDetail, String> {
    if !has_playlists(connection)? {
        return Err("Music Library has no saved playlists yet.".into());
    }
    let (name, description, track_count): (String, String, i64) = connection
        .query_row(
            "SELECT name, COALESCE(json_extract(playlist_json, '$.description'), ''),
                COALESCE(json_array_length(playlist_json, '$.tracks'), 0)
         FROM saved_playlists WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|error| format!("Could not read Music Library playlist: {error}"))?
        .ok_or_else(|| {
            "This playlist is no longer saved in Music Library. Refresh the list.".to_owned()
        })?;

    // Match the saved file identity, not the old numeric track ID: Music Library may
    // reimport a track under another ID while the playlist still points to its file.
    let mut statement = connection
        .prepare(
            r#"SELECT t.id, t.title, t.album_artist_display, t.album, t.release_year,
                  COALESCE(t.normalized_rating, CASE trim(t.rating_raw)
                    WHEN '0.5' THEN 10 WHEN '1' THEN 20 WHEN '1.0' THEN 20
                    WHEN '1.5' THEN 30 WHEN '2' THEN 40 WHEN '2.0' THEN 40
                    WHEN '2.5' THEN 50 WHEN '3' THEN 60 WHEN '3.0' THEN 60
                    WHEN '3.5' THEN 70 WHEN '4' THEN 80 WHEN '4.0' THEN 80
                    WHEN '4.5' THEN 90 WHEN '5' THEN 100 WHEN '5.0' THEN 100 END),
                  t.love, t.time_seconds, t.canonical_genre,
                  l.play_count, t.album_id, t.file_path, t.filename, t.import_run_id,
                  t.display_artist AS display_artist, t.year AS original_year,
                  t.publisher AS publisher
           FROM saved_playlists AS p
           JOIN json_each(p.playlist_json, '$.tracks') AS item
           JOIN tracks AS t
             ON t.file_path = json_extract(item.value, '$.filePath')
            AND t.filename = json_extract(item.value, '$.filename')
           LEFT JOIN lastfm_track_popularity AS l
             ON l.artist_key = lower(trim(t.album_artist_display))
            AND l.track_key = lower(trim(t.title))
           WHERE p.id = ?1 ORDER BY CAST(item.key AS INTEGER)"#,
        )
        .map_err(|error| format!("Could not prepare playlist tracks: {error}"))?;
    let mut tracks = statement
        .query_map(params![id], catalog::map_track_row)
        .map_err(|error| format!("Could not read playlist tracks: {error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("Could not decode playlist tracks: {error}"))?;
    catalog::apply_overlays(&mut tracks, store)?;
    let missing_count = track_count.saturating_sub(tracks.len() as i64);
    Ok(SavedPlaylistDetail {
        id,
        name,
        description,
        track_count,
        missing_count,
        tracks,
    })
}

pub(crate) fn detail(id: i64, store: &StateStore) -> Result<SavedPlaylistDetail, String> {
    let connection = catalog::open_catalog(&catalog::default_catalog_path()?)?;
    detail_on(&connection, id, Some(store))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_playlist_order_and_resolves_current_file_identity() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("CREATE TABLE saved_playlists (id INTEGER PRIMARY KEY, name TEXT, playlist_json TEXT, updated_at TEXT);
            CREATE TABLE tracks (id INTEGER PRIMARY KEY, title TEXT, album_artist_display TEXT, album TEXT,
              release_year INTEGER, normalized_rating INTEGER, rating_raw TEXT, love TEXT, time_seconds INTEGER,
              canonical_genre TEXT, album_id TEXT, file_path TEXT, filename TEXT, import_run_id INTEGER,
              display_artist TEXT, year INTEGER, publisher TEXT);
            CREATE TABLE lastfm_track_popularity (artist_key TEXT, track_key TEXT, play_count INTEGER);
            INSERT INTO tracks (id,title,album_artist_display,album,file_path,filename,import_run_id)
              VALUES (77,'Second','Artist','Album','D:\\Music','b.mp3',1),
                     (88,'First','Artist','Album','D:\\Music','a.mp3',1);") .unwrap();
        connection.execute("INSERT INTO saved_playlists VALUES (1,'Ordered',?1,'now')",
            params![r#"{"description":"Saved in Music Library","tracks":[{"trackId":1,"filePath":"D:\\Music","filename":"a.mp3"},{"trackId":2,"filePath":"D:\\Music","filename":"missing.mp3"},{"trackId":3,"filePath":"D:\\Music","filename":"b.mp3"}]}"#]).unwrap();
        let list = list_on(&connection).unwrap();
        assert_eq!(list[0].track_count, 3);
        let detail = detail_on(&connection, 1, None).unwrap();
        assert_eq!(detail.missing_count, 1);
        assert_eq!(
            detail
                .tracks
                .iter()
                .map(|track| track.id.as_str())
                .collect::<Vec<_>>(),
            vec!["88", "77"]
        );
    }
}
