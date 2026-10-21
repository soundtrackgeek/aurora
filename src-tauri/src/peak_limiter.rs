//! Lookahead peak limiter for gain raised from an analysis estimate.
//!
//! The analysis stores no peak, so a boosted track may exceed full scale.
//! This brickwall limiter runs in the PCM producer thread, never in the audio
//! callback. Rodio's `limit()` is feed-forward without lookahead, so it lets
//! fast transients overshoot during its attack; this one cannot.
//!
//! Per frame (channels linked): `required = min(1, ceiling / peak)`, then a
//! sliding minimum over `L` frames, a moving average over `L` frames and a
//! one-pole release. Frame `p` is emitted with the gain computed at frame
//! `p + L − 1`, so the gain at every peak is at or below `required` and
//! changes by at most `1 / L` per frame. Every input sample is returned, in
//! order, so gapless handoff and track length are unchanged.
use rodio::{ChannelCount, SampleRate, Source, source::SeekError};
use std::{collections::VecDeque, time::Duration};

const CEILING_DB: f32 = -1.0;
const LOOKAHEAD: Duration = Duration::from_millis(5);
const RELEASE: Duration = Duration::from_millis(150);

pub(crate) struct PeakLimiter<S> {
    inner: S,
    channels: usize,
    ceiling: f32,
    lookahead: u64,
    release_step: f32,
    frame: Vec<f32>,
    delayed: VecDeque<f32>,
    delayed_lengths: VecDeque<usize>,
    minima: VecDeque<(u64, f32)>,
    holds: VecDeque<f32>,
    hold_sum: f64,
    envelope: f32,
    /// Frames pushed through the gain pipeline, including flush frames.
    pushed: u64,
    /// Real frames already emitted.
    emitted: u64,
    output: VecDeque<f32>,
    exhausted: bool,
}

impl<S: Source> PeakLimiter<S> {
    pub(crate) fn new(inner: S) -> Self {
        let channels = usize::from(inner.channels().get()).max(1);
        let rate = f64::from(inner.sample_rate().get());
        let lookahead = ((rate * LOOKAHEAD.as_secs_f64()).round() as u64).max(1);
        let release_step = 1.0 - (-1.0 / (rate * RELEASE.as_secs_f64())).exp() as f32;
        let frames = lookahead as usize;
        Self {
            inner,
            channels,
            ceiling: 10_f32.powf(CEILING_DB / 20.0),
            lookahead,
            release_step,
            frame: Vec::with_capacity(channels),
            delayed: VecDeque::with_capacity(frames * channels),
            delayed_lengths: VecDeque::with_capacity(frames),
            minima: VecDeque::with_capacity(frames),
            holds: VecDeque::with_capacity(frames + 1),
            hold_sum: 0.0,
            envelope: 1.0,
            pushed: 0,
            emitted: 0,
            output: VecDeque::with_capacity(channels),
            exhausted: false,
        }
    }

    fn reset(&mut self) {
        self.frame.clear();
        self.delayed.clear();
        self.delayed_lengths.clear();
        self.minima.clear();
        self.holds.clear();
        self.hold_sum = 0.0;
        self.envelope = 1.0;
        self.pushed = 0;
        self.emitted = 0;
        self.output.clear();
        self.exhausted = false;
    }

    /// Advance the gain pipeline by one frame. `None` is a silent flush frame.
    fn push_frame(&mut self, samples: Option<&[f32]>) {
        let peak = samples.map_or(0.0, |frame| {
            frame
                .iter()
                .fold(0.0_f32, |peak, sample| peak.max(sample.abs()))
        });
        let required = if peak > self.ceiling {
            self.ceiling / peak
        } else {
            1.0
        };
        let index = self.pushed;
        self.pushed += 1;

        while self
            .minima
            .back()
            .is_some_and(|(_, value)| *value >= required)
        {
            self.minima.pop_back();
        }
        self.minima.push_back((index, required));
        while self
            .minima
            .front()
            .is_some_and(|(start, _)| start + self.lookahead <= index)
        {
            self.minima.pop_front();
        }
        let hold = self.minima.front().map_or(1.0, |(_, value)| *value);
        self.holds.push_back(hold);
        self.hold_sum += f64::from(hold);
        if self.holds.len() as u64 > self.lookahead {
            self.hold_sum -= f64::from(self.holds.pop_front().unwrap_or(1.0));
        }
        let average = ((self.hold_sum / self.holds.len() as f64) as f32).min(1.0);
        self.envelope = if average < self.envelope {
            average
        } else {
            self.envelope + self.release_step * (average - self.envelope)
        };

        if let Some(frame) = samples {
            self.delayed.extend(frame);
            self.delayed_lengths.push_back(frame.len());
        }
        if index + 1 >= self.emitted + self.lookahead
            && let Some(length) = self.delayed_lengths.pop_front()
        {
            self.emitted += 1;
            let (gain, ceiling) = (self.envelope, self.ceiling);
            self.output.extend(
                self.delayed
                    .drain(..length)
                    .map(|sample| (sample * gain).clamp(-ceiling, ceiling)),
            );
        }
    }

    fn fill_output(&mut self) {
        while self.output.is_empty() {
            if self.exhausted {
                if self.delayed_lengths.is_empty() {
                    return;
                }
                self.push_frame(None);
                continue;
            }
            match self.inner.next() {
                Some(sample) => {
                    self.frame.push(sample);
                    if self.frame.len() == self.channels {
                        let frame = std::mem::take(&mut self.frame);
                        self.push_frame(Some(&frame));
                        self.frame = frame;
                        self.frame.clear();
                    }
                }
                None => {
                    self.exhausted = true;
                    if !self.frame.is_empty() {
                        let frame = std::mem::take(&mut self.frame);
                        self.push_frame(Some(&frame));
                    }
                }
            }
        }
    }
}

impl<S: Source> Iterator for PeakLimiter<S> {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        self.fill_output();
        self.output.pop_front()
    }
}

impl<S: Source> Source for PeakLimiter<S> {
    fn current_span_len(&self) -> Option<usize> {
        (self.exhausted && self.output.is_empty() && self.delayed_lengths.is_empty()).then_some(0)
    }

    fn channels(&self) -> ChannelCount {
        self.inner.channels()
    }

    fn sample_rate(&self) -> SampleRate {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.inner.try_seek(position)?;
        self.reset();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, atomic::AtomicU64};

    const RATE: u32 = 8_000;
    /// Lookahead frames at the test rate.
    const L: usize = 40;

    struct Samples {
        values: Vec<f32>,
        cursor: usize,
        channels: ChannelCount,
    }

    impl Iterator for Samples {
        type Item = f32;
        fn next(&mut self) -> Option<f32> {
            let value = self.values.get(self.cursor).copied();
            self.cursor += usize::from(value.is_some());
            value
        }
    }

    impl Source for Samples {
        fn current_span_len(&self) -> Option<usize> {
            Some(self.values.len() - self.cursor)
        }
        fn channels(&self) -> ChannelCount {
            self.channels
        }
        fn sample_rate(&self) -> SampleRate {
            SampleRate::new(RATE).unwrap()
        }
        fn total_duration(&self) -> Option<Duration> {
            None
        }
        fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
            let frame = (position.as_secs_f64() * f64::from(RATE)) as usize;
            self.cursor = (frame * usize::from(self.channels.get())).min(self.values.len());
            Ok(())
        }
    }

    fn source(values: Vec<f32>, channels: u16) -> Samples {
        Samples {
            values,
            cursor: 0,
            channels: ChannelCount::new(channels).unwrap(),
        }
    }

    fn ceiling() -> f32 {
        10_f32.powf(CEILING_DB / 20.0)
    }

    #[test]
    fn lookahead_matches_five_milliseconds() {
        assert_eq!(PeakLimiter::new(source(vec![], 1)).lookahead, L as u64);
    }

    #[test]
    fn quiet_audio_passes_unchanged_with_every_sample() {
        for length in [0, 1, 7, L - 1, L, L + 1, 3 * L + 5] {
            let values = (0..length * 2)
                .map(|i| ((i as f32) * 0.37).sin() * 0.8)
                .collect::<Vec<_>>();
            let output = PeakLimiter::new(source(values.clone(), 2)).collect::<Vec<_>>();
            assert_eq!(output, values, "length {length}");
        }
    }

    #[test]
    fn boosted_spikes_and_squares_never_exceed_the_ceiling() {
        let mut values = vec![0.5_f32; 4_000];
        for spike in [L / 2, 1_000, 1_001, 2_999] {
            values[spike] = 4.0;
        }
        values.extend((0..4_000).map(|i| if (i / 20) % 2 == 0 { 3.0 } else { -3.0 }));
        let output = PeakLimiter::new(source(values.clone(), 1)).collect::<Vec<_>>();
        assert_eq!(output.len(), values.len());
        assert!(output.iter().all(|sample| sample.abs() <= ceiling() + 1e-6));
        // Short tracks that end inside the lookahead are still protected.
        let short = PeakLimiter::new(source(vec![0.2, 0.2, 5.0, 0.2], 1)).collect::<Vec<_>>();
        assert_eq!(short.len(), 4);
        assert!(short.iter().all(|sample| sample.abs() <= ceiling() + 1e-6));
    }

    #[test]
    fn channels_share_one_gain_and_mono_works() {
        let mut values = vec![0.25_f32; 2 * 400];
        values[2 * 200] = 3.0;
        let output = PeakLimiter::new(source(values, 2)).collect::<Vec<_>>();
        let left = output[2 * 200] / 3.0;
        let right = output[2 * 200 + 1] / 0.25;
        assert!((left - right).abs() < 1e-6);
        assert!(output[2 * 200 + 1] < 0.25);
        let mono = PeakLimiter::new(source(vec![2.0; 100], 1)).collect::<Vec<_>>();
        assert!(mono.iter().all(|sample| (sample - ceiling()).abs() < 1e-5));
    }

    #[test]
    fn gain_ramps_smoothly_and_recovers() {
        let mut values = vec![0.5_f32; RATE as usize * 2];
        values[2_000] = 4.0;
        let output = PeakLimiter::new(source(values.clone(), 1)).collect::<Vec<_>>();
        let gains = output
            .iter()
            .zip(&values)
            .map(|(out, input)| out / input)
            .collect::<Vec<_>>();
        let largest_step = gains
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .fold(0.0, f32::max);
        assert!(largest_step <= 1.0 / L as f32 + 1e-4, "step {largest_step}");
        assert!(
            gains[2_000 - L] > 0.99,
            "reduction starts within the lookahead"
        );
        assert!(*gains.last().unwrap() > 0.998, "release returns to unity");
    }

    #[test]
    fn seeking_discards_delayed_audio() {
        let values = (0..RATE as usize * 3)
            .map(|i| (i % 1_000) as f32 / 2_000.0)
            .collect::<Vec<_>>();
        let mut limiter = PeakLimiter::new(source(values.clone(), 1));
        for _ in 0..100 {
            limiter.next();
        }
        limiter.try_seek(Duration::from_secs(2)).unwrap();
        let after = limiter.collect::<Vec<_>>();
        assert_eq!(after, values[RATE as usize * 2..]);
    }

    #[test]
    fn buffered_playback_keeps_order_and_length() {
        let values = (0..2_000)
            .map(|i| ((i as f32) * 0.01).cos() * 0.6)
            .collect::<Vec<_>>();
        // The 500 ms prefill exceeds this source, so spawn returns fully buffered.
        let underruns = Arc::new(AtomicU64::new(0));
        let buffered = crate::pcm_buffer::PcmBufferSource::spawn(
            Box::new(PeakLimiter::new(source(values.clone(), 2))),
            Arc::clone(&underruns),
        )
        .unwrap();
        assert_eq!(buffered.collect::<Vec<_>>(), values);
        assert_eq!(underruns.load(std::sync::atomic::Ordering::Relaxed), 0);
    }
}
