mod artwork;
mod catalog;
mod curation;
mod curation_store;
mod device_mode;
mod explorer;
mod laptop_mode;
mod musicbrainz;
mod playback;
mod state_store;
mod state_sync;
mod tag_model;
mod tagging;

use catalog::{LibrarySnapshot, TrackReference, TrackSummary};
use curation::{ArtistDecisionRequest, CurationExportResult, ReleaseDecisionRequest};
use explorer::{
    AlbumDetail, AlbumPage, AlbumPageRequest, ArtistDetail, ArtistPage, ArtistPageRequest,
    TrackPage, TrackPageRequest,
};
use laptop_mode::{LaptopModeRuntime, LaptopModeStatus};
use musicbrainz::{ArtistIntelligence, ArtistReviewPage, ArtistReviewPageRequest};
use playback::{PlaybackRuntime, PlaybackSnapshot, RepeatMode};
use state_store::StateStore;
use std::sync::Mutex;
use tag_model::TagEditRequest;
use tagging::{TagReconciliationReport, TagService, TrackTagSnapshot};
use tauri::{AppHandle, Manager, State};

type PlaybackState = Mutex<PlaybackRuntime>;
type TagState = Mutex<TagService>;
type LaptopState = Mutex<LaptopModeRuntime>;

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
async fn library_snapshot(app: AppHandle) -> Result<LibrarySnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<StateStore>();
        catalog::load_default_snapshot(&store)
    })
    .await
    .map_err(|error| format!("The catalog worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn artist_tracks(app: AppHandle, artist: String) -> Result<Vec<TrackSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<StateStore>();
        catalog::load_artist_tracks(artist, &store)
    })
    .await
    .map_err(|error| format!("The artist worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn search_tracks(app: AppHandle, query: String) -> Result<Vec<TrackSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<StateStore>();
        catalog::load_search_tracks(query, &store)
    })
    .await
    .map_err(|error| format!("The search worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn explore_tracks(app: AppHandle, request: TrackPageRequest) -> Result<TrackPage, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<StateStore>();
        explorer::load_track_page(request, &store)
    })
    .await
    .map_err(|error| format!("The track explorer stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn explore_albums(request: AlbumPageRequest) -> Result<AlbumPage, String> {
    tauri::async_runtime::spawn_blocking(move || explorer::load_album_page(request))
        .await
        .map_err(|error| format!("The album explorer stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn explore_artists(request: ArtistPageRequest) -> Result<ArtistPage, String> {
    tauri::async_runtime::spawn_blocking(move || explorer::load_artist_page(request))
        .await
        .map_err(|error| format!("The artist explorer stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn album_detail(app: AppHandle, album_id: String) -> Result<AlbumDetail, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<StateStore>();
        explorer::load_album_detail(album_id, &store)
    })
    .await
    .map_err(|error| format!("The album detail worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn artist_detail(artist: String) -> Result<ArtistDetail, String> {
    tauri::async_runtime::spawn_blocking(move || explorer::load_artist_detail(artist))
        .await
        .map_err(|error| format!("The artist detail worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn artist_intelligence(app: AppHandle, artist: String) -> Result<ArtistIntelligence, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<StateStore>();
        musicbrainz::load_artist_intelligence_with_store(artist, &store)
    })
    .await
    .map_err(|error| format!("The MusicBrainz worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn musicbrainz_review_page(
    app: AppHandle,
    request: ArtistReviewPageRequest,
) -> Result<ArtistReviewPage, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<StateStore>();
        musicbrainz::load_artist_review_page(request, &store)
    })
    .await
    .map_err(|error| format!("The MusicBrainz review worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn update_artist_identity_decision(
    app: AppHandle,
    request: ArtistDecisionRequest,
) -> Result<ArtistIntelligence, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<StateStore>();
        curation::update_artist_decision(&store, request)
    })
    .await
    .map_err(|error| format!("The artist curation worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn update_release_group_decision(
    app: AppHandle,
    request: ReleaseDecisionRequest,
) -> Result<ArtistIntelligence, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<StateStore>();
        curation::update_release_decision(&store, request)
    })
    .await
    .map_err(|error| format!("The release curation worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn undo_musicbrainz_curation(app: AppHandle) -> Result<Option<ArtistIntelligence>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<StateStore>();
        curation::undo_latest(&store)
    })
    .await
    .map_err(|error| format!("The curation undo worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn export_musicbrainz_curation(app: AppHandle) -> Result<CurationExportResult, String> {
    let app_for_worker = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let store = app_for_worker.state::<StateStore>();
        curation::export_overlay_snapshot(&app_for_worker, &store)
    })
    .await
    .map_err(|error| format!("The curation export worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
fn playback_state(state: State<'_, PlaybackState>) -> Result<PlaybackSnapshot, String> {
    with_playback(state, |runtime| Ok(runtime.snapshot()))
}

#[tauri::command]
fn playback_replace_queue(
    state: State<'_, PlaybackState>,
    track_references: Vec<TrackReference>,
    start_track_key: String,
) -> Result<PlaybackSnapshot, String> {
    with_playback(state, |runtime| {
        runtime.replace_queue(track_references, start_track_key)
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

fn refresh_playback_track(app: &AppHandle, track: &TrackSummary) {
    let playback = app.state::<PlaybackState>();
    if let Ok(mut runtime) = playback.lock() {
        runtime.refresh_track_metadata(track);
    }
}

#[tauri::command]
async fn track_tag_state(
    app: AppHandle,
    track_id: String,
    track_key: String,
) -> Result<TrackTagSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = {
            let state = app.state::<TagState>();
            let service = state
                .lock()
                .map_err(|_| "Aurora's tag writer stopped unexpectedly.".to_owned())?;
            service.inspect(&track_id, &track_key)?
        };
        refresh_playback_track(&app, &result.track);
        Ok(result)
    })
    .await
    .map_err(|error| format!("The tag reader stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn update_track_tags(
    app: AppHandle,
    request: TagEditRequest,
) -> Result<TrackTagSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = {
            let state = app.state::<TagState>();
            let service = state
                .lock()
                .map_err(|_| "Aurora's tag writer stopped unexpectedly.".to_owned())?;
            service.update(request)?
        };
        refresh_playback_track(&app, &result.track);
        Ok(result)
    })
    .await
    .map_err(|error| format!("The tag writer stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn undo_track_tag_edit(
    app: AppHandle,
    track_id: String,
    track_key: String,
) -> Result<TrackTagSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = {
            let state = app.state::<TagState>();
            let service = state
                .lock()
                .map_err(|_| "Aurora's tag writer stopped unexpectedly.".to_owned())?;
            service.undo(&track_id, &track_key)?
        };
        refresh_playback_track(&app, &result.track);
        Ok(result)
    })
    .await
    .map_err(|error| format!("The tag undo worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn refresh_external_tag_changes(app: AppHandle) -> Result<TagReconciliationReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<TagState>();
        let service = state
            .lock()
            .map_err(|_| "Aurora's tag reader stopped unexpectedly.".to_owned())?;
        service.reconcile_pending_overlays(100)
    })
    .await
    .map_err(|error| format!("The external-tag refresh stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn laptop_mode_status(app: AppHandle) -> Result<LaptopModeStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<LaptopState>();
        let mut runtime = state
            .lock()
            .map_err(|_| "Aurora's Laptop Mode monitor stopped unexpectedly.".to_owned())?;
        Ok(runtime.status(false))
    })
    .await
    .map_err(|error| format!("The Laptop Mode monitor stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn set_laptop_mode(app: AppHandle, enabled: bool) -> Result<LaptopModeStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<LaptopState>();
        let mut runtime = state
            .lock()
            .map_err(|_| "Aurora's Laptop Mode setting stopped unexpectedly.".to_owned())?;
        runtime.set_enabled(enabled)
    })
    .await
    .map_err(|error| format!("The Laptop Mode setting stopped unexpectedly: {error}"))?
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
            let state_path = state_directory.join("aurora-state.sqlite3");
            let remote_state_path =
                state_sync::default_remote_state_path().map_err(std::io::Error::other)?;
            let startup_sync =
                state_sync::prepare_state_before_open(&state_path, &remote_state_path);
            let store = StateStore::new(state_path).map_err(std::io::Error::other)?;
            let mut laptop_runtime = LaptopModeRuntime::new(
                &state_directory,
                store.clone(),
                remote_state_path,
                startup_sync,
            )
            .map_err(std::io::Error::other)?;
            let runtime = PlaybackRuntime::new(store.clone()).map_err(std::io::Error::other)?;
            let tag_service = TagService::new(store.clone()).map_err(std::io::Error::other)?;
            let _ = laptop_runtime.status(true);
            app.manage(store);
            app.manage(Mutex::new(runtime));
            app.manage(Mutex::new(tag_service));
            app.manage(Mutex::new(laptop_runtime));
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                let state = window.state::<PlaybackState>();
                if let Ok(mut runtime) = state.lock() {
                    let _ = runtime.persist_for_shutdown();
                }
                let laptop = window.state::<LaptopState>();
                if let Ok(mut runtime) = laptop.lock() {
                    let _ = runtime.status(true);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            library_snapshot,
            artist_tracks,
            search_tracks,
            explore_tracks,
            explore_albums,
            explore_artists,
            album_detail,
            artist_detail,
            artist_intelligence,
            musicbrainz_review_page,
            update_artist_identity_decision,
            update_release_group_decision,
            undo_musicbrainz_curation,
            export_musicbrainz_curation,
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
            track_tag_state,
            update_track_tags,
            undo_track_tag_edit,
            refresh_external_tag_changes,
            laptop_mode_status,
            set_laptop_mode,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Aurora");
}
