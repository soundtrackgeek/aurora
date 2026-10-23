//! The atlas and its snapshots belong to Music Library. Pending local tags are
//! projected in one pass without opening the catalog for every plotted track.
use crate::{state_store::StateStore, tag_model::LoveState};
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};

fn project(result: &mut Value, mut resolve: impl FnMut(&mut Value) -> bool) {
    let mut total = [0usize; 5];
    if let Some(regions) = result["regions"].as_array_mut() {
        for region in regions {
            let mut counts = [0usize; 5];
            if let Some(points) = region["points"].as_array_mut() {
                for point in points {
                    let available = point["available"] == true && resolve(&mut point["track"]);
                    point["available"] = json!(available);
                    if !available {
                        counts[4] += 1;
                        continue;
                    }
                    counts[0] += 1;
                    counts[1] += usize::from(point["track"]["rating"].as_i64().unwrap_or(0) > 0);
                    counts[2] += usize::from(point["track"]["loved"] == true);
                    counts[3] += usize::from(point["inHistory"] != true);
                }
            }
            region["counts"] = count_value(counts);
            for i in 0..5 {
                total[i] += counts[i];
            }
        }
    }
    result["counts"] = count_value(total);
}
fn count_value(c: [usize; 5]) -> Value {
    json!({"owned":c[0],"rated":c[1],"loved":c[2],"absentFromHistory":c[3],"unavailable":c[4]})
}
#[tauri::command]
pub async fn sonic_atlas(app: AppHandle, request: Value) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<StateStore>();
        let overlays = store.all_overlays()?;
        if matches!(request["action"].as_str(), Some("save" | "audition")) {
            let keys = request["trackKeys"]
                .as_array()
                .ok_or("Review the region first")?;
            if overlays.iter().any(|o| {
                o.values.love_state == LoveState::Banned
                    && keys.iter().any(|k| k.as_str() == Some(&o.track_key))
            }) {
                return Err(
                    "A reviewed track was banned here. Reload the atlas before continuing.".into(),
                );
            }
        }
        let mut result = crate::library_bridge::sonic_atlas(&app, request.clone())?;
        let overlays: std::collections::HashMap<_, _> =
            overlays.iter().map(|o| (o.track_key.as_str(), o)).collect();
        project(&mut result, |track| {
            if let Some(o) = track["trackKey"].as_str().and_then(|k| overlays.get(k)) {
                if o.values.love_state == LoveState::Banned {
                    return false;
                }
                track["rating"] = json!(o.values.rating.map(|r| (r * 20.).round() as i32));
                track["loved"] = json!(o.values.love_state == LoveState::Loved);
            }
            true
        });
        // Resolve selected files immediately before handing them to the player.
        if request["action"] == "audition" {
            let keys = request["trackKeys"]
                .as_array()
                .ok_or("Review the region first")?;
            for region in result["regions"]
                .as_array_mut()
                .ok_or("Invalid atlas response")?
            {
                for point in region["points"]
                    .as_array_mut()
                    .ok_or("Invalid atlas points")?
                {
                    if keys.contains(&point["track"]["trackKey"])
                        && !crate::hidden_favorites::current(&mut point["track"], &store, false)
                    {
                        return Err(
                            "A reviewed track is no longer available. Reload the region.".into(),
                        );
                    }
                }
            }
        }
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_tags_change_counts_without_moving_points_or_regions() {
        let mut a = json!({"regions":[{"id":"r","points":[{"track":{"trackKey":"one","rating":null,"loved":false},"available":true,"inHistory":false,"x":0.2,"y":0.6},{"track":{"trackKey":"two"},"available":true,"inHistory":true,"x":0.8,"y":0.3}]}]});
        project(&mut a, |t| {
            if t["trackKey"] == "two" {
                false
            } else {
                t["rating"] = json!(100);
                t["loved"] = json!(true);
                true
            }
        });
        assert_eq!(a["counts"], count_value([1, 1, 1, 1, 1]));
        assert_eq!(a["regions"][0]["points"][0]["x"], 0.2);
        assert_eq!(a["regions"][0]["id"], "r");
    }
}
