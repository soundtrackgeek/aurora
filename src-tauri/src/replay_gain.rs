use crate::audio_settings::ReplayGainMode;
use id3::Tag;
use std::path::Path;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ReplayGainAdjustment {
    pub(crate) linear: f32,
    pub(crate) applied_db: Option<f32>,
    pub(crate) source: Option<ReplayGainMode>,
    pub(crate) clipping_prevented: bool,
}

#[derive(Default)]
struct ReplayGainTags {
    track_gain_db: Option<f32>,
    track_peak: Option<f32>,
    album_gain_db: Option<f32>,
    album_peak: Option<f32>,
}

pub(crate) fn adjustment_for_path(path: &Path, mode: ReplayGainMode) -> ReplayGainAdjustment {
    if mode == ReplayGainMode::Off {
        return ReplayGainAdjustment {
            linear: 1.0,
            ..Default::default()
        };
    }
    let Some(tags) = read_tags(path) else {
        return ReplayGainAdjustment {
            linear: 1.0,
            ..Default::default()
        };
    };
    let selected = match mode {
        ReplayGainMode::Off => None,
        ReplayGainMode::Track => tags
            .track_gain_db
            .map(|gain| (gain, tags.track_peak, ReplayGainMode::Track)),
        ReplayGainMode::Album => tags
            .album_gain_db
            .map(|gain| (gain, tags.album_peak, ReplayGainMode::Album))
            .or_else(|| {
                tags.track_gain_db
                    .map(|gain| (gain, tags.track_peak, ReplayGainMode::Track))
            }),
    };
    let Some((requested_db, peak, source)) = selected else {
        return ReplayGainAdjustment {
            linear: 1.0,
            ..Default::default()
        };
    };
    adjustment(requested_db, peak, source)
}

fn read_tags(path: &Path) -> Option<ReplayGainTags> {
    let tag = Tag::read_from_path(path).ok()?;
    let mut result = ReplayGainTags::default();
    for value in tag.extended_texts() {
        match normalized_key(&value.description).as_str() {
            "REPLAYGAIN_TRACK_GAIN" => result.track_gain_db = parse_number(&value.value),
            "REPLAYGAIN_TRACK_PEAK" => result.track_peak = parse_number(&value.value),
            "REPLAYGAIN_ALBUM_GAIN" => result.album_gain_db = parse_number(&value.value),
            "REPLAYGAIN_ALBUM_PEAK" => result.album_peak = parse_number(&value.value),
            _ => {}
        }
    }
    Some(result)
}

fn normalized_key(value: &str) -> String {
    value.trim().replace([' ', '-'], "_").to_ascii_uppercase()
}

fn parse_number(value: &str) -> Option<f32> {
    value
        .split_whitespace()
        .next()?
        .replace(',', ".")
        .parse::<f32>()
        .ok()
        .filter(|number| number.is_finite())
}

fn adjustment(
    requested_db: f32,
    peak: Option<f32>,
    source: ReplayGainMode,
) -> ReplayGainAdjustment {
    let requested_linear = 10_f32.powf(requested_db / 20.0);
    let ceiling = peak.filter(|peak| *peak > 0.0).map(|peak| 1.0 / peak);
    let linear = ceiling.map_or(requested_linear, |limit| requested_linear.min(limit));
    let clipping_prevented = linear + f32::EPSILON < requested_linear;
    let applied_db = if linear > 0.0 {
        Some(20.0 * linear.log10())
    } else {
        Some(requested_db)
    };
    ReplayGainAdjustment {
        linear,
        applied_db,
        source: Some(source),
        clipping_prevented,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_gain_is_limited_by_peak() {
        let result = adjustment(5.75, Some(0.608_429), ReplayGainMode::Track);
        assert!(result.clipping_prevented);
        assert!(result.linear <= 1.0 / 0.608_429 + f32::EPSILON);
    }

    #[test]
    fn negative_gain_is_not_changed_by_a_peak_over_one() {
        let result = adjustment(-2.16, Some(1.007_812), ReplayGainMode::Track);
        assert!(!result.clipping_prevented);
        assert!((result.applied_db.unwrap() + 2.16).abs() < 0.001);
    }

    #[test]
    fn musicbee_numbers_accept_decimal_commas_and_db_suffixes() {
        assert_eq!(parse_number("-7,43 dB"), Some(-7.43));
        assert_eq!(parse_number("1.010132"), Some(1.010132));
    }

    #[test]
    #[ignore = "requires AURORA_TEST_REPLAYGAIN_MP3"]
    fn live_musicbee_replaygain_frames_are_applied() {
        let path = std::env::var_os("AURORA_TEST_REPLAYGAIN_MP3")
            .map(std::path::PathBuf::from)
            .expect("AURORA_TEST_REPLAYGAIN_MP3");
        let result = adjustment_for_path(&path, ReplayGainMode::Track);
        assert_eq!(result.source, Some(ReplayGainMode::Track));
        assert!(result.applied_db.is_some());
        assert!(result.linear.is_finite() && result.linear > 0.0);
    }
}
