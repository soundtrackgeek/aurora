//! Read-only sonic discovery over Music Library's derived analysis store.
use crate::{
    catalog::{self, TrackSummary},
    state_store::StateStore,
    tag_model::LoveState,
};
use music_sonic_core::{Analysis, Metric, PROFILE, file_is_current};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SonicRequest {
    pub seed_key: String,
    #[serde(default)]
    pub seed_album_id: Option<String>,
    #[serde(default = "default_coverage")]
    pub minimum_coverage: u32,
    pub exclude_keys: Vec<String>,
    pub limit: usize,
    pub minimum_rating: Option<f64>,
    pub same_genre: bool,
    pub radio: bool,
}
fn default_coverage() -> u32 {
    50
}

fn journey_overrides(store: &StateStore) -> Result<crate::sonic_journey::Overrides, String> {
    Ok(store
        .all_overlays()?
        .into_iter()
        .map(|o| {
            (
                o.track_key,
                (
                    o.values.love_state == LoveState::Banned,
                    o.values.rating.map(|v| (v * 20.).round() as i32),
                ),
            )
        })
        .collect())
}
fn journey_catalog() -> Result<(Connection, bool), String> {
    let path = catalog::default_catalog_path()?;
    let c = catalog::open_catalog(&path)?;
    let has = path
        .parent()
        .ok_or("Catalog directory missing")?
        .join("music-analysis.sqlite3")
        .is_file();
    Ok((c, has))
}
pub(crate) fn journey_query(
    request: crate::sonic_journey::JourneyRequest,
    store: &StateStore,
) -> Result<crate::sonic_journey::JourneyResponse, String> {
    let (c, has) = journey_catalog()?;
    crate::sonic_journey::query(&c, has, &request, &journey_overrides(store)?)
}
pub(crate) fn journey_search(
    text: &str,
    store: &StateStore,
) -> Result<Vec<crate::sonic_journey::JourneyTrack>, String> {
    let (c, has) = journey_catalog()?;
    crate::sonic_journey::search(&c, has, text, &journey_overrides(store)?)
}
pub(crate) fn validate_journey_save(
    input: &crate::sonic_journey::SaveJourneyRequest,
    store: &StateStore,
) -> Result<(), String> {
    let (c, has) = journey_catalog()?;
    if !has {
        return Err("Analyze your chosen stops and more music in Music Library first.".into());
    }
    let tx = c.unchecked_transaction().map_err(|e| e.to_string())?;
    crate::sonic_journey::reviewed(
        &tx,
        &input.journey,
        &input.track_keys,
        &journey_overrides(store)?,
    )?;
    Ok(())
}

fn ban_overrides(
    store: Option<&StateStore>,
) -> Result<std::collections::HashMap<String, bool>, String> {
    Ok(store
        .map(StateStore::all_overlays)
        .transpose()?
        .unwrap_or_default()
        .into_iter()
        .map(|o| (o.track_key, o.values.love_state == LoveState::Banned))
        .collect())
}

pub(crate) fn album_query(
    request: crate::sonic_albums::SonicAlbumRequest,
    store: &StateStore,
) -> Result<crate::sonic_albums::SonicAlbumMatches, String> {
    let path = catalog::default_catalog_path()?;
    let c = catalog::open_catalog(&path)?;
    let has_analysis = path
        .parent()
        .ok_or("Catalog directory missing")?
        .join("music-analysis.sqlite3")
        .is_file();
    crate::sonic_albums::query(&c, has_analysis, &request, &ban_overrides(Some(store))?)
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SonicResponse {
    pub analyzed: i64,
    pub total: i64,
    pub seed_ready: bool,
    pub tracks: Vec<TrackSummary>,
}

const SELECT: &str = "t.id,t.title,t.album_artist_display,t.album,t.release_year,t.normalized_rating,t.love,t.time_seconds,t.canonical_genre,NULL,t.album_id,t.file_path,t.filename,t.import_run_id,t.display_artist AS display_artist,t.year AS original_year,t.publisher AS publisher";

/// Search only reads existing profile results; an empty projection makes
/// sonic:no work before the first analysis without creating a results store.
pub(crate) fn prepare_search(c: &Connection, catalog_path: &std::path::Path) -> Result<(), String> {
    let analysis = catalog_path
        .parent()
        .ok_or("Catalog directory missing")?
        .join("music-analysis.sqlite3");
    if analysis.is_file() {
        c.execute(
            "ATTACH DATABASE ?1 AS sonic",
            [analysis.to_string_lossy().as_ref()],
        )
        .map_err(|e| e.to_string())?;
        c.execute_batch(&format!("CREATE TEMP VIEW aurora_sonic_paths AS SELECT directory AS file_path,filename FROM sonic.sonic_tracks WHERE profile='{PROFILE}';")).map_err(|e|e.to_string())?;
    } else {
        c.execute_batch("CREATE TEMP TABLE aurora_sonic_paths(file_path TEXT,filename TEXT,PRIMARY KEY(file_path,filename));").map_err(|e|e.to_string())?;
    }
    Ok(())
}

pub(crate) fn run(
    c: &Connection,
    request: &SonicRequest,
    store: Option<&StateStore>,
) -> Result<SonicResponse, String> {
    if !(1..=100).contains(&request.limit)
        || request.seed_key.len() > 4096
        || request
            .seed_album_id
            .as_ref()
            .is_some_and(|id| id.is_empty() || id.len() > 4096)
        || !(50..=100).contains(&request.minimum_coverage)
        || request.exclude_keys.len() > 2000
        || request.exclude_keys.iter().any(|k| k.len() > 4096)
        || request
            .minimum_rating
            .is_some_and(|r| !r.is_finite() || !(0.0..=5.0).contains(&r))
    {
        return Err("Invalid or oversized sonic request".into());
    }
    let total = c
        .query_row(
            "SELECT count(*) FROM tracks WHERE lower(filename) LIKE '%.mp3'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let mut response = SonicResponse {
        analyzed: 0,
        total,
        seed_ready: false,
        tracks: vec![],
    };
    let (metric, seed_genre) = if let Some(album_id) = &request.seed_album_id {
        let transaction = c.unchecked_transaction().map_err(|e| e.to_string())?;
        let (album, analysis) = crate::sonic_albums::seed(
            &transaction,
            album_id,
            request.minimum_coverage,
            &ban_overrides(store)?,
        )?;
        let Some(analysis) = analysis else {
            return Ok(response);
        };
        let metric = Metric::new(&analysis).ok_or("Incompatible album analysis profile")?;
        (metric, album.and_then(|a| a.genre))
    } else {
        let seed=c.query_row(&format!("SELECT {SELECT},a.features,p.weights,s.size,s.modified FROM sonic.sonic_tracks s JOIN sonic.sonic_audio a USING(audio_hash,profile) JOIN sonic.sonic_profiles p USING(profile) JOIN tracks t ON s.directory=t.file_path AND s.filename=t.filename WHERE s.track_key=?1 AND s.profile=?2"),params![request.seed_key,PROFILE],|r|Ok((catalog::map_track_row(r)?,r.get::<_,String>(17)?,r.get::<_,String>(18)?,r.get::<_,i64>(19)? as u64,r.get::<_,String>(20)?))).optional().map_err(|e|e.to_string())?;
        let Some((mut seed_track, features, weights, size, modified)) = seed else {
            return Ok(response);
        };
        if !file_is_current(&seed_track.directory, &seed_track.filename, size, &modified) {
            return Ok(response);
        }
        catalog::apply_overlays(std::slice::from_mut(&mut seed_track), store)?;
        let seed = Analysis {
            profile: PROFILE.into(),
            features: serde_json::from_str(&features).map_err(|e| e.to_string())?,
            weights: serde_json::from_str(&weights).map_err(|e| e.to_string())?,
        };
        let metric = Metric::new(&seed).ok_or(
            "Incompatible sonic analysis profile. Update Music Library and reanalyze the seed.",
        )?;
        (metric, seed_track.genre)
    };
    response.seed_ready = true;
    let mut q=c.prepare(&format!("SELECT {SELECT},a.features,s.size,s.modified FROM tracks t JOIN sonic.sonic_tracks s ON s.directory=t.file_path AND s.filename=t.filename JOIN sonic.sonic_audio a USING(audio_hash,profile) WHERE s.profile=?1")).map_err(|e|e.to_string())?;
    let rows = q
        .query_map([PROFILE], |r| {
            Ok((
                catalog::map_track_row(r)?,
                r.get::<_, String>(17)?,
                r.get::<_, i64>(18)? as u64,
                r.get::<_, String>(19)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let excluded: HashSet<&str> = request
        .exclude_keys
        .iter()
        .map(String::as_str)
        .chain(std::iter::once(request.seed_key.as_str()))
        .collect();
    let mut ranked: Vec<(f64, TrackSummary, u64, String)> = vec![];
    // Apply current Aurora overlays in bounded batches before top-k selection.
    let mut batch: Vec<(TrackSummary, String, u64, String)> = vec![];
    let mut rank_batch =
        |batch: &mut Vec<(TrackSummary, String, u64, String)>| -> Result<(), String> {
            let entries = std::mem::take(batch);
            let (mut tracks, observations): (Vec<_>, Vec<_>) = entries
                .into_iter()
                .map(|(track, features, size, modified)| (track, (features, size, modified)))
                .unzip();
            catalog::apply_overlays(&mut tracks, store)?;
            for (track, (features, size, modified)) in tracks.into_iter().zip(observations) {
                response.analyzed += 1;
                if excluded.contains(track.track_key.as_str())
                    || request
                        .seed_album_id
                        .as_ref()
                        .is_some_and(|id| track.album_id.as_ref() == Some(id))
                    || track.love_state == LoveState::Banned
                    || request
                        .minimum_rating
                        .is_some_and(|r| track.rating.is_none_or(|v| v < r))
                    || request.same_genre
                        && seed_genre
                            .as_ref()
                            .is_none_or(|g| track.genre.as_ref() != Some(g))
                {
                    continue;
                }
                let features: Vec<f32> =
                    serde_json::from_str(&features).map_err(|e| e.to_string())?;
                if let Some(d) = metric.distance(&features) {
                    ranked.push((d, track, size, modified));
                }
            }
            if ranked.len() > 500 {
                prune(&mut ranked, request.radio);
            }
            Ok(())
        };
    for row in rows {
        batch.push(row.map_err(|e| e.to_string())?);
        if batch.len() == 500 {
            rank_batch(&mut batch)?;
        }
    }
    rank_batch(&mut batch)?;
    if request.same_genre {
        ranked.retain(|(_, t, _, _)| {
            seed_genre
                .as_ref()
                .is_some_and(|g| t.genre.as_ref() == Some(g))
        });
    }
    ranked.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.track_key.cmp(&b.1.track_key)));
    let mut artists = std::collections::HashMap::<String, usize>::new();
    let mut albums = std::collections::HashMap::<String, usize>::new();
    for (_, track, size, modified) in ranked {
        if !file_is_current(&track.directory, &track.filename, size, &modified) {
            continue;
        }
        let artist = track
            .display_artist
            .as_ref()
            .unwrap_or(&track.artist)
            .to_lowercase();
        let album = track
            .album_id
            .clone()
            .unwrap_or_else(|| track.album.clone());
        if request.radio
            && (*artists.get(&artist).unwrap_or(&0) >= 3 || *albums.get(&album).unwrap_or(&0) >= 2)
        {
            continue;
        }
        *artists.entry(artist).or_default() += 1;
        *albums.entry(album).or_default() += 1;
        response.tracks.push(track);
        if response.tracks.len() == request.limit {
            break;
        }
    }
    Ok(response)
}

pub(crate) fn query(request: SonicRequest, store: &StateStore) -> Result<SonicResponse, String> {
    let path = catalog::default_catalog_path()?;
    let c = catalog::open_catalog(&path)?;
    let analysis = path
        .parent()
        .ok_or("Catalog directory missing")?
        .join("music-analysis.sqlite3");
    if !analysis.is_file() {
        let total = c
            .query_row("SELECT count(*) FROM tracks", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        return Ok(SonicResponse {
            analyzed: 0,
            total,
            seed_ready: false,
            tracks: vec![],
        });
    }
    run(&c, &request, Some(store))
}

fn prune(ranked: &mut Vec<(f64, TrackSummary, u64, String)>, radio: bool) {
    ranked.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.track_key.cmp(&b.1.track_key)));
    if radio {
        let mut artists = std::collections::HashMap::<String, usize>::new();
        let mut albums = std::collections::HashMap::<String, usize>::new();
        ranked.retain(|(_, t, _, _)| {
            let artist = t
                .display_artist
                .as_ref()
                .unwrap_or(&t.artist)
                .to_lowercase();
            let album = t.album_id.clone().unwrap_or_else(|| t.album.clone());
            let a = artists.entry(artist).or_default();
            let b = albums.entry(album).or_default();
            if *a >= 3 || *b >= 2 {
                return false;
            }
            *a += 1;
            *b += 1;
            true
        });
    }
    ranked.truncate(400);
}

#[cfg(test)]
mod tests {
    use super::*;
    use music_sonic_core::{DIMENSIONS, file_signature, track_key};
    fn fixture() -> (tempfile::TempDir, Connection, StateStore) {
        let dir = tempfile::tempdir().unwrap();
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE tracks(id INTEGER PRIMARY KEY,title TEXT,album_artist_display TEXT,album TEXT,release_year INTEGER,normalized_rating INTEGER,love TEXT,time_seconds INTEGER,canonical_genre TEXT,album_id TEXT,file_path TEXT,filename TEXT,import_run_id INTEGER,display_artist TEXT,year INTEGER,publisher TEXT);
            ATTACH DATABASE ':memory:' AS sonic;CREATE TABLE sonic.sonic_profiles(profile TEXT PRIMARY KEY,weights TEXT);
            CREATE TABLE sonic.sonic_audio(audio_hash TEXT,profile TEXT,features TEXT,PRIMARY KEY(audio_hash,profile));
            CREATE TABLE sonic.sonic_tracks(track_key TEXT PRIMARY KEY,directory TEXT,filename TEXT,audio_hash TEXT,profile TEXT,size INTEGER,modified TEXT);
            CREATE TEMP VIEW aurora_sonic_paths AS SELECT directory AS file_path,filename FROM sonic.sonic_tracks;").unwrap();
        let mut weights = vec![0f32; DIMENSIONS * DIMENSIONS];
        for i in 0..DIMENSIONS {
            weights[i * DIMENSIONS + i] = 1.;
        }
        c.execute(
            "INSERT INTO sonic.sonic_profiles VALUES(?1,?2)",
            params![PROFILE, serde_json::to_string(&weights).unwrap()],
        )
        .unwrap();
        for id in 1..=4 {
            let filename = format!("{id}.mp3");
            let path = dir.path().join(&filename);
            std::fs::write(&path, [255; 256]).unwrap();
            c.execute("INSERT INTO tracks VALUES(?1,'Song','Various Artists','Album',2020,80,NULL,180,'Pop',?2,?3,?4,10,'Singer',2020,'Label')",params![id,format!("album{id}"),dir.path().to_string_lossy(),filename]).unwrap();
            if id == 4 {
                continue;
            }
            let (size, modified) = file_signature(&path).unwrap();
            let mut features = vec![0f32; DIMENSIONS];
            features[0] = id as f32;
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
        let store = StateStore::new(dir.path().join("state.sqlite3")).unwrap();
        (dir, c, store)
    }
    fn request(dir: &std::path::Path) -> SonicRequest {
        SonicRequest {
            seed_key: track_key(&dir.to_string_lossy(), "1.mp3"),
            seed_album_id: None,
            minimum_coverage: 50,
            exclude_keys: vec![],
            limit: 10,
            minimum_rating: None,
            same_genre: false,
            radio: false,
        }
    }
    #[test]
    fn rank_uses_current_ids_overlays_and_partial_coverage() {
        let (dir, c, store) = fixture();
        c.execute("UPDATE tracks SET id=900 WHERE id=3", [])
            .unwrap();
        let key = track_key(&dir.path().to_string_lossy(), "2.mp3");
        let before = crate::tag_model::TagValues {
            rating: Some(4.),
            love_state: LoveState::Neutral,
            release_year: Some(2020),
        };
        let desired = crate::tag_model::TagValues {
            love_state: LoveState::Banned,
            ..before.clone()
        };
        store
            .upsert_overlay(
                &key,
                &dir.path().to_string_lossy(),
                "2.mp3",
                &before,
                &desired,
                10,
                Some(2),
            )
            .unwrap();
        let result = run(&c, &request(dir.path()), Some(&store)).unwrap();
        assert!(result.seed_ready);
        assert_eq!(result.analyzed, 3);
        assert_eq!(result.total, 4);
        assert_eq!(result.tracks.len(), 1);
        assert_eq!(result.tracks[0].id, "900");
        let mut filtered = request(dir.path());
        filtered.minimum_rating = Some(5.);
        assert!(run(&c, &filtered, Some(&store)).unwrap().tracks.is_empty());
        filtered.minimum_rating = None;
        filtered.exclude_keys = vec![result.tracks[0].track_key.clone()];
        assert!(run(&c, &filtered, Some(&store)).unwrap().tracks.is_empty());
        std::fs::write(dir.path().join("1.mp3"), [255; 257]).unwrap();
        assert!(
            !run(&c, &request(dir.path()), Some(&store))
                .unwrap()
                .seed_ready
        );
    }
    #[test]
    fn album_radio_uses_the_mean_and_excludes_every_seed_album_track() {
        let (dir, c, store) = fixture();
        c.execute(
            "UPDATE tracks SET album_id='seed-album' WHERE id IN (1,2)",
            [],
        )
        .unwrap();
        let mut album_request = request(dir.path());
        album_request.seed_key.clear();
        album_request.seed_album_id = Some("seed-album".into());
        album_request.minimum_coverage = 100;
        album_request.radio = true;
        let result = run(&c, &album_request, Some(&store)).unwrap();
        assert!(result.seed_ready);
        assert_eq!(
            result
                .tracks
                .iter()
                .map(|t| t.id.as_str())
                .collect::<Vec<_>>(),
            vec!["3"]
        );
        std::fs::write(dir.path().join("2.mp3"), [255; 257]).unwrap();
        assert!(!run(&c, &album_request, Some(&store)).unwrap().seed_ready);
    }
    #[test]
    fn sonic_search_selects_only_saved_paths_and_supports_negation() {
        let (_dir, c, _) = fixture();
        for (query, expected) in [
            ("sonic:yes", 3),
            ("sonic:no", 1),
            ("NOT sonic:yes", 1),
            ("sonic=yes OR sonic:no", 4),
        ] {
            let search = catalog::parse_catalog_search(query).unwrap();
            let mut params = vec![];
            let mut sql = "SELECT count(*) FROM tracks t WHERE 1=1".to_string();
            catalog::push_track_search_predicates(&mut sql, &mut params, &search);
            assert_eq!(
                c.query_row(&sql, rusqlite::params_from_iter(params), |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                expected,
                "{query}"
            );
        }
        assert!(catalog::parse_catalog_search("sonic:maybe").is_err());
    }
    #[test]
    fn search_without_analysis_does_not_create_results() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.sqlite3");
        let c = Connection::open(&path).unwrap();
        prepare_search(&c, &path).unwrap();
        assert_eq!(
            c.query_row("SELECT count(*) FROM temp.aurora_sonic_paths", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert!(!dir.path().join("music-analysis.sqlite3").exists());
    }
}
