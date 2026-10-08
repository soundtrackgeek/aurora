//! Album rollups are derived at query time from the current catalog paths.
//! Keep this adapter aligned in the companion app; aggregation math is shared.
use music_sonic_core::{
    AlbumAccumulator, Analysis, Metric, PROFILE, album_ready, file_is_current, track_key,
};
use rusqlite::{Connection, OptionalExtension, params_from_iter, types::Value};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SonicAlbumRequest {
    pub album_id: String,
    pub limit: usize,
    pub minimum_coverage: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SonicAlbum {
    pub album_id: String,
    pub title: String,
    pub album_artist: String,
    pub genre: Option<String>,
    pub total_tracks: usize,
    pub analyzed_tracks: usize,
    pub distance: Option<f64>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SonicAlbumMatches {
    pub seed: Option<SonicAlbum>,
    pub seed_ready: bool,
    pub analyzed_albums: usize,
    pub albums: Vec<SonicAlbum>,
}
struct Entry {
    album: SonicAlbum,
    features: Option<String>,
    directory: String,
    filename: String,
    size: Option<i64>,
    modified: Option<String>,
    banned: bool,
}
struct Rollup {
    album: SonicAlbum,
    features: AlbumAccumulator,
}
impl Rollup {
    fn add(&mut self, entry: Entry, fresh: bool, overrides: &HashMap<String, bool>) {
        if entry.album.total_tracks > 0 {
            self.album.total_tracks = entry.album.total_tracks;
        } else {
            self.album.total_tracks += 1;
        }
        let key = track_key(&entry.directory, &entry.filename);
        if *overrides.get(&key).unwrap_or(&entry.banned) {
            return;
        }
        if fresh
            && !entry
                .size
                .zip(entry.modified.as_deref())
                .is_some_and(|(size, modified)| {
                    size >= 0
                        && file_is_current(&entry.directory, &entry.filename, size as u64, modified)
                })
        {
            return;
        }
        if let Some(features) = entry
            .features
            .and_then(|json| serde_json::from_str::<Vec<f32>>(&json).ok())
        {
            self.features.add(&features);
        }
        self.album.analyzed_tracks = self.features.count();
    }
}
fn scan(
    c: &Connection,
    id: Option<&str>,
    fresh: bool,
    overrides: &HashMap<String, bool>,
    mut visit: impl FnMut(Rollup),
) -> Result<(), String> {
    let columns =
        "t.album_id,COALESCE(t.album,''),COALESCE(t.album_artist_display,''),t.canonical_genre,
        a.features,t.file_path,t.filename,s.size,s.modified,COALESCE(t.love,'')";
    let sql = if id.is_some() {
        format!("SELECT {columns},0 FROM tracks t
            LEFT JOIN sonic.sonic_tracks s ON s.directory=t.file_path AND s.filename=t.filename AND s.profile=?1
            LEFT JOIN sonic.sonic_audio a ON a.audio_hash=s.audio_hash AND a.profile=s.profile
            WHERE t.album_id=?2 AND lower(t.filename) LIKE '%.mp3' ORDER BY t.id")
    } else {
        // Start from analyzed paths, then count ALL MP3s in those albums.
        // CROSS JOIN keeps SQLite from driving this scan with the full catalog.
        format!(
            "WITH candidate_albums AS MATERIALIZED (
            SELECT DISTINCT t.album_id FROM sonic.sonic_tracks s
            CROSS JOIN tracks t ON s.directory=t.file_path AND s.filename=t.filename
            WHERE s.profile=?1 AND lower(t.filename) LIKE '%.mp3'),
            totals AS MATERIALIZED (
            SELECT album_id,(SELECT COUNT(*) FROM tracks coverage
                WHERE coverage.album_id=candidate_albums.album_id
                AND lower(coverage.filename) LIKE '%.mp3') AS total FROM candidate_albums)
            SELECT {columns},totals.total FROM sonic.sonic_tracks s
            CROSS JOIN tracks t ON s.directory=t.file_path AND s.filename=t.filename
            JOIN sonic.sonic_audio a ON a.audio_hash=s.audio_hash AND a.profile=s.profile
            JOIN totals ON totals.album_id=t.album_id
            WHERE s.profile=?1 AND lower(t.filename) LIKE '%.mp3' ORDER BY t.album_id,t.id"
        )
    };
    let mut values = vec![Value::Text(PROFILE.into())];
    if let Some(id) = id {
        values.push(Value::Text(id.into()));
    }
    let mut statement = c.prepare(&sql).map_err(|e| e.to_string())?;
    let entries = statement
        .query_map(params_from_iter(values), |r| {
            Ok(Entry {
                album: SonicAlbum {
                    album_id: r.get(0)?,
                    title: r.get(1)?,
                    album_artist: r.get(2)?,
                    genre: r.get(3)?,
                    total_tracks: r.get::<_, i64>(10)? as usize,
                    analyzed_tracks: 0,
                    distance: None,
                },
                features: r.get(4)?,
                directory: r.get::<_, Option<String>>(5)?.unwrap_or_default(),
                filename: r.get(6)?,
                size: r.get(7)?,
                modified: r.get(8)?,
                banned: r.get::<_, String>(9)? == "B",
            })
        })
        .map_err(|e| e.to_string())?;
    let mut current: Option<Rollup> = None;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        if current
            .as_ref()
            .is_some_and(|r| r.album.album_id != entry.album.album_id)
        {
            visit(current.take().unwrap());
        }
        let rollup = current.get_or_insert_with(|| Rollup {
            album: entry.album.clone(),
            features: AlbumAccumulator::default(),
        });
        rollup.add(entry, fresh, overrides);
    }
    if let Some(rollup) = current {
        visit(rollup);
    }
    Ok(())
}
pub(crate) fn seed(
    c: &Connection,
    id: &str,
    minimum: u32,
    overrides: &HashMap<String, bool>,
) -> Result<(Option<SonicAlbum>, Option<Analysis>), String> {
    let mut result = None;
    scan(c, Some(id), true, overrides, |r| result = Some(r))?;
    let Some(rollup) = result else {
        return Ok((None, None));
    };
    let analysis = if album_ready(
        rollup.album.analyzed_tracks,
        rollup.album.total_tracks,
        minimum,
    ) {
        let weights: Option<String> = c
            .query_row(
                "SELECT weights FROM sonic.sonic_profiles WHERE profile=?1",
                [PROFILE],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        weights
            .and_then(|json| serde_json::from_str(&json).ok())
            .map(|weights| Analysis {
                profile: PROFILE.into(),
                features: rollup.features.mean().unwrap(),
                weights,
            })
            .filter(Analysis::valid)
    } else {
        None
    };
    Ok((Some(rollup.album), analysis))
}
fn compare(a: &SonicAlbum, b: &SonicAlbum) -> std::cmp::Ordering {
    a.distance
        .unwrap_or(f64::MAX)
        .total_cmp(&b.distance.unwrap_or(f64::MAX))
        .then(a.album_id.cmp(&b.album_id))
}
pub(crate) fn query(
    c: &Connection,
    has_analysis: bool,
    request: &SonicAlbumRequest,
    overrides: &HashMap<String, bool>,
) -> Result<SonicAlbumMatches, String> {
    if request.album_id.is_empty()
        || request.album_id.len() > 4096
        || !(1..=100).contains(&request.limit)
        || !(50..=100).contains(&request.minimum_coverage)
    {
        return Err("Choose an album, 1–100 results, and 50–100% analysis coverage.".into());
    }
    let mut response = SonicAlbumMatches {
        seed: None,
        seed_ready: false,
        analyzed_albums: 0,
        albums: vec![],
    };
    // One read transaction gives rollups a consistent catalog/results snapshot.
    let transaction = c.unchecked_transaction().map_err(|e| e.to_string())?;
    let c = &*transaction;
    if !has_analysis {
        response.seed = c.query_row("SELECT album_id,COALESCE(MIN(album),''),COALESCE(MIN(album_artist_display),''),MIN(canonical_genre),COUNT(*)
            FROM tracks WHERE album_id=?1 AND lower(filename) LIKE '%.mp3' GROUP BY album_id", [&request.album_id], |r| Ok(SonicAlbum {
                album_id: r.get(0)?, title: r.get(1)?, album_artist: r.get(2)?, genre: r.get(3)?, total_tracks: r.get::<_, i64>(4)? as usize, analyzed_tracks: 0, distance: None,
            })).optional().map_err(|e| e.to_string())?;
        return Ok(response);
    }
    let (album, analysis) = seed(c, &request.album_id, request.minimum_coverage, overrides)?;
    response.seed = album;
    let Some(metric) = analysis.as_ref().and_then(Metric::new) else {
        return Ok(response);
    };
    response.seed_ready = true;
    let mut ranked = vec![];
    scan(c, None, false, overrides, |mut r| {
        if !album_ready(
            r.album.analyzed_tracks,
            r.album.total_tracks,
            request.minimum_coverage,
        ) {
            return;
        }
        response.analyzed_albums += 1;
        if r.album.album_id == request.album_id {
            return;
        }
        r.album.distance = metric.distance(&r.features.mean().unwrap());
        if r.album.distance.is_none() {
            return;
        }
        ranked.push(r.album);
        if ranked.len() > request.limit * 16 {
            ranked.sort_by(compare);
            ranked.truncate(request.limit * 8);
        }
    })?;
    ranked.sort_by(compare);
    ranked.truncate(request.limit * 8);
    // Verify every contributing file in shortlisted albums and recompute means.
    // Do not stat the million-track catalog during a recommendation request.
    for candidate in ranked {
        let (album, analysis) = seed(c, &candidate.album_id, request.minimum_coverage, overrides)?;
        if let (Some(mut album), Some(analysis)) = (album, analysis) {
            album.distance = metric.distance(&analysis.features);
            if album.distance.is_some() {
                response.albums.push(album);
            }
        }
    }
    response.albums.sort_by(compare);
    response.albums.truncate(request.limit);
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use music_sonic_core::{DIMENSIONS, file_signature};
    use rusqlite::params;
    fn fixture() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE tracks(id INTEGER PRIMARY KEY,album_id TEXT,album TEXT,album_artist_display TEXT,canonical_genre TEXT,file_path TEXT,filename TEXT,love TEXT);
          ATTACH DATABASE ':memory:' AS sonic;
          CREATE TABLE sonic.sonic_profiles(profile TEXT PRIMARY KEY,weights TEXT);
          CREATE TABLE sonic.sonic_audio(audio_hash TEXT,profile TEXT,features TEXT,PRIMARY KEY(audio_hash,profile));
          CREATE TABLE sonic.sonic_tracks(track_key TEXT PRIMARY KEY,directory TEXT,filename TEXT,audio_hash TEXT,profile TEXT,size INTEGER,modified TEXT);").unwrap();
        let mut weights = vec![0f32; DIMENSIONS * DIMENSIONS];
        for i in 0..DIMENSIONS {
            weights[i * DIMENSIONS + i] = 1.;
        }
        c.execute(
            "INSERT INTO sonic.sonic_profiles VALUES(?1,?2)",
            params![PROFILE, serde_json::to_string(&weights).unwrap()],
        )
        .unwrap();
        // Same printed title, distinct album IDs; partial and tiny coverage.
        for (id, total, analyzed, value) in [
            ("seed", 6, 6, 1.),
            ("near", 6, 6, 1.2),
            ("partial", 6, 3, 1.1),
            ("thin", 6, 1, 1.01),
            ("wrong", 6, 6, 1.02),
            ("banned", 6, 6, 1.03),
            ("single", 1, 1, 8.),
        ] {
            for track in 0..total {
                let filename = format!("{id}-{track}.mp3");
                let path = dir.path().join(&filename);
                std::fs::write(&path, [255; 256]).unwrap();
                c.execute("INSERT INTO tracks(album_id,album,album_artist_display,canonical_genre,file_path,filename,love) VALUES(?1,'Same title',?2,'Pop',?3,?4,?5)",
                    params![id,format!("Album artist {id}"),dir.path().to_string_lossy(),filename,if id=="banned" {"B"}else{""}]).unwrap();
                if track >= analyzed {
                    continue;
                }
                let profile = if id == "wrong" {
                    "old-profile"
                } else {
                    PROFILE
                };
                let mut features = vec![0.; DIMENSIONS];
                features[0] = value;
                c.execute(
                    "INSERT INTO sonic.sonic_audio VALUES(?1,?2,?3)",
                    params![filename, profile, serde_json::to_string(&features).unwrap()],
                )
                .unwrap();
                let (size, modified) = file_signature(&path).unwrap();
                c.execute(
                    "INSERT INTO sonic.sonic_tracks VALUES(?1,?2,?3,?4,?5,?6,?7)",
                    params![
                        track_key(&dir.path().to_string_lossy(), &filename),
                        dir.path().to_string_lossy(),
                        filename,
                        filename,
                        profile,
                        size as i64,
                        modified
                    ],
                )
                .unwrap();
            }
        }
        (dir, c)
    }
    fn request(minimum_coverage: u32) -> SonicAlbumRequest {
        SonicAlbumRequest {
            album_id: "seed".into(),
            limit: 20,
            minimum_coverage,
        }
    }
    #[test]
    fn sparse_analysis_counts_full_candidate_coverage_without_scanning_unrelated_albums() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let (_dir, c) = fixture();
        c.execute_batch("CREATE INDEX album_tracks ON tracks(album_id);
            CREATE INDEX track_files ON tracks(file_path,filename);
            WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<10000)
            INSERT INTO tracks(album_id,filename) SELECT 'unanalysed-'||x,'song.mp3' FROM n;
            INSERT INTO tracks(album_id,filename) VALUES('partial','extra.MP3'),('partial','extra.flac');").unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        c.create_scalar_function(
            "lower",
            1,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8
                | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
            move |ctx| {
                counter.fetch_add(1, Ordering::Relaxed);
                Ok(ctx.get::<String>(0)?.to_ascii_lowercase())
            },
        )
        .unwrap();
        let mut albums = Vec::new();
        scan(&c, None, false, &HashMap::new(), |r| albums.push(r.album)).unwrap();
        let partial = albums.iter().find(|a| a.album_id == "partial").unwrap();
        assert_eq!(partial.total_tracks, 7);
        assert_eq!(partial.analyzed_tracks, 3);
        assert!(!albums.iter().any(|a| a.album_id.starts_with("unanalysed-")));
        assert!(
            calls.load(Ordering::Relaxed) < 500,
            "coverage visited unrelated catalog filenames"
        );
    }
    #[test]
    fn album_ranking_preserves_identity_and_partial_coverage() {
        let (_dir, c) = fixture();
        c.execute("UPDATE tracks SET id=id+10000", []).unwrap();
        let response = query(&c, true, &request(50), &HashMap::new()).unwrap();
        assert!(response.seed_ready);
        assert_eq!(response.seed.unwrap().analyzed_tracks, 6);
        assert_eq!(
            response
                .albums
                .iter()
                .map(|a| a.album_id.as_str())
                .collect::<Vec<_>>(),
            vec!["partial", "near", "single"]
        );
        assert_eq!(response.albums[0].album_artist, "Album artist partial");
        assert_eq!(response.albums[0].analyzed_tracks, 3);
        assert_eq!(response.albums[0].total_tracks, 6);
        let complete = query(&c, true, &request(100), &HashMap::new()).unwrap();
        assert_eq!(
            complete
                .albums
                .iter()
                .map(|a| a.album_id.as_str())
                .collect::<Vec<_>>(),
            vec!["near", "single"]
        );
        assert!(query(&c, true, &request(49), &HashMap::new()).is_err());
    }
    #[test]
    fn changed_files_and_ban_overrides_recompute_album_means() {
        let (dir, c) = fixture();
        std::fs::write(dir.path().join("partial-0.mp3"), [255; 257]).unwrap();
        let result = query(&c, true, &request(50), &HashMap::new()).unwrap();
        assert!(!result.albums.iter().any(|a| a.album_id == "partial"));
        std::fs::write(dir.path().join("seed-0.mp3"), [255; 257]).unwrap();
        assert!(
            !query(&c, true, &request(100), &HashMap::new())
                .unwrap()
                .seed_ready
        );
        assert!(
            query(&c, true, &request(50), &HashMap::new())
                .unwrap()
                .seed_ready
        );
        let overrides = (0..6)
            .map(|i| {
                (
                    track_key(&dir.path().to_string_lossy(), &format!("near-{i}.mp3")),
                    true,
                )
            })
            .collect();
        assert!(
            !query(&c, true, &request(50), &overrides)
                .unwrap()
                .albums
                .iter()
                .any(|a| a.album_id == "near")
        );
        let unban = (0..6)
            .map(|i| {
                (
                    track_key(&dir.path().to_string_lossy(), &format!("banned-{i}.mp3")),
                    false,
                )
            })
            .collect();
        assert_eq!(
            query(&c, true, &request(50), &unban).unwrap().albums[0].album_id,
            "banned"
        );
        c.execute(
            "UPDATE tracks SET album_id='new-id' WHERE album_id='near'",
            [],
        )
        .unwrap();
        assert!(
            query(&c, true, &request(50), &HashMap::new())
                .unwrap()
                .albums
                .iter()
                .any(|a| a.album_id == "new-id")
        );
    }
    #[test]
    fn missing_analysis_empty_seeds_and_read_only_catalogs_are_supported() {
        let (_dir, c) = fixture();
        c.execute_batch("PRAGMA query_only=ON;").unwrap();
        let result = query(&c, false, &request(50), &HashMap::new()).unwrap();
        assert!(!result.seed_ready);
        assert_eq!(result.seed.unwrap().total_tracks, 6);
        assert!(
            query(&c, true, &request(50), &HashMap::new())
                .unwrap()
                .seed_ready
        );
        let mut missing = request(50);
        missing.album_id = "removed".into();
        assert!(
            query(&c, true, &missing, &HashMap::new())
                .unwrap()
                .seed
                .is_none()
        );
        assert!(c.execute("DELETE FROM tracks", []).is_err());
        assert!(c.execute("DELETE FROM sonic.sonic_audio", []).is_err());
    }
}
