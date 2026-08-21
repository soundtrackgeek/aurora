mod artwork;
mod catalog;
mod playback;
mod state_store;

use catalog::{LibrarySnapshot, TrackSummary};
use playback::{PlaybackRuntime, PlaybackSnapshot, RepeatMode};
use std::sync::Mutex;
use tauri::{Manager, State};

type PlaybackState = Mutex<PlaybackRuntime>;

fn with_playback<T>(
    state: State<'_, PlaybackState>,
    operation: impl FnOnce(&mut PlaybackRuntime) -> Result<T, String>,
) -> Result<T, String> {
    let mut runtime = state
        .lock()
        .map_err(|_| "Aurora's playback engine stopped unexpectedly.".to_owned())?;
    operation(&mut runtime)
}

#[tauri::command]
async fn library_snapshot() -> Result<LibrarySnapshot, String> {
    tauri::async_runtime::spawn_blocking(catalog::load_default_snapshot)
        .await
        .map_err(|error| format!("The catalog worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn artist_tracks(artist: String) -> Result<Vec<TrackSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || catalog::load_artist_tracks(artist))
        .await
        .map_err(|error| format!("The artist worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn search_tracks(query: String) -> Result<Vec<TrackSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || catalog::load_search_tracks(query))
        .await
        .map_err(|error| format!("The search worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
fn playback_state(state: State<'_, PlaybackState>) -> Result<PlaybackSnapshot, String> {
    with_playback(state, |runtime| Ok(runtime.snapshot()))
}

#[tauri::command]
fn playback_replace_queue(
    state: State<'_, PlaybackState>,
    track_ids: Vec<String>,
    start_track_id: String,
) -> Result<PlaybackSnapshot, String> {
    with_playback(state, |runtime| {
        runtime.replace_queue(track_ids, start_track_id)
    })
}

#[tauri::command]
fn playback_toggle(state: State<'_, PlaybackState>) -> Result<PlaybackSnapshot, String> {
    with_playback(state, PlaybackRuntime::toggle)
}

#[tauri::command]
fn playback_next(state: State<'_, PlaybackState>) -> Result<PlaybackSnapshot, String> {
    with_playback(state, PlaybackRuntime::next)
}

#[tauri::command]
fn playback_previous(state: State<'_, PlaybackState>) -> Result<PlaybackSnapshot, String> {
    with_playback(state, PlaybackRuntime::previous)
}

#[tauri::command]
fn playback_seek(
    state: State<'_, PlaybackState>,
    position_seconds: f64,
) -> Result<PlaybackSnapshot, String> {
    with_playback(state, |runtime| runtime.seek(position_seconds))
}

#[tauri::command]
fn playback_set_volume(
    state: State<'_, PlaybackState>,
    volume: f32,
) -> Result<PlaybackSnapshot, String> {
    with_playback(state, |runtime| runtime.set_volume(volume))
}

#[tauri::command]
fn playback_set_shuffle(
    state: State<'_, PlaybackState>,
    enabled: bool,
) -> Result<PlaybackSnapshot, String> {
    with_playback(state, |runtime| runtime.set_shuffle(enabled))
}

#[tauri::command]
fn playback_set_repeat_mode(
    state: State<'_, PlaybackState>,
    repeat_mode: RepeatMode,
) -> Result<PlaybackSnapshot, String> {
    with_playback(state, |runtime| runtime.set_repeat_mode(repeat_mode))
}

#[tauri::command]
fn playback_remove_queue_item(
    state: State<'_, PlaybackState>,
    index: usize,
) -> Result<PlaybackSnapshot, String> {
    with_playback(state, |runtime| runtime.remove_queue_item(index))
}

#[tauri::command]
fn playback_move_queue_item(
    state: State<'_, PlaybackState>,
    from: usize,
    to: usize,
) -> Result<PlaybackSnapshot, String> {
    with_playback(state, |runtime| runtime.move_queue_item(from, to))
}

#[tauri::command]
fn playback_clear_queue(state: State<'_, PlaybackState>) -> Result<PlaybackSnapshot, String> {
    with_playback(state, PlaybackRuntime::clear_queue)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .register_uri_scheme_protocol("aurora-cover", |context, request| {
            artwork::handle_cover_request(context.app_handle(), &request)
        })
        .setup(|app| {
            let state_directory = app.path().app_data_dir()?;
            let runtime = PlaybackRuntime::new(state_directory.join("aurora-state.sqlite3"))
                .map_err(std::io::Error::other)?;
            app.manage(Mutex::new(runtime));
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                let state = window.state::<PlaybackState>();
                if let Ok(mut runtime) = state.lock() {
                    let _ = runtime.persist_for_shutdown();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            library_snapshot,
            artist_tracks,
            search_tracks,
            playback_state,
            playback_replace_queue,
            playback_toggle,
            playback_next,
            playback_previous,
            playback_seek,
            playback_set_volume,
            playback_set_shuffle,
            playback_set_repeat_mode,
            playback_remove_queue_item,
            playback_move_queue_item,
            playback_clear_queue,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Aurora");
}
