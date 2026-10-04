//! Native schedules are independent of WebView visibility. Each job has its own
//! thread so an unavailable share/bridge cannot delay playback or catalog checks.
use crate::{
    HistoryStore, LaptopState, LibrarySyncCoordinator, PlaybackState, TagReconciliationProjection,
    TagState, catalog, connections, playback::PlaybackSnapshot,
};
use serde::Serialize;
use std::{
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
pub(crate) struct NativeBackgroundTasks {
    stop: Arc<(Mutex<bool>, Condvar)>,
    catalog_acknowledged: Arc<Mutex<Option<String>>>,
}

impl NativeBackgroundTasks {
    pub(crate) fn acknowledge_catalog(&self, revision: String) {
        *self
            .catalog_acknowledged
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(revision);
    }
    pub(crate) fn stop(&self) {
        let (stopped, wake) = &*self.stop;
        *stopped.lock().unwrap_or_else(|error| error.into_inner()) = true;
        wake.notify_all();
    }

    fn schedule(
        &self,
        name: &str,
        interval: Duration,
        mut job: impl FnMut() -> Result<(), String> + Send + 'static,
    ) -> Result<(), String> {
        let stop = Arc::clone(&self.stop);
        std::thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || {
                loop {
                    let (stopped, wake) = &*stop;
                    if *stopped.lock().unwrap_or_else(|error| error.into_inner()) {
                        break;
                    }
                    if let Err(error) = job() {
                        eprintln!("Aurora native background task: {error}");
                    }
                    let guard = stopped.lock().unwrap_or_else(|error| error.into_inner());
                    let (guard, _) = wake
                        .wait_timeout_while(guard, interval, |stopped| !*stopped)
                        .unwrap_or_else(|error| error.into_inner());
                    if *guard {
                        break;
                    }
                }
            })
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub(crate) fn start(&self, app: &AppHandle) -> Result<(), String> {
        let sync_app = app.clone();
        let mut sync_previous = None;
        self.schedule("aurora-state-sync", Duration::from_secs(5), move || {
            let status = sync_app
                .state::<LaptopState>()
                .lock()
                .map_err(|_| "State sync monitor stopped")?
                .status(false);
            emit_changed(&sync_app, "sync://status", &status, &mut sync_previous)
        })?;

        let catalog_app = app.clone();
        let acknowledged = Arc::clone(&self.catalog_acknowledged);
        self.schedule("aurora-catalog", Duration::from_secs(5), move || {
            let revision = catalog::completed_import_revision()?;
            if acknowledged
                .lock()
                .map_err(|_| "Catalog acknowledgement stopped")?
                .as_ref()
                != Some(&revision)
            {
                catalog_app
                    .emit("catalog://revision", revision)
                    .map_err(|error| error.to_string())?;
            }
            Ok(())
        })?;

        let tags_app = app.clone();
        let mut tag_previous = None;
        self.schedule(
            "aurora-tag-reconciliation",
            Duration::from_secs(5),
            move || {
                let coordinator = tags_app.state::<LibrarySyncCoordinator>();
                let projection_token = coordinator.reserve_background_projection_token();
                let service = tags_app
                    .state::<TagState>()
                    .lock()
                    .map_err(|_| "Tag reader stopped")?
                    .clone();
                let report = service.reconcile_pending_overlays(100)?;
                // Empty reports settle initial notices too; omit duplicate reports.
                if changed_payload(&report, &mut tag_previous)? {
                    tags_app
                        .emit(
                            "tags://reconciled",
                            TagReconciliationProjection {
                                report,
                                projection_token,
                            },
                        )
                        .map_err(|error| error.to_string())?;
                }
                Ok(())
            },
        )?;

        let library_app = app.clone();
        let mut sync_previous = None;
        self.schedule("aurora-library-sync", Duration::from_secs(5), move || {
            if !connections::network_mode() {
                let coordinator = library_app.state::<LibrarySyncCoordinator>();
                let projection_token = coordinator.reserve_background_projection_token();
                let report = coordinator.retry_one(&library_app);
                let made_progress = report.made_progress();
                let mut sync = report.catalog_sync;
                let changed = changed_payload(&sync, &mut sync_previous)?;
                sync.projection_token = Some(projection_token);
                if changed || made_progress {
                    library_app
                        .emit("library-sync://status", &sync)
                        .map_err(|error| error.to_string())?;
                }
            }
            Ok(())
        })?;

        let playback_app = app.clone();
        let mut playback_previous = None;
        self.schedule("aurora-playback", Duration::from_millis(250), move || {
            let snapshot = playback_app
                .state::<PlaybackState>()
                .lock()
                .map_err(|_| "Playback engine stopped")?
                .snapshot();
            // Runtime emits transitions while holding its lock, including native
            // shortcut/media commands; this tick drives automatic advancement.
            let mut comparable = snapshot.clone();
            comparable.position_seconds = 0.0;
            comparable.event_sequence = 0;
            if playback_previous.as_ref() != Some(&comparable) {
                playback_previous = Some(comparable);
                crate::media_controls::publish(&playback_app, &snapshot);
            }
            Ok(())
        })?;

        let history_app = app.clone();
        let mut history_previous = None;
        self.schedule("aurora-history", Duration::from_secs(5), move || {
            let history = history_app.state::<HistoryStore>();
            let _ = history.publish_if_due(false);
            emit_changed(
                &history_app,
                "history://revision",
                &history.revision()?,
                &mut history_previous,
            )
        })?;
        Ok(())
    }
}

fn changed_payload<T: Serialize>(
    payload: &T,
    previous: &mut Option<serde_json::Value>,
) -> Result<bool, String> {
    let next = serde_json::to_value(payload).map_err(|error| error.to_string())?;
    if previous.as_ref() == Some(&next) {
        return Ok(false);
    }
    *previous = Some(next);
    Ok(true)
}

fn emit_changed<T: Serialize + Clone>(
    app: &AppHandle,
    name: &str,
    payload: &T,
    previous: &mut Option<serde_json::Value>,
) -> Result<(), String> {
    if changed_payload(payload, previous)? {
        app.emit(name, payload).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[derive(Default)]
pub(crate) struct PlaybackEvents {
    publish: Option<PlaybackPublisher>,
    previous: Option<PlaybackSnapshot>,
    sequence: u64,
    last_position: f64,
    pub(crate) force: bool,
}

type PlaybackPublisher = Box<dyn Fn(&PlaybackSnapshot) + Send + Sync>;

impl PlaybackEvents {
    pub(crate) fn attach(&mut self, publish: PlaybackPublisher) {
        self.publish = Some(publish);
    }

    pub(crate) fn publish(&mut self, snapshot: &mut PlaybackSnapshot) -> bool {
        self.sequence += 1;
        snapshot.event_sequence = self.sequence;
        let mut comparable = snapshot.clone();
        comparable.position_seconds = 0.0;
        comparable.event_sequence = 0;
        let restarted = snapshot.position_seconds + 0.5 < self.last_position;
        self.last_position = snapshot.position_seconds;
        if self.force || restarted || self.previous.as_ref() != Some(&comparable) {
            self.force = false;
            self.previous = Some(comparable);
            if let Some(publish) = &self.publish {
                publish(snapshot);
            }
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        audio_settings::ReplayGainMode,
        device_mode::DeviceModeStore,
        laptop_mode::LaptopModeRuntime,
        playback::{PlaybackStatus, RepeatMode},
        state_store::{StateStore, StoredPlaybackState},
        state_sync::StartupSyncOutcome,
    };
    use std::sync::mpsc;

    #[test]
    fn catalog_acknowledgement_only_settles_the_delivered_revision() {
        let tasks = NativeBackgroundTasks::default();
        assert_eq!(*tasks.catalog_acknowledged.lock().unwrap(), None);
        tasks.acknowledge_catalog("completed-one".to_owned());
        let acknowledged = tasks.catalog_acknowledged.lock().unwrap();
        assert_eq!(acknowledged.as_deref(), Some("completed-one"));
        assert_ne!(acknowledged.as_deref(), Some("completed-two"));
    }

    #[test]
    fn jobs_run_without_webview_requests_and_stop_even_while_another_job_is_blocked() {
        let tasks = NativeBackgroundTasks::default();
        let (started, slow_started) = mpsc::channel();
        let (release, wait_release) = mpsc::channel();
        tasks
            .schedule("slow-test", Duration::from_secs(60), move || {
                started.send(()).unwrap();
                wait_release.recv().unwrap();
                Ok(())
            })
            .unwrap();
        slow_started.recv_timeout(Duration::from_secs(5)).unwrap();
        let (ran, fast_ran) = mpsc::channel();
        tasks
            .schedule("fast-test", Duration::from_secs(60), move || {
                ran.send(()).unwrap();
                Ok(())
            })
            .unwrap();
        fast_ran.recv_timeout(Duration::from_secs(5)).unwrap();
        tasks.stop();
        release.send(()).unwrap();
        assert_eq!(
            slow_started.recv_timeout(Duration::from_secs(5)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        );
        assert_eq!(
            fast_ran.recv_timeout(Duration::from_secs(5)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        );
    }

    #[test]
    fn native_schedule_publishes_actual_state_while_cached_status_reads_do_not() {
        let root = tempfile::tempdir().unwrap();
        let store = StateStore::new(root.path().join("local.sqlite3")).unwrap();
        store
            .save(&StoredPlaybackState {
                volume: 0.42,
                ..Default::default()
            })
            .unwrap();
        let remote = root.path().join("aurora-state.sqlite3");
        let mut runtime = LaptopModeRuntime::new(
            DeviceModeStore::load(root.path().join("device.json")),
            store,
            remote.clone(),
            StartupSyncOutcome::None,
        )
        .unwrap();
        let _ = runtime.cached_status();
        assert!(!remote.exists());
        let tasks = NativeBackgroundTasks::default();
        let (published, did_publish) = mpsc::channel();
        tasks
            .schedule("publication-test", Duration::from_secs(60), move || {
                let _ = runtime.status(false);
                published.send(()).unwrap();
                Ok(())
            })
            .unwrap();
        did_publish.recv_timeout(Duration::from_secs(5)).unwrap();
        tasks.stop();
        assert_eq!(
            did_publish.recv_timeout(Duration::from_secs(5)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        );
        assert_eq!(
            StateStore::new(remote).unwrap().load().unwrap().volume,
            0.42
        );
    }

    fn snapshot() -> PlaybackSnapshot {
        PlaybackSnapshot {
            event_sequence: 0,
            queue: vec![],
            current_index: None,
            current_track: None,
            status: PlaybackStatus::Playing,
            position_seconds: 10.0,
            volume: 0.7,
            shuffle: false,
            repeat_mode: RepeatMode::Off,
            error: None,
            output_device_label: None,
            using_device_fallback: false,
            replay_gain_mode: ReplayGainMode::Off,
            replay_gain_db: None,
            replay_gain_source: None,
            clipping_prevented: false,
            audio_underrun_count: 0,
            realtime_scheduling_denied: false,
        }
    }

    #[test]
    fn playback_sequences_every_snapshot_but_only_publishes_transitions_and_seeks() {
        let mut events = PlaybackEvents::default();
        let delivered = Arc::new(Mutex::new(Vec::new()));
        let received = Arc::clone(&delivered);
        events.attach(Box::new(move |state| {
            received.lock().unwrap().push(state.event_sequence);
        }));
        let mut state = snapshot();
        assert!(events.publish(&mut state));
        assert_eq!(state.event_sequence, 1);
        let unchanged = events.previous.clone();
        state.position_seconds += 0.25;
        assert!(!events.publish(&mut state));
        assert_eq!(state.event_sequence, 2);
        assert_eq!(events.previous, unchanged);
        state.status = PlaybackStatus::Paused;
        assert!(events.publish(&mut state));
        assert_ne!(events.previous, unchanged);
        events.force = true;
        assert!(events.publish(&mut state));
        assert!(!events.force);
        state.position_seconds = 0.0;
        assert!(events.publish(&mut state));
        assert_eq!(events.last_position, 0.0);
        assert_eq!(*delivered.lock().unwrap(), vec![1, 3, 4, 5]);
    }
}
