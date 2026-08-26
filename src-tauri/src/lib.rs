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
mod inbox;
mod laptop_mode;
mod library_bridge;
mod library_sync;
mod media_controls;
mod musicbrainz;
mod pcm_buffer;
mod playback;
mod publishers;
mod ratings;
mod replay_gain;
mod shortcuts;
mod state_store;
mod state_sync;
mod tag_model;
mod tagging;
mod track_deletion;
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
use inbox::{
    DiscogsCredentialsRequest, InboxRenameRequest, InboxRenameResult, InboxRuntime,
    InboxSettingsStatus, InboxSnapshot, InboxTagApplyRequest, InboxTagApplyResult,
    ReleaseCandidateDetail, ReleaseDetailRequest, ReleaseSearchRequest, ReleaseSearchResult,
};
use laptop_mode::{LaptopModeRuntime, LaptopModeStatus};
use library_bridge::{
    apply_library_intake_batch, library_bridge_capabilities, preview_library_intake_batch,
    select_library_intake_folder,
};
use library_sync::{CatalogSync, LibrarySyncCoordinator};
use musicbrainz::{ArtistIntelligence, ArtistReviewPage, ArtistReviewPageRequest};
use playback::{PlaybackCatalogRebind, PlaybackRuntime, PlaybackSnapshot, RepeatMode};
use publishers::{PublisherDetail, PublisherOverview, PublisherQueueRequest};
use ratings::{
    CompletionKind, RatingAlbumPage, RatingAlbumQueueRequest, RatingCollectionRequest,
    RatingsOverview,
};
use state_store::StateStore;
use std::sync::Mutex;
use tag_model::{
    TagEditRequest, TagEditorSnapshot, TagEditorTarget, TagEditorUpdateRequest,
    TagEditorUpdateResult,
};
use tagging::{TagReconciliationReport, TagService, TrackTagSnapshot};
use tauri::{AppHandle, Manager};
use waveform::{FileSignature, WaveformSnapshot, WaveformStore, WaveformWorkCoordinator};
use years::{YearDetail, YearOverview, YearQueueRequest, YearSelection};

type PlaybackState = Mutex<PlaybackRuntime>;
type TagState = Mutex<TagService>;
type LaptopState = Mutex<LaptopModeRuntime>;
type WaveformState = Mutex<WaveformStore>;
type GlobalShortcutState = Mutex<shortcuts::GlobalShortcutRuntime>;
type InboxState = Mutex<InboxRuntime>;

async fn with_playback<T: Send + 'static>(
    app: AppHandle,
    operation: impl FnOnce(&mut PlaybackRuntime) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<PlaybackState>();
        let mut runtime = state
            .lock()
            .map_err(|_| "Aurora's playback engine stopped unexpectedly.".to_owned())?;
        operation(&mut runtime)
    })
    .await
    .map_err(|error| format!("Aurora's playback worker stopped unexpectedly: {error}"))?
}

async fn with_playback_snapshot(
    app: AppHandle,
    operation: impl FnOnce(&mut PlaybackRuntime) -> Result<PlaybackSnapshot, String> + Send + 'static,
) -> Result<PlaybackSnapshot, String> {
    let publish_app = app.clone();
    let snapshot = with_playback(app, operation).await?;
    media_controls::publish(&publish_app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
async fn inbox_snapshot(app: AppHandle) -> Result<InboxSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<InboxState>();
        state
            .lock()
            .map_err(|_| "Aurora's Inbox stopped unexpectedly.".to_owned())?
            .scan()
    })
    .await
    .map_err(|error| format!("Aurora's Inbox scan stopped unexpectedly: {error}"))?
}

#[tauri::command]
fn inbox_settings(app: AppHandle) -> Result<InboxSettingsStatus, String> {
    app.state::<InboxState>()
        .lock()
        .map_err(|_| "Aurora's Inbox stopped unexpectedly.".to_owned())
        .map(|runtime| runtime.status())
}

#[tauri::command]
async fn select_inbox_monitor_folder(app: AppHandle) -> Result<Option<String>, String> {
    library_bridge::select_library_intake_folder(app).await
}

#[tauri::command]
fn add_inbox_monitor_folder(app: AppHandle, folder: String) -> Result<InboxSettingsStatus, String> {
    app.state::<InboxState>()
        .lock()
        .map_err(|_| "Aurora's Inbox stopped unexpectedly.".to_owned())?
        .add_folder(folder)
}

#[tauri::command]
fn remove_inbox_monitor_folder(
    app: AppHandle,
    folder: String,
) -> Result<InboxSettingsStatus, String> {
    app.state::<InboxState>()
        .lock()
        .map_err(|_| "Aurora's Inbox stopped unexpectedly.".to_owned())?
        .remove_folder(&folder)
}

#[tauri::command]
fn update_discogs_credentials(
    app: AppHandle,
    request: DiscogsCredentialsRequest,
) -> Result<InboxSettingsStatus, String> {
    inbox::save_discogs_credentials(request)?;
    inbox_settings(app)
}

#[tauri::command]
async fn search_inbox_releases(
    request: ReleaseSearchRequest,
) -> Result<ReleaseSearchResult, String> {
    tauri::async_runtime::spawn_blocking(move || inbox::search_releases(request))
        .await
        .map_err(|error| format!("Aurora's metadata search stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn inbox_release_detail(
    request: ReleaseDetailRequest,
) -> Result<ReleaseCandidateDetail, String> {
    tauri::async_runtime::spawn_blocking(move || inbox::release_detail(request))
        .await
        .map_err(|error| format!("Aurora's release lookup stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn apply_inbox_tags(request: InboxTagApplyRequest) -> Result<InboxTagApplyResult, String> {
    tauri::async_runtime::spawn_blocking(move || inbox::apply_tags(request))
        .await
        .map_err(|error| format!("Aurora's Inbox tag worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn rename_inbox_album(request: InboxRenameRequest) -> Result<InboxRenameResult, String> {
    tauri::async_runtime::spawn_blocking(move || inbox::rename_album(request))
        .await
        .map_err(|error| format!("Aurora's Inbox rename worker stopped unexpectedly: {error}"))?
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
async fn catalog_revision() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(catalog::completed_import_revision)
        .await
        .map_err(|error| format!("The catalog revision worker stopped unexpectedly: {error}"))?
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
async fn explore_albums(app: AppHandle, request: AlbumPageRequest) -> Result<AlbumPage, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<StateStore>();
        explorer::load_album_page(request, &store)
    })
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

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TrackDeletionResult {
    deleted_track_keys: Vec<String>,
    failures: Vec<TrackDeletionFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    catalog_sync: Option<library_sync::CatalogSync>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TrackDeletionFailure {
    track_key: String,
    title: String,
    message: String,
}

#[tauri::command]
async fn delete_album_track(
    app: AppHandle,
    album_id: String,
    track_references: Vec<TrackReference>,
) -> Result<TrackDeletionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let coordinator = app.state::<LibrarySyncCoordinator>();
        let (result, projection_token) = coordinator.serialize_tag_edit(|| {
            if track_references.is_empty() || track_references.len() > 100 {
                return Err("Choose between 1 and 100 album tracks to delete.".to_owned());
            }
            let store = app.state::<StateStore>();
            let mut resolved_tracks = Vec::with_capacity(track_references.len());
            let mut seen = std::collections::HashSet::new();
            for reference in track_references {
                let resolved = catalog::resolve_track(&reference.id, &reference.track_key, &store)?;
                if resolved.summary.album_id.as_deref() != Some(album_id.as_str()) {
                    return Err(
                        "Every selected track must still belong to the open album.".to_owned()
                    );
                }
                if !seen.insert(resolved.summary.track_key.clone()) {
                    return Err("The track deletion selection contains a duplicate MP3.".to_owned());
                }
                resolved_tracks.push(resolved);
            }
            let changed_files = resolved_tracks
                .iter()
                .map(|resolved| {
                    (
                        resolved.summary.directory.clone(),
                        resolved.summary.filename.clone(),
                    )
                })
                .collect::<Vec<_>>();

            // Queue every selected file before the destructive filesystem step. If Aurora exits
            // after any deletion, the next background retry still tells Music Library about it.
            store.queue_library_file_syncs(&changed_files)?;
            let mut deleted_track_keys = Vec::with_capacity(resolved_tracks.len());
            let mut failures = Vec::new();
            let mut changed_directories = std::collections::HashSet::new();
            for resolved in resolved_tracks {
                match track_deletion::remove_verified_mp3(&resolved.audio_path) {
                    Ok(()) => {
                        deleted_track_keys.push(resolved.summary.track_key);
                        changed_directories.insert(resolved.summary.directory);
                    }
                    Err(message) => failures.push(TrackDeletionFailure {
                        track_key: resolved.summary.track_key,
                        title: resolved.summary.title,
                        message,
                    }),
                }
            }

            let catalog_sync = if changed_directories.is_empty() {
                None
            } else {
                let changed_directories = changed_directories.into_iter().collect::<Vec<_>>();
                Some(
                    coordinator
                        .sync_directories(&app, &changed_directories)
                        .catalog_sync,
                )
            };
            Ok::<TrackDeletionResult, String>(TrackDeletionResult {
                deleted_track_keys,
                failures,
                catalog_sync,
            })
        });
        let mut result = result?;
        if let Some(sync) = &mut result.catalog_sync {
            sync.projection_token = Some(projection_token);
        }
        Ok(result)
    })
    .await
    .map_err(|error| format!("The track deletion worker stopped unexpectedly: {error}"))?
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
async fn publisher_overview(search: Option<String>) -> Result<PublisherOverview, String> {
    tauri::async_runtime::spawn_blocking(move || publishers::load_publisher_overview(search))
        .await
        .map_err(|error| format!("The publisher overview worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn publisher_detail(publisher: String) -> Result<PublisherDetail, String> {
    tauri::async_runtime::spawn_blocking(move || publishers::load_publisher_detail(publisher))
        .await
        .map_err(|error| format!("The publisher detail worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn publisher_queue_tracks(
    app: AppHandle,
    request: PublisherQueueRequest,
) -> Result<Vec<TrackSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<StateStore>();
        publishers::load_publisher_queue(request, &store)
    })
    .await
    .map_err(|error| format!("The publisher queue worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn rating_album_page(
    app: AppHandle,
    kind: CompletionKind,
    remaining_tracks: Option<i64>,
) -> Result<RatingAlbumPage, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<StateStore>();
        ratings::load_rating_album_page(kind, remaining_tracks, &store)
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
async fn playback_state(app: AppHandle) -> Result<PlaybackSnapshot, String> {
    with_playback_snapshot(app, |runtime| Ok(runtime.snapshot())).await
}

#[tauri::command]
async fn playback_rebind_catalog(app: AppHandle) -> Result<PlaybackCatalogRebind, String> {
    let publish_app = app.clone();
    let rebind = with_playback(app, PlaybackRuntime::rebind_catalog).await?;
    media_controls::publish(&publish_app, &rebind.playback);
    Ok(rebind)
}

#[tauri::command]
async fn playback_replace_queue(
    app: AppHandle,
    track_references: Vec<TrackReference>,
    start_track_key: String,
) -> Result<PlaybackSnapshot, String> {
    with_playback_snapshot(app, move |runtime| {
        runtime.replace_queue(track_references, start_track_key)
    })
    .await
}

#[tauri::command]
async fn playback_append_queue(
    app: AppHandle,
    track_references: Vec<TrackReference>,
) -> Result<PlaybackSnapshot, String> {
    with_playback_snapshot(app, move |runtime| runtime.append_queue(track_references)).await
}

#[tauri::command]
async fn playback_toggle(app: AppHandle) -> Result<PlaybackSnapshot, String> {
    with_playback_snapshot(app, PlaybackRuntime::toggle).await
}

#[tauri::command]
async fn playback_next(app: AppHandle) -> Result<PlaybackSnapshot, String> {
    with_playback_snapshot(app, PlaybackRuntime::next).await
}

#[tauri::command]
async fn playback_previous(app: AppHandle) -> Result<PlaybackSnapshot, String> {
    with_playback_snapshot(app, PlaybackRuntime::previous).await
}

#[tauri::command]
async fn playback_stop(app: AppHandle) -> Result<PlaybackSnapshot, String> {
    with_playback_snapshot(app, PlaybackRuntime::stop).await
}

#[tauri::command]
async fn playback_seek(app: AppHandle, position_seconds: f64) -> Result<PlaybackSnapshot, String> {
    with_playback_snapshot(app, move |runtime| runtime.seek(position_seconds)).await
}

#[tauri::command]
async fn playback_set_volume(app: AppHandle, volume: f32) -> Result<PlaybackSnapshot, String> {
    with_playback_snapshot(app, move |runtime| runtime.set_volume(volume)).await
}

#[tauri::command]
async fn playback_set_shuffle(app: AppHandle, enabled: bool) -> Result<PlaybackSnapshot, String> {
    with_playback_snapshot(app, move |runtime| runtime.set_shuffle(enabled)).await
}

#[tauri::command]
async fn playback_set_repeat_mode(
    app: AppHandle,
    repeat_mode: RepeatMode,
) -> Result<PlaybackSnapshot, String> {
    with_playback_snapshot(app, move |runtime| runtime.set_repeat_mode(repeat_mode)).await
}

#[tauri::command]
async fn playback_remove_queue_item(
    app: AppHandle,
    index: usize,
) -> Result<PlaybackSnapshot, String> {
    with_playback_snapshot(app, move |runtime| runtime.remove_queue_item(index)).await
}

#[tauri::command]
async fn playback_move_queue_item(
    app: AppHandle,
    from: usize,
    to: usize,
) -> Result<PlaybackSnapshot, String> {
    with_playback_snapshot(app, move |runtime| runtime.move_queue_item(from, to)).await
}

#[tauri::command]
async fn playback_clear_queue(app: AppHandle) -> Result<PlaybackSnapshot, String> {
    with_playback_snapshot(app, PlaybackRuntime::clear_queue).await
}

#[tauri::command]
async fn track_waveform(
    app: AppHandle,
    track_id: String,
    track_key: String,
) -> Result<WaveformSnapshot, String> {
    let generation = app.state::<WaveformWorkCoordinator>().begin();
    let worker_app = app.clone();
    let work = app.state::<WaveformWorkCoordinator>();
    work.run_serialized(generation, || async move {
        tauri::async_runtime::spawn_blocking(move || {
            let work = worker_app.state::<WaveformWorkCoordinator>();
            let cancellation = work.cancellation(generation)?;
            let resolved = {
                let store = worker_app.state::<StateStore>();
                catalog::resolve_track(&track_id, &track_key, &store)?
            };
            cancellation.checkpoint()?;
            let signature = FileSignature::read(&resolved.audio_path)?;
            if let Some(cached) = {
                let cache = worker_app.state::<WaveformState>();
                let store = cache
                    .lock()
                    .map_err(|_| "Aurora's waveform cache stopped unexpectedly.".to_owned())?;
                store.load(&resolved.summary.track_key, signature)?
            } {
                cancellation.checkpoint()?;
                return Ok(cached);
            }

            let snapshot = waveform::decode_mp3_waveform(
                &resolved.audio_path,
                &resolved.summary.track_key,
                resolved.summary.duration_seconds,
                &cancellation,
            )?;
            cancellation.checkpoint()?;
            let cache = worker_app.state::<WaveformState>();
            let store = cache
                .lock()
                .map_err(|_| "Aurora's waveform cache stopped unexpectedly.".to_owned())?;
            signature.verify_unchanged(&resolved.audio_path)?;
            cancellation.checkpoint()?;
            store.save(&snapshot, signature)?;
            Ok(snapshot)
        })
        .await
        .map_err(|error| format!("The waveform worker stopped unexpectedly: {error}"))?
    })
    .await
}

fn refresh_playback_track_tags(app: &AppHandle, track: &TrackSummary) {
    let playback = app.state::<PlaybackState>();
    if let Ok(mut runtime) = playback.lock() {
        runtime.refresh_track_tags(track);
    }
}

fn refresh_playback_track_metadata(app: &AppHandle, track: &TrackSummary) {
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
        refresh_playback_track_tags(&app, &result.track);
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
        let coordinator = app.state::<LibrarySyncCoordinator>();
        let (result, projection_token) = coordinator.serialize_tag_edit(|| {
            let mut result = {
                let state = app.state::<TagState>();
                let service = state
                    .lock()
                    .map_err(|_| "Aurora's tag writer stopped unexpectedly.".to_owned())?;
                service.update(request)?
            };
            refresh_playback_track_tags(&app, &result.track);
            let directory = result.track.directory.clone();
            let sync = coordinator.queue_after_edit(&app, std::slice::from_ref(&directory));
            if sync.completed(&directory) {
                result.track.tag_sync_state = None;
                result.tag_state.sync_state = None;
            }
            result.catalog_sync = Some(sync.catalog_sync);
            refresh_playback_track_tags(&app, &result.track);
            Ok::<TrackTagSnapshot, String>(result)
        });
        let mut result = result?;
        if let Some(sync) = &mut result.catalog_sync {
            sync.projection_token = Some(projection_token);
        }
        Ok(result)
    })
    .await
    .map_err(|error| format!("The tag writer stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn tag_editor_state(
    app: AppHandle,
    target: TagEditorTarget,
) -> Result<TagEditorSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<TagState>();
        let service = state
            .lock()
            .map_err(|_| "Aurora's tag reader stopped unexpectedly.".to_owned())?;
        service.inspect_editor(target)
    })
    .await
    .map_err(|error| format!("The tag reader stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn update_tag_editor(
    app: AppHandle,
    request: TagEditorUpdateRequest,
) -> Result<TagEditorUpdateResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let coordinator = app.state::<LibrarySyncCoordinator>();
        let (result, projection_token) = coordinator.serialize_tag_edit(|| {
            let mut result = {
                let state = app.state::<TagState>();
                let service = state
                    .lock()
                    .map_err(|_| "Aurora's tag writer stopped unexpectedly.".to_owned())?;
                service.update_editor(request)?
            };

            for track in &result.tracks {
                refresh_playback_track_metadata(&app, track);
            }
            let directories = result
                .tracks
                .iter()
                .filter(|track| track.tag_sync_state.is_some())
                .map(|track| track.directory.clone())
                .collect::<Vec<_>>();
            let sync = coordinator.queue_after_edit(&app, &directories);
            for track in &mut result.tracks {
                if sync.completed(&track.directory) {
                    track.tag_sync_state = None;
                }
            }
            result.catalog_sync = Some(sync.catalog_sync);
            for track in &result.tracks {
                refresh_playback_track_metadata(&app, track);
            }
            Ok::<TagEditorUpdateResult, String>(result)
        });
        let mut result = result?;
        if let Some(sync) = &mut result.catalog_sync {
            sync.projection_token = Some(projection_token);
        }
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
        let coordinator = app.state::<LibrarySyncCoordinator>();
        let (result, projection_token) = coordinator.serialize_tag_edit(|| {
            let mut result = {
                let state = app.state::<TagState>();
                let service = state
                    .lock()
                    .map_err(|_| "Aurora's tag writer stopped unexpectedly.".to_owned())?;
                service.undo(&track_id, &track_key)?
            };
            refresh_playback_track_tags(&app, &result.track);
            let directory = result.track.directory.clone();
            let sync = coordinator.queue_after_edit(&app, std::slice::from_ref(&directory));
            if sync.completed(&directory) {
                result.track.tag_sync_state = None;
                result.tag_state.sync_state = None;
            }
            result.catalog_sync = Some(sync.catalog_sync);
            refresh_playback_track_tags(&app, &result.track);
            Ok::<TrackTagSnapshot, String>(result)
        });
        let mut result = result?;
        if let Some(sync) = &mut result.catalog_sync {
            sync.projection_token = Some(projection_token);
        }
        Ok(result)
    })
    .await
    .map_err(|error| format!("The tag undo worker stopped unexpectedly: {error}"))?
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TagReconciliationProjection {
    #[serde(flatten)]
    report: TagReconciliationReport,
    projection_token: u64,
}

#[tauri::command]
async fn refresh_external_tag_changes(
    app: AppHandle,
) -> Result<TagReconciliationProjection, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let coordinator = app.state::<LibrarySyncCoordinator>();
        let projection_token = coordinator.reserve_background_projection_token();
        let report = {
            let state = app.state::<TagState>();
            let service = state
                .lock()
                .map_err(|_| "Aurora's tag reader stopped unexpectedly.".to_owned())?
                .clone();
            service.reconcile_pending_overlays(100)
        };
        Ok(TagReconciliationProjection {
            report: report?,
            projection_token,
        })
    })
    .await
    .map_err(|error| format!("The external-tag refresh stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn retry_pending_library_sync(app: AppHandle) -> Result<CatalogSync, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let coordinator = app.state::<LibrarySyncCoordinator>();
        let projection_token = coordinator.reserve_background_projection_token();
        let mut sync = coordinator.retry_one(&app).catalog_sync;
        sync.projection_token = Some(projection_token);
        Ok(sync)
    })
    .await
    .map_err(|error| format!("The Music Library retry worker stopped unexpectedly: {error}"))?
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
async fn audio_settings(app: AppHandle) -> Result<AudioSettingsStatus, String> {
    with_playback(app, |runtime| Ok(runtime.audio_settings_status())).await
}

#[tauri::command]
async fn update_audio_settings(
    app: AppHandle,
    request: AudioSettingsRequest,
) -> Result<AudioSettingsStatus, String> {
    with_playback(app, |runtime| runtime.update_audio_settings(request)).await
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

fn release_global_shortcuts(app: &AppHandle) {
    let state = app.state::<GlobalShortcutState>();
    if let Ok(mut runtime) = state.lock() {
        let _ = runtime.release(app);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
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
            #[cfg(debug_assertions)]
            {
                let _ = dotenvy::from_filename(".env.local")
                    .or_else(|_| dotenvy::from_filename("../.env.local"));
            }
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
            let mut runtime = PlaybackRuntime::new(store.clone(), history.clone(), audio_store)
                .map_err(std::io::Error::other)?;
            let initial_playback = runtime.snapshot();
            let tag_service = TagService::new(store.clone()).map_err(std::io::Error::other)?;
            let _ = laptop_runtime.status(true);
            app.manage(store);
            app.manage(history);
            app.manage(Mutex::new(runtime));
            app.manage(Mutex::new(tag_service));
            app.manage(Mutex::new(laptop_runtime));
            app.manage(Mutex::new(waveform_store));
            app.manage(Mutex::new(InboxRuntime::load(
                state_directory.join("aurora-inbox.json"),
            )));
            app.manage(WaveformWorkCoordinator::default());
            app.manage(LibrarySyncCoordinator::default());
            app.manage(Mutex::new(shortcuts::GlobalShortcutRuntime::load(
                state_directory.join("aurora-shortcuts.json"),
            )));
            if let Ok(mut shortcut_runtime) = app.state::<GlobalShortcutState>().lock() {
                shortcut_runtime.initialize(app.handle());
            }
            if let Err(error) = media_controls::initialize(app.handle(), &initial_playback) {
                eprintln!("{error}");
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                release_global_shortcuts(window.app_handle());
                media_controls::release(window.app_handle());
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
            catalog_revision,
            artist_tracks,
            search_tracks,
            explore_tracks,
            explore_albums,
            explore_artists,
            album_detail,
            delete_album_track,
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
            publisher_overview,
            publisher_detail,
            publisher_queue_tracks,
            artist_intelligence,
            musicbrainz_review_page,
            update_artist_identity_decision,
            update_release_group_decision,
            undo_musicbrainz_curation,
            export_musicbrainz_curation,
            playback_state,
            playback_rebind_catalog,
            playback_replace_queue,
            playback_append_queue,
            playback_toggle,
            playback_next,
            playback_previous,
            playback_stop,
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
            tag_editor_state,
            update_tag_editor,
            undo_track_tag_edit,
            refresh_external_tag_changes,
            retry_pending_library_sync,
            laptop_mode_status,
            set_laptop_mode,
            listening_history_page,
            track_history_insight,
            set_history_play_threshold,
            global_shortcut_settings,
            update_global_shortcut_settings,
            audio_settings,
            update_audio_settings,
            library_bridge_capabilities,
            select_library_intake_folder,
            preview_library_intake_batch,
            apply_library_intake_batch,
            inbox_snapshot,
            inbox_settings,
            select_inbox_monitor_folder,
            add_inbox_monitor_folder,
            remove_inbox_monitor_folder,
            update_discogs_credentials,
            search_inbox_releases,
            inbox_release_detail,
            apply_inbox_tags,
            rename_inbox_album,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Aurora")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
                release_global_shortcuts(app);
                media_controls::release(app);
            }
        });
}
