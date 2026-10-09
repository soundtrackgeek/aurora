use crate::{
    catalog::{self, TrackSummary},
    state_store::StateStore,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SavedPlaylistSummary {
    id: i64,
    name: String,
    description: String,
    track_count: i64,
    updated_at: String,
    smart: bool,
    editable: bool,
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
    request: Option<serde_json::Value>,
    smart_settings: Option<serde_json::Value>,
    next_cursor: Option<i64>,
    revision: String,
    positions: Vec<i64>,
    editable: bool,
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

fn smart_expression(connection: &Connection) -> Result<&'static str, String> {
    let has_automation: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='playlist_automations')",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(if has_automation {
        "COALESCE((SELECT smart FROM playlist_automations WHERE saved_playlist_id=saved_playlists.id),0)"
    } else {
        "0"
    })
}

fn list_on(connection: &Connection) -> Result<Vec<SavedPlaylistSummary>, String> {
    if !has_playlists(connection)? {
        return Ok(Vec::new());
    }
    let smart = smart_expression(connection)?;
    let mut statement = connection
        .prepare(&format!(
            "SELECT id, name, COALESCE(json_extract(playlist_json, '$.description'), ''),
                COALESCE(json_array_length(playlist_json, '$.tracks'), 0), updated_at
                , {smart}, json_extract(playlist_json, '$.mixtape') IS NULL FROM saved_playlists ORDER BY updated_at DESC, id DESC"
        ))
        .map_err(|error| format!("Could not prepare Music Library playlists: {error}"))?;
    statement
        .query_map([], |row| {
            Ok(SavedPlaylistSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                track_count: row.get(3)?,
                updated_at: row.get(4)?,
                smart: row.get(5)?,
                editable: row.get::<_, bool>(6)? && !row.get::<_, bool>(5)?,
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
    after: Option<i64>,
    expected_revision: Option<&str>,
    shuffle_seed: Option<i64>,
) -> Result<SavedPlaylistDetail, String> {
    // Metadata, rows and missing count must describe the same catalog snapshot.
    let _snapshot = connection
        .is_autocommit()
        .then(|| connection.unchecked_transaction())
        .transpose()
        .map_err(|e| e.to_string())?;
    if !has_playlists(connection)? {
        return Err("Music Library has no saved playlists yet.".into());
    }
    let (name, description, track_count, revision, request, settings): (String, String, i64, String, Option<String>, Option<String>) = connection
        .query_row(
            "SELECT name, COALESCE(json_extract(playlist_json, '$.description'), ''),
                COALESCE(json_array_length(playlist_json, '$.tracks'), 0), updated_at,
                json_extract(playlist_json, '$.request'), json_extract(playlist_json, '$.smartSettings')
         FROM saved_playlists WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
        )
        .optional()
        .map_err(|error| format!("Could not read Music Library playlist: {error}"))?
        .ok_or_else(|| {
            "This playlist is no longer saved in Music Library. Refresh the list.".to_owned()
        })?;

    if expected_revision.is_some_and(|expected| expected != revision) {
        return Err("This playlist changed in Music Library. Refresh before continuing.".into());
    }
    let order = if shuffle_seed.is_some() {
        "((CAST(item.key AS INTEGER) * ((?3 * 1103515245 + 12345) % 2147483646 + 1) + ?3) % 2147483647)"
    } else {
        "CAST(item.key AS INTEGER) + (0 * ?3)"
    };
    // Match the saved file identity, not the old numeric track ID: Music Library may
    // reimport a track under another ID while the playlist still points to its file.
    let mut statement = connection
        .prepare(&format!(
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
                  t.publisher AS publisher, {order} AS playlist_position
           FROM saved_playlists AS p
           JOIN json_each(p.playlist_json, '$.tracks') AS item
           JOIN tracks AS t
             ON t.file_path = json_extract(item.value, '$.filePath')
            AND t.filename = json_extract(item.value, '$.filename')
           LEFT JOIN lastfm_track_popularity AS l
             ON l.artist_key = lower(trim(t.album_artist_display))
            AND l.track_key = lower(trim(t.title))
           WHERE p.id = ?1 AND {order} > ?2 ORDER BY playlist_position LIMIT 101"#
        ))
        .map_err(|error| format!("Could not prepare playlist tracks: {error}"))?;
    let mut rows = statement
        .query_map(
            params![
                id,
                after.unwrap_or(-1),
                shuffle_seed.unwrap_or(0).rem_euclid(2147483647)
            ],
            |row| {
                Ok((
                    catalog::map_track_row(row)?,
                    row.get::<_, i64>("playlist_position")?,
                ))
            },
        )
        .map_err(|error| format!("Could not read playlist tracks: {error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("Could not decode playlist tracks: {error}"))?;
    let next_cursor = if rows.len() > 100 {
        rows.pop();
        rows.last().map(|r| r.1)
    } else {
        None
    };
    let positions = rows.iter().map(|r| r.1).collect();
    let mut tracks = rows.into_iter().map(|r| r.0).collect::<Vec<_>>();
    catalog::apply_overlays(&mut tracks, store)?;
    let available: i64 = connection.query_row("SELECT COUNT(*) FROM saved_playlists p JOIN json_each(p.playlist_json,'$.tracks') item WHERE p.id=?1 AND EXISTS(SELECT 1 FROM tracks t WHERE t.file_path=json_extract(item.value,'$.filePath') AND t.filename=json_extract(item.value,'$.filename'))", [id], |r| r.get(0)).map_err(|e| e.to_string())?;
    let missing_count = track_count.saturating_sub(available);
    let smart = smart_expression(connection)?;
    let editable = connection.query_row(
        &format!("SELECT json_extract(playlist_json,'$.mixtape') IS NULL AND {smart}=0 FROM saved_playlists WHERE id=?1"),
        [id], |row| row.get(0),
    ).map_err(|e| format!("Could not read playlist editing permissions: {e}"))?;
    Ok(SavedPlaylistDetail {
        id,
        name,
        description,
        track_count,
        missing_count,
        tracks,
        request: request
            .map(|s| serde_json::from_str(&s))
            .transpose()
            .map_err(|e| e.to_string())?,
        smart_settings: settings
            .map(|s| serde_json::from_str(&s))
            .transpose()
            .map_err(|e| e.to_string())?,
        next_cursor,
        revision,
        positions,
        editable,
    })
}

pub(crate) fn detail(
    id: i64,
    store: &StateStore,
    after: Option<i64>,
    expected_revision: Option<&str>,
    shuffle_seed: Option<i64>,
) -> Result<SavedPlaylistDetail, String> {
    let connection = catalog::open_catalog(&catalog::default_catalog_path()?)?;
    detail_on(
        &connection,
        id,
        Some(store),
        after,
        expected_revision,
        shuffle_seed,
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveSelectionRequest {
    name: String,
    #[serde(default)]
    tracks: Vec<catalog::TrackReference>,
    #[serde(default)]
    album_ids: Vec<String>,
}

#[tauri::command]
pub(crate) async fn save_playlist_selection(
    app: AppHandle,
    input: SaveSelectionRequest,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if input.tracks.len() > 1000 || input.album_ids.len() > 100 { return Err("Select at most 1000 songs or 100 albums.".into()); }
        let tracks = input.tracks.iter().map(|reference| {
            let track = catalog::load_catalog_track_by_id(&reference.id, &reference.track_key)?;
            Ok(serde_json::json!({"id": track.id.parse::<i64>().map_err(|e| e.to_string())?, "filePath": track.directory, "filename": track.filename}))
        }).collect::<Result<Vec<_>, String>>()?;
        crate::library_bridge::playlist_operation(&app, "playlistSaveSelection", serde_json::json!({"name": input.name, "tracks": tracks, "albumIds": input.album_ids}))
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn save_smart_playlist(
    app: AppHandle,
    input: serde_json::Value,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::library_bridge::playlist_operation(&app, "playlistSaveSmart", input)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn refresh_smart_playlist(
    app: AppHandle,
    id: i64,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::library_bridge::playlist_operation(
            &app,
            "playlistRefresh",
            serde_json::json!({"id": id}),
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_all_entries_and_seeded_shuffle_without_loss_and_rejects_stale_revision() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("CREATE TABLE saved_playlists (id INTEGER PRIMARY KEY, name TEXT, playlist_json TEXT, updated_at TEXT);
            CREATE TABLE playlist_automations (saved_playlist_id INTEGER PRIMARY KEY, smart INTEGER);
            CREATE TABLE tracks (id INTEGER PRIMARY KEY, title TEXT, album_artist_display TEXT, album TEXT,
              release_year INTEGER, normalized_rating INTEGER, rating_raw TEXT, love TEXT, time_seconds INTEGER,
              canonical_genre TEXT, album_id TEXT, file_path TEXT, filename TEXT, import_run_id INTEGER,
              display_artist TEXT, year INTEGER, publisher TEXT);
            CREATE TABLE lastfm_track_popularity (artist_key TEXT, track_key TEXT, play_count INTEGER);").unwrap();
        let mut items = Vec::new();
        for id in 1..=250 {
            let filename = format!("{id}.mp3");
            connection.execute("INSERT INTO tracks (id,title,file_path,filename,import_run_id) VALUES (?1,?2,'D:\\Music',?3,1)",params![id,format!("Song {id}"),filename]).unwrap();
            items.push(serde_json::json!({"filePath":"D:\\Music","filename":filename}));
        }
        connection.execute("INSERT INTO saved_playlists VALUES (1,'Shared',?1,'v1')",[serde_json::json!({"tracks":items,"request":{"view":"tracks"},"smartSettings":{"trackLimit":250,"refreshPolicy":"manual"}}).to_string()]).unwrap();
        connection
            .execute("INSERT INTO playlist_automations VALUES (1,1)", [])
            .unwrap();
        connection.pragma_update(None, "query_only", true).unwrap();
        assert!(list_on(&connection).unwrap()[0].smart);
        let collect = |seed| {
            let mut after = None;
            let mut ids = Vec::new();
            let mut lengths = Vec::new();
            loop {
                let page = detail_on(&connection, 1, None, after, Some("v1"), seed).unwrap();
                lengths.push(page.tracks.len());
                ids.extend(
                    page.tracks
                        .into_iter()
                        .map(|track| track.id.parse::<i64>().unwrap()),
                );
                after = page.next_cursor;
                if after.is_none() {
                    break;
                }
            }
            assert_eq!(lengths, vec![100, 100, 50]);
            ids
        };
        let ordered = collect(None);
        assert_eq!(ordered, (1..=250).collect::<Vec<_>>());
        let shuffled = collect(Some(42));
        assert_ne!(shuffled, ordered);
        assert_ne!(shuffled, collect(Some(201)));
        assert_eq!(shuffled, collect(Some(42)));
        let mut sorted = shuffled;
        sorted.sort_unstable();
        assert_eq!(sorted, ordered);
        assert!(
            detail_on(&connection, 1, None, Some(99), Some("old"), None)
                .unwrap_err()
                .contains("changed")
        );
        assert!(
            connection
                .pragma_query_value(None, "query_only", |row| row.get::<_, bool>(0))
                .unwrap()
        );
    }

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
        let detail = detail_on(&connection, 1, None, None, None, None).unwrap();
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
