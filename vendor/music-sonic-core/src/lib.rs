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
        if candidate.len() != DIMENSIONS || candidate.iter().any(|v| !v.is_finite()) {
            return None;
        }
        let mut delta = [0.0; DIMENSIONS];
        for (i, (a, b)) in self.seed.iter().zip(candidate).enumerate() {
            delta[i] = f64::from(*a) - f64::from(*b);
        }
        let mut squared = 0.0;
        for &(i, j, w) in &self.weights {
            squared += delta[i] * w * delta[j];
        }
        (squared.is_finite() && squared >= -1e-6).then(|| squared.max(0.0).sqrt())
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
