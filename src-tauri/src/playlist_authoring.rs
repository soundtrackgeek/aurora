use crate::catalog::{self, TrackReference};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

const MAX_IMPORT_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthorRequest {
    action: String,
    id: Option<i64>,
    expected_updated_at: Option<String>,
    name: Option<String>,
    from: Option<usize>,
    to: Option<usize>,
    #[serde(default)]
    tracks: Vec<TrackReference>,
    #[serde(default)]
    album_ids: Vec<String>,
}

#[tauri::command]
pub(crate) async fn author_music_library_playlist(
    app: AppHandle,
    input: AuthorRequest,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if input.tracks.len()>1000 || input.album_ids.len()>100 { return Err("Add at most 1000 songs or 100 albums.".into()) }
        let tracks = input.tracks.iter().map(|reference| {
            let t = catalog::load_catalog_track_by_id(&reference.id,&reference.track_key)?;
            Ok(serde_json::json!({"id":t.id.parse::<i64>().map_err(|e|e.to_string())?,"filePath":t.directory,"filename":t.filename}))
        }).collect::<Result<Vec<_>,String>>()?;
        crate::library_bridge::playlist_operation(&app,"playlistAuthor",serde_json::json!({"action":input.action,"id":input.id,"expectedUpdatedAt":input.expected_updated_at,"name":input.name,"from":input.from,"to":input.to,"tracks":tracks,"albumIds":input.album_ids}))
    }).await.map_err(|e|e.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportPreview {
    path: String,
    fingerprint: String,
    name: String,
    track_count: usize,
    samples: Vec<String>,
}

fn read_import(path: &Path) -> Result<(String, String), String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_IMPORT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_IMPORT_BYTES {
        return Err("M3U8 files must be no larger than 2 MiB.".into());
    }
    let fingerprint = format!("{:x}", Sha256::digest(&bytes));
    Ok((
        String::from_utf8(bytes).map_err(|_| "M3U8 must use UTF-8 text.".to_owned())?,
        fingerprint,
    ))
}

fn m3u_paths(text: &str, base: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let path = if line.starts_with("file:") {
            url::Url::parse(line)
                .map_err(|e| e.to_string())?
                .to_file_path()
                .map_err(|_| "Invalid local file URL.".to_owned())?
        } else {
            if line.contains("://") || line.starts_with("#") {
                return Err("Import supports local music paths, not stream URLs.".into());
            }
            PathBuf::from(line)
        };
        let full = if path.is_absolute() {
            path
        } else {
            base.join(path)
        };
        // Lexical cleanup permits portable relative playlists without touching audio files.
        let mut normalized = PathBuf::new();
        for part in full.components() {
            match part {
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    normalized.pop();
                }
                other => normalized.push(other.as_os_str()),
            }
        }
        paths.push(normalized);
        if paths.len() > 1000 {
            return Err("Import at most 1000 songs at a time.".into());
        }
    }
    if paths.is_empty() {
        return Err("This M3U8 contains no songs.".into());
    }
    Ok(paths)
}

fn resolve_import(conn: &Connection, paths: &[PathBuf]) -> Result<Vec<serde_json::Value>, String> {
    let mut tracks = Vec::new();
    let mut missing = Vec::new();
    for path in paths {
        let directory = path
            .parent()
            .and_then(Path::to_str)
            .ok_or("Invalid music path")?;
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("Invalid music filename")?;
        let matches = conn.prepare("SELECT id,file_path,filename FROM tracks WHERE rtrim(replace(file_path,'/','\\'),'\\') = rtrim(replace(?1,'/','\\'),'\\') COLLATE NOCASE AND filename=?2 COLLATE NOCASE LIMIT 2").map_err(|e|e.to_string())?.query_map(params![directory,filename],|r|Ok(serde_json::json!({"id":r.get::<_,i64>(0)?,"filePath":r.get::<_,String>(1)?,"filename":r.get::<_,String>(2)?}))).map_err(|e|e.to_string())?.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?;
        if matches.len() == 1 {
            tracks.push(matches.into_iter().next().unwrap());
        } else {
            missing.push(path.display().to_string());
        }
    }
    if !missing.is_empty() {
        return Err(format!(
            "{} paths are missing or ambiguous in the catalog. No playlist was saved. First paths: {}",
            missing.len(),
            missing.into_iter().take(5).collect::<Vec<_>>().join("; ")
        ));
    }
    Ok(tracks)
}

#[tauri::command]
pub(crate) async fn preview_playlist_import(
    app: AppHandle,
) -> Result<Option<ImportPreview>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(selected) = app
            .dialog()
            .file()
            .set_title("Import M3U8 playlist")
            .add_filter("UTF-8 playlists", &["m3u8", "m3u"])
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        let path = selected.into_path().map_err(|e| e.to_string())?;
        let (text, fingerprint) = read_import(&path)?;
        let paths = m3u_paths(&text, path.parent().ok_or("Invalid playlist path")?)?;
        let conn = catalog::open_catalog(&catalog::default_catalog_path()?)?;
        resolve_import(&conn, &paths)?;
        Ok(Some(ImportPreview {
            path: path.to_string_lossy().into_owned(),
            fingerprint,
            name: path
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("Imported playlist")
                .chars()
                .take(120)
                .collect(),
            track_count: paths.len(),
            samples: paths
                .iter()
                .take(10)
                .map(|p| {
                    p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect(),
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn save_playlist_import(
    app: AppHandle,
    path: String,
    fingerprint: String,
    name: String,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = Path::new(&path);
        let (text, current) = read_import(path)?;
        if current != fingerprint {
            return Err("The M3U8 changed. Preview it again before saving.".into());
        }
        let paths = m3u_paths(&text, path.parent().ok_or("Invalid playlist path")?)?;
        let conn = catalog::open_catalog(&catalog::default_catalog_path()?)?;
        let tracks = resolve_import(&conn, &paths)?;
        crate::library_bridge::playlist_operation(
            &app,
            "playlistAuthor",
            serde_json::json!({"action":"create","name":name,"tracks":tracks}),
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

fn export_on(conn: &Connection, id: i64, expected: &str) -> Result<String, String> {
    let (json, revision): (String, String) = conn
        .query_row(
            "SELECT playlist_json,updated_at FROM saved_playlists WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or("This playlist no longer exists.")?;
    if expected != revision {
        return Err("This playlist changed. Refresh before exporting.".into());
    }
    let value: serde_json::Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    let tracks = value["tracks"].as_array().ok_or("Invalid playlist songs")?;
    if tracks.len() > 10000 {
        return Err("Export supports at most 10000 songs.".into());
    }
    let mut output = String::from("#EXTM3U\n");
    for t in tracks {
        let dir = t["filePath"]
            .as_str()
            .ok_or("A saved song has no file path.")?;
        let filename = t["filename"]
            .as_str()
            .ok_or("A saved song has no filename.")?;
        let path = Path::new(dir).join(filename).to_string_lossy().into_owned();
        if path.contains(['\r', '\n'])
            || !Path::new(filename)
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
        {
            return Err("A saved song has an invalid path.".into());
        }
        let title = t["title"]
            .as_str()
            .unwrap_or(filename)
            .replace(['\r', '\n'], " ");
        let artist = t["displayArtist"]
            .as_str()
            .or(t["albumArtist"].as_str())
            .unwrap_or("")
            .replace(['\r', '\n'], " ");
        output.push_str(&format!(
            "#EXTINF:{},{} - {}\n{}\n",
            t["seconds"].as_i64().unwrap_or(-1),
            artist,
            title,
            path
        ));
    }
    Ok(output)
}

#[tauri::command]
pub(crate) async fn export_music_library_playlist(
    app: AppHandle,
    id: i64,
    revision: String,
) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let conn = catalog::open_catalog(&catalog::default_catalog_path()?)?;
        let output = export_on(&conn, id, &revision)?;
        let Some(selected) = app
            .dialog()
            .file()
            .set_title("Export M3U8 playlist")
            .set_file_name("playlist.m3u8")
            .add_filter("UTF-8 playlist", &["m3u8"])
            .blocking_save_file()
        else {
            return Ok(false);
        };
        let mut path = selected.into_path().map_err(|e| e.to_string())?;
        if path.extension().is_none() {
            path.set_extension("m3u8");
        }
        if !path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("m3u8"))
        {
            return Err("Choose a .m3u8 export file.".into());
        }
        // Recheck after the user spends time in the Save dialog.
        let output_now = export_on(&conn, id, &revision)?;
        if output_now != output {
            return Err("This playlist changed. Export it again.".into());
        }
        let mut temporary =
            tempfile::NamedTempFile::new_in(path.parent().ok_or("Invalid export destination")?)
                .map_err(|e| e.to_string())?;
        temporary
            .write_all(output.as_bytes())
            .map_err(|e| e.to_string())?;
        temporary.as_file().sync_all().map_err(|e| e.to_string())?;
        temporary.persist(path).map_err(|e| e.to_string())?;
        Ok(true)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
#[path = "playlist_authoring_tests.rs"]
mod tests;
