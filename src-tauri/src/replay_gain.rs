use crate::{audio_settings::ReplayGainMode, sonic_gain};
use id3::Tag;
use std::path::Path;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ReplayGainAdjustment {
    pub(crate) linear: f32,
    pub(crate) applied_db: Option<f32>,
    pub(crate) source: Option<ReplayGainMode>,
    pub(crate) clipping_prevented: bool,
    /// The gain came from Music Library's analysis, not ReplayGain frames.
    pub(crate) estimated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GainPolicy {
    pub(crate) mode: ReplayGainMode,
    pub(crate) estimate_missing: bool,
}

#[derive(Default)]
struct ReplayGainTags {
    track_gain_db: Option<f32>,
    track_peak: Option<f32>,
    album_gain_db: Option<f32>,
    album_peak: Option<f32>,
}

fn unity() -> ReplayGainAdjustment {
    ReplayGainAdjustment {
        linear: 1.0,
        ..Default::default()
    }
}

pub(crate) fn adjustment_for_track(
    path: &Path,
    track: sonic_gain::GainTrack<'_>,
    policy: GainPolicy,
) -> ReplayGainAdjustment {
    if policy.mode == ReplayGainMode::Off {
        return unity();
    }
    select(read_tags(path).unwrap_or_default(), policy, |album| {
        sonic_gain::estimate_lufs(track, album)
    })
}

/// Tags win. Album mode keeps album dynamics before falling back to track gain.
fn select(
    tags: ReplayGainTags,
    policy: GainPolicy,
    estimate_lufs: impl Fn(bool) -> Option<f32>,
) -> ReplayGainAdjustment {
    let tagged = |gain: Option<f32>, peak: Option<f32>, source| {
        gain.map(|gain| adjustment(gain, peak, source))
    };
    let estimated = |album: bool| {
        policy
            .estimate_missing
            .then(|| estimate_lufs(album))
            .flatten()
            .map(|lufs| {
                estimated_adjustment(
                    sonic_gain::attenuation_db(lufs),
                    if album {
                        ReplayGainMode::Album
                    } else {
                        ReplayGainMode::Track
                    },
                )
            })
    };
    let selected = match policy.mode {
        ReplayGainMode::Off => None,
        ReplayGainMode::Track => tagged(tags.track_gain_db, tags.track_peak, ReplayGainMode::Track)
            .or_else(|| estimated(false)),
        ReplayGainMode::Album => tagged(tags.album_gain_db, tags.album_peak, ReplayGainMode::Album)
            .or_else(|| estimated(true))
            .or_else(|| tagged(tags.track_gain_db, tags.track_peak, ReplayGainMode::Track))
            .or_else(|| estimated(false)),
    };
    selected.unwrap_or_else(unity)
}

fn estimated_adjustment(gain_db: f32, source: ReplayGainMode) -> ReplayGainAdjustment {
    ReplayGainAdjustment {
        linear: 10_f32.powf(gain_db / 20.0),
        applied_db: Some(gain_db),
        source: Some(source),
        clipping_prevented: false,
        estimated: true,
    }
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
        estimated: false,
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

    fn policy(mode: ReplayGainMode, estimate_missing: bool) -> GainPolicy {
        GainPolicy {
            mode,
            estimate_missing,
        }
    }

    /// −10 LUFS for the track estimate, −14 LUFS for the album estimate.
    fn estimates(album: bool) -> Option<f32> {
        Some(if album { -14.0 } else { -10.0 })
    }

    fn tags(track: Option<f32>, album: Option<f32>) -> ReplayGainTags {
        ReplayGainTags {
            track_gain_db: track,
            album_gain_db: album,
            ..Default::default()
        }
    }

    #[test]
    fn tags_win_over_estimates() {
        let result = select(
            tags(Some(-3.0), None),
            policy(ReplayGainMode::Track, true),
            estimates,
        );
        assert!((result.applied_db.unwrap() + 3.0).abs() < 0.001);
        assert!(!result.estimated);
    }

    #[test]
    fn untagged_tracks_use_attenuation_only_estimates() {
        let result = select(
            tags(None, None),
            policy(ReplayGainMode::Track, true),
            estimates,
        );
        assert!(result.estimated);
        assert_eq!(result.source, Some(ReplayGainMode::Track));
        assert!((result.applied_db.unwrap() + 8.0).abs() < 0.001);
        assert!((result.linear - 10_f32.powf(-0.4)).abs() < 1e-6);
        assert!(!result.clipping_prevented);
        let quiet = select(
            tags(None, None),
            policy(ReplayGainMode::Track, true),
            |_| Some(-25.0),
        );
        assert_eq!(quiet.applied_db, Some(0.0));
    }

    #[test]
    fn album_mode_prefers_album_estimates_over_track_tags() {
        let album = policy(ReplayGainMode::Album, true);
        let result = select(tags(Some(-3.0), None), album, estimates);
        assert!(result.estimated);
        assert_eq!(result.source, Some(ReplayGainMode::Album));
        assert!((result.applied_db.unwrap() + 4.0).abs() < 0.001);
        // Without an album estimate, the track tag comes before the track estimate.
        let no_album = |album: bool| (!album).then_some(-10.0);
        let result = select(tags(Some(-3.0), None), album, no_album);
        assert!(!result.estimated && result.source == Some(ReplayGainMode::Track));
        let result = select(tags(None, None), album, no_album);
        assert!(result.estimated && result.source == Some(ReplayGainMode::Track));
        // Album tags still win.
        let result = select(tags(None, Some(-6.0)), album, estimates);
        assert!(!result.estimated && result.source == Some(ReplayGainMode::Album));
    }

    #[test]
    fn disabled_estimates_and_missing_analysis_keep_unity_gain() {
        let result = select(
            tags(None, None),
            policy(ReplayGainMode::Track, false),
            estimates,
        );
        assert_eq!(result.linear, 1.0);
        assert!(result.applied_db.is_none() && !result.estimated);
        let result = select(
            tags(None, None),
            policy(ReplayGainMode::Album, true),
            |_| None,
        );
        assert_eq!(result.linear, 1.0);
        assert!(result.source.is_none());
    }

    #[test]
    #[ignore = "requires AURORA_TEST_REPLAYGAIN_MP3"]
    fn live_musicbee_replaygain_frames_are_applied() {
        let path = std::env::var_os("AURORA_TEST_REPLAYGAIN_MP3")
            .map(std::path::PathBuf::from)
            .expect("AURORA_TEST_REPLAYGAIN_MP3");
        let track = sonic_gain::GainTrack {
            directory: "",
            filename: "",
            album_id: None,
        };
        let result = adjustment_for_track(&path, track, policy(ReplayGainMode::Track, false));
        assert_eq!(result.source, Some(ReplayGainMode::Track));
        assert!(result.applied_db.is_some());
        assert!(result.linear.is_finite() && result.linear > 0.0);
    }
}
