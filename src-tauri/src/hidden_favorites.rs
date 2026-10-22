//! Selection and persistence remain in Music Library. Apply Aurora's pending
//! tag edits before showing or saving tracks returned by the shared shelf.
use crate::{catalog, state_store::StateStore, tag_model::LoveState};
use serde_json::Value;
use tauri::{AppHandle, Manager};

fn current(track: &mut Value, store: &StateStore, suggestion: bool) -> bool {
    let Some(key) = track["trackKey"].as_str() else {
        return false;
    };
    let Some(id) = track["trackId"].as_i64() else {
        return false;
    };
    let Ok(resolved) = catalog::resolve_track(&id.to_string(), key, store) else {
        return false;
    };
    let summary = resolved.summary;
    if summary.love_state == LoveState::Banned
        || (suggestion && summary.rating.is_some_and(|r| r < 4.))
    {
        return false;
    }
    track["rating"] = serde_json::json!(summary.rating.map(|r| (r * 20.).round() as i32));
    track["loved"] = Value::Bool(summary.loved);
    true
}

pub(crate) fn project(shelf: &mut Value, store: &StateStore) {
    if let Some(items) = shelf["suggestions"].as_array_mut() {
        items.retain_mut(|item| {
            if !current(&mut item["track"], store, true) {
                return false;
            }
            let Some(anchors) = item["anchors"].as_array_mut() else {
                return false;
            };
            anchors.retain_mut(|t| {
                current(t, store, false) && (t["rating"] == 100 || t["loved"] == true)
            });
            anchors.len() >= 2
        });
    }
    if let Some(items) = shelf["kept"].as_array_mut() {
        items.retain_mut(|t| current(t, store, false));
    }
}

#[tauri::command]
pub async fn hidden_favorites(app: AppHandle, request: Value) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if request["action"] == "keep" {
            let key = request["trackKey"].as_str().ok_or("Choose a track")?;
            let overlays = app.state::<StateStore>().all_overlays()?;
            if overlays.iter().any(|o| {
                o.track_key == key
                    && (o.values.love_state == LoveState::Banned
                        || o.values.rating.is_some_and(|r| r < 4.))
            }) {
                return Err(
                    "This track's tags changed. Reload Hidden favorites before keeping it.".into(),
                );
            }
        }
        if request["action"] == "save" {
            let keys = request["trackKeys"]
                .as_array()
                .ok_or("Reload the kept shortlist before saving")?;
            let overlays = app.state::<StateStore>().all_overlays()?;
            if overlays.iter().any(|o| {
                o.values.love_state == LoveState::Banned
                    && keys.iter().any(|k| k.as_str() == Some(&o.track_key))
            }) {
                return Err(
                    "A kept track is now banned. Reload the shortlist before saving.".into(),
                );
            }
        }
        let mut result = crate::library_bridge::hidden_favorites(&app, request)?;
        project(&mut result, &app.state::<StateStore>());
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}
