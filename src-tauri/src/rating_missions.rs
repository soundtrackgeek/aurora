//! Music Library owns missions; pending Aurora ratings affect display and every
//! reviewed write/play request before the catalog catches up.
use crate::{hidden_favorites::current, state_store::StateStore};
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};

pub(crate) fn project_mission(mission: &mut Value, store: &StateStore) {
    project_with(mission, |track| current(track, store, false));
}

fn project_with(mission: &mut Value, mut resolve: impl FnMut(&mut Value) -> bool) {
    if matches!(mission["status"].as_str(), Some("completed" | "abandoned")) {
        return;
    }
    let Some(tracks) = mission["tracks"].as_array_mut() else {
        return;
    };
    let mut valid = true;
    let mut rated = 0;
    let mut seconds = 0;
    let mut rating_delta = 0;
    for track in tracks {
        let was_rated = track["rating"].as_i64().unwrap_or(0) > 0;
        if !resolve(track) {
            valid = false;
        }
        if track["rating"].as_i64().unwrap_or(0) > 0 {
            rated += 1;
            if !was_rated {
                rating_delta += 1;
            }
        } else {
            seconds += track["seconds"].as_i64().unwrap_or(0);
            if was_rated {
                rating_delta -= 1;
            }
        }
    }
    mission["completedTracks"] = json!(rated);
    mission["seconds"] = json!(seconds);
    if mission["albumTotal"].as_i64().unwrap_or(0) > 0 {
        mission["albumRated"] = json!(mission["albumRated"].as_i64().unwrap_or(0) + rating_delta);
    }
    if !valid {
        mission["status"] = json!("unavailable");
    }
    if let Some(anchors) = mission["anchors"].as_array_mut() {
        anchors.retain_mut(|t| resolve(t) && (t["rating"] == 100 || t["loved"] == true));
    }
    if mission["status"] == "choice" && mission["anchors"].as_array().is_some_and(|a| a.is_empty())
    {
        mission["status"] = json!("unavailable");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_ratings_update_progress_without_recording_shared_completion() {
        let mut mission = json!({"status":"active","tracks":[{"trackKey":"one","rating":null,"seconds":180},{"trackKey":"two","rating":20,"seconds":200}],"anchors":[],"albumTotal":10,"albumRated":9});
        project_with(&mut mission, |track| {
            track["rating"] = if track["trackKey"] == "one" {
                json!(100)
            } else {
                Value::Null
            };
            true
        });
        assert_eq!(mission["completedTracks"], 1);
        assert_eq!(mission["seconds"], 200);
        assert_eq!(mission["albumRated"], 9);
        assert_eq!(mission["status"], "active");
        project_with(&mut mission, |track| {
            track["rating"] = json!(100);
            true
        });
        assert_eq!(mission["completedTracks"], 2);
        assert_eq!(mission["seconds"], 0);
        assert_eq!(mission["status"], "active");
    }
    #[test]
    fn unavailable_recordings_disable_audition_and_stale_anchors_are_removed() {
        let mut mission = json!({"status":"choice","tracks":[{"trackKey":"missing","rating":null,"seconds":180}],"anchors":[{"trackKey":"old-favorite","rating":100}],"albumTotal":0,"albumRated":0});
        project_with(&mut mission, |t| {
            if t["trackKey"] == "missing" {
                false
            } else {
                t["rating"] = json!(20);
                true
            }
        });
        assert_eq!(mission["status"], "unavailable");
        assert_eq!(mission["anchors"], json!([]));
    }
}

#[tauri::command]
pub async fn rating_missions(app: AppHandle, request: Value) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store=app.state::<StateStore>();
        if matches!(request["action"].as_str(),Some("start"|"save")) {
            let keys=request["trackKeys"].as_array().ok_or("Reload and review the mission first")?;
            let overlays=store.all_overlays()?;
            for key in keys {
                let key=key.as_str().ok_or("Invalid mission track")?;
                if overlays.iter().any(|o|o.track_key==key && (o.values.love_state==crate::tag_model::LoveState::Banned || o.values.rating.is_some_and(|r|r>0.))) {
                    return Err("A mission track was rated or banned here. Reload progress and review the remaining tracks.".into());
                }
            }
        }
        let mut result=crate::library_bridge::rating_missions(&app,request)?;
        if let Some(choices)=result["choices"].as_array_mut() {
            for mission in choices.iter_mut() { project_mission(mission,&store); }
            choices.retain(|m|m["status"]=="choice" && m["tracks"].as_array().is_some_and(|t|t.len() as i64>m["completedTracks"].as_i64().unwrap_or(0)));
        }
        if result["active"].is_object() { project_mission(&mut result["active"],&store); }
        Ok(result)
    }).await.map_err(|e|e.to_string())?
}
