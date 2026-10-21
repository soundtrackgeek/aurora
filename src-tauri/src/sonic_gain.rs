//! Estimated playback gain from Music Library's stored audio analysis.
//!
//! Used only when an MP3 has no ReplayGain frames. The estimate reads the
//! existing read-only analysis store; MP3s are never decoded, measured, or
//! rewritten here.
use crate::catalog;
use music_sonic_core::{DIMENSIONS, PROFILE, file_is_current, track_key};
use rusqlite::{Connection, OptionalExtension, params};

// Calibration `bliss-v2-r128-2026-10-a`: fitted 2026-10-21 against FFmpeg
// EBU R128 integrated loudness on 360 random library MP3s. Mean absolute
// error 0.89 dB, 95th percentile 2.3 dB.
/// Bliss feature 8 is mean frame power, normalized from −90…0 dBFS to −1…1.
const LEVEL_INDEX: usize = 8;
const SLOPE: f32 = 0.88;
const INTERCEPT_LUFS: f32 = -1.0;
/// Reference implied by the library's existing ReplayGain frames.
const TARGET_LUFS: f32 = -18.0;
/// Estimates only attenuate: the analysis has no peak to make a boost safe.
const FLOOR_DB: f32 = -24.0;
const SILENCE_DBFS: f32 = -60.0;
const ALBUM_COVERAGE_PERCENT: usize = 80;

/// The catalog fields needed to find a track's analysis.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GainTrack<'a> {
    pub(crate) directory: &'a str,
    pub(crate) filename: &'a str,
    pub(crate) album_id: Option<&'a str>,
}

pub(crate) fn estimated_lufs(features: &[f32]) -> Option<f32> {
    if features.len() != DIMENSIONS || features.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let level_dbfs = (features[LEVEL_INDEX] + 1.0) * 45.0 - 90.0;
    (level_dbfs >= SILENCE_DBFS).then_some(SLOPE * level_dbfs + INTERCEPT_LUFS)
}

pub(crate) fn attenuation_db(lufs: f32) -> f32 {
    (TARGET_LUFS - lufs).clamp(FLOOR_DB, 0.0)
}

/// Duration-weighted energy mean, the usual approximation of album loudness.
fn album_lufs(tracks: &[(f32, f64)]) -> Option<f32> {
    let (energy, seconds) =
        tracks
            .iter()
            .fold((0.0, 0.0), |(energy, seconds), (lufs, duration)| {
                let weight = duration.max(1.0);
                (
                    energy + weight * 10f64.powf(f64::from(*lufs) / 10.0),
                    seconds + weight,
                )
            });
    (seconds > 0.0).then(|| (10.0 * (energy / seconds).log10()) as f32)
}

/// Open the shared catalog with the analysis store attached, if available.
pub(crate) fn estimate_lufs(track: GainTrack<'_>, album: bool) -> Option<f32> {
    let path = catalog::default_catalog_path().ok()?;
    if !path.parent()?.join("music-analysis.sqlite3").is_file() {
        return None;
    }
    let connection = catalog::open_catalog(&path).ok()?;
    if album {
        album_estimate(&connection, track.album_id?)
    } else {
        track_estimate(&connection, track.directory, track.filename)
    }
}

fn current_estimate(
    directory: &str,
    filename: &str,
    features: &str,
    size: i64,
    modified: &str,
) -> Option<f32> {
    let size = u64::try_from(size).ok()?;
    if !file_is_current(directory, filename, size, modified) {
        return None;
    }
    estimated_lufs(&serde_json::from_str::<Vec<f32>>(features).ok()?)
}

pub(crate) fn track_estimate(
    connection: &Connection,
    directory: &str,
    filename: &str,
) -> Option<f32> {
    let (features, size, modified) = connection
        .query_row(
            "SELECT a.features,s.size,s.modified FROM sonic.sonic_tracks s
             JOIN sonic.sonic_audio a USING(audio_hash,profile)
             WHERE s.track_key=?1 AND s.profile=?2",
            params![track_key(directory, filename), PROFILE],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .ok()??;
    current_estimate(directory, filename, &features, size, &modified)
}

/// Needs current estimates for at least 80% of the album's cataloged MP3s.
pub(crate) fn album_estimate(connection: &Connection, album_id: &str) -> Option<f32> {
    let mut statement = connection
        .prepare(
            "SELECT t.file_path,t.filename,t.time_seconds,a.features,s.size,s.modified
             FROM tracks t
             LEFT JOIN sonic.sonic_tracks s
               ON s.directory=t.file_path AND s.filename=t.filename AND s.profile=?2
             LEFT JOIN sonic.sonic_audio a
               ON a.audio_hash=s.audio_hash AND a.profile=s.profile
             WHERE t.album_id=?1 AND lower(t.filename) LIKE '%.mp3'",
        )
        .ok()?;
    let rows = statement
        .query_map(params![album_id, PROFILE], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })
        .ok()?
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let estimates = rows
        .iter()
        .filter_map(|(directory, filename, seconds, features, size, modified)| {
            let lufs = current_estimate(
                directory,
                filename,
                features.as_deref()?,
                (*size)?,
                modified.as_deref()?,
            )?;
            Some((lufs, seconds.unwrap_or(0) as f64))
        })
        .collect::<Vec<_>>();
    if rows.is_empty() || estimates.len() * 100 < rows.len() * ALBUM_COVERAGE_PERCENT {
        return None;
    }
    album_lufs(&estimates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use music_sonic_core::file_signature;

    fn features_at(level_dbfs: f32) -> Vec<f32> {
        let mut features = vec![0f32; DIMENSIONS];
        features[LEVEL_INDEX] = (level_dbfs + 90.0) / 45.0 - 1.0;
        features
    }

    #[test]
    fn calibrated_level_maps_to_attenuation_only_gain() {
        let lufs = estimated_lufs(&features_at(-14.0)).unwrap();
        assert!((lufs + 13.32).abs() < 0.01);
        assert!((attenuation_db(lufs) + 4.68).abs() < 0.01);
        // A quiet score is never boosted; a very loud master is turned down.
        assert_eq!(
            attenuation_db(estimated_lufs(&features_at(-26.0)).unwrap()),
            0.0
        );
        assert!((attenuation_db(estimated_lufs(&features_at(-4.5)).unwrap()) + 13.04).abs() < 0.01);
        assert_eq!(attenuation_db(20.0), FLOOR_DB);
    }

    #[test]
    fn invalid_or_near_silent_features_have_no_estimate() {
        assert!(estimated_lufs(&features_at(-61.0)).is_none());
        assert!(estimated_lufs(&[0.0; 3]).is_none());
        let mut broken = features_at(-12.0);
        broken[2] = f32::NAN;
        assert!(estimated_lufs(&broken).is_none());
    }

    #[test]
    fn album_loudness_is_an_energy_mean_weighted_by_duration() {
        assert!((album_lufs(&[(-10.0, 60.0), (-10.0, 600.0)]).unwrap() + 10.0).abs() < 1e-4);
        // Equal durations: −10 and −20 LUFS average to −12.6 LUFS, not −15.
        assert!((album_lufs(&[(-10.0, 100.0), (-20.0, 100.0)]).unwrap() + 12.596).abs() < 0.01);
        // The longer quiet track pulls the album down.
        assert!(album_lufs(&[(-10.0, 60.0), (-20.0, 600.0)]).unwrap() < -15.0);
        assert!(album_lufs(&[]).is_none());
    }

    fn fixture(tracks: usize, analyzed: usize) -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(
            "CREATE TABLE tracks(id INTEGER PRIMARY KEY,album_id TEXT,file_path TEXT,filename TEXT,time_seconds INTEGER);
             ATTACH DATABASE ':memory:' AS sonic;
             CREATE TABLE sonic.sonic_audio(audio_hash TEXT,profile TEXT,features TEXT,PRIMARY KEY(audio_hash,profile));
             CREATE TABLE sonic.sonic_tracks(track_key TEXT PRIMARY KEY,directory TEXT,filename TEXT,audio_hash TEXT,profile TEXT,size INTEGER,modified TEXT);",
        )
        .unwrap();
        let directory = dir.path().to_string_lossy().to_string();
        for id in 0..tracks {
            let filename = format!("{id}.mp3");
            let path = dir.path().join(&filename);
            std::fs::write(&path, [255; 64]).unwrap();
            c.execute(
                "INSERT INTO tracks VALUES(?1,'album',?2,?3,200)",
                params![id as i64, directory, filename],
            )
            .unwrap();
            if id >= analyzed {
                continue;
            }
            let (size, modified) = file_signature(&path).unwrap();
            c.execute(
                "INSERT INTO sonic.sonic_audio VALUES(?1,?2,?3)",
                params![
                    format!("hash{id}"),
                    PROFILE,
                    serde_json::to_string(&features_at(-12.0)).unwrap()
                ],
            )
            .unwrap();
            c.execute(
                "INSERT INTO sonic.sonic_tracks VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    track_key(&directory, &filename),
                    directory,
                    filename,
                    format!("hash{id}"),
                    PROFILE,
                    size as i64,
                    modified
                ],
            )
            .unwrap();
        }
        (dir, c)
    }

    #[test]
    fn track_estimates_require_current_files_and_the_current_profile() {
        let (dir, c) = fixture(2, 1);
        let directory = dir.path().to_string_lossy().to_string();
        let lufs = track_estimate(&c, &directory, "0.mp3").unwrap();
        assert!((lufs + 11.56).abs() < 0.01);
        assert!(track_estimate(&c, &directory, "1.mp3").is_none());

        std::fs::write(dir.path().join("0.mp3"), [255; 65]).unwrap();
        assert!(
            track_estimate(&c, &directory, "0.mp3").is_none(),
            "a changed file is not trusted"
        );

        let (dir, c) = fixture(1, 1);
        c.execute("UPDATE sonic.sonic_tracks SET profile='older'", [])
            .unwrap();
        assert!(track_estimate(&c, &dir.path().to_string_lossy(), "0.mp3").is_none());
    }

    #[test]
    fn album_estimates_need_eighty_percent_coverage() {
        let (_dir, c) = fixture(5, 4);
        assert!((album_estimate(&c, "album").unwrap() + 11.56).abs() < 0.01);
        let (_dir, c) = fixture(5, 3);
        assert!(album_estimate(&c, "album").is_none());
        assert!(album_estimate(&c, "missing").is_none());
    }

    #[test]
    fn missing_analysis_store_has_no_estimate() {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE tracks(id INTEGER PRIMARY KEY,album_id TEXT,file_path TEXT,filename TEXT,time_seconds INTEGER);")
            .unwrap();
        assert!(track_estimate(&c, "C:\\Music", "a.mp3").is_none());
        assert!(album_estimate(&c, "album").is_none());
    }
}
