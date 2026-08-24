use rodio::{ChannelCount, SampleRate, Source, source::SeekError};
use rtrb::{Consumer, Producer, RingBuffer};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::Duration,
};

const PCM_BUFFER_SECONDS: usize = 3;
const PCM_PREFILL_MILLIS: usize = 500;
const PCM_PRODUCER_BATCH_SAMPLES: usize = 4_096;
const MIN_PCM_BUFFER_SAMPLES: usize = 8_192;
const MAX_PCM_BUFFER_SAMPLES: usize = 4_194_304;

#[derive(Clone, Copy)]
struct BufferedSample {
    generation: u32,
    value: f32,
}

enum ProducerCommand {
    Seek {
        position: Duration,
        generation: u32,
        response: Sender<Result<(), SeekError>>,
    },
}

pub(crate) struct PcmBufferSource {
    consumer: Consumer<BufferedSample>,
    commands: Sender<ProducerCommand>,
    generation: Arc<AtomicU32>,
    finished: Arc<AtomicBool>,
    underrun_count: Arc<AtomicU64>,
    channels: ChannelCount,
    sample_rate: SampleRate,
    total_duration: Option<Duration>,
    starving: bool,
}

impl PcmBufferSource {
    pub(crate) fn spawn(
        source: Box<dyn Source + Send>,
        underrun_count: Arc<AtomicU64>,
    ) -> Result<Self, String> {
        let channels = source.channels();
        let sample_rate = source.sample_rate();
        let total_duration = source.total_duration();
        let samples_per_second = sample_rate.get() as usize * channels.get() as usize;
        let capacity = samples_per_second
            .saturating_mul(PCM_BUFFER_SECONDS)
            .clamp(MIN_PCM_BUFFER_SAMPLES, MAX_PCM_BUFFER_SAMPLES);
        let prefill_samples = samples_per_second
            .saturating_mul(PCM_PREFILL_MILLIS)
            .div_ceil(1_000)
            .min(capacity);
        let (producer, consumer) = RingBuffer::new(capacity);
        let (command_tx, command_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let generation = Arc::new(AtomicU32::new(0));
        let finished = Arc::new(AtomicBool::new(false));
        let worker_finished = Arc::clone(&finished);

        thread::Builder::new()
            .name("aurora-pcm-producer".to_owned())
            .spawn(move || {
                run_producer(
                    source,
                    producer,
                    command_rx,
                    ready_tx,
                    worker_finished,
                    prefill_samples,
                );
            })
            .map_err(|error| format!("Aurora could not start PCM buffering: {error}"))?;

        ready_rx
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| "Aurora's PCM buffer did not become ready in time.".to_owned())?;

        Ok(Self {
            consumer,
            commands: command_tx,
            generation,
            finished,
            underrun_count,
            channels,
            sample_rate,
            total_duration,
            starving: false,
        })
    }
}

impl Iterator for PcmBufferSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.consumer.pop() {
                Ok(sample) if sample.generation == self.generation.load(Ordering::Acquire) => {
                    self.starving = false;
                    return Some(sample.value);
                }
                Ok(_) => continue,
                Err(_) if self.finished.load(Ordering::Acquire) || self.consumer.is_abandoned() => {
                    return None;
                }
                Err(_) => {
                    if !self.starving {
                        self.underrun_count.fetch_add(1, Ordering::Relaxed);
                        self.starving = true;
                    }
                    return Some(0.0);
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

impl Source for PcmBufferSource {
    fn current_span_len(&self) -> Option<usize> {
        if (self.finished.load(Ordering::Acquire) || self.consumer.is_abandoned())
            && self.consumer.is_empty()
        {
            Some(0)
        } else {
            None
        }
    }

    fn channels(&self) -> ChannelCount {
        self.channels
    }

    fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        self.total_duration
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        let previous_generation = self.generation.load(Ordering::Acquire);
        let generation = previous_generation.wrapping_add(1);
        self.generation.store(generation, Ordering::Release);
        while self.consumer.pop().is_ok() {}
        self.starving = false;

        let (response_tx, response_rx) = mpsc::channel();
        if self
            .commands
            .send(ProducerCommand::Seek {
                position,
                generation,
                response: response_tx,
            })
            .is_err()
        {
            return Err(disconnected_seek_error());
        }
        match response_rx.recv() {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => {
                self.generation
                    .store(previous_generation, Ordering::Release);
                Err(error)
            }
            Err(_) => Err(disconnected_seek_error()),
        }
    }
}

fn disconnected_seek_error() -> SeekError {
    SeekError::Other(Arc::new(std::io::Error::other(
        "Aurora's PCM producer stopped before it could seek.",
    )))
}

fn run_producer(
    mut source: Box<dyn Source + Send>,
    mut producer: Producer<BufferedSample>,
    commands: Receiver<ProducerCommand>,
    ready: mpsc::SyncSender<()>,
    finished: Arc<AtomicBool>,
    prefill_samples: usize,
) {
    let mut generation = 0;
    if fill_available(source.as_mut(), &mut producer, generation, prefill_samples) {
        finished.store(true, Ordering::Release);
    }
    let _ = ready.send(());

    loop {
        if producer.is_abandoned() {
            return;
        }
        match commands.try_recv() {
            Ok(ProducerCommand::Seek {
                position,
                generation: requested_generation,
                response,
            }) => {
                let result = source.try_seek(position);
                if result.is_ok() {
                    generation = requested_generation;
                    finished.store(false, Ordering::Release);
                    if fill_available(source.as_mut(), &mut producer, generation, prefill_samples) {
                        finished.store(true, Ordering::Release);
                    }
                }
                let _ = response.send(result);
                continue;
            }
            Err(mpsc::TryRecvError::Disconnected) => return,
            Err(mpsc::TryRecvError::Empty) => {}
        }

        if finished.load(Ordering::Acquire) || producer.is_full() {
            thread::sleep(Duration::from_millis(1));
            continue;
        }
        if fill_available(
            source.as_mut(),
            &mut producer,
            generation,
            PCM_PRODUCER_BATCH_SAMPLES,
        ) {
            finished.store(true, Ordering::Release);
        }
    }
}

fn fill_available(
    source: &mut dyn Source,
    producer: &mut Producer<BufferedSample>,
    generation: u32,
    target_samples: usize,
) -> bool {
    let mut written = 0;
    while written < target_samples && !producer.is_full() {
        let Some(value) = source.next() else {
            return true;
        };
        if producer.push(BufferedSample { generation, value }).is_err() {
            break;
        }
        written += 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SeekableSamples {
        values: Vec<f32>,
        cursor: usize,
        channels: ChannelCount,
        sample_rate: SampleRate,
    }

    impl Iterator for SeekableSamples {
        type Item = f32;

        fn next(&mut self) -> Option<Self::Item> {
            let value = self.values.get(self.cursor).copied();
            self.cursor += usize::from(value.is_some());
            value
        }
    }

    impl Source for SeekableSamples {
        fn current_span_len(&self) -> Option<usize> {
            Some(self.values.len().saturating_sub(self.cursor))
        }

        fn channels(&self) -> ChannelCount {
            self.channels
        }

        fn sample_rate(&self) -> SampleRate {
            self.sample_rate
        }

        fn total_duration(&self) -> Option<Duration> {
            let frames = self.values.len() / self.channels.get() as usize;
            Some(Duration::from_secs_f64(
                frames as f64 / self.sample_rate.get() as f64,
            ))
        }

        fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
            let frame = (position.as_secs_f64() * self.sample_rate.get() as f64) as usize;
            self.cursor = frame
                .saturating_mul(self.channels.get() as usize)
                .min(self.values.len());
            Ok(())
        }
    }

    fn test_source(values: Vec<f32>) -> SeekableSamples {
        SeekableSamples {
            values,
            cursor: 0,
            channels: ChannelCount::new(2).expect("valid test channels"),
            sample_rate: SampleRate::new(100).expect("valid test rate"),
        }
    }

    #[test]
    fn prefilled_source_preserves_pcm_order_without_underruns() {
        let values = (0..200).map(|value| value as f32).collect::<Vec<_>>();
        let underruns = Arc::new(AtomicU64::new(0));
        let buffered = PcmBufferSource::spawn(
            Box::new(test_source(values.clone())),
            Arc::clone(&underruns),
        )
        .expect("spawn PCM producer");

        assert_eq!(buffered.collect::<Vec<_>>(), values);
        assert_eq!(underruns.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn seek_discards_prefetched_pcm_and_refills_from_requested_position() {
        let values = (0..1_000).map(|value| value as f32).collect::<Vec<_>>();
        let underruns = Arc::new(AtomicU64::new(0));
        let mut buffered = PcmBufferSource::spawn(Box::new(test_source(values)), underruns)
            .expect("spawn PCM producer");

        assert_eq!(buffered.next(), Some(0.0));
        buffered
            .try_seek(Duration::from_secs(2))
            .expect("seek buffered source");
        assert_eq!(buffered.next(), Some(400.0));
        assert_eq!(buffered.next(), Some(401.0));
    }
}
