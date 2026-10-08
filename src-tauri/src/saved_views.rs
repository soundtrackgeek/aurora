use crate::state_store::StateStore;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SavedView {
    id: Option<i64>,
    name: String,
    view: String,
    filters: serde_json::Value,
}

fn list(store: &StateStore) -> Result<Vec<SavedView>, String> {
    let c = store.open()?;
    let mut stmt = c
        .prepare(
            "SELECT id,name,view,filters_json FROM saved_views ORDER BY name COLLATE NOCASE,id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    rows.into_iter()
        .map(|(id, name, view, json)| {
            Ok(SavedView {
                id: Some(id),
                name,
                view,
                filters: serde_json::from_str(&json).map_err(|e| e.to_string())?,
            })
        })
        .collect()
}

fn save(store: &StateStore, mut input: SavedView) -> Result<SavedView, String> {
    input.name = input.name.trim().to_owned();
    let json = serde_json::to_string(&input.filters).map_err(|e| e.to_string())?;
    if input.name.is_empty()
        || input.name.chars().count() > 120
        || !matches!(input.view.as_str(), "tracks" | "albums" | "artists")
        || !input.filters.is_object()
        || json.len() > 4096
    {
        return Err("Name the view with 1–120 characters and use valid explorer filters.".into());
    }
    let c = store.open()?;
    if let Some(id) = input.id {
        if c.execute(
            "UPDATE saved_views SET name=?1,view=?2,filters_json=?3 WHERE id=?4",
            params![input.name, input.view, json, id],
        )
        .map_err(|e| e.to_string())?
            == 0
        {
            return Err("This saved view no longer exists.".into());
        }
    } else {
        let tx = c.unchecked_transaction().map_err(|e| e.to_string())?;
        let count: i64 = tx
            .query_row("SELECT COUNT(*) FROM saved_views", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if count >= 100 {
            return Err("Keep at most 100 saved views.".into());
        }
        tx.execute(
            "INSERT INTO saved_views(name,view,filters_json) VALUES (?1,?2,?3)",
            params![input.name, input.view, json],
        )
        .map_err(|e| e.to_string())?;
        input.id = Some(tx.last_insert_rowid());
        tx.commit().map_err(|e| e.to_string())?;
    }
    Ok(input)
}

#[tauri::command]
pub(crate) async fn list_saved_views(app: AppHandle) -> Result<Vec<SavedView>, String> {
    tauri::async_runtime::spawn_blocking(move || list(&app.state::<StateStore>()))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub(crate) async fn save_explorer_view(
    app: AppHandle,
    input: SavedView,
) -> Result<SavedView, String> {
    tauri::async_runtime::spawn_blocking(move || save(&app.state::<StateStore>(), input))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub(crate) async fn delete_saved_view(app: AppHandle, id: i64) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<StateStore>()
            .open()?
            .execute("DELETE FROM saved_views WHERE id=?1", [id])
            .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_views_are_durable_and_participate_in_state_sync() {
        let dir = tempfile::tempdir().unwrap();
        let store = StateStore::new(dir.path().join("state.sqlite3")).unwrap();
        let revision = || {
            store
                .open()
                .unwrap()
                .query_row("SELECT content_revision FROM state_sync_meta", [], |r| {
                    r.get::<_, i64>(0)
                })
                .unwrap()
        };
        let before = revision();
        let mut v = save(
            &store,
            SavedView {
                id: None,
                name: " Synth ".into(),
                view: "albums".into(),
                filters: serde_json::json!({"query":"genre:synthwave","sort":"yearDesc"}),
            },
        )
        .unwrap();
        assert!(revision() > before);
        assert_eq!(list(&store).unwrap()[0].name, "Synth");
        v.name = "1980s".into();
        let before = revision();
        save(&store, v.clone()).unwrap();
        assert!(revision() > before);
        let reopened = StateStore::new(store.path().to_owned()).unwrap();
        assert_eq!(list(&reopened).unwrap()[0].filters, v.filters);
        assert_eq!(list(&reopened).unwrap()[0].name, "1980s");
        let before = revision();
        store
            .open()
            .unwrap()
            .execute("DELETE FROM saved_views WHERE id=?1", [v.id.unwrap()])
            .unwrap();
        assert!(revision() > before);
        assert!(list(&reopened).unwrap().is_empty());
    }
}
