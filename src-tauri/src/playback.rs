use crate::{
    catalog::{self, TrackSummary},
    state_store::{StateStore, StoredPlaybackState},
};
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

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
    store: StateStore,
    last_saved_position_bucket: u64,
}

impl PlaybackRuntime {
    pub(crate) fn new(state_path: PathBuf) -> Result<Self, String> {
        let store = StateStore::new(state_path)?;
        let stored = store.load()?;
        let queue_result = if stored.track_ids.is_empty() {
            Ok(Vec::new())
        } else {
            catalog::load_tracks_by_ids(&stored.track_ids)
        };
        let (queue, error) = match queue_result {
            Ok(queue) => (queue, None),
            Err(error) => (
                Vec::new(),
                Some(format!(
                    "Aurora could not restore the previous queue: {error}"
                )),
            ),
        };
        let current_index = stored.current_index.filter(|index| *index < queue.len());
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
            store,
            last_saved_position_bucket: (position_seconds / 10.0).floor() as u64,
        })
    }

    fn current_track(&self) -> Option<&TrackSummary> {
        self.current_index.and_then(|index| self.queue.get(index))
    }

    fn ensure_player(&mut self) -> Result<(), String> {
        if self.player.is_some() {
            return Ok(());
        }
        let output = DeviceSinkBuilder::open_default_sink()
            .map_err(|error| format!("Aurora could not open the default audio device: {error}"))?;
        let player = Player::connect_new(output.mixer());
        player.set_volume(self.volume);
        self.output = Some(output);
        self.player = Some(player);
        Ok(())
    }

    fn load_current(&mut self, should_play: bool, position_seconds: f64) -> Result<(), String> {
        let track = self
            .current_track()
            .cloned()
            .ok_or_else(|| "The playback queue has no current track.".to_owned())?;
        let audio_path = catalog::resolve_audio_path(&track.id)?;
        let file = File::open(&audio_path)
            .map_err(|error| format!("Aurora could not open this MP3: {error}"))?;
        let byte_len = file
            .metadata()
            .map_err(|error| format!("Aurora could not inspect this MP3: {error}"))?
            .len();
        let decoder = Decoder::builder()
            .with_data(file)
            .with_byte_len(byte_len)
            .with_hint("mp3")
            .with_seekable(true)
            .build()
            .map_err(|error| format!("Aurora could not decode this MP3: {error}"))?;

        self.ensure_player()?;
        let volume = self.volume;
        let player = self.player.as_ref().expect("player initialized");
        player.stop();
        player.set_volume(volume);
        player.append(decoder);
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
        self.error = None;
        Ok(())
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
            track_ids: self.queue.iter().map(|track| track.id.clone()).collect(),
            current_index: self.current_index,
            position_seconds: self.position_seconds,
            volume: self.volume,
            shuffle: self.shuffle,
            repeat_mode: self.repeat_mode.as_stored().to_owned(),
        })
    }

    pub(crate) fn persist_for_shutdown(&mut self) -> Result<(), String> {
        self.capture_position();
        self.persist()
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

    fn finish_current(&mut self) {
        let next = if self.repeat_mode == RepeatMode::One {
            self.current_index
        } else {
            self.choose_next_index(self.repeat_mode == RepeatMode::All)
        };
        if let Some(next) = next {
            self.current_index = Some(next);
            if let Err(error) = self.load_current(true, 0.0) {
                self.set_error(error);
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
        if self.status == PlaybackStatus::Playing {
            let ended = self.player.as_ref().is_none_or(Player::empty);
            if ended {
                self.finish_current();
            } else {
                self.capture_position();
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
        }
    }

    pub(crate) fn replace_queue(
        &mut self,
        track_ids: Vec<String>,
        start_track_id: String,
    ) -> Result<PlaybackSnapshot, String> {
        let start_index = track_ids
            .iter()
            .position(|track_id| track_id == &start_track_id)
            .ok_or_else(|| "The selected track is not part of this queue.".to_owned())?;
        let queue = catalog::load_tracks_by_ids(&track_ids)?;
        self.queue = queue;
        self.current_index = Some(start_index);
        self.position_seconds = 0.0;
        if let Err(error) = self.load_current(true, 0.0) {
            let error = self.set_error(error);
            let _ = self.persist();
            return Err(error);
        }
        self.persist()?;
        Ok(self.snapshot())
    }

    pub(crate) fn toggle(&mut self) -> Result<PlaybackSnapshot, String> {
        if self.current_index.is_none() {
            return Err("Choose a track before starting playback.".to_owned());
        }
        if self.status == PlaybackStatus::Playing {
            self.capture_position();
            if let Some(player) = &self.player {
                player.pause();
            }
            self.status = PlaybackStatus::Paused;
        } else if self.player.as_ref().is_some_and(|player| !player.empty()) {
            self.player.as_ref().expect("player exists").play();
            self.status = PlaybackStatus::Playing;
            self.error = None;
        } else if let Err(error) = self.load_current(true, self.position_seconds) {
            let error = self.set_error(error);
            let _ = self.persist();
            return Err(error);
        }
        self.persist()?;
        Ok(self.snapshot())
    }

    pub(crate) fn next(&mut self) -> Result<PlaybackSnapshot, String> {
        let next = self
            .choose_next_index(self.repeat_mode == RepeatMode::All)
            .ok_or_else(|| "There is no next track in the queue.".to_owned())?;
        self.current_index = Some(next);
        if let Err(error) = self.load_current(true, 0.0) {
            let error = self.set_error(error);
            return Err(error);
        }
        self.persist()?;
        Ok(self.snapshot())
    }

    pub(crate) fn previous(&mut self) -> Result<PlaybackSnapshot, String> {
        self.capture_position();
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
        self.current_index = Some(previous);
        if let Err(error) = self.load_current(true, 0.0) {
            let error = self.set_error(error);
            return Err(error);
        }
        self.persist()?;
        Ok(self.snapshot())
    }

    pub(crate) fn seek(&mut self, position_seconds: f64) -> Result<PlaybackSnapshot, String> {
        let duration = self
            .current_track()
            .and_then(|track| track.duration_seconds)
            .unwrap_or_default() as f64;
        let target = position_seconds.clamp(0.0, duration.max(0.0));
        let was_playing = self.status == PlaybackStatus::Playing;
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
        self.persist()?;
        Ok(self.snapshot())
    }

    pub(crate) fn set_volume(&mut self, volume: f32) -> Result<PlaybackSnapshot, String> {
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
        self.shuffle = enabled;
        self.persist()?;
        Ok(self.snapshot())
    }

    pub(crate) fn set_repeat_mode(
        &mut self,
        repeat_mode: RepeatMode,
    ) -> Result<PlaybackSnapshot, String> {
        self.repeat_mode = repeat_mode;
        self.persist()?;
        Ok(self.snapshot())
    }

    pub(crate) fn remove_queue_item(&mut self, index: usize) -> Result<PlaybackSnapshot, String> {
        if index >= self.queue.len() {
            return Err("This queue item no longer exists.".to_owned());
        }
        self.capture_position();
        let was_playing = self.status == PlaybackStatus::Playing;
        let current = self.current_index;
        self.queue.remove(index);
        if self.queue.is_empty() {
            if let Some(player) = &self.player {
                player.stop();
            }
            self.current_index = None;
            self.position_seconds = 0.0;
            self.status = PlaybackStatus::Stopped;
        } else if current == Some(index) {
            self.current_index = Some(index.min(self.queue.len() - 1));
            self.load_current(was_playing, 0.0)
                .map_err(|error| self.set_error(error))?;
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
        if from >= self.queue.len() || to >= self.queue.len() {
            return Err("The queue changed before this reorder completed.".to_owned());
        }
        if from != to {
            let current_id = self.current_track().map(|track| track.id.clone());
            let track = self.queue.remove(from);
            self.queue.insert(to, track);
            self.current_index = current_id
                .as_ref()
                .and_then(|id| self.queue.iter().position(|track| &track.id == id));
        }
        self.persist()?;
        Ok(self.snapshot())
    }

    pub(crate) fn clear_queue(&mut self) -> Result<PlaybackSnapshot, String> {
        if let Some(player) = &self.player {
            player.stop();
        }
        self.queue.clear();
        self.current_index = None;
        self.position_seconds = 0.0;
        self.status = PlaybackStatus::Stopped;
        self.error = None;
        self.persist()?;
        Ok(self.snapshot())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeat_modes_round_trip_through_storage() {
        for mode in [RepeatMode::Off, RepeatMode::All, RepeatMode::One] {
            assert_eq!(RepeatMode::from_stored(mode.as_stored()), mode);
        }
    }
}
