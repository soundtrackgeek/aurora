//! Bounded, read-only discovery; catalog ownership stays with Music Library.
use crate::{
    catalog::{self, TrackSummary},
    sonic,
    state_store::StateStore,
    tag_model::LoveState,
};
use music_sonic_core::{
    Analysis, Metric, PROFILE, file_is_current,
    worlds::{self, Boundary, Direction, Metadata},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WorldRequest {
    pub seed_key: String,
    pub boundary: Boundary,
    pub direction: Direction,
    pub exclude_keys: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorldMatch {
    pub track: TrackSummary,
    pub value: Option<String>,
    pub distance: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorldResponse {
    pub version: String,
    pub seed_ready: bool,
    pub seed_value: Option<String>,
    pub max_distance: f64,
    pub search_limited: bool,
    pub matches: Vec<WorldMatch>,
}

fn value(boundary: Boundary, track: &TrackSummary) -> Option<String> {
    boundary.value(&Metadata {
        year: track.original_year,
        album_artist: &track.artist,
        genre: track.genre.as_deref(),
    })
}

// A conservative repeat rule, not an acoustic recording-identity claim.
fn song_credit(track: &TrackSummary) -> Option<(String, String)> {
    let title = track
        .title
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    if title.is_empty() || title == "untitled" {
        return None;
    }
    let performer = Boundary::AlbumArtist.value(&Metadata {
        year: None,
        album_artist: track
            .display_artist
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(&track.artist),
        genre: None,
    })?;
    Some((title, performer.to_lowercase()))
}

struct Entry {
    track: TrackSummary,
    features: String,
    size: u64,
    modified: String,
    hash: String,
}

fn entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<Entry> {
    let size: i64 = row.get(18)?;
    Ok(Entry {
        track: catalog::map_track_row(row)?,
        features: row.get(17)?,
        size: u64::try_from(size)
            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(18, size))?,
        modified: row.get(19)?,
        hash: row.get(20)?,
    })
}

pub(crate) fn query(request: WorldRequest, store: &StateStore) -> Result<WorldResponse, String> {
    let path = catalog::default_catalog_path()?;
    let has = path
        .parent()
        .ok_or("Catalog directory missing")?
        .join("music-analysis.sqlite3")
        .is_file();
    let c = catalog::open_catalog(&path)?;
    run(&c, has, &request, Some(store), true)
}

fn run(
    c: &Connection,
    has: bool,
    request: &WorldRequest,
    store: Option<&StateStore>,
    indexed: bool,
) -> Result<WorldResponse, String> {
    if request.seed_key.is_empty()
        || request.seed_key.len() > 4096
        || request.exclude_keys.len() > 300
        || request
            .exclude_keys
            .iter()
            .any(|k| k.is_empty() || k.len() > 4096)
    {
        return Err("Invalid or oversized sound challenge".into());
    }
    let mut response = WorldResponse {
        version: worlds::VERSION.into(),
        seed_ready: false,
        seed_value: None,
        max_distance: worlds::MAX_DISTANCE,
        search_limited: indexed,
        matches: vec![],
    };
    if !has {
        return Ok(response);
    }
    let tx = c.unchecked_transaction().map_err(|e| e.to_string())?;
    let c = &*tx;
    let columns = format!(
        "{},a.features,s.size,s.modified,s.audio_hash",
        sonic::SELECT
    );
    let join = "FROM sonic.sonic_tracks s JOIN tracks t ON t.file_path=s.directory AND t.filename=s.filename JOIN sonic.sonic_audio a USING(audio_hash,profile)";
    let seed = c.query_row(&format!("SELECT {columns} {join} WHERE s.track_key=?1 AND s.profile=?2 AND lower(t.filename) LIKE '%.mp3'"), params![request.seed_key, PROFILE], entry).optional().map_err(|e|e.to_string())?;
    let Some(mut seed) = seed else {
        return Ok(response);
    };
    catalog::apply_overlays(std::slice::from_mut(&mut seed.track), store)?;
    if seed.track.love_state == LoveState::Banned
        || !file_is_current(
            &seed.track.directory,
            &seed.track.filename,
            seed.size,
            &seed.modified,
        )
    {
        return Ok(response);
    }
    let weights: String = c
        .query_row(
            "SELECT weights FROM sonic.sonic_profiles WHERE profile=?1",
            [PROFILE],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let analysis = Analysis {
        profile: PROFILE.into(),
        features: serde_json::from_str(&seed.features).map_err(|e| e.to_string())?,
        weights: serde_json::from_str(&weights).map_err(|e| e.to_string())?,
    };
    let metric = Metric::new(&analysis)
        .ok_or("Incompatible audio analysis. Update Music Library and reanalyze this track.")?;
    response.seed_ready = true;
    response.seed_value = value(request.boundary, &seed.track);
    if request.direction != Direction::Either && response.seed_value.is_none() {
        return Err("The seed has no reliable catalog value for this boundary. Choose another boundary or Either. Decades use album year, not release year.".into());
    }
    let candidates = if indexed {
        Some(crate::sonic_index::candidates(c, &analysis, std::slice::from_ref(&analysis.features), false, 4096, None, &Default::default())
            .ok_or("The sound index is not ready. Open Music Library, allow its index to finish, then retry.")?)
    } else {
        None
    };
    let selection = if candidates.is_some() {
        " AND s.track_key IN (SELECT value FROM json_each(?2))"
    } else {
        ""
    };
    let mut values = vec![rusqlite::types::Value::Text(PROFILE.into())];
    if let Some(candidates) = candidates {
        values.push(rusqlite::types::Value::Text(candidates.keys));
    }
    let mut q = c.prepare(&format!("SELECT {columns} {join} WHERE s.profile=?1 AND lower(t.filename) LIKE '%.mp3'{selection}")).map_err(|e|e.to_string())?;
    let entries = q
        .query_map(rusqlite::params_from_iter(values), entry)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let (mut tracks, evidence): (Vec<_>, Vec<_>) = entries
        .into_iter()
        .map(|e| (e.track, (e.features, e.size, e.modified, e.hash)))
        .unzip();
    catalog::apply_overlays(&mut tracks, store)?;
    let excluded: HashSet<_> = request.exclude_keys.iter().collect();
    let mut ranked = vec![];
    for (track, (json, size, modified, hash)) in tracks.into_iter().zip(evidence) {
        if track.track_key == request.seed_key
            || excluded.contains(&track.track_key)
            || track.love_state == LoveState::Banned
            || hash.is_empty()
            || hash == seed.hash
        {
            continue;
        }
        let candidate_value = value(request.boundary, &track);
        if !worlds::accepts(
            request.direction,
            response.seed_value.as_deref(),
            candidate_value.as_deref(),
        ) {
            continue;
        }
        let Ok(features) = serde_json::from_str::<Vec<f32>>(&json) else {
            continue;
        };
        let Some(distance) = metric
            .distance(&features)
            .filter(|d| *d <= worlds::MAX_DISTANCE)
        else {
            continue;
        };
        ranked.push((
            WorldMatch {
                track,
                value: candidate_value,
                distance,
            },
            size,
            modified,
            hash,
        ));
    }
    ranked.sort_by(|a, b| {
        a.0.distance
            .total_cmp(&b.0.distance)
            .then(a.0.track.track_key.cmp(&b.0.track.track_key))
    });
    let mut hashes = HashSet::new();
    let mut albums: HashSet<_> = seed.track.album_id.clone().into_iter().collect();
    let mut songs: HashSet<_> = song_credit(&seed.track).into_iter().collect();
    for (candidate, size, modified, hash) in ranked {
        let song = song_credit(&candidate.track);
        if hashes.contains(&hash)
            || candidate
                .track
                .album_id
                .as_ref()
                .is_some_and(|id| albums.contains(id))
            || song.as_ref().is_some_and(|key| songs.contains(key))
            || !file_is_current(
                &candidate.track.directory,
                &candidate.track.filename,
                size,
                &modified,
            )
        {
            continue;
        }
        hashes.insert(hash);
        if let Some(album) = &candidate.track.album_id {
            albums.insert(album.clone());
        }
        if let Some(song) = song {
            songs.insert(song);
        }
        response.matches.push(candidate);
        if response.matches.len() == 3 {
            break;
        }
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use music_sonic_core::{DIMENSIONS, file_signature, track_key};

    fn fixture() -> (tempfile::TempDir, Connection, WorldRequest) {
        let dir = tempfile::tempdir().unwrap();
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE tracks(id INTEGER PRIMARY KEY,title TEXT,album_artist_display TEXT,album TEXT,release_year INTEGER,normalized_rating INTEGER,love TEXT,time_seconds INTEGER,canonical_genre TEXT,album_id TEXT,file_path TEXT,filename TEXT,import_run_id INTEGER,display_artist TEXT,year INTEGER,publisher TEXT);
            ATTACH DATABASE ':memory:' AS sonic;CREATE TABLE sonic.sonic_profiles(profile TEXT PRIMARY KEY,weights TEXT);
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
        for id in 0..10 {
            let filename = format!("{id}.mp3");
            let path = dir.path().join(&filename);
            std::fs::write(&path, [255; 256]).unwrap();
            let (size, modified) = file_signature(&path).unwrap();
            // Identical release years deliberately disagree with album years.
            c.execute("INSERT INTO tracks VALUES(?1,?2,?3,'Album',2026,80,NULL,180,?4,?2,?5,?6,10,'Track singer',?7,'Label')",
                params![id, format!("Song {id}"), if id % 2 == 0 { "Artist A" } else { "Artist B" }, if id % 2 == 0 { "Pop" } else { "Rock" }, dir.path().to_string_lossy(), filename, if id % 2 == 0 { 1980 } else { 1990 }]).unwrap();
            let mut features = vec![0.; DIMENSIONS];
            features[0] = id as f32 * 0.03;
            c.execute(
                "INSERT INTO sonic.sonic_audio VALUES(?1,?2,?3)",
                params![
                    format!("hash{id}"),
                    PROFILE,
                    serde_json::to_string(&features).unwrap()
                ],
            )
            .unwrap();
            c.execute(
                "INSERT INTO sonic.sonic_tracks VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    track_key(&dir.path().to_string_lossy(), &filename),
                    dir.path().to_string_lossy(),
                    filename,
                    format!("hash{id}"),
                    PROFILE,
                    size as i64,
                    modified
                ],
            )
            .unwrap();
        }
        let request = WorldRequest {
            seed_key: track_key(&dir.path().to_string_lossy(), "0.mp3"),
            boundary: Boundary::Decade,
            direction: Direction::Cross,
            exclude_keys: vec![],
        };
        (dir, c, request)
    }

    fn ids(response: WorldResponse) -> Vec<String> {
        response.matches.into_iter().map(|m| m.track.id).collect()
    }

    #[test]
    fn boundaries_apply_before_the_three_result_limit_and_use_album_year() {
        let (_dir, c, mut request) = fixture();
        c.pragma_update(None, "query_only", true).unwrap();
        for boundary in [Boundary::Decade, Boundary::AlbumArtist, Boundary::Genre] {
            request.boundary = boundary;
            request.direction = Direction::Cross;
            assert_eq!(
                ids(run(&c, true, &request, None, false).unwrap()),
                ["1", "3", "5"]
            );
            request.direction = Direction::Stay;
            assert_eq!(
                ids(run(&c, true, &request, None, false).unwrap()),
                ["2", "4", "6"]
            );
            request.direction = Direction::Either;
            assert_eq!(
                ids(run(&c, true, &request, None, false).unwrap()),
                ["1", "2", "3"]
            );
        }
        assert!(c.execute("DELETE FROM sonic.sonic_tracks", []).is_err());
    }

    #[test]
    fn missing_metadata_stale_files_bans_and_bad_vectors_never_satisfy_crossing() {
        let (dir, c, mut request) = fixture();
        c.execute("UPDATE tracks SET year=NULL WHERE id=1", [])
            .unwrap();
        c.execute("UPDATE tracks SET love='B' WHERE id=3", [])
            .unwrap();
        std::fs::write(dir.path().join("5.mp3"), [255; 257]).unwrap();
        c.execute(
            "UPDATE sonic.sonic_audio SET features='[1,2]' WHERE audio_hash='hash7'",
            [],
        )
        .unwrap();
        assert!(
            run(&c, true, &request, None, false)
                .unwrap()
                .matches
                .is_empty()
        ); // 9 is beyond distance ceiling.
        c.execute("UPDATE tracks SET year=NULL WHERE id=0", [])
            .unwrap();
        assert!(
            run(&c, true, &request, None, false)
                .unwrap_err()
                .contains("no reliable catalog value")
        );
        request.direction = Direction::Either;
        assert!(
            !run(&c, true, &request, None, false)
                .unwrap()
                .matches
                .is_empty()
        );
        std::fs::remove_file(dir.path().join("0.mp3")).unwrap();
        assert!(!run(&c, true, &request, None, false).unwrap().seed_ready);
    }

    #[test]
    fn avoids_identical_recordings_and_previous_round_keys_and_honors_pending_bans() {
        let (dir, c, mut request) = fixture();
        c.execute(
            "UPDATE sonic.sonic_tracks SET audio_hash='hash0' WHERE filename='1.mp3'",
            [],
        )
        .unwrap();
        c.execute(
            "UPDATE sonic.sonic_tracks SET audio_hash='hash3' WHERE filename='5.mp3'",
            [],
        )
        .unwrap();
        assert_eq!(
            ids(run(&c, true, &request, None, false).unwrap()),
            ["3", "7"]
        );
        request
            .exclude_keys
            .push(track_key(&dir.path().to_string_lossy(), "7.mp3"));
        let store = StateStore::new(dir.path().join("state.sqlite3")).unwrap();
        let before = crate::tag_model::TagValues {
            rating: Some(4.),
            love_state: LoveState::Neutral,
            release_year: Some(2026),
        };
        let desired = crate::tag_model::TagValues {
            love_state: LoveState::Banned,
            ..before.clone()
        };
        for id in [3, 5] {
            store
                .upsert_overlay(
                    &track_key(&dir.path().to_string_lossy(), &format!("{id}.mp3")),
                    &dir.path().to_string_lossy(),
                    &format!("{id}.mp3"),
                    &before,
                    &desired,
                    10,
                    Some(id),
                )
                .unwrap();
        }
        assert!(
            run(&c, true, &request, Some(&store), false)
                .unwrap()
                .matches
                .is_empty()
        );
        request.exclude_keys = vec!["x".repeat(4097)];
        assert!(run(&c, true, &request, None, false).is_err());
    }

    #[test]
    fn reports_missing_analysis_or_index_without_unbounded_production_scan() {
        let (_dir, c, request) = fixture();
        assert!(!run(&c, false, &request, None, true).unwrap().seed_ready);
        assert!(
            run(&c, true, &request, None, true)
                .unwrap_err()
                .contains("index is not ready")
        );
        c.execute(
            "UPDATE sonic.sonic_tracks SET profile='old' WHERE filename='0.mp3'",
            [],
        )
        .unwrap();
        assert!(!run(&c, true, &request, None, false).unwrap().seed_ready);
    }

    #[test]
    fn spreads_albums_and_excludes_repeated_song_credits_without_claiming_audio_identity() {
        let (_dir, c, request) = fixture();
        // Different payload, title/performer match on a newer compilation.
        c.execute(
            "UPDATE tracks SET title='  SONG   0 ',display_artist='track SINGER' WHERE id=1",
            [],
        )
        .unwrap();
        // Another encoding in the seed album cannot masquerade as a discovery.
        c.execute("UPDATE tracks SET album_id='Song 0' WHERE id=3", [])
            .unwrap();
        // Two close tracks from one album get just one slot.
        c.execute("UPDATE tracks SET album_id='Song 5' WHERE id=7", [])
            .unwrap();
        assert_eq!(ids(run(&c, true, &request, None, false).unwrap()), ["5"]);
        c.execute(
            "UPDATE tracks SET display_artist='A different performer',album_id='other' WHERE id=1",
            [],
        )
        .unwrap();
        assert_eq!(
            ids(run(&c, true, &request, None, false).unwrap()),
            ["1", "5"]
        );
    }

    #[test]
    #[ignore = "read-only live catalog probe; run explicitly"]
    fn live_worlds_probe() {
        let c = catalog::open_catalog(&catalog::default_catalog_path().unwrap()).unwrap();
        let mut q = c.prepare("SELECT s.track_key FROM tracks t JOIN sonic.sonic_tracks s ON s.directory=t.file_path AND s.filename=t.filename WHERE s.profile=?1 AND t.normalized_rating=100 AND t.year BETWEEN 1970 AND 1989 AND COALESCE(t.love,'')!='B' LIMIT 3").unwrap();
        let seeds = q
            .query_map([PROFILE], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(!seeds.is_empty());
        for seed_key in seeds {
            for direction in [Direction::Stay, Direction::Cross] {
                let request = WorldRequest {
                    seed_key: seed_key.clone(),
                    boundary: Boundary::Decade,
                    direction,
                    exclude_keys: vec![],
                };
                let start = std::time::Instant::now();
                let result = run(&c, true, &request, None, true).unwrap();
                println!(
                    "{} {:?} {:.3}s {}",
                    request.seed_key,
                    direction,
                    start.elapsed().as_secs_f64(),
                    serde_json::to_string(&result).unwrap()
                );
                assert!(result.seed_ready);
                assert!(result.matches.iter().all(|m| worlds::accepts(
                    direction,
                    result.seed_value.as_deref(),
                    m.value.as_deref()
                ) && m.distance <= worlds::MAX_DISTANCE));
            }
        }
    }
}
