use crate::{
    audio_settings::{
        self, AudioSettingsRequest, AudioSettingsStatus, AudioSettingsStore, ReplayGainMode,
    },
    catalog::{self, TrackReference, TrackSummary},
    history::{ActiveHistorySession, HistoryStore},
    replay_gain::{self, ReplayGainAdjustment},
    state_store::{StateStore, StoredPlaybackState, StoredQueueEntry},
};
use cpal::BufferSize;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, SampleRate, Source};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashSet, VecDeque},
    fs::File,
    io::{BufReader, Cursor, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const PRELOAD_WINDOW_SECONDS: f64 = 15.0;
const OUTPUT_BUFFER_TARGET_MS: u32 = 100;
const MIN_OUTPUT_BUFFER_FRAMES: u32 = 2_048;
const MAX_OUTPUT_BUFFER_FRAMES: u32 = 16_384;
const PLAYBACK_FILE_BUFFER_BYTES: usize = 1024 * 1024;
const MAX_CACHED_TRACK_BYTES: u64 = 96 * 1024 * 1024;
const CACHED_TRACK_LIMIT: usize = 2;
const MAX_PLAYBACK_QUEUE: usize = 200;
const MAX_QUEUE_APPEND_BATCH: usize = 100;
const RETAINED_QUEUE_HISTORY: usize = 20;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PlaybackStatus {
    Stopped,
    Playing,
    Paused,
    Error,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum RepeatMode {
    Off,
    All,
    One,
}

impl RepeatMode {
    fn from_stored(value: &str) -> Self {
        match value {
            "all" => Self::All,
            "one" => Self::One,
            _ => Self::Off,
        }
    }

    fn as_stored(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::All => "all",
            Self::One => "one",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaybackSnapshot {
    pub(crate) queue: Vec<TrackSummary>,
    pub(crate) current_index: Option<usize>,
    pub(crate) current_track: Option<TrackSummary>,
    pub(crate) status: PlaybackStatus,
    pub(crate) position_seconds: f64,
    pub(crate) volume: f32,
    pub(crate) shuffle: bool,
    pub(crate) repeat_mode: RepeatMode,
    pub(crate) error: Option<String>,
    pub(crate) output_device_label: Option<String>,
    pub(crate) using_device_fallback: bool,
    pub(crate) replay_gain_mode: ReplayGainMode,
    pub(crate) replay_gain_db: Option<f32>,
    pub(crate) replay_gain_source: Option<ReplayGainMode>,
    pub(crate) clipping_prevented: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaybackCatalogRebind {
    pub(crate) playback: PlaybackSnapshot,
    pub(crate) catalog_revision: String,
}

struct PreparedTrack {
    index: usize,
    gain: ReplayGainAdjustment,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PlaybackFileSignature {
    byte_len: u64,
    modified: Option<SystemTime>,
}

struct CachedPlaybackMedia {
    path: PathBuf,
    signature: PlaybackFileSignature,
    bytes: Arc<[u8]>,
}

#[derive(Default)]
struct PlaybackMediaCache {
    entries: VecDeque<CachedPlaybackMedia>,
}

enum PlaybackMedia {
    Memory(Cursor<Arc<[u8]>>),
    File(BufReader<File>),
}

impl Read for PlaybackMedia {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Memory(reader) => reader.read(buffer),
            Self::File(reader) => reader.read(buffer),
        }
    }
}

impl Seek for PlaybackMedia {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        match self {
            Self::Memory(reader) => reader.seek(position),
            Self::File(reader) => reader.seek(position),
        }
    }
}

impl PlaybackMediaCache {
    fn open(&mut self, path: &Path) -> Result<(PlaybackMedia, u64), String> {
        let file =
            File::open(path).map_err(|error| format!("Aurora could not open this MP3: {error}"))?;
        let metadata = file
            .metadata()
            .map_err(|error| format!("Aurora could not inspect this MP3: {error}"))?;
        let byte_len = metadata.len();
        if byte_len > MAX_CACHED_TRACK_BYTES {
            return Ok((
                PlaybackMedia::File(BufReader::with_capacity(PLAYBACK_FILE_BUFFER_BYTES, file)),
                byte_len,
            ));
        }

        let signature = PlaybackFileSignature {
            byte_len,
            modified: metadata.modified().ok(),
        };
        if let Some(index) = signature.modified.and_then(|_| {
            self.entries
                .iter()
                .position(|entry| entry.path == path && entry.signature == signature)
        }) {
            let entry = self.entries.remove(index).expect("cached entry exists");
            let bytes = Arc::clone(&entry.bytes);
            self.entries.push_back(entry);
            return Ok((PlaybackMedia::Memory(Cursor::new(bytes)), byte_len));
        }

        let expected_len = usize::try_from(byte_len)
            .map_err(|_| "This MP3 is too large to prepare safely for playback.".to_owned())?;
        let mut bytes = Vec::new();
        if bytes.try_reserve_exact(expected_len).is_err() {
            return Ok((
                PlaybackMedia::File(BufReader::with_capacity(PLAYBACK_FILE_BUFFER_BYTES, file)),
                byte_len,
            ));
        }
        let mut reader = BufReader::with_capacity(PLAYBACK_FILE_BUFFER_BYTES, file);
        reader
            .by_ref()
            .take(byte_len.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| format!("Aurora could not preload this MP3: {error}"))?;
        if bytes.len() as u64 != byte_len {
            return Err("This MP3 changed while Aurora was preparing it for playback.".to_owned());
        }

        let final_metadata = std::fs::metadata(path)
            .map_err(|error| format!("Aurora could not recheck this MP3: {error}"))?;
        let final_signature = PlaybackFileSignature {
            byte_len: final_metadata.len(),
            modified: final_metadata.modified().ok(),
        };
        if final_signature != signature {
            return Err("This MP3 changed while Aurora was preparing it for playback.".to_owned());
        }

        let bytes: Arc<[u8]> = bytes.into();
        if signature.modified.is_some() {
            self.entries.retain(|entry| entry.path != path);
            self.entries.push_back(CachedPlaybackMedia {
                path: path.to_path_buf(),
                signature,
                bytes: Arc::clone(&bytes),
            });
            while self.entries.len() > CACHED_TRACK_LIMIT {
                self.entries.pop_front();
            }
        }
        Ok((PlaybackMedia::Memory(Cursor::new(bytes)), byte_len))
    }
}

#[derive(Debug, PartialEq)]
struct CatalogRebindPlan {
    key_order_unchanged: bool,
    current_index: Option<usize>,
    current_removed: bool,
}

fn catalog_rebind_plan(
    current_queue: &[TrackSummary],
    current_index: Option<usize>,
    refreshed_queue: &[TrackSummary],
) -> CatalogRebindPlan {
    let key_order_unchanged = current_queue.len() == refreshed_queue.len()
        && current_queue
            .iter()
            .zip(refreshed_queue)
            .all(|(current, refreshed)| current.track_key == refreshed.track_key);
    let Some(current_index) = current_index.filter(|index| *index < current_queue.len()) else {
        return CatalogRebindPlan {
            key_order_unchanged,
            current_index: None,
            current_removed: false,
        };
    };
    let current_key = &current_queue[current_index].track_key;
    if let Some(refreshed_index) = refreshed_queue
        .iter()
        .position(|track| &track.track_key == current_key)
    {
        return CatalogRebindPlan {
            key_order_unchanged,
            current_index: Some(refreshed_index),
            current_removed: false,
        };
    }

    let replacement_key = current_queue[current_index + 1..]
        .iter()
        .chain(current_queue[..current_index].iter().rev())
        .find_map(|track| {
            refreshed_queue
                .iter()
                .position(|refreshed| refreshed.track_key == track.track_key)
        });
    CatalogRebindPlan {
        key_order_unchanged,
        current_index: replacement_key,
        current_removed: true,
    }
}

fn append_queue_entries(
    queue: &mut Vec<TrackSummary>,
    current_index: &mut usize,
    prepared_next: &mut Option<PreparedTrack>,
    additions: Vec<TrackSummary>,
) -> usize {
    let keep_from = current_index.saturating_sub(RETAINED_QUEUE_HISTORY);
    if keep_from > 0 {
        queue.drain(..keep_from);
        *current_index -= keep_from;
        if prepared_next
            .as_ref()
            .is_some_and(|prepared| prepared.index < keep_from)
        {
            *prepared_next = None;
        } else if let Some(prepared) = prepared_next.as_mut() {
            prepared.index -= keep_from;
        }
    }
    let mut keys = queue
        .iter()
        .map(|track| track.track_key.clone())
        .collect::<HashSet<_>>();
    let capacity = MAX_PLAYBACK_QUEUE.saturating_sub(queue.len());
    let mut appended = 0;
    for track in additions {
        if appended >= capacity {
            break;
        }
        if keys.insert(track.track_key.clone()) {
            queue.push(track);
            appended += 1;
        }
    }
    appended
}

pub(crate) struct PlaybackRuntime {
    output: Option<MixerDeviceSink>,
    player: Option<Player>,
    queue: Vec<TrackSummary>,
    current_index: Option<usize>,
    status: PlaybackStatus,
    position_seconds: f64,
    volume: f32,
    shuffle: bool,
    repeat_mode: RepeatMode,
    error: Option<String>,
    audio_store: AudioSettingsStore,
    active_device_id: Option<String>,
    active_device_label: Option<String>,
    using_device_fallback: bool,
    audio_message: Option<String>,
    configured_source_rate: Option<SampleRate>,
    stream_error: Arc<Mutex<Option<String>>>,
    current_gain: ReplayGainAdjustment,
    media_cache: PlaybackMediaCache,
    prepared_next: Option<PreparedTrack>,
    preparation_attempted: bool,
    store: StateStore,
    history: HistoryStore,
    history_session: Option<ActiveHistorySession>,
    last_saved_position_bucket: u64,
}

impl PlaybackRuntime {
    pub(crate) fn new(
        store: StateStore,
        history: HistoryStore,
        audio_store: AudioSettingsStore,
    ) -> Result<Self, String> {
        let stored = store.load()?;
        let queue_result = if stored.queue.is_empty() {
            Ok((Vec::new(), 0, String::new()))
        } else {
            catalog::load_tracks_by_references(&stored.queue, &store)
        };
        let (queue, error) = match queue_result {
            Ok((queue, missing, _)) => {
                let error = (missing > 0).then(|| {
                    format!(
                        "Aurora restored the surviving queue and skipped {missing} unavailable track{}.",
                        if missing == 1 { "" } else { "s" }
                    )
                });
                (queue, error)
            }
            Err(error) => (
                Vec::new(),
                Some(format!(
                    "Aurora could not restore the previous queue: {error}"
                )),
            ),
        };
        let current_index = stored
            .current_index
            .and_then(|index| stored.queue.get(index))
            .and_then(|reference| {
                queue.iter().position(|track| {
                    reference
                        .track_key
                        .as_ref()
                        .map_or(track.id == reference.track_id, |key| {
                            track.track_key == *key
                        })
                })
            });
        let position_seconds = current_index
            .and_then(|index| queue[index].duration_seconds)
            .map(|duration| stored.position_seconds.clamp(0.0, duration as f64))
            .unwrap_or(0.0);
        Ok(Self {
            output: None,
            player: None,
            queue,
            current_index,
            status: if current_index.is_some() {
                PlaybackStatus::Paused
            } else {
                PlaybackStatus::Stopped
            },
            position_seconds,
            volume: stored.volume,
            shuffle: stored.shuffle,
            repeat_mode: RepeatMode::from_stored(&stored.repeat_mode),
            error,
            audio_store,
            active_device_id: None,
            active_device_label: None,
            using_device_fallback: false,
            audio_message: None,
            configured_source_rate: None,
            stream_error: Arc::new(Mutex::new(None)),
            current_gain: ReplayGainAdjustment {
                linear: 1.0,
                ..Default::default()
            },
            media_cache: PlaybackMediaCache::default(),
            prepared_next: None,
            preparation_attempted: false,
            store,
            history,
            history_session: None,
            last_saved_position_bucket: (position_seconds / 10.0).floor() as u64,
        })
    }

    fn current_track(&self) -> Option<&TrackSummary> {
        self.current_index.and_then(|index| self.queue.get(index))
    }

    fn close_output(&mut self) {
        self.player = None;
        self.output = None;
        self.configured_source_rate = None;
        self.active_device_id = None;
        self.active_device_label = None;
        self.using_device_fallback = false;
        self.audio_message = None;
    }

    fn ensure_player(
        &mut self,
        force_system_default: bool,
        preferred_sample_rate: SampleRate,
    ) -> Result<(), String> {
        if self.player.is_some() && self.configured_source_rate == Some(preferred_sample_rate) {
            return Ok(());
        }
        self.close_output();
        let mut selected = audio_settings::select_output_device(
            &self.audio_store.settings().output_device_id,
            force_system_default,
        )?;
        let output = match open_output_sink(&selected, &self.stream_error, preferred_sample_rate) {
            Ok(output) => output,
            Err(primary_error)
                if !selected.using_fallback
                    && self.audio_store.settings().output_device_id
                        != audio_settings::SYSTEM_DEFAULT_DEVICE_ID =>
            {
                selected = audio_settings::select_output_device(
                    &self.audio_store.settings().output_device_id,
                    true,
                )?;
                open_output_sink(&selected, &self.stream_error, preferred_sample_rate).map_err(|fallback_error| {
                    format!(
                        "Aurora could not open the selected output ({primary_error}) or the Windows default ({fallback_error})."
                    )
                })?
            }
            Err(error) => return Err(error),
        };
        let player = Player::connect_new(output.mixer());
        player.set_volume(self.volume);
        self.active_device_id = Some(selected.id);
        self.active_device_label = Some(selected.label);
        self.using_device_fallback = selected.using_fallback;
        self.audio_message = selected.message;
        self.configured_source_rate = Some(preferred_sample_rate);
        self.output = Some(output);
        self.player = Some(player);
        Ok(())
    }

    fn build_source_for_index(
        &mut self,
        index: usize,
    ) -> Result<(Box<dyn Source + Send>, ReplayGainAdjustment, SampleRate), String> {
        let (track_id, track_key) = self
            .queue
            .get(index)
            .map(|track| (track.id.clone(), track.track_key.clone()))
            .ok_or_else(|| "The playback queue no longer contains this track.".to_owned())?;
        let audio_path = catalog::resolve_audio_path(&track_id, &track_key, &self.store)?;
        let (media, byte_len) = self.media_cache.open(&audio_path)?;
        let decoder = Decoder::builder()
            .with_data(media)
            .with_byte_len(byte_len)
            .with_hint("mp3")
            .with_seekable(true)
            .with_gapless(true)
            .build()
            .map_err(|error| format!("Aurora could not decode this MP3: {error}"))?;
        let sample_rate = decoder.sample_rate();
        let gain = replay_gain::adjustment_for_path(
            &audio_path,
            self.audio_store.settings().replay_gain_mode,
        );
        Ok((Box::new(decoder.amplify(gain.linear)), gain, sample_rate))
    }

    fn load_current_on_device(
        &mut self,
        should_play: bool,
        position_seconds: f64,
        force_system_default: bool,
    ) -> Result<(), String> {
        let index = self
            .current_index
            .ok_or_else(|| "The playback queue has no current track.".to_owned())?;
        let (source, gain, sample_rate) = self.build_source_for_index(index)?;
        self.ensure_player(force_system_default, sample_rate)?;
        let volume = self.volume;
        let player = self.player.as_ref().expect("player initialized");
        player.stop();
        player.set_volume(volume);
        player.append(source);
        if position_seconds > 0.0 {
            player
                .try_seek(Duration::from_secs_f64(position_seconds))
                .map_err(|error| {
                    format!("Aurora could not restore the playback position: {error}")
                })?;
        }
        if should_play {
            player.play();
            self.status = PlaybackStatus::Playing;
        } else {
            player.pause();
            self.status = PlaybackStatus::Paused;
        }
        self.position_seconds = position_seconds.max(0.0);
        self.current_gain = gain;
        self.prepared_next = None;
        self.preparation_attempted = false;
        self.error = None;
        Ok(())
    }

    fn load_current(&mut self, should_play: bool, position_seconds: f64) -> Result<(), String> {
        self.load_current_on_device(should_play, position_seconds, false)
    }

    fn set_error(&mut self, error: String) -> String {
        if let Some(player) = &self.player {
            player.pause();
        }
        self.status = PlaybackStatus::Error;
        self.error = Some(error.clone());
        error
    }

    fn persist(&self) -> Result<(), String> {
        self.store.save(&StoredPlaybackState {
            queue: self
                .queue
                .iter()
                .map(|track| StoredQueueEntry {
                    track_id: track.id.clone(),
                    track_key: Some(track.track_key.clone()),
                    directory: Some(track.directory.clone()),
                    filename: Some(track.filename.clone()),
                })
                .collect(),
            current_index: self.current_index,
            position_seconds: self.position_seconds,
            volume: self.volume,
            shuffle: self.shuffle,
            repeat_mode: self.repeat_mode.as_stored().to_owned(),
        })
    }

    pub(crate) fn persist_for_shutdown(&mut self) -> Result<(), String> {
        self.reconcile_current_source();
        self.capture_position();
        self.observe_history();
        self.finish_history("interrupted");
        let history_result = self.history.publish_if_due(true).map(|_| ());
        self.persist()?;
        history_result
    }

    fn capture_position(&mut self) {
        if matches!(
            self.status,
            PlaybackStatus::Playing | PlaybackStatus::Paused
        ) && let Some(player) = &self.player
            && !player.empty()
        {
            self.position_seconds = player.get_pos().as_secs_f64();
        }
    }

    fn take_stream_error(&self) -> Option<String> {
        self.stream_error
            .lock()
            .ok()
            .and_then(|mut error| error.take())
    }

    fn recover_audio_output_if_needed(&mut self) {
        let Some(_stream_error) = self.take_stream_error() else {
            return;
        };
        if self.current_index.is_none() {
            return;
        }
        self.capture_position();
        self.observe_history();
        let position = self.position_seconds;
        let should_play = self.status == PlaybackStatus::Playing;
        self.close_output();
        self.prepared_next = None;
        if let Err(error) = self.load_current_on_device(should_play, position, true) {
            self.set_error(format!(
                "Aurora lost the selected audio output and could not continue on the Windows default: {error}"
            ));
        } else {
            self.reset_history_position();
        }
    }

    fn begin_history(&mut self) {
        let Some(track) = self.current_track().cloned() else {
            return;
        };
        match self.history.begin_session(&track, self.position_seconds) {
            Ok(session) => self.history_session = Some(session),
            Err(error) => self.history.record_error(error),
        }
    }

    fn observe_history(&mut self) {
        let Some(active) = self.history_session.as_mut() else {
            return;
        };
        if let Err(error) = self.history.observe_position(active, self.position_seconds) {
            self.history.record_error(error);
        }
    }

    fn reset_history_position(&mut self) {
        if let Some(active) = self.history_session.as_mut() {
            self.history
                .reset_position(active, self.position_seconds.max(0.0));
        }
    }

    fn finish_history(&mut self, outcome: &'static str) {
        let Some(active) = self.history_session.take() else {
            return;
        };
        if let Err(error) = self.history.finish_session(&active, outcome) {
            self.history.record_error(error);
        }
    }

    pub(crate) fn set_play_threshold_seconds(&mut self, value: u32) {
        if let Some(active) = self.history_session.as_mut()
            && let Err(error) = self.history.refresh_active_threshold(active, value)
        {
            self.history.record_error(error);
        }
    }

    fn choose_next_index(&self, allow_wrap: bool) -> Option<usize> {
        let current = self.current_index?;
        if self.queue.len() <= 1 {
            return allow_wrap.then_some(current);
        }
        if self.shuffle {
            let entropy = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as usize;
            let candidate = entropy % (self.queue.len() - 1);
            return Some(if candidate >= current {
                candidate + 1
            } else {
                candidate
            });
        }
        if current + 1 < self.queue.len() {
            Some(current + 1)
        } else {
            allow_wrap.then_some(0)
        }
    }

    fn intended_next_index(&self) -> Option<usize> {
        if self.repeat_mode == RepeatMode::One {
            self.current_index
        } else {
            self.choose_next_index(self.repeat_mode == RepeatMode::All)
        }
    }

    fn prepare_next_if_due(&mut self) {
        if self.prepared_next.is_some()
            || self.preparation_attempted
            || self.status != PlaybackStatus::Playing
        {
            return;
        }
        let Some(duration) = self
            .current_track()
            .and_then(|track| track.duration_seconds)
            .map(|duration| duration.max(0) as f64)
        else {
            return;
        };
        if duration <= 0.0 || duration - self.position_seconds > PRELOAD_WINDOW_SECONDS {
            return;
        }
        let Some(player) = self.player.as_ref() else {
            return;
        };
        if player.len() != 1 {
            return;
        }
        let Some(next_index) = self.intended_next_index() else {
            return;
        };
        self.preparation_attempted = true;
        let Ok((source, gain, _)) = self.build_source_for_index(next_index) else {
            return;
        };
        let Some(player) = self.player.as_ref() else {
            return;
        };
        player.append(source);
        self.prepared_next = Some(PreparedTrack {
            index: next_index,
            gain,
        });
    }

    fn complete_prepared_transition(&mut self) {
        let Some(prepared) = self.prepared_next.take() else {
            return;
        };
        self.position_seconds = self
            .current_track()
            .and_then(|track| track.duration_seconds)
            .unwrap_or_default() as f64;
        self.observe_history();
        self.finish_history("completed");
        self.current_index = Some(prepared.index);
        self.current_gain = prepared.gain;
        self.preparation_attempted = false;
        self.position_seconds = 0.0;
        self.begin_history();
        self.capture_position();
        self.observe_history();
        let _ = self.persist();
    }

    fn reconcile_current_source(&mut self) {
        let transitioned = self.prepared_next.is_some()
            && self.player.as_ref().is_some_and(|player| player.len() <= 1);
        if transitioned {
            self.complete_prepared_transition();
        }
    }

    fn synchronize_audio_runtime(&mut self) {
        self.reconcile_current_source();
        self.recover_audio_output_if_needed();
    }

    fn invalidate_prepared_queue(&mut self) -> Result<(), String> {
        if self.prepared_next.is_none() {
            return Ok(());
        }
        self.capture_position();
        self.observe_history();
        let position = self.position_seconds;
        let should_play = self.status == PlaybackStatus::Playing;
        self.load_current(should_play, position)?;
        self.reset_history_position();
        Ok(())
    }

    fn finish_current(&mut self) {
        self.position_seconds = self
            .current_track()
            .and_then(|track| track.duration_seconds)
            .unwrap_or_default() as f64;
        self.observe_history();
        self.finish_history("completed");
        let next = self.intended_next_index();
        if let Some(next) = next {
            self.current_index = Some(next);
            if let Err(error) = self.load_current(true, 0.0) {
                self.set_error(error);
            } else {
                self.begin_history();
            }
        } else {
            self.status = PlaybackStatus::Stopped;
            self.position_seconds = self
                .current_track()
                .and_then(|track| track.duration_seconds)
                .unwrap_or_default() as f64;
        }
        let _ = self.persist();
    }

    pub(crate) fn snapshot(&mut self) -> PlaybackSnapshot {
        self.synchronize_audio_runtime();
        if self.status == PlaybackStatus::Playing {
            let ended = self.player.as_ref().is_none_or(Player::empty);
            if ended {
                self.finish_current();
            } else {
                self.capture_position();
                self.observe_history();
                self.prepare_next_if_due();
            }
        }
        let bucket = (self.position_seconds / 10.0).floor() as u64;
        if bucket != self.last_saved_position_bucket {
            self.last_saved_position_bucket = bucket;
            let _ = self.persist();
        }
        PlaybackSnapshot {
            queue: self.queue.clone(),
            current_index: self.current_index,
            current_track: self.current_track().cloned(),
            status: self.status,
            position_seconds: self.position_seconds,
            volume: self.volume,
            shuffle: self.shuffle,
            repeat_mode: self.repeat_mode,
            error: self.error.clone(),
            output_device_label: self.active_device_label.clone(),
            using_device_fallback: self.using_device_fallback,
            replay_gain_mode: self.audio_store.settings().replay_gain_mode,
            replay_gain_db: self.current_gain.applied_db,
            replay_gain_source: self.current_gain.source,
            clipping_prevented: self.current_gain.clipping_prevented,
        }
    }

    pub(crate) fn replace_queue(
        &mut self,
        track_references: Vec<TrackReference>,
        start_track_key: String,
    ) -> Result<PlaybackSnapshot, String> {
        self.synchronize_audio_runtime();
        let start_index = track_references
            .iter()
            .position(|reference| reference.track_key == start_track_key)
            .ok_or_else(|| "The selected track is not part of this queue.".to_owned())?;
        let queue = catalog::load_tracks_by_ids(&track_references, &self.store)?;
        self.capture_position();
        self.observe_history();
        self.finish_history("skipped");
        self.queue = queue;
        self.current_index = Some(start_index);
        self.position_seconds = 0.0;
        if let Err(error) = self.load_current(true, 0.0) {
            let error = self.set_error(error);
            let _ = self.persist();
            return Err(error);
        }
        self.begin_history();
        self.persist()?;
        Ok(self.snapshot())
    }

    pub(crate) fn append_queue(
        &mut self,
        track_references: Vec<TrackReference>,
    ) -> Result<PlaybackSnapshot, String> {
        if track_references.is_empty() || track_references.len() > MAX_QUEUE_APPEND_BATCH {
            return Err(format!(
                "Queue refill batches must contain between 1 and {MAX_QUEUE_APPEND_BATCH} tracks."
            ));
        }
        self.synchronize_audio_runtime();
        let mut current_index = self
            .current_index
            .ok_or_else(|| "Choose a track before extending its queue.".to_owned())?;
        let additions = catalog::load_tracks_by_ids(&track_references, &self.store)?;
        append_queue_entries(
            &mut self.queue,
            &mut current_index,
            &mut self.prepared_next,
            additions,
        );
        self.current_index = Some(current_index);
        self.persist()?;
        Ok(self.snapshot())
    }

    pub(crate) fn rebind_catalog(&mut self) -> Result<PlaybackCatalogRebind, String> {
        self.synchronize_audio_runtime();
        if self.queue.is_empty() {
            return Ok(PlaybackCatalogRebind {
                playback: self.snapshot(),
                catalog_revision: catalog::completed_import_revision()?,
            });
        }

        let references = self
            .queue
            .iter()
            .map(|track| StoredQueueEntry {
                track_id: track.id.clone(),
                track_key: Some(track.track_key.clone()),
                directory: Some(track.directory.clone()),
                filename: Some(track.filename.clone()),
            })
            .collect::<Vec<_>>();
        let (refreshed_queue, _, catalog_revision) =
            catalog::load_tracks_by_references(&references, &self.store)?;
        self.reconcile_current_source();
        let plan = catalog_rebind_plan(&self.queue, self.current_index, &refreshed_queue);

        if plan.key_order_unchanged {
            self.queue = refreshed_queue;
            self.current_index = plan.current_index;
            self.persist()?;
            return Ok(PlaybackCatalogRebind {
                playback: self.snapshot(),
                catalog_revision,
            });
        }

        self.capture_position();
        self.observe_history();
        let should_play = self.status == PlaybackStatus::Playing;
        let had_prepared_track = self.prepared_next.is_some();
        self.queue = refreshed_queue;
        self.current_index = plan.current_index;
        self.prepared_next = None;
        self.preparation_attempted = false;

        if plan.current_removed {
            self.finish_history("interrupted");
            if let Some(player) = &self.player {
                player.stop();
            }
            self.position_seconds = 0.0;
            self.status = if self.current_index.is_some() {
                PlaybackStatus::Paused
            } else {
                PlaybackStatus::Stopped
            };
            self.current_gain = ReplayGainAdjustment {
                linear: 1.0,
                ..Default::default()
            };
        } else if self.current_index.is_none() {
            if let Some(player) = &self.player {
                player.stop();
            }
            self.position_seconds = 0.0;
            self.status = PlaybackStatus::Stopped;
        } else if had_prepared_track {
            let position = self
                .current_track()
                .and_then(|track| track.duration_seconds)
                .map_or(self.position_seconds.max(0.0), |duration| {
                    self.position_seconds.clamp(0.0, duration.max(0) as f64)
                });
            if let Err(error) = self.load_current(should_play, position) {
                self.set_error(error);
            } else {
                self.reset_history_position();
            }
        }

        self.persist()?;
        Ok(PlaybackCatalogRebind {
            playback: self.snapshot(),
            catalog_revision,
        })
    }

    pub(crate) fn toggle(&mut self) -> Result<PlaybackSnapshot, String> {
        self.synchronize_audio_runtime();
        if self.current_index.is_none() {
            return Err("Choose a track before starting playback.".to_owned());
        }
        if self.status == PlaybackStatus::Playing {
            self.capture_position();
            self.observe_history();
            if let Some(player) = &self.player {
                player.pause();
            }
            self.status = PlaybackStatus::Paused;
        } else if self.player.as_ref().is_some_and(|player| !player.empty()) {
            self.player.as_ref().expect("player exists").play();
            self.status = PlaybackStatus::Playing;
            self.error = None;
            if self.history_session.is_none() {
                self.begin_history();
            } else {
                self.reset_history_position();
            }
        } else if let Err(error) = self.load_current(
            true,
            resume_position(
                self.status,
                self.position_seconds,
                self.current_track()
                    .and_then(|track| track.duration_seconds),
            ),
        ) {
            let error = self.set_error(error);
            let _ = self.persist();
            return Err(error);
        } else {
            self.begin_history();
        }
        self.persist()?;
        Ok(self.snapshot())
    }

    pub(crate) fn next(&mut self) -> Result<PlaybackSnapshot, String> {
        self.synchronize_audio_runtime();
        let next = self
            .choose_next_index(self.repeat_mode == RepeatMode::All)
            .ok_or_else(|| "There is no next track in the queue.".to_owned())?;
        self.capture_position();
        self.observe_history();
        self.finish_history("skipped");
        self.current_index = Some(next);
        if let Err(error) = self.load_current(true, 0.0) {
            let error = self.set_error(error);
            return Err(error);
        }
        self.begin_history();
        self.persist()?;
        Ok(self.snapshot())
    }

    pub(crate) fn previous(&mut self) -> Result<PlaybackSnapshot, String> {
        self.synchronize_audio_runtime();
        self.capture_position();
        self.observe_history();
        if self.position_seconds > 3.0 {
            return self.seek(0.0);
        }
        let current = self
            .current_index
            .ok_or_else(|| "The queue has no previous track.".to_owned())?;
        let previous = if current > 0 {
            current - 1
        } else if self.repeat_mode == RepeatMode::All && !self.queue.is_empty() {
            self.queue.len() - 1
        } else {
            0
        };
        self.finish_history("skipped");
        self.current_index = Some(previous);
        if let Err(error) = self.load_current(true, 0.0) {
            let error = self.set_error(error);
            return Err(error);
        }
        self.begin_history();
        self.persist()?;
        Ok(self.snapshot())
    }

    pub(crate) fn seek(&mut self, position_seconds: f64) -> Result<PlaybackSnapshot, String> {
        self.synchronize_audio_runtime();
        let duration = self
            .current_track()
            .and_then(|track| track.duration_seconds)
            .unwrap_or_default() as f64;
        let target = position_seconds.clamp(0.0, duration.max(0.0));
        let was_playing = self.status == PlaybackStatus::Playing;
        if was_playing {
            self.capture_position();
            self.observe_history();
        }
        if self.player.as_ref().is_none_or(Player::empty) {
            self.load_current(was_playing, target)
                .map_err(|error| self.set_error(error))?;
        } else {
            self.player
                .as_ref()
                .expect("player exists")
                .try_seek(Duration::from_secs_f64(target))
                .map_err(|error| {
                    self.set_error(format!("Aurora could not seek this track: {error}"))
                })?;
            self.position_seconds = target;
        }
        self.position_seconds = target;
        self.reset_history_position();
        self.persist()?;
        Ok(self.snapshot())
    }

    pub(crate) fn set_volume(&mut self, volume: f32) -> Result<PlaybackSnapshot, String> {
        self.synchronize_audio_runtime();
        if !volume.is_finite() {
            return Err("Volume must be a finite value.".to_owned());
        }
        self.volume = volume.clamp(0.0, 1.0);
        if let Some(player) = &self.player {
            player.set_volume(self.volume);
        }
        self.persist()?;
        Ok(self.snapshot())
    }

    pub(crate) fn set_shuffle(&mut self, enabled: bool) -> Result<PlaybackSnapshot, String> {
        self.synchronize_audio_runtime();
        self.invalidate_prepared_queue()
            .map_err(|error| self.set_error(error))?;
        self.shuffle = enabled;
        self.preparation_attempted = false;
        self.persist()?;
        Ok(self.snapshot())
    }

    pub(crate) fn set_repeat_mode(
        &mut self,
        repeat_mode: RepeatMode,
    ) -> Result<PlaybackSnapshot, String> {
        self.synchronize_audio_runtime();
        self.invalidate_prepared_queue()
            .map_err(|error| self.set_error(error))?;
        self.repeat_mode = repeat_mode;
        self.preparation_attempted = false;
        self.persist()?;
        Ok(self.snapshot())
    }

    pub(crate) fn remove_queue_item(&mut self, index: usize) -> Result<PlaybackSnapshot, String> {
        self.synchronize_audio_runtime();
        if index >= self.queue.len() {
            return Err("This queue item no longer exists.".to_owned());
        }
        self.invalidate_prepared_queue()
            .map_err(|error| self.set_error(error))?;
        self.capture_position();
        self.observe_history();
        let was_playing = self.status == PlaybackStatus::Playing;
        let current = self.current_index;
        if current == Some(index) {
            self.finish_history("skipped");
        }
        self.queue.remove(index);
        self.preparation_attempted = false;
        if self.queue.is_empty() {
            self.close_output();
            self.current_index = None;
            self.position_seconds = 0.0;
            self.status = PlaybackStatus::Stopped;
            self.current_gain = ReplayGainAdjustment {
                linear: 1.0,
                ..Default::default()
            };
        } else if current == Some(index) {
            self.current_index = Some(index.min(self.queue.len() - 1));
            self.load_current(was_playing, 0.0)
                .map_err(|error| self.set_error(error))?;
            if was_playing {
                self.begin_history();
            }
        } else if current.is_some_and(|current| index < current) {
            self.current_index = current.map(|current| current - 1);
        }
        self.persist()?;
        Ok(self.snapshot())
    }

    pub(crate) fn move_queue_item(
        &mut self,
        from: usize,
        to: usize,
    ) -> Result<PlaybackSnapshot, String> {
        self.synchronize_audio_runtime();
        if from >= self.queue.len() || to >= self.queue.len() {
            return Err("The queue changed before this reorder completed.".to_owned());
        }
        self.invalidate_prepared_queue()
            .map_err(|error| self.set_error(error))?;
        if from != to {
            let current_id = self.current_track().map(|track| track.id.clone());
            let track = self.queue.remove(from);
            self.queue.insert(to, track);
            self.current_index = current_id
                .as_ref()
                .and_then(|id| self.queue.iter().position(|track| &track.id == id));
        }
        self.preparation_attempted = false;
        self.persist()?;
        Ok(self.snapshot())
    }

    pub(crate) fn clear_queue(&mut self) -> Result<PlaybackSnapshot, String> {
        self.synchronize_audio_runtime();
        self.capture_position();
        self.observe_history();
        self.finish_history("skipped");
        self.close_output();
        self.queue.clear();
        self.current_index = None;
        self.position_seconds = 0.0;
        self.status = PlaybackStatus::Stopped;
        self.error = None;
        self.prepared_next = None;
        self.preparation_attempted = false;
        self.current_gain = ReplayGainAdjustment {
            linear: 1.0,
            ..Default::default()
        };
        self.persist()?;
        Ok(self.snapshot())
    }

    pub(crate) fn audio_settings_status(&self) -> AudioSettingsStatus {
        match audio_settings::discover_output_devices() {
            Ok(devices) => {
                let device_infos = devices
                    .into_iter()
                    .map(|device| device.info)
                    .collect::<Vec<_>>();
                let requested = &self.audio_store.settings().output_device_id;
                let preference_missing = requested != audio_settings::SYSTEM_DEFAULT_DEVICE_ID
                    && !device_infos.iter().any(|device| &device.id == requested);
                AudioSettingsStatus {
                    settings: self.audio_store.settings().clone(),
                    devices: device_infos,
                    active_device_id: self.active_device_id.clone(),
                    active_device_label: self.active_device_label.clone(),
                    using_fallback: self.using_device_fallback || preference_missing,
                    message: self
                        .audio_message
                        .clone()
                        .or_else(|| {
                            preference_missing.then(|| {
                                "The selected output is unavailable. Aurora will use the Windows default when playback starts."
                                    .to_owned()
                            })
                        })
                        .or_else(|| self.audio_store.warning().map(str::to_owned)),
                    error: None,
                }
            }
            Err(error) => AudioSettingsStatus {
                settings: self.audio_store.settings().clone(),
                devices: Vec::new(),
                active_device_id: self.active_device_id.clone(),
                active_device_label: self.active_device_label.clone(),
                using_fallback: self.using_device_fallback,
                message: self
                    .audio_message
                    .clone()
                    .or_else(|| self.audio_store.warning().map(str::to_owned)),
                error: Some(error),
            },
        }
    }

    pub(crate) fn update_audio_settings(
        &mut self,
        request: AudioSettingsRequest,
    ) -> Result<AudioSettingsStatus, String> {
        self.reconcile_current_source();
        self.capture_position();
        self.observe_history();
        let position = self.position_seconds;
        let should_play = self.status == PlaybackStatus::Playing;
        let had_player = self.player.is_some();
        self.audio_store.update(request)?;
        self.close_output();
        let _ = self.take_stream_error();
        self.prepared_next = None;
        self.preparation_attempted = false;
        if had_player && self.current_index.is_some() {
            self.load_current(should_play, position)
                .map_err(|error| self.set_error(error))?;
            self.reset_history_position();
        } else {
            self.current_gain = ReplayGainAdjustment {
                linear: 1.0,
                ..Default::default()
            };
        }
        Ok(self.audio_settings_status())
    }

    pub(crate) fn refresh_track_metadata(&mut self, updated: &TrackSummary) {
        for track in &mut self.queue {
            if track.track_key == updated.track_key {
                track.apply_tag_projection(updated);
            }
        }
    }
}

fn stability_buffer_frames(sample_rate: u32) -> u32 {
    let target = (u64::from(sample_rate) * u64::from(OUTPUT_BUFFER_TARGET_MS) / 1_000)
        .clamp(1, u64::from(MAX_OUTPUT_BUFFER_FRAMES)) as u32;
    let upper = target.next_power_of_two();
    let lower = (upper / 2).max(1);
    let nearest = if target - lower <= upper - target {
        lower
    } else {
        upper
    };
    nearest.clamp(MIN_OUTPUT_BUFFER_FRAMES, MAX_OUTPUT_BUFFER_FRAMES)
}

fn open_output_sink(
    selected: &audio_settings::SelectedOutputDevice,
    stream_error: &Arc<Mutex<Option<String>>>,
    preferred_sample_rate: SampleRate,
) -> Result<MixerDeviceSink, String> {
    let callback = {
        let stream_error = Arc::clone(stream_error);
        move |error: cpal::StreamError| {
            if let Ok(mut slot) = stream_error.lock() {
                *slot = Some(error.to_string());
            }
        }
    };
    let preferred_frames = stability_buffer_frames(preferred_sample_rate.get());
    let primary = DeviceSinkBuilder::from_device(selected.device.clone())
        .map_err(|error| format!("Aurora could not configure this audio output: {error}"))?
        .with_sample_rate(preferred_sample_rate)
        .with_buffer_size(BufferSize::Fixed(preferred_frames))
        .with_error_callback(callback.clone())
        .open_stream();
    let primary_error = match primary {
        Ok(output) => return Ok(output),
        Err(error) => error,
    };

    if let Ok(configs) = rodio::stream::supported_output_configs(&selected.device) {
        let mut alternatives = configs.collect::<Vec<_>>();
        alternatives.sort_by_key(|config| config.sample_rate() != preferred_sample_rate.get());
        for config in alternatives {
            let frames = stability_buffer_frames(config.sample_rate());
            if let Ok(output) = DeviceSinkBuilder::default()
                .with_device(selected.device.clone())
                .with_supported_config(&config)
                .with_buffer_size(BufferSize::Fixed(frames))
                .with_error_callback(callback.clone())
                .open_stream()
            {
                return Ok(output);
            }
        }
    }

    DeviceSinkBuilder::from_device(selected.device.clone())
        .map_err(|error| format!("Aurora could not configure this audio output: {error}"))?
        .with_error_callback(callback)
        .open_sink_or_fallback()
        .map_err(|fallback_error| {
            format!(
                "Aurora could not open this audio output with its stable buffer ({primary_error}) or its compatibility fallback ({fallback_error})."
            )
        })
}

fn resume_position(
    status: PlaybackStatus,
    position_seconds: f64,
    duration_seconds: Option<i64>,
) -> f64 {
    let duration = duration_seconds.unwrap_or_default().max(0) as f64;
    if status == PlaybackStatus::Stopped && duration > 0.0 && position_seconds >= duration - 0.25 {
        0.0
    } else {
        position_seconds.max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queue_track(index: usize) -> TrackSummary {
        TrackSummary {
            id: index.to_string(),
            track_key: format!("d:\\music\\track-{index}.mp3"),
            album_id: Some("album".to_owned()),
            title: format!("Track {index}"),
            artist: "Artist".to_owned(),
            display_artist: None,
            album: "Album".to_owned(),
            release_year: Some(2026),
            original_year: Some(2026),
            publisher: None,
            rating: None,
            loved: false,
            love_state: crate::tag_model::LoveState::Neutral,
            tag_sync_state: None,
            can_undo_tag_edit: false,
            duration_seconds: Some(240),
            genre: Some("Synthwave".to_owned()),
            play_count: None,
            track_number: None,
            track_total: None,
            disc_number: None,
            disc_total: None,
            directory: "D:\\MUSIC".to_owned(),
            filename: format!("track-{index}.mp3"),
            catalog_import_run_id: 1,
        }
    }

    #[test]
    fn repeat_modes_round_trip_through_storage() {
        for mode in [RepeatMode::Off, RepeatMode::All, RepeatMode::One] {
            assert_eq!(RepeatMode::from_stored(mode.as_stored()), mode);
        }
    }

    #[test]
    fn completed_tracks_restart_from_the_beginning() {
        assert_eq!(
            resume_position(PlaybackStatus::Stopped, 243.0, Some(243)),
            0.0
        );
        assert_eq!(
            resume_position(PlaybackStatus::Paused, 243.0, Some(243)),
            243.0
        );
        assert_eq!(
            resume_position(PlaybackStatus::Stopped, 120.0, Some(243)),
            120.0
        );
    }

    #[test]
    fn output_buffer_policy_prefers_music_playback_stability() {
        assert_eq!(stability_buffer_frames(44_100), 4_096);
        assert_eq!(stability_buffer_frames(48_000), 4_096);
        assert_eq!(stability_buffer_frames(96_000), 8_192);
        assert_eq!(stability_buffer_frames(384_000), MAX_OUTPUT_BUFFER_FRAMES);
    }

    #[test]
    fn playback_media_cache_removes_later_disk_reads_and_invalidates_changes() {
        let directory = tempfile::tempdir().expect("temporary media cache directory");
        let path = directory.path().join("track.mp3");
        std::fs::write(&path, b"first encoded track").expect("write first track");
        let mut cache = PlaybackMediaCache::default();

        let (mut first, first_len) = cache.open(&path).expect("preload first track");
        let first_bytes = match &first {
            PlaybackMedia::Memory(reader) => Arc::clone(reader.get_ref()),
            PlaybackMedia::File(_) => panic!("small tracks should be held in memory"),
        };
        let mut decoded_input = Vec::new();
        first
            .read_to_end(&mut decoded_input)
            .expect("read cached track");
        assert_eq!(first_len, decoded_input.len() as u64);
        assert_eq!(decoded_input, b"first encoded track");
        first.seek(SeekFrom::Start(6)).expect("seek cached track");
        let mut tail = Vec::new();
        first.read_to_end(&mut tail).expect("read cached tail");
        assert_eq!(tail, b"encoded track");

        let (second, _) = cache.open(&path).expect("reuse cached track");
        let second_bytes = match second {
            PlaybackMedia::Memory(reader) => reader.into_inner(),
            PlaybackMedia::File(_) => panic!("small tracks should be held in memory"),
        };
        assert!(Arc::ptr_eq(&first_bytes, &second_bytes));

        std::thread::sleep(Duration::from_millis(2));
        std::fs::write(&path, b"other encoded track").expect("replace track at same length");
        let (replacement, _) = cache.open(&path).expect("load replacement track");
        let replacement_bytes = match replacement {
            PlaybackMedia::Memory(reader) => reader.into_inner(),
            PlaybackMedia::File(_) => panic!("small tracks should be held in memory"),
        };
        assert!(!Arc::ptr_eq(&first_bytes, &replacement_bytes));
        assert_eq!(&*replacement_bytes, b"other encoded track");

        let oversized_path = directory.path().join("oversized.mp3");
        File::create(&oversized_path)
            .and_then(|file| file.set_len(MAX_CACHED_TRACK_BYTES + 1))
            .expect("create sparse oversized track");
        let (oversized, oversized_len) = cache.open(&oversized_path).expect("open oversized track");
        assert_eq!(oversized_len, MAX_CACHED_TRACK_BYTES + 1);
        assert!(matches!(oversized, PlaybackMedia::File(_)));
    }

    #[test]
    fn queue_refill_keeps_recent_history_current_track_and_prepared_successor() {
        let mut queue = (0..MAX_PLAYBACK_QUEUE).map(queue_track).collect::<Vec<_>>();
        let mut current_index = 181;
        let mut prepared = Some(PreparedTrack {
            index: 182,
            gain: ReplayGainAdjustment::default(),
        });
        let appended = append_queue_entries(
            &mut queue,
            &mut current_index,
            &mut prepared,
            (200..300).map(queue_track).collect(),
        );
        assert_eq!(appended, 100);
        assert_eq!(queue.len(), 139);
        assert_eq!(current_index, RETAINED_QUEUE_HISTORY);
        assert_eq!(queue[current_index].id, "181");
        assert_eq!(prepared.as_ref().map(|next| next.index), Some(21));
    }

    #[test]
    fn queue_refill_deduplicates_track_identity() {
        let mut queue = vec![queue_track(1)];
        let mut current_index = 0;
        let mut prepared = None;
        let appended = append_queue_entries(
            &mut queue,
            &mut current_index,
            &mut prepared,
            vec![queue_track(1), queue_track(2)],
        );
        assert_eq!(appended, 1);
        assert_eq!(queue.len(), 2);
        assert_eq!(queue[1].id, "2");
    }

    #[test]
    fn catalog_rebind_preserves_current_index_when_stable_order_is_unchanged() {
        let current = (0..3).map(queue_track).collect::<Vec<_>>();
        let mut refreshed = current.clone();
        for (index, track) in refreshed.iter_mut().enumerate() {
            track.id = format!("fresh-{index}");
            track.catalog_import_run_id = 2;
        }

        assert_eq!(
            catalog_rebind_plan(&current, Some(1), &refreshed),
            CatalogRebindPlan {
                key_order_unchanged: true,
                current_index: Some(1),
                current_removed: false,
            }
        );
    }

    #[test]
    fn catalog_rebind_remaps_survivors_and_selects_the_next_track_if_current_was_removed() {
        let current = (0..4).map(queue_track).collect::<Vec<_>>();
        let refreshed = vec![queue_track(1), queue_track(3)];

        assert_eq!(
            catalog_rebind_plan(&current, Some(1), &refreshed),
            CatalogRebindPlan {
                key_order_unchanged: false,
                current_index: Some(0),
                current_removed: false,
            }
        );
        assert_eq!(
            catalog_rebind_plan(&current, Some(2), &refreshed),
            CatalogRebindPlan {
                key_order_unchanged: false,
                current_index: Some(1),
                current_removed: true,
            }
        );
    }

    #[test]
    #[ignore = "requires an interactive Windows audio session"]
    fn opens_the_windows_default_audio_output() {
        let selected =
            audio_settings::select_output_device(audio_settings::SYSTEM_DEFAULT_DEVICE_ID, false)
                .expect("select Windows default output");
        let stream_error = Arc::new(Mutex::new(None));
        let preferred_sample_rate = SampleRate::new(44_100).expect("valid sample rate");
        let output = open_output_sink(&selected, &stream_error, preferred_sample_rate)
            .expect("open Windows output");
        eprintln!("Aurora test output config: {:?}", output.config());
        assert_eq!(output.config().sample_rate(), preferred_sample_rate);
        assert_eq!(
            output.config().buffer_size(),
            &BufferSize::Fixed(stability_buffer_frames(preferred_sample_rate.get()))
        );
    }
}
