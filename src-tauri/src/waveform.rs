use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use std::{
    fs::{self, File},
    future::Future,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::DecoderOptions,
    errors::Error as SymphoniaError,
    formats::{FormatOptions, SeekMode, SeekTo},
    io::{MediaSource, MediaSourceStream},
    meta::MetadataOptions,
    probe::Hint,
    units::Time,
};

pub(crate) const PEAK_COUNT: usize = 320;
const MAX_CACHE_ENTRIES: i64 = 2_000;
const SAMPLE_WINDOWS: usize = 64;
const BINS_PER_WINDOW: usize = PEAK_COUNT / SAMPLE_WINDOWS;
const MAX_BUFFERED_MP3_BYTES: u64 = 96 * 1024 * 1024;
const MP3_READ_BUFFER_BYTES: usize = 1024 * 1024;
const SUPERSEDED_WORK_ERROR: &str = "Aurora stopped an outdated waveform request.";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WaveformWorkGeneration(u64);

pub(crate) struct WaveformWorkCoordinator {
    latest_generation: AtomicU64,
    decode_slot: tauri::async_runtime::Mutex<()>,
}

impl Default for WaveformWorkCoordinator {
    fn default() -> Self {
        Self {
            latest_generation: AtomicU64::new(0),
            decode_slot: tauri::async_runtime::Mutex::new(()),
        }
    }
}

pub(crate) struct WaveformCancellation<'a> {
    coordinator: &'a WaveformWorkCoordinator,
    generation: WaveformWorkGeneration,
}

impl WaveformCancellation<'_> {
    pub(crate) fn is_cancelled(&self) -> bool {
        self.coordinator.latest_generation.load(Ordering::Acquire) != self.generation.0
    }

    pub(crate) fn checkpoint(&self) -> Result<(), String> {
        if self.is_cancelled() {
            Err(SUPERSEDED_WORK_ERROR.to_owned())
        } else {
            Ok(())
        }
    }
}

impl WaveformWorkCoordinator {
    pub(crate) fn begin(&self) -> WaveformWorkGeneration {
        let generation = self
            .latest_generation
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1);
        WaveformWorkGeneration(generation)
    }

    pub(crate) fn checkpoint(&self, generation: WaveformWorkGeneration) -> Result<(), String> {
        WaveformCancellation {
            coordinator: self,
            generation,
        }
        .checkpoint()
    }

    pub(crate) fn cancellation(
        &self,
        generation: WaveformWorkGeneration,
    ) -> Result<WaveformCancellation<'_>, String> {
        let cancellation = WaveformCancellation {
            coordinator: self,
            generation,
        };
        cancellation.checkpoint()?;
        Ok(cancellation)
    }

    pub(crate) async fn run_serialized<T, F, Fut>(
        &self,
        generation: WaveformWorkGeneration,
        operation: F,
    ) -> Result<T, String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, String>>,
    {
        let _slot = self.decode_slot.lock().await;
        self.checkpoint(generation)?;
        let result = operation().await?;
        self.checkpoint(generation)?;
        Ok(result)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WaveformSnapshot {
    pub(crate) track_key: String,
    pub(crate) peaks: Vec<f32>,
    pub(crate) sample_rate: Option<u32>,
    pub(crate) channels: Option<u16>,
    pub(crate) source: WaveformSource,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum WaveformSource {
    Decoded,
    Cache,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FileSignature {
    size: u64,
    modified_ns: u128,
}

impl FileSignature {
    pub(crate) fn read(path: &Path) -> Result<Self, String> {
        let metadata = fs::metadata(path)
            .map_err(|error| format!("Could not inspect the MP3 for its waveform: {error}"))?;
        let modified_ns = metadata
            .modified()
            .unwrap_or(SystemTime::UNIX_EPOCH)
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_nanos();
        Ok(Self {
            size: metadata.len(),
            modified_ns,
        })
    }

    pub(crate) fn verify_unchanged(self, path: &Path) -> Result<(), String> {
        if Self::read(path)? != self {
            return Err(
                "Aurora did not cache the waveform because the MP3 changed during decoding."
                    .to_owned(),
            );
        }
        Ok(())
    }
}

pub(crate) struct WaveformStore {
    path: PathBuf,
}

impl WaveformStore {
    pub(crate) fn new(path: PathBuf) -> Result<Self, String> {
        let parent = path
            .parent()
            .ok_or_else(|| "Aurora's waveform cache path has no parent directory.".to_owned())?;
        fs::create_dir_all(parent).map_err(|error| {
            format!("Could not create Aurora's waveform cache directory: {error}")
        })?;
        let store = Self { path };
        store.migrate()?;
        Ok(store)
    }

    fn open(&self) -> Result<Connection, String> {
        let connection = Connection::open(&self.path)
            .map_err(|error| format!("Could not open Aurora's waveform cache: {error}"))?;
        connection
            .busy_timeout(Duration::from_secs(3))
            .map_err(|error| format!("Could not configure Aurora's waveform cache: {error}"))?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|error| format!("Could not enable waveform cache journaling: {error}"))?;
        Ok(connection)
    }

    fn migrate(&self) -> Result<(), String> {
        self.open()?
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS waveform_cache (
                    track_key TEXT PRIMARY KEY,
                    file_size INTEGER NOT NULL,
                    modified_ns TEXT NOT NULL,
                    sample_rate INTEGER,
                    channels INTEGER,
                    peaks_json TEXT NOT NULL,
                    accessed_at_ms INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_waveform_cache_accessed
                    ON waveform_cache(accessed_at_ms);
                "#,
            )
            .map_err(|error| format!("Could not prepare Aurora's waveform cache: {error}"))?;
        Ok(())
    }

    pub(crate) fn load(
        &self,
        track_key: &str,
        signature: FileSignature,
    ) -> Result<Option<WaveformSnapshot>, String> {
        let connection = self.open()?;
        let cached = connection
            .query_row(
                r#"
                SELECT sample_rate, channels, peaks_json
                FROM waveform_cache
                WHERE track_key = ?1 AND file_size = ?2 AND modified_ns = ?3
                "#,
                params![
                    track_key,
                    i64::try_from(signature.size).unwrap_or(i64::MAX),
                    signature.modified_ns.to_string()
                ],
                |row| {
                    Ok((
                        row.get::<_, Option<u32>>(0)?,
                        row.get::<_, Option<u16>>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| format!("Could not read Aurora's waveform cache: {error}"))?;

        let Some((sample_rate, channels, peaks_json)) = cached else {
            return Ok(None);
        };
        let peaks = match serde_json::from_str::<Vec<f32>>(&peaks_json) {
            Ok(peaks) => peaks,
            Err(_) => {
                connection
                    .execute(
                        "DELETE FROM waveform_cache WHERE track_key = ?1",
                        [track_key],
                    )
                    .map_err(|error| {
                        format!("Could not discard an unreadable waveform cache row: {error}")
                    })?;
                return Ok(None);
            }
        };
        if !valid_peaks(&peaks) {
            connection
                .execute(
                    "DELETE FROM waveform_cache WHERE track_key = ?1",
                    [track_key],
                )
                .map_err(|error| {
                    format!("Could not discard an invalid waveform cache row: {error}")
                })?;
            return Ok(None);
        }
        connection
            .execute(
                "UPDATE waveform_cache SET accessed_at_ms = ?2 WHERE track_key = ?1",
                params![track_key, now_ms()],
            )
            .map_err(|error| format!("Could not refresh Aurora's waveform cache entry: {error}"))?;
        Ok(Some(WaveformSnapshot {
            track_key: track_key.to_owned(),
            peaks,
            sample_rate,
            channels,
            source: WaveformSource::Cache,
        }))
    }

    pub(crate) fn save(
        &self,
        snapshot: &WaveformSnapshot,
        signature: FileSignature,
    ) -> Result<(), String> {
        if !valid_peaks(&snapshot.peaks) {
            return Err("Aurora refused to cache an invalid waveform.".to_owned());
        }
        let peaks_json = serde_json::to_string(&snapshot.peaks)
            .map_err(|error| format!("Could not serialize the waveform cache: {error}"))?;
        let connection = self.open()?;
        connection
            .execute(
                r#"
                INSERT INTO waveform_cache (
                    track_key, file_size, modified_ns, sample_rate, channels, peaks_json, accessed_at_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ON CONFLICT(track_key) DO UPDATE SET
                    file_size = excluded.file_size,
                    modified_ns = excluded.modified_ns,
                    sample_rate = excluded.sample_rate,
                    channels = excluded.channels,
                    peaks_json = excluded.peaks_json,
                    accessed_at_ms = excluded.accessed_at_ms
                "#,
                params![
                    snapshot.track_key,
                    i64::try_from(signature.size).unwrap_or(i64::MAX),
                    signature.modified_ns.to_string(),
                    snapshot.sample_rate,
                    snapshot.channels,
                    peaks_json,
                    now_ms(),
                ],
            )
            .map_err(|error| format!("Could not cache the decoded waveform: {error}"))?;
        connection
            .execute(
                r#"
                DELETE FROM waveform_cache
                WHERE track_key IN (
                    SELECT track_key FROM waveform_cache
                    ORDER BY accessed_at_ms DESC
                    LIMIT -1 OFFSET ?1
                )
                "#,
                [MAX_CACHE_ENTRIES],
            )
            .map_err(|error| format!("Could not trim Aurora's waveform cache: {error}"))?;
        Ok(())
    }
}

enum WaveformMedia {
    Buffered(Cursor<Vec<u8>>),
    File(File),
}

impl WaveformMedia {
    fn into_source(self) -> Box<dyn MediaSource> {
        match self {
            Self::Buffered(cursor) => Box::new(cursor),
            Self::File(file) => Box::new(file),
        }
    }
}

fn open_waveform_media(
    path: &Path,
    max_buffered_bytes: u64,
    cancellation: &WaveformCancellation<'_>,
) -> Result<WaveformMedia, String> {
    cancellation.checkpoint()?;
    let mut file = File::open(path)
        .map_err(|error| format!("Could not open the MP3 for waveform decoding: {error}"))?;
    let byte_len = file
        .metadata()
        .map_err(|error| format!("Could not inspect the MP3 for waveform decoding: {error}"))?
        .len();
    cancellation.checkpoint()?;

    if byte_len > max_buffered_bytes {
        return Ok(WaveformMedia::File(file));
    }
    let expected_len = usize::try_from(byte_len)
        .map_err(|_| "The MP3 is too large to buffer safely for waveform decoding.".to_owned())?;
    let mut encoded = Vec::new();
    if encoded.try_reserve_exact(expected_len).is_err() {
        return Ok(WaveformMedia::File(file));
    }

    let mut chunk = vec![0_u8; MP3_READ_BUFFER_BYTES];
    loop {
        cancellation.checkpoint()?;
        let read = file
            .read(&mut chunk)
            .map_err(|error| format!("Could not preload the MP3 for waveform decoding: {error}"))?;
        if read == 0 {
            break;
        }
        if encoded.len().saturating_add(read) > expected_len {
            return Err("The MP3 changed while Aurora was preparing its waveform.".to_owned());
        }
        encoded.extend_from_slice(&chunk[..read]);
    }
    if encoded.len() != expected_len {
        return Err("The MP3 changed while Aurora was preparing its waveform.".to_owned());
    }
    cancellation.checkpoint()?;
    Ok(WaveformMedia::Buffered(Cursor::new(encoded)))
}

pub(crate) fn decode_mp3_waveform(
    path: &Path,
    track_key: &str,
    duration_seconds: Option<i64>,
    cancellation: &WaveformCancellation<'_>,
) -> Result<WaveformSnapshot, String> {
    let media = open_waveform_media(path, MAX_BUFFERED_MP3_BYTES, cancellation)?;
    let source = MediaSourceStream::new(media.into_source(), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            source,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|error| format!("Could not recognize the MP3 stream: {error}"))?;
    cancellation.checkpoint()?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| "The MP3 does not contain a playable audio stream.".to_owned())?;
    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let sample_rate = codec_params.sample_rate;
    let channels = codec_params
        .channels
        .and_then(|channels| u16::try_from(channels.count()).ok());
    let estimated_frames = codec_params.n_frames.or_else(|| {
        sample_rate
            .zip(duration_seconds.and_then(|seconds| u64::try_from(seconds).ok()))
            .map(|(rate, seconds)| u64::from(rate).saturating_mul(seconds))
    });
    let stream_duration = codec_params
        .n_frames
        .zip(sample_rate)
        .map(|(frames, rate)| frames as f64 / f64::from(rate))
        .or_else(|| duration_seconds.map(|seconds| seconds as f64));
    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|error| format!("Could not create the MP3 waveform decoder: {error}"))?;
    cancellation.checkpoint()?;
    let mut raw_peaks = vec![0.0_f32; PEAK_COUNT];
    let mut decoded_frames = 0_u64;
    let mut sample_buffer: Option<SampleBuffer<f32>> = None;

    if let Some(stream_duration) = stream_duration.filter(|duration| *duration > 0.0) {
        for window in 0..SAMPLE_WINDOWS {
            cancellation.checkpoint()?;
            if window > 0 {
                let target = stream_duration * window as f64 / SAMPLE_WINDOWS as f64;
                let time = Time::new(target.floor() as u64, target.fract());
                if format
                    .seek(
                        SeekMode::Coarse,
                        SeekTo::Time {
                            time,
                            track_id: Some(track_id),
                        },
                    )
                    .is_err()
                {
                    continue;
                }
                decoder.reset();
            }

            let mut attempts = 0;
            while attempts < 6 {
                cancellation.checkpoint()?;
                attempts += 1;
                let packet = match format.next_packet() {
                    Ok(packet) => packet,
                    Err(_) => break,
                };
                if packet.track_id() != track_id {
                    continue;
                }
                let decoded = match decoder.decode(&packet) {
                    Ok(decoded) => decoded,
                    Err(SymphoniaError::DecodeError(_)) => continue,
                    Err(_) => break,
                };
                let spec = *decoded.spec();
                let channel_count = spec.channels.count().max(1);
                if sample_buffer
                    .as_ref()
                    .is_none_or(|buffer| buffer.capacity() < decoded.capacity())
                {
                    sample_buffer = Some(SampleBuffer::<f32>::new(decoded.capacity() as u64, spec));
                }
                let buffer = sample_buffer
                    .as_mut()
                    .ok_or_else(|| "Could not allocate the MP3 waveform buffer.".to_owned())?;
                buffer.copy_interleaved_ref(decoded);
                let frames = buffer.samples().len() / channel_count;
                if frames == 0 {
                    continue;
                }
                for (frame_index, frame) in buffer.samples().chunks(channel_count).enumerate() {
                    let amplitude = frame
                        .iter()
                        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
                    let local_bin =
                        (frame_index * BINS_PER_WINDOW / frames).min(BINS_PER_WINDOW - 1);
                    let bin = window * BINS_PER_WINDOW + local_bin;
                    raw_peaks[bin] = raw_peaks[bin].max(amplitude);
                    decoded_frames = decoded_frames.saturating_add(1);
                }
                break;
            }
        }
    } else {
        loop {
            cancellation.checkpoint()?;
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(SymphoniaError::IoError(error))
                    if error.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(error) => {
                    return Err(format!("Could not read the MP3 waveform stream: {error}"));
                }
            };
            if packet.track_id() != track_id {
                continue;
            }
            let decoded = match decoder.decode(&packet) {
                Ok(decoded) => decoded,
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(SymphoniaError::IoError(error))
                    if error.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(error) => return Err(format!("Could not decode the MP3 waveform: {error}")),
            };
            let spec = *decoded.spec();
            let channel_count = spec.channels.count().max(1);
            if sample_buffer
                .as_ref()
                .is_none_or(|buffer| buffer.capacity() < decoded.capacity())
            {
                sample_buffer = Some(SampleBuffer::<f32>::new(decoded.capacity() as u64, spec));
            }
            let buffer = sample_buffer
                .as_mut()
                .ok_or_else(|| "Could not allocate the MP3 waveform buffer.".to_owned())?;
            buffer.copy_interleaved_ref(decoded);
            for frame in buffer.samples().chunks(channel_count) {
                let amplitude = frame
                    .iter()
                    .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
                let total = estimated_frames
                    .unwrap_or(decoded_frames.saturating_add(1))
                    .max(1);
                let bin = ((decoded_frames.saturating_mul(PEAK_COUNT as u64)) / total)
                    .min((PEAK_COUNT - 1) as u64) as usize;
                raw_peaks[bin] = raw_peaks[bin].max(amplitude);
                decoded_frames = decoded_frames.saturating_add(1);
            }
        }
    }

    if decoded_frames == 0 {
        return Err("Aurora could not decode any audio samples for this waveform.".to_owned());
    }
    cancellation.checkpoint()?;
    Ok(WaveformSnapshot {
        track_key: track_key.to_owned(),
        peaks: shape_peaks(raw_peaks),
        sample_rate,
        channels,
        source: WaveformSource::Decoded,
    })
}

fn shape_peaks(raw: Vec<f32>) -> Vec<f32> {
    let mut ranked = raw
        .iter()
        .copied()
        .filter(|value| value.is_finite() && *value > 0.0)
        .collect::<Vec<_>>();
    ranked.sort_by(f32::total_cmp);
    let percentile = ranked
        .get(((ranked.len() as f32 * 0.95).floor() as usize).min(ranked.len().saturating_sub(1)))
        .copied()
        .unwrap_or(1.0);
    let scale = percentile
        .max(ranked.last().copied().unwrap_or(1.0) * 0.5)
        .max(f32::EPSILON);
    let normalized = raw
        .iter()
        .map(|value| (value / scale).clamp(0.0, 1.0).powf(0.68))
        .collect::<Vec<_>>();
    normalized
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let previous = index
                .checked_sub(1)
                .map(|item| normalized[item])
                .unwrap_or(*value);
            let next = normalized.get(index + 1).copied().unwrap_or(*value);
            (previous * 0.18 + value * 0.64 + next * 0.18).clamp(0.0, 1.0)
        })
        .collect()
}

fn valid_peaks(peaks: &[f32]) -> bool {
    peaks.len() == PEAK_COUNT
        && peaks
            .iter()
            .all(|peak| peak.is_finite() && (0.0..=1.0).contains(peak))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis()
        .try_into()
        .unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering as AtomicOrdering},
    };

    #[test]
    fn newer_generation_cancels_older_work() {
        let coordinator = WaveformWorkCoordinator::default();
        let older = coordinator.begin();
        assert!(coordinator.checkpoint(older).is_ok());

        let current = coordinator.begin();
        assert_eq!(
            coordinator
                .checkpoint(older)
                .expect_err("older work is stale"),
            SUPERSEDED_WORK_ERROR
        );
        assert!(coordinator.checkpoint(current).is_ok());

        let mut stale_operation_ran = false;
        assert_eq!(
            tauri::async_runtime::block_on(coordinator.run_serialized(older, || async {
                stale_operation_ran = true;
                Ok(())
            }))
            .expect_err("stale operation is rejected"),
            SUPERSEDED_WORK_ERROR
        );
        assert!(!stale_operation_ran);
    }

    #[test]
    fn decode_slot_serializes_work_while_cancellation_releases_the_active_job() {
        tauri::async_runtime::block_on(async {
            let coordinator = Arc::new(WaveformWorkCoordinator::default());
            let first = coordinator.begin();
            let (first_started_tx, mut first_started_rx) = tauri::async_runtime::channel(1);
            let (release_first_tx, mut release_first_rx) = tauri::async_runtime::channel(1);
            let first_coordinator = Arc::clone(&coordinator);
            let first_checkpoint = Arc::clone(&coordinator);
            let first_worker = tauri::async_runtime::spawn(async move {
                first_coordinator
                    .run_serialized(first, || async move {
                        first_started_tx.send(()).await.expect("signal first start");
                        release_first_rx.recv().await.expect("release first worker");
                        first_checkpoint.checkpoint(first)
                    })
                    .await
            });
            first_started_rx.recv().await.expect("first worker started");

            let second = coordinator.begin();
            let second_started = Arc::new(AtomicBool::new(false));
            let second_started_worker = Arc::clone(&second_started);
            let (second_waiting_tx, mut second_waiting_rx) = tauri::async_runtime::channel(1);
            let second_coordinator = Arc::clone(&coordinator);
            let second_worker = tauri::async_runtime::spawn(async move {
                second_waiting_tx
                    .send(())
                    .await
                    .expect("signal second waiter");
                second_coordinator
                    .run_serialized(second, || async move {
                        second_started_worker.store(true, AtomicOrdering::Release);
                        Ok(())
                    })
                    .await
            });
            second_waiting_rx
                .recv()
                .await
                .expect("second worker reached the async gate");
            assert!(!second_started.load(AtomicOrdering::Acquire));

            release_first_tx
                .send(())
                .await
                .expect("release first decode");
            assert_eq!(
                first_worker
                    .await
                    .expect("join first worker")
                    .expect_err("first worker was superseded"),
                SUPERSEDED_WORK_ERROR
            );
            assert!(second_worker.await.expect("join second worker").is_ok());
            assert!(second_started.load(AtomicOrdering::Acquire));
        });
    }

    #[test]
    fn waveform_media_buffers_within_the_cap_and_falls_back_above_it() {
        let directory = tempfile::tempdir().expect("create waveform media directory");
        let path = directory.path().join("track.mp3");
        fs::write(&path, [1_u8, 2, 3, 4]).expect("write waveform media");
        let coordinator = WaveformWorkCoordinator::default();

        let buffered_generation = coordinator.begin();
        let cancellation = coordinator
            .cancellation(buffered_generation)
            .expect("current buffered generation");
        match open_waveform_media(&path, 4, &cancellation).expect("buffer small waveform media") {
            WaveformMedia::Buffered(cursor) => {
                assert_eq!(cursor.into_inner(), vec![1, 2, 3, 4]);
            }
            WaveformMedia::File(_) => panic!("small media should be buffered"),
        }

        let fallback_generation = coordinator.begin();
        let cancellation = coordinator
            .cancellation(fallback_generation)
            .expect("current fallback generation");
        assert!(matches!(
            open_waveform_media(&path, 3, &cancellation)
                .expect("fall back for waveform media above the cap"),
            WaveformMedia::File(_)
        ));
    }

    #[test]
    fn file_signature_rejects_input_changed_before_cache_save() {
        let directory = tempfile::tempdir().expect("create signature test directory");
        let path = directory.path().join("track.mp3");
        fs::write(&path, [1_u8, 2, 3, 4]).expect("write original MP3");
        let signature = FileSignature::read(&path).expect("read original signature");
        signature
            .verify_unchanged(&path)
            .expect("unchanged MP3 remains valid");

        std::thread::sleep(Duration::from_millis(2));
        fs::write(&path, [4_u8, 3, 2, 1]).expect("rewrite MP3 at the same length after decoding");
        assert_eq!(
            signature
                .verify_unchanged(&path)
                .expect_err("changed MP3 must not be cached"),
            "Aurora did not cache the waveform because the MP3 changed during decoding."
        );
    }

    #[test]
    fn peak_shaping_is_bounded_and_preserves_a_loud_center() {
        let mut raw = vec![0.02; PEAK_COUNT];
        raw[159] = 0.8;
        raw[160] = 1.0;
        raw[161] = 0.7;
        let shaped = shape_peaks(raw);
        assert!(valid_peaks(&shaped));
        assert!(shaped[160] > shaped[10]);
        assert!(shaped[159] > shaped[10]);
    }

    #[test]
    fn cache_round_trip_rejects_a_changed_file_signature() {
        let directory = std::env::temp_dir().join(format!("aurora-waveform-{}", now_ms()));
        fs::create_dir_all(&directory).expect("create waveform test directory");
        let store = WaveformStore::new(directory.join("cache.sqlite3")).expect("create cache");
        let signature = FileSignature {
            size: 123,
            modified_ns: 456,
        };
        let snapshot = WaveformSnapshot {
            track_key: "test-track".to_owned(),
            peaks: vec![0.42; PEAK_COUNT],
            sample_rate: Some(44_100),
            channels: Some(2),
            source: WaveformSource::Decoded,
        };
        store.save(&snapshot, signature).expect("save waveform");
        let loaded = store
            .load("test-track", signature)
            .expect("load waveform")
            .expect("cached waveform");
        assert_eq!(loaded.peaks, snapshot.peaks);
        assert!(matches!(loaded.source, WaveformSource::Cache));
        assert!(
            store
                .load(
                    "test-track",
                    FileSignature {
                        size: 124,
                        modified_ns: 456
                    }
                )
                .expect("read changed signature")
                .is_none()
        );
        drop(store);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    #[ignore = "requires AURORA_TEST_MP3"]
    fn live_mp3_decodes_to_a_complete_waveform() {
        let path = std::env::var_os("AURORA_TEST_MP3")
            .map(PathBuf::from)
            .expect("set AURORA_TEST_MP3 to a local MP3 path");
        let coordinator = WaveformWorkCoordinator::default();
        let generation = coordinator.begin();
        let cancellation = coordinator
            .cancellation(generation)
            .expect("current live decode generation");
        let waveform =
            decode_mp3_waveform(&path, "live-test", None, &cancellation).expect("decode live MP3");
        assert!(valid_peaks(&waveform.peaks));
        assert!(waveform.peaks.iter().any(|peak| *peak > 0.1));
        assert!(waveform.sample_rate.is_some());
        assert!(waveform.channels.is_some());
    }
}
