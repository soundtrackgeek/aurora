//! App-independent result contract. No decoder, SQLite, or Bliss dependency.
use serde::{Deserialize, Serialize};

pub const PROFILE: &str = "bliss-0.13.0-symphonia-0.6.1-v2-full-mp3";
pub const DIMENSIONS: usize = 23;

/// Equal weight per usable track: long songs and multi-disc albums do not
/// dominate other albums. Accumulate in f64 without retaining track vectors.
#[derive(Default)]
pub struct AlbumAccumulator {
    sum: [f64; DIMENSIONS],
    count: usize,
}
impl AlbumAccumulator {
    pub fn add(&mut self, features: &[f32]) -> bool {
        if features.len() != DIMENSIONS || features.iter().any(|v| !v.is_finite()) {
            return false;
        }
        for (sum, value) in self.sum.iter_mut().zip(features) {
            *sum += f64::from(*value);
        }
        self.count += 1;
        true
    }
    pub fn count(&self) -> usize {
        self.count
    }
    pub fn mean(&self) -> Option<Vec<f32>> {
        (self.count > 0).then(|| {
            self.sum
                .iter()
                .map(|v| (v / self.count as f64) as f32)
                .collect()
        })
    }
}

/// Partial albums need at least three usable tracks (or all of a shorter
/// album), as well as the requested percentage of the catalog's MP3 tracks.
pub fn album_ready(analyzed: usize, total: usize, minimum_percent: u32) -> bool {
    (50..=100).contains(&minimum_percent)
        && total > 0
        && analyzed <= total
        && analyzed >= total.min(3)
        && (analyzed as u128) * 100 >= (total as u128) * u128::from(minimum_percent)
}

/// File observations are a cheap freshness guard, not an audio identity.
pub fn file_signature(path: &std::path::Path) -> std::io::Result<(u64, String)> {
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(std::io::Error::other("Not a regular file"));
    }
    let modified = metadata
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(std::io::Error::other)?
        .as_nanos()
        .to_string();
    Ok((metadata.len(), modified))
}

pub fn file_is_current(directory: &str, filename: &str, size: u64, modified: &str) -> bool {
    let name = std::path::Path::new(filename);
    if !std::path::Path::new(directory).is_absolute()
        || name.components().count() != 1
        || !matches!(
            name.components().next(),
            Some(std::path::Component::Normal(_))
        )
    {
        return false;
    }
    file_signature(&std::path::Path::new(directory).join(filename))
        .is_ok_and(|signature| signature.0 == size && signature.1 == modified)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Analysis {
    pub profile: String,
    pub features: Vec<f32>,
    pub weights: Vec<f32>,
}

impl Analysis {
    pub fn valid(&self) -> bool {
        self.profile == PROFILE
            && self.features.len() == DIMENSIONS
            && self.weights.len() == DIMENSIONS * DIMENSIONS
            && self
                .features
                .iter()
                .chain(&self.weights)
                .all(|v| v.is_finite())
    }
}

/// Compute a quadratic distance with a producer-supplied weight matrix.
/// Matrix values are result data; this crate contains no analyzer coefficients.
pub fn distance(seed: &Analysis, candidate: &[f32]) -> Option<f64> {
    Metric::new(seed)?.distance(candidate)
}

pub struct Metric {
    seed: Vec<f32>,
    weights: Vec<(usize, usize, f64)>,
}
impl Metric {
    pub fn new(seed: &Analysis) -> Option<Self> {
        if !seed.valid() {
            return None;
        }
        Some(Self {
            seed: seed.features.clone(),
            weights: seed
                .weights
                .iter()
                .enumerate()
                .filter(|(_, v)| **v != 0.)
                .map(|(i, v)| (i / DIMENSIONS, i % DIMENSIONS, f64::from(*v)))
                .collect(),
        })
    }
    pub fn distance(&self, candidate: &[f32]) -> Option<f64> {
        self.between(&self.seed, candidate)
    }
    pub fn between(&self, a: &[f32], b: &[f32]) -> Option<f64> {
        if a.len() != DIMENSIONS
            || b.len() != DIMENSIONS
            || a.iter().chain(b).any(|v| !v.is_finite())
        {
            return None;
        }
        let mut delta = [0.0; DIMENSIONS];
        for (i, (a, b)) in a.iter().zip(b).enumerate() {
            delta[i] = f64::from(*a) - f64::from(*b);
        }
        let mut squared = 0.0;
        for &(i, j, w) in &self.weights {
            squared += delta[i] * w * delta[j];
        }
        (squared.is_finite() && squared >= -1e-6).then(|| squared.max(0.0).sqrt())
    }
}

/// A chosen stop in an ordered sonic journey. Payloads belong to the caller.
#[derive(Clone)]
pub struct JourneyStop<T> {
    pub key: String,
    pub features: Vec<f32>,
    pub data: T,
}
struct JourneyCandidate<T> {
    stop: JourneyStop<T>,
    target_squared: f64,
}
/// Bounded approximation: retain neighbors per interpolated waypoint, then
/// use a 32-wide beam to balance adjacent jumps and waypoint proximity. This
/// is not an all-library shortest-path graph, nor a beat/key mixing guarantee.
pub struct JourneyBuilder<T> {
    stops: Vec<JourneyStop<T>>,
    metric: Metric,
    per_leg: usize,
    layers: Vec<Vec<JourneyCandidate<T>>>,
    leg_squared: Vec<f64>,
    candidate_limit: usize,
}
impl<T: Clone> JourneyBuilder<T> {
    pub fn new(analysis: &Analysis, stops: Vec<JourneyStop<T>>, per_leg: usize) -> Option<Self> {
        if !(2..=10).contains(&stops.len()) || !(1..=10).contains(&per_leg) {
            return None;
        }
        let metric = Metric::new(analysis)?;
        let mut keys = std::collections::HashSet::new();
        for stop in &stops {
            if stop.key.is_empty()
                || !keys.insert(&stop.key)
                || metric.distance(&stop.features).is_none()
            {
                return None;
            }
        }
        let leg_squared = stops
            .windows(2)
            .map(|pair| {
                metric
                    .between(&pair[0].features, &pair[1].features)
                    .map(|d| d * d)
            })
            .collect::<Option<Vec<_>>>()?;
        let layers = (0..(stops.len() - 1) * per_leg)
            .map(|_| Vec::new())
            .collect();
        let candidate_limit = 32.max((stops.len() - 1) * per_leg + 16);
        Some(Self {
            stops,
            metric,
            per_leg,
            layers,
            leg_squared,
            candidate_limit,
        })
    }
    pub fn offer(&mut self, key: &str, features: &[f32], data: T) {
        if self.stops.iter().any(|s| s.key == key) {
            return;
        }
        let Some(distances) = self
            .stops
            .iter()
            .map(|s| self.metric.between(&s.features, features).map(|d| d * d))
            .collect::<Option<Vec<_>>>()
        else {
            return;
        };
        for (index, layer) in self.layers.iter_mut().enumerate() {
            let leg = index / self.per_leg;
            let t = (index % self.per_leg + 1) as f64 / (self.per_leg + 1) as f64;
            // Quadratic interpolation needs only one distance per chosen stop,
            // rather than a full metric calculation for every waypoint.
            let target_squared = ((1. - t) * distances[leg] + t * distances[leg + 1]
                - t * (1. - t) * self.leg_squared[leg])
                .max(0.);
            if layer.len() >= self.candidate_limit
                && layer.last().is_some_and(|c| {
                    target_squared > c.target_squared
                        || target_squared == c.target_squared && key > c.stop.key.as_str()
                })
            {
                continue;
            }
            if layer.iter().any(|candidate| candidate.stop.key == key) {
                continue;
            }
            layer.push(JourneyCandidate {
                stop: JourneyStop {
                    key: key.into(),
                    features: features.into(),
                    data: data.clone(),
                },
                target_squared,
            });
            layer.sort_by(|a, b| {
                a.target_squared
                    .total_cmp(&b.target_squared)
                    .then(a.stop.key.cmp(&b.stop.key))
            });
            layer.truncate(self.candidate_limit);
        }
    }
    /// Freshness is checked once per retained identity; no catalog-sized cache.
    pub fn finish(self, mut usable: impl FnMut(&T) -> bool) -> Option<Vec<T>> {
        let mut checked = std::collections::HashMap::new();
        let layers = self
            .layers
            .iter()
            .map(|layer| {
                layer
                    .iter()
                    .filter(|c| {
                        *checked
                            .entry(c.stop.key.clone())
                            .or_insert_with(|| usable(&c.stop.data))
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        struct Beam<'a, T> {
            points: Vec<&'a JourneyStop<T>>,
            cost: f64,
        }
        let mut beams = vec![Beam {
            points: vec![&self.stops[0]],
            cost: 0.,
        }];
        for (index, layer) in layers.iter().enumerate() {
            let mut next = Vec::new();
            for beam in &beams {
                for candidate in layer {
                    if beam.points.iter().any(|s| s.key == candidate.stop.key) {
                        continue;
                    }
                    let mut points = beam.points.clone();
                    let jump = self
                        .metric
                        .between(&points.last()?.features, &candidate.stop.features)?;
                    let mut cost = beam.cost + jump * jump + candidate.target_squared * 0.25;
                    points.push(&candidate.stop);
                    if (index + 1) % self.per_leg == 0 {
                        let stop = &self.stops[index / self.per_leg + 1];
                        let jump = self
                            .metric
                            .between(&candidate.stop.features, &stop.features)?;
                        cost += jump * jump;
                        points.push(stop);
                    }
                    next.push(Beam { points, cost });
                }
            }
            next.sort_by(|a, b| {
                a.cost.total_cmp(&b.cost).then_with(|| {
                    a.points
                        .iter()
                        .map(|p| &p.key)
                        .cmp(b.points.iter().map(|p| &p.key))
                })
            });
            next.truncate(32);
            if next.is_empty() {
                return None;
            }
            beams = next;
        }
        Some(
            beams
                .first()?
                .points
                .iter()
                .map(|p| p.data.clone())
                .collect(),
        )
    }
}

pub fn track_key(directory: &str, filename: &str) -> String {
    let directory = directory.trim().replace('/', "\\");
    let directory = directory.strip_prefix("\\\\?\\").unwrap_or(&directory);
    format!("{}\\{}", directory.trim_end_matches('\\'), filename.trim()).to_lowercase()
}

/// MP3 audio payload boundaries, excluding ID3v2, ID3v1 and a trailing APE tag.
/// Reject malformed sizes rather than hash a tag as audio.
pub fn audio_range(
    file: &mut (impl std::io::Read + std::io::Seek),
    length: u64,
) -> std::io::Result<(u64, u64)> {
    use std::io::{Error, ErrorKind, SeekFrom};
    let invalid = || Error::new(ErrorKind::InvalidData, "Invalid MP3 tag bounds");
    let mut start = 0;
    let mut end = length;
    if length >= 10 {
        let mut header = [0; 10];
        file.seek(SeekFrom::Start(0))?;
        file.read_exact(&mut header)?;
        if &header[..3] == b"ID3" {
            if header[6..10].iter().any(|b| b & 0x80 != 0) || !matches!(header[3], 2..=4) {
                return Err(invalid());
            }
            let size = header[6..10]
                .iter()
                .fold(0u64, |v, b| (v << 7) | u64::from(*b));
            start = 10
                + size
                + if header[3] == 4 && header[5] & 0x10 != 0 {
                    10
                } else {
                    0
                };
        }
    }
    if end >= 128 {
        let mut marker = [0; 3];
        file.seek(SeekFrom::Start(end - 128))?;
        file.read_exact(&mut marker)?;
        if &marker == b"TAG" {
            end -= 128;
        }
    }
    if end >= 32 {
        let mut footer = [0; 32];
        file.seek(SeekFrom::Start(end - 32))?;
        file.read_exact(&mut footer)?;
        if &footer[..8] == b"APETAGEX" {
            let size = u32::from_le_bytes(footer[12..16].try_into().unwrap()) as u64;
            if size < 32 || size > end {
                return Err(invalid());
            }
            end -= size;
            // An optional APE header immediately precedes the items.
            if end >= 32 {
                file.seek(SeekFrom::Start(end - 32))?;
                file.read_exact(&mut footer)?;
                if &footer[..8] == b"APETAGEX" {
                    end -= 32;
                }
            }
        }
    }
    if start >= end {
        return Err(invalid());
    }
    Ok((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn journey_analysis() -> Analysis {
        let mut weights = vec![0.; DIMENSIONS * DIMENSIONS];
        weights[0] = 4.;
        weights[DIMENSIONS + 1] = 1.;
        Analysis {
            profile: PROFILE.into(),
            features: vec![0.; DIMENSIONS],
            weights,
        }
    }
    fn journey_stop(key: &str, value: f32) -> JourneyStop<String> {
        let mut features = vec![0.; DIMENSIONS];
        features[0] = value;
        JourneyStop {
            key: key.into(),
            features,
            data: key.into(),
        }
    }
    #[test]
    fn journey_preserves_five_stops_and_connects_in_order() {
        let stops = (0..5)
            .map(|i| journey_stop(&format!("stop{i}"), (i * 3) as f32))
            .collect();
        let mut builder = JourneyBuilder::new(&journey_analysis(), stops, 2).unwrap();
        for i in (1..12).rev().filter(|i| i % 3 != 0) {
            let s = journey_stop(&format!("bridge{i}"), i as f32);
            builder.offer(&s.key, &s.features, s.data.clone());
        }
        let result = builder.finish(|_| true).unwrap();
        assert_eq!(
            result,
            [
                "stop0", "bridge1", "bridge2", "stop1", "bridge4", "bridge5", "stop2", "bridge7",
                "bridge8", "stop3", "bridge10", "bridge11", "stop4"
            ]
        );
    }
    #[test]
    fn journey_handles_equal_sounds_and_never_repeats_a_track() {
        let mut builder = JourneyBuilder::new(
            &journey_analysis(),
            vec![
                journey_stop("a", 0.),
                journey_stop("b", 0.),
                journey_stop("c", 0.),
            ],
            2,
        )
        .unwrap();
        for key in ["c", "b", "a", "1", "2", "3", "4", "5"] {
            let s = journey_stop(key, 0.);
            builder.offer(key, &s.features, s.data.clone());
            builder.offer(key, &s.features, s.data);
        }
        let mut checked = 0;
        let result = builder
            .finish(|key| {
                checked += 1;
                key != "1"
            })
            .unwrap();
        assert_eq!(checked, 5);
        assert_eq!(result.len(), 7);
        assert_eq!(
            result
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            7
        );
        assert!(!result.contains(&"1".into()));
        assert_eq!(result[0], "a");
        assert_eq!(result[3], "b");
        assert_eq!(result[6], "c");
    }
    #[test]
    fn journey_rejects_invalid_stops_and_insufficient_fresh_bridges() {
        let stops = vec![journey_stop("a", 0.), journey_stop("b", 10.)];
        assert!(JourneyBuilder::new(&journey_analysis(), stops.clone(), 11).is_none());
        assert!(JourneyBuilder::new(
            &journey_analysis(),
            vec![stops[0].clone(), stops[0].clone()],
            1
        )
        .is_none());
        let mut builder = JourneyBuilder::new(&journey_analysis(), stops, 2).unwrap();
        let s = journey_stop("middle", 5.);
        builder.offer(&s.key, &s.features, s.data.clone());
        builder.offer("invalid", &[f32::NAN; DIMENSIONS], "invalid".into());
        assert!(builder.finish(|_| true).is_none());
    }
    #[test]
    fn long_equal_sound_journeys_have_a_bounded_pool_large_enough_for_no_repeats() {
        let stops = (0..10)
            .map(|i| journey_stop(&format!("stop{i}"), 0.))
            .collect();
        let mut builder = JourneyBuilder::new(&journey_analysis(), stops, 10).unwrap();
        for i in 0..150 {
            let s = journey_stop(&format!("candidate{i:03}"), 0.);
            builder.offer(&s.key, &s.features, s.data.clone());
        }
        assert!(builder.layers.iter().all(|layer| layer.len() <= 106));
        let result = builder.finish(|_| true).unwrap();
        assert_eq!(result.len(), 100);
        assert_eq!(
            result
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            100
        );
    }
    #[test]
    fn album_mean_and_partial_coverage_are_reproducible() {
        let mut album = AlbumAccumulator::default();
        assert!(album.mean().is_none());
        assert!(!album.add(&[0.]));
        assert!(!album.add(&[f32::NAN; DIMENSIONS]));
        assert!(album.add(&[1.; DIMENSIONS]));
        assert!(album.add(&[3.; DIMENSIONS]));
        assert_eq!(album.mean().unwrap(), vec![2.; DIMENSIONS]);
        assert_eq!(album.count(), 2);
        assert!(!album_ready(2, 4, 50));
        assert!(album_ready(3, 6, 50));
        assert!(!album_ready(3, 6, 80));
        assert!(album_ready(5, 6, 80));
        assert!(!album_ready(5, 6, 100));
        assert!(album_ready(2, 2, 50));
        assert!(album_ready(1, 1, 100));
        assert!(!album_ready(0, 0, 50));
        assert!(!album_ready(3, 2, 50));
        assert!(!album_ready(3, 6, 49));
    }
    #[test]
    fn weighted_distance_and_invalid_profiles() {
        let mut seed = Analysis {
            profile: PROFILE.into(),
            features: vec![0.; DIMENSIONS],
            weights: vec![0.; DIMENSIONS * DIMENSIONS],
        };
        for i in 0..DIMENSIONS {
            seed.weights[i * DIMENSIONS + i] = 1.;
        }
        seed.weights[0] = 4.;
        let mut candidate = vec![0.; DIMENSIONS];
        candidate[0] = 3.;
        candidate[1] = 4.;
        assert!((distance(&seed, &candidate).unwrap() - 52_f64.sqrt()).abs() < 1e-6);
        assert_eq!(distance(&seed, &seed.features), Some(0.));
        candidate[0] = f32::NAN;
        assert_eq!(distance(&seed, &candidate), None);
        seed.profile = "older".into();
        assert!(!seed.valid());
    }
    #[test]
    fn identity_survives_windows_path_spellings() {
        assert_eq!(
            track_key("\\\\?\\D:\\Music\\Artist\\", "01.mp3"),
            track_key("d:/music/artist", "01.mp3")
        );
    }
    #[test]
    fn payload_bounds_exclude_tags_and_reject_malformed_headers() {
        use std::io::Cursor;
        let mut bytes = b"ID3\x04\0\0\0\0\0\x03tag".to_vec();
        bytes.extend_from_slice(&[0xff; 256]);
        let mut ape = [0u8; 32];
        ape[..8].copy_from_slice(b"APETAGEX");
        ape[12..16].copy_from_slice(&32u32.to_le_bytes());
        bytes.extend_from_slice(&ape);
        bytes.extend_from_slice(b"TAG");
        bytes.extend_from_slice(&[0; 125]);
        assert_eq!(
            audio_range(&mut Cursor::new(&bytes), bytes.len() as u64).unwrap(),
            (13, 269)
        );
        bytes[6] = 128;
        assert!(audio_range(&mut Cursor::new(&bytes), bytes.len() as u64).is_err());
        assert!(audio_range(&mut Cursor::new(b"ID3\x04\0\0\0\0\x7f\x7f"), 10).is_err());
    }
}
