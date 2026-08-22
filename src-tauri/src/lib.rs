mod artwork;
mod audio_settings;
mod catalog;
mod charts;
mod curation;
mod curation_store;
mod device_mode;
mod explorer;
mod genres;
mod history;
mod laptop_mode;
mod musicbrainz;
mod playback;
mod ratings;
mod replay_gain;
mod shortcuts;
mod state_store;
mod state_sync;
mod tag_model;
mod tagging;
mod waveform;
mod years;

use audio_settings::{AudioSettingsRequest, AudioSettingsStatus, AudioSettingsStore};
use catalog::{LibrarySnapshot, TrackReference, TrackSummary};
use charts::{ChartItemDetail, ChartItemDetailRequest, ChartPage, ChartPageRequest};
use curation::{ArtistDecisionRequest, CurationExportResult, ReleaseDecisionRequest};
use explorer::{
    AlbumDetail, AlbumPage, AlbumPageRequest, ArtistDetail, ArtistPage, ArtistPageRequest,
    TrackPage, TrackPageRequest,
};
use genres::{GenreDetail, GenreQueueRequest, GenreSummary};
use history::{HistoryPage, HistoryPageRequest, HistoryStore, TrackHistoryInsight};
use laptop_mode::{LaptopModeRuntime, LaptopModeStatus};
use musicbrainz::{ArtistIntelligence, ArtistReviewPage, ArtistReviewPageRequest};
use playback::{PlaybackRuntime, PlaybackSnapshot, RepeatMode};
use ratings::{
    CompletionKind, RatingAlbumPage, RatingAlbumQueueRequest, RatingCollectionRequest,
    RatingsOverview,
};
use state_store::StateStore;
use std::sync::Mutex;
use tag_model::TagEditRequest;
use tagging::{TagReconciliationReport, TagService, TrackTagSnapshot};
use tauri::{AppHandle, Manager, State};
use waveform::{FileSignature, WaveformSnapshot, WaveformStore};
use years::{YearDetail, YearOverview, YearQueueRequest, YearSelection};

type PlaybackState = Mutex<PlaybackRuntime>;
type TagState = Mutex<TagService>;
type LaptopState = Mutex<LaptopModeRuntime>;
type WaveformState = Mutex<WaveformStore>;
type GlobalShortcutState = Mutex<shortcuts::GlobalShortcutRuntime>;

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
async fn genre_index(app: AppHandle) -> Result<Vec<GenreSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let history = app.state::<HistoryStore>();
        let store = app.state::<StateStore>();
        genres::load_genre_index(&history, &store)
    })
    .await
    .map_err(|error| format!("The genre-atlas worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn genre_detail(app: AppHandle, genre: String) -> Result<GenreDetail, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let history = app.state::<HistoryStore>();
        let store = app.state::<StateStore>();
        genres::load_genre_detail(genre, &history, &store)
    })
    .await
    .map_err(|error| format!("The genre-detail worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn genre_queue_tracks(
    app: AppHandle,
    request: GenreQueueRequest,
) -> Result<Vec<TrackSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let history = app.state::<HistoryStore>();
        let store = app.state::<StateStore>();
        genres::load_genre_queue(request, &history, &store)
    })
    .await
    .map_err(|error| format!("The genre-queue worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn year_overview() -> Result<YearOverview, String> {
    tauri::async_runtime::spawn_blocking(years::load_year_overview)
        .await
        .map_err(|error| format!("The Years overview worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn year_detail(selection: YearSelection) -> Result<YearDetail, String> {
    tauri::async_runtime::spawn_blocking(move || years::load_year_detail(selection))
        .await
        .map_err(|error| format!("The year-detail worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn year_queue_tracks(
    app: AppHandle,
    request: YearQueueRequest,
) -> Result<Vec<TrackSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<StateStore>();
        years::load_year_queue(request, &store)
    })
    .await
    .map_err(|error| format!("The year-playback worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn chart_page(request: ChartPageRequest) -> Result<ChartPage, String> {
    tauri::async_runtime::spawn_blocking(move || charts::load_chart_page(request))
        .await
        .map_err(|error| format!("The chart-page worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn chart_item_detail(request: ChartItemDetailRequest) -> Result<ChartItemDetail, String> {
    tauri::async_runtime::spawn_blocking(move || charts::load_chart_item_detail(request))
        .await
        .map_err(|error| format!("The chart-detail worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn chart_entry_track(app: AppHandle, track_id: String) -> Result<TrackSummary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<StateStore>();
        charts::load_chart_entry_track(track_id, &store)
    })
    .await
    .map_err(|error| format!("The chart-track worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn chart_queue_tracks(
    app: AppHandle,
    request: ChartPageRequest,
) -> Result<Vec<TrackSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<StateStore>();
        charts::load_chart_queue(request, &store)
    })
    .await
    .map_err(|error| format!("The chart-playback worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn ratings_overview(app: AppHandle) -> Result<RatingsOverview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<StateStore>();
        ratings::load_ratings_overview(&store)
    })
    .await
    .map_err(|error| format!("The Ratings overview worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn rating_album_page(
    app: AppHandle,
    kind: CompletionKind,
) -> Result<RatingAlbumPage, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<StateStore>();
        ratings::load_rating_album_page(kind, &store)
    })
    .await
    .map_err(|error| format!("The album-rating worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn rating_collection_tracks(
    app: AppHandle,
    request: RatingCollectionRequest,
) -> Result<Vec<TrackSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<StateStore>();
        ratings::load_rating_collection(request, &store)
    })
    .await
    .map_err(|error| format!("The rating-collection worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn rating_album_queue_tracks(
    app: AppHandle,
    request: RatingAlbumQueueRequest,
) -> Result<Vec<TrackSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<StateStore>();
        ratings::load_rating_album_queue(request, &store)
    })
    .await
    .map_err(|error| format!("The album-rating queue worker stopped unexpectedly: {error}"))?
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
fn playback_append_queue(
    state: State<'_, PlaybackState>,
    track_references: Vec<TrackReference>,
) -> Result<PlaybackSnapshot, String> {
    with_playback(state, |runtime| runtime.append_queue(track_references))
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

#[tauri::command]
async fn track_waveform(
    app: AppHandle,
    track_id: String,
    track_key: String,
) -> Result<WaveformSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resolved = {
            let store = app.state::<StateStore>();
            catalog::resolve_track(&track_id, &track_key, &store)?
        };
        let signature = FileSignature::read(&resolved.audio_path)?;
        if let Some(cached) = {
            let cache = app.state::<WaveformState>();
            let store = cache
                .lock()
                .map_err(|_| "Aurora's waveform cache stopped unexpectedly.".to_owned())?;
            store.load(&resolved.summary.track_key, signature)?
        } {
            return Ok(cached);
        }

        let snapshot = waveform::decode_mp3_waveform(
            &resolved.audio_path,
            &resolved.summary.track_key,
            resolved.summary.duration_seconds,
        )?;
        {
            let cache = app.state::<WaveformState>();
            let store = cache
                .lock()
                .map_err(|_| "Aurora's waveform cache stopped unexpectedly.".to_owned())?;
            store.save(&snapshot, signature)?;
        }
        Ok(snapshot)
    })
    .await
    .map_err(|error| format!("The waveform worker stopped unexpectedly: {error}"))?
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

#[tauri::command]
async fn listening_history_page(
    app: AppHandle,
    request: HistoryPageRequest,
) -> Result<HistoryPage, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let history = app.state::<HistoryStore>();
        let store = app.state::<StateStore>();
        history.page(request, &store)
    })
    .await
    .map_err(|error| format!("The listening-history worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn track_history_insight(
    app: AppHandle,
    track_key: String,
) -> Result<TrackHistoryInsight, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let history = app.state::<HistoryStore>();
        history.track_insight(&track_key)
    })
    .await
    .map_err(|error| format!("The track-history worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
fn set_history_play_threshold(app: AppHandle, play_threshold_seconds: u32) -> Result<u32, String> {
    let history = app.state::<HistoryStore>();
    let value = history.set_play_threshold_seconds(play_threshold_seconds)?;
    let playback = app.state::<PlaybackState>();
    let mut runtime = playback
        .lock()
        .map_err(|_| "Aurora's playback engine stopped unexpectedly.".to_owned())?;
    runtime.set_play_threshold_seconds(value);
    Ok(value)
}

#[tauri::command]
fn global_shortcut_settings(app: AppHandle) -> Result<shortcuts::GlobalShortcutStatus, String> {
    let state = app.state::<GlobalShortcutState>();
    let runtime = state
        .lock()
        .map_err(|_| "Aurora's global shortcut manager stopped unexpectedly.".to_owned())?;
    Ok(runtime.status())
}

#[tauri::command]
fn audio_settings(state: State<'_, PlaybackState>) -> Result<AudioSettingsStatus, String> {
    with_playback(state, |runtime| Ok(runtime.audio_settings_status()))
}

#[tauri::command]
fn update_audio_settings(
    state: State<'_, PlaybackState>,
    request: AudioSettingsRequest,
) -> Result<AudioSettingsStatus, String> {
    with_playback(state, |runtime| runtime.update_audio_settings(request))
}

#[tauri::command]
fn update_global_shortcut_settings(
    app: AppHandle,
    request: shortcuts::GlobalShortcutSettingsRequest,
) -> Result<shortcuts::GlobalShortcutStatus, String> {
    let state = app.state::<GlobalShortcutState>();
    let mut runtime = state
        .lock()
        .map_err(|_| "Aurora's global shortcut manager stopped unexpectedly.".to_owned())?;
    runtime.update(&app, request)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    shortcuts::handle_shortcut(app, shortcut, event.state());
                })
                .build(),
        )
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
            let history_path = state_directory.join("aurora-history.sqlite3");
            let waveform_store =
                WaveformStore::new(state_directory.join("aurora-waveforms.sqlite3"))
                    .map_err(std::io::Error::other)?;
            let mut device_settings =
                device_mode::DeviceModeStore::load(state_directory.join("aurora-device.json"));
            if let Some(device_id) =
                HistoryStore::local_device_id(&history_path).map_err(std::io::Error::other)?
            {
                device_settings
                    .adopt_device_id(&device_id)
                    .map_err(std::io::Error::other)?;
            }
            let history_directory = remote_state_path.parent().ok_or_else(|| {
                std::io::Error::other("Aurora's OneDrive state path has no parent directory.")
            })?;
            let history = HistoryStore::new(
                history_path,
                history_directory.to_path_buf(),
                device_settings.device_id().to_owned(),
                device_settings.device_name().to_owned(),
            )
            .map_err(std::io::Error::other)?;
            let mut laptop_runtime = LaptopModeRuntime::new(
                device_settings,
                store.clone(),
                remote_state_path,
                startup_sync,
            )
            .map_err(std::io::Error::other)?;
            let audio_store = AudioSettingsStore::load(state_directory.join("aurora-audio.json"));
            let runtime = PlaybackRuntime::new(store.clone(), history.clone(), audio_store)
                .map_err(std::io::Error::other)?;
            let tag_service = TagService::new(store.clone()).map_err(std::io::Error::other)?;
            let _ = laptop_runtime.status(true);
            app.manage(store);
            app.manage(history);
            app.manage(Mutex::new(runtime));
            app.manage(Mutex::new(tag_service));
            app.manage(Mutex::new(laptop_runtime));
            app.manage(Mutex::new(waveform_store));
            app.manage(Mutex::new(shortcuts::GlobalShortcutRuntime::load(
                state_directory.join("aurora-shortcuts.json"),
            )));
            if let Ok(mut shortcut_runtime) = app.state::<GlobalShortcutState>().lock() {
                shortcut_runtime.initialize(app.handle());
            }
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
            genre_index,
            genre_detail,
            genre_queue_tracks,
            year_overview,
            year_detail,
            year_queue_tracks,
            chart_page,
            chart_item_detail,
            chart_entry_track,
            chart_queue_tracks,
            ratings_overview,
            rating_album_page,
            rating_collection_tracks,
            rating_album_queue_tracks,
            artist_intelligence,
            musicbrainz_review_page,
            update_artist_identity_decision,
            update_release_group_decision,
            undo_musicbrainz_curation,
            export_musicbrainz_curation,
            playback_state,
            playback_replace_queue,
            playback_append_queue,
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
            track_waveform,
            track_tag_state,
            update_track_tags,
            undo_track_tag_edit,
            refresh_external_tag_changes,
            laptop_mode_status,
            set_laptop_mode,
            listening_history_page,
            track_history_insight,
            set_history_play_threshold,
            global_shortcut_settings,
            update_global_shortcut_settings,
            audio_settings,
            update_audio_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Aurora");
}
