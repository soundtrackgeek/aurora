//! Read-only journey adapter; keep aligned with Aurora. Math lives in sonic-core.
use music_sonic_core::{
    Analysis, JourneyBuilder, JourneyStop, PROFILE, file_is_current, track_key,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct JourneyRequest {
    pub stop_keys: Vec<String>,
    pub connecting_tracks: usize,
    /// Catalog scale, 0–100. Applies to connectors, not explicitly chosen stops.
    pub minimum_rating: Option<i32>,
    pub same_genre: bool,
}
impl JourneyRequest {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if !(2..=10).contains(&self.stop_keys.len())
            || !(1..=10).contains(&self.connecting_tracks)
            || self
                .stop_keys
                .iter()
                .any(|k| k.is_empty() || k.len() > 4096)
            || self.stop_keys.iter().collect::<HashSet<_>>().len() != self.stop_keys.len()
            || self.minimum_rating.is_some_and(|r| !(0..=100).contains(&r))
        {
            return Err(
                "Choose 2–10 different stops and 1–10 connecting tracks between stops.".into(),
            );
        }
        Ok(())
    }
    pub(crate) fn count(&self) -> usize {
        self.stop_keys.len() + (self.stop_keys.len() - 1) * self.connecting_tracks
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JourneyTrack {
    pub track_id: i64,
    pub track_key: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_id: String,
    pub album_artist: String,
    pub file_path: String,
    pub filename: String,
    pub genre: Option<String>,
    pub rating: Option<i32>,
    pub seconds: i64,
    pub loved: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JourneyResponse {
    pub stops_ready: Vec<bool>,
    pub analyzed: usize,
    pub complete: bool,
    pub tracks: Vec<JourneyTrack>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SaveJourneyRequest {
    pub journey: JourneyRequest,
    pub track_keys: Vec<String>,
    pub name: String,
}
#[derive(Clone)]
struct Entry {
    track: JourneyTrack,
    features: Vec<f32>,
    size: u64,
    modified: String,
    banned: bool,
}
pub(crate) type Overrides = HashMap<String, (bool, Option<i32>)>;
const SELECT: &str = "t.id,COALESCE(t.title,''),COALESCE(NULLIF(t.display_artist,''),t.album_artist_display,''),COALESCE(t.album,''),COALESCE(t.album_id,''),COALESCE(t.album_artist_display,''),t.file_path,t.filename,t.canonical_genre,t.normalized_rating,COALESCE(t.time_seconds,0),COALESCE(t.love,''),a.features,s.size,s.modified";
const JOIN: &str = "FROM sonic.sonic_tracks s JOIN tracks t ON t.file_path=s.directory AND t.filename=s.filename JOIN sonic.sonic_audio a ON a.audio_hash=s.audio_hash AND a.profile=s.profile";
fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Entry> {
    let directory: String = r.get(6)?;
    let filename: String = r.get(7)?;
    let love: String = r.get(11)?;
    let json: String = r.get(12)?;
    Ok(Entry {
        track: JourneyTrack {
            track_id: r.get(0)?,
            track_key: track_key(&directory, &filename),
            title: r.get(1)?,
            artist: r.get(2)?,
            album: r.get(3)?,
            album_id: r.get(4)?,
            album_artist: r.get(5)?,
            file_path: directory,
            filename,
            genre: r.get(8)?,
            rating: r.get(9)?,
            seconds: r.get(10)?,
            loved: love == "L",
        },
        features: serde_json::from_str(&json).unwrap_or_default(),
        size: r.get::<_, i64>(13)?.max(0) as u64,
        modified: r.get(14)?,
        banned: love == "B",
    })
}
fn apply(entry: &mut Entry, overrides: &Overrides) {
    if let Some((ban, rating)) = overrides.get(&entry.track.track_key) {
        entry.banned = *ban;
        entry.track.rating = *rating;
    }
}
fn fresh(entry: &Entry) -> bool {
    file_is_current(
        &entry.track.file_path,
        &entry.track.filename,
        entry.size,
        &entry.modified,
    )
}
fn load(c: &Connection, key: &str, overrides: &Overrides) -> Result<Option<Entry>, String> {
    let mut entry = c.query_row(&format!("SELECT {SELECT} {JOIN} WHERE s.track_key=?1 AND s.profile=?2 AND lower(t.filename) LIKE '%.mp3'"), params![key, PROFILE], row).optional().map_err(|e| e.to_string())?;
    if let Some(e) = &mut entry {
        apply(e, overrides);
    }
    Ok(entry)
}
fn analysis(c: &Connection, features: Vec<f32>) -> Result<Option<Analysis>, String> {
    let weights = c
        .query_row(
            "SELECT weights FROM sonic.sonic_profiles WHERE profile=?1",
            [PROFILE],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(weights
        .and_then(|json| serde_json::from_str(&json).ok())
        .map(|weights| Analysis {
            profile: PROFILE.into(),
            features,
            weights,
        })
        .filter(Analysis::valid))
}
pub(crate) fn query(
    c: &Connection,
    has_analysis: bool,
    request: &JourneyRequest,
    overrides: &Overrides,
) -> Result<JourneyResponse, String> {
    request.validate()?;
    let mut response = JourneyResponse {
        stops_ready: vec![false; request.stop_keys.len()],
        analyzed: 0,
        complete: false,
        tracks: vec![],
    };
    if !has_analysis {
        return Ok(response);
    }
    let tx = c.unchecked_transaction().map_err(|e| e.to_string())?;
    let mut stops = Vec::new();
    for (i, key) in request.stop_keys.iter().enumerate() {
        let Some(entry) = load(&tx, key, overrides)? else {
            continue;
        };
        if !entry.banned && fresh(&entry) && analysis(&tx, entry.features.clone())?.is_some() {
            response.stops_ready[i] = true;
            stops.push(JourneyStop {
                key: entry.track.track_key.clone(),
                features: entry.features.clone(),
                data: entry,
            });
        }
    }
    if response.stops_ready.iter().any(|ready| !ready) {
        return Ok(response);
    }
    let first_genre = stops[0].data.track.genre.clone();
    let weights =
        analysis(&tx, stops[0].features.clone())?.ok_or("Incompatible analysis profile")?;
    let mut builder = JourneyBuilder::new(&weights, stops, request.connecting_tracks)
        .ok_or("Invalid sonic journey")?;
    let mut q = tx
        .prepare(&format!(
            "SELECT {SELECT} {JOIN} WHERE s.profile=?1 AND lower(t.filename) LIKE '%.mp3'"
        ))
        .map_err(|e| e.to_string())?;
    for entry in q.query_map([PROFILE], row).map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        response.analyzed += 1;
        apply(&mut entry, overrides);
        if entry.banned
            || request
                .minimum_rating
                .is_some_and(|r| entry.track.rating.is_none_or(|v| v < r))
            || request.same_genre
                && first_genre
                    .as_ref()
                    .is_none_or(|g| entry.track.genre.as_ref() != Some(g))
        {
            continue;
        }
        builder.offer(&entry.track.track_key, &entry.features, entry.clone());
    }
    if let Some(entries) = builder.finish(fresh) {
        // Recheck chosen stops too, in case files changed while candidates streamed.
        if entries.iter().all(fresh) {
            response.tracks = entries.into_iter().map(|e| e.track).collect();
            response.complete = true;
        }
    }
    Ok(response)
}
pub(crate) fn search(
    c: &Connection,
    has_analysis: bool,
    text: &str,
    overrides: &Overrides,
) -> Result<Vec<JourneyTrack>, String> {
    if text.len() > 200 {
        return Err("Search with no more than 200 characters.".into());
    }
    if !has_analysis || text.trim().chars().count() < 2 {
        return Ok(vec![]);
    }
    let pattern = format!(
        "%{}%",
        text.trim()
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let mut q = c.prepare(&format!("SELECT {SELECT} {JOIN} WHERE s.profile=?1 AND lower(t.filename) LIKE '%.mp3' AND (t.title LIKE ?2 ESCAPE '\\' OR t.display_artist LIKE ?2 ESCAPE '\\' OR t.album_artist_display LIKE ?2 ESCAPE '\\') ORDER BY t.id DESC LIMIT 100")).map_err(|e| e.to_string())?;
    let mut tracks = Vec::new();
    for entry in q
        .query_map(params![PROFILE, pattern], row)
        .map_err(|e| e.to_string())?
    {
        let mut e = entry.map_err(|e| e.to_string())?;
        apply(&mut e, overrides);
        if !e.banned && fresh(&e) && analysis(c, e.features.clone())?.is_some() {
            tracks.push(e.track);
        }
        if tracks.len() == 20 {
            break;
        }
    }
    Ok(tracks)
}
/// Save exactly the reviewed sequence; never rerun recommendation selection.
pub(crate) fn reviewed(
    c: &Connection,
    request: &JourneyRequest,
    keys: &[String],
    overrides: &Overrides,
) -> Result<Vec<JourneyTrack>, String> {
    request.validate()?;
    if keys.len() != request.count()
        || keys.iter().collect::<HashSet<_>>().len() != keys.len()
        || keys.iter().any(|k| k.is_empty() || k.len() > 4096)
        || request
            .stop_keys
            .iter()
            .enumerate()
            .any(|(i, key)| &keys[i * (request.connecting_tracks + 1)] != key)
    {
        return Err("The journey changed. Build and review it again before saving.".into());
    }
    let mut result = Vec::new();
    let mut genre = None;
    for (i, key) in keys.iter().enumerate() {
        let entry = load(c, key, overrides)?
            .ok_or("A journey track is no longer analyzed. Build again.")?;
        let chosen = i % (request.connecting_tracks + 1) == 0;
        if i == 0 {
            genre = entry.track.genre.clone();
        }
        if entry.banned
            || !fresh(&entry)
            || analysis(c, entry.features.clone())?.is_none()
            || !chosen
                && (request
                    .minimum_rating
                    .is_some_and(|r| entry.track.rating.is_none_or(|v| v < r))
                    || request.same_genre
                        && genre
                            .as_ref()
                            .is_none_or(|g| entry.track.genre.as_ref() != Some(g)))
        {
            return Err(
                "A journey track changed or no longer qualifies. Build and review again.".into(),
            );
        }
        result.push(entry.track);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use music_sonic_core::{DIMENSIONS, file_signature};
    fn fixture() -> (tempfile::TempDir, Connection, JourneyRequest) {
        let dir = tempfile::tempdir().unwrap();
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE tracks(id INTEGER PRIMARY KEY,title TEXT,display_artist TEXT,album_artist_display TEXT,album TEXT,album_id TEXT,canonical_genre TEXT,file_path TEXT,filename TEXT,normalized_rating INTEGER,time_seconds INTEGER,love TEXT);
            ATTACH DATABASE ':memory:' AS sonic;CREATE TABLE sonic.sonic_profiles(profile TEXT PRIMARY KEY,weights TEXT);CREATE TABLE sonic.sonic_audio(audio_hash TEXT,profile TEXT,features TEXT,PRIMARY KEY(audio_hash,profile));CREATE TABLE sonic.sonic_tracks(track_key TEXT PRIMARY KEY,directory TEXT,filename TEXT,audio_hash TEXT,profile TEXT,size INTEGER,modified TEXT);").unwrap();
        let mut weights = vec![0f32; DIMENSIONS * DIMENSIONS];
        weights[0] = 4.;
        c.execute(
            "INSERT INTO sonic.sonic_profiles VALUES(?1,?2)",
            params![PROFILE, serde_json::to_string(&weights).unwrap()],
        )
        .unwrap();
        for i in 0..15 {
            let file = format!("{i}.mp3");
            let path = dir.path().join(&file);
            std::fs::write(&path, [255; 256]).unwrap();
            c.execute("INSERT INTO tracks VALUES(?1,?2,'Singer','Various Artists','Album','album','Pop',?3,?4,?5,180,?6)",params![i+1,format!("Song {i}"),dir.path().to_string_lossy(),file,if i==0 {20}else{80},if i==14 {"B"}else{""}]).unwrap();
            let mut features = vec![0f32; DIMENSIONS];
            features[0] = i as f32;
            let profile = if i == 13 { "old" } else { PROFILE };
            c.execute(
                "INSERT INTO sonic.sonic_audio VALUES(?1,?2,?3)",
                params![
                    file,
                    profile,
                    if i == 12 {
                        "invalid".into()
                    } else {
                        serde_json::to_string(&features).unwrap()
                    }
                ],
            )
            .unwrap();
            let (size, modified) = file_signature(&path).unwrap();
            c.execute(
                "INSERT INTO sonic.sonic_tracks VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    track_key(&dir.path().to_string_lossy(), &file),
                    dir.path().to_string_lossy(),
                    file,
                    file,
                    profile,
                    size as i64,
                    modified
                ],
            )
            .unwrap();
        }
        let request = JourneyRequest {
            stop_keys: [0, 3, 6, 9, 11]
                .map(|i| track_key(&dir.path().to_string_lossy(), &format!("{i}.mp3")))
                .to_vec(),
            connecting_tracks: 1,
            minimum_rating: Some(80),
            same_genre: true,
        };
        (dir, c, request)
    }
    #[test]
    fn five_stop_journey_rebinds_current_ids_and_saves_the_reviewed_order() {
        let (_dir, c, request) = fixture();
        c.execute("UPDATE tracks SET id=id+100", []).unwrap();
        c.execute_batch("PRAGMA query_only=ON;").unwrap();
        let response = query(&c, true, &request, &Overrides::new()).unwrap();
        assert!(response.complete);
        assert_eq!(response.tracks.len(), 9);
        assert_eq!(response.tracks[0].track_id, 101);
        assert_eq!(response.tracks[0].artist, "Singer");
        assert_eq!(response.tracks[0].album_artist, "Various Artists");
        let keys = response
            .tracks
            .iter()
            .map(|t| t.track_key.clone())
            .collect::<Vec<_>>();
        assert_eq!(keys.iter().collect::<HashSet<_>>().len(), 9);
        for (i, key) in request.stop_keys.iter().enumerate() {
            assert_eq!(&keys[i * 2], key);
        }
        assert_eq!(
            reviewed(&c, &request, &keys, &Overrides::new())
                .unwrap()
                .iter()
                .map(|t| t.track_key.clone())
                .collect::<Vec<_>>(),
            keys
        );
        let mut reordered = keys;
        reordered.swap(0, 2);
        assert!(reviewed(&c, &request, &reordered, &Overrides::new()).is_err());
        assert!(c.execute("DELETE FROM tracks", []).is_err());
        assert!(c.execute("DELETE FROM sonic.sonic_audio", []).is_err());
    }
    #[test]
    fn journeys_report_missing_analysis_and_reject_changed_or_banned_tracks() {
        let (dir, c, request) = fixture();
        assert!(
            !query(&c, false, &request, &Overrides::new())
                .unwrap()
                .complete
        );
        let response = query(&c, true, &request, &Overrides::new()).unwrap();
        let keys = response
            .tracks
            .iter()
            .map(|t| t.track_key.clone())
            .collect::<Vec<_>>();
        std::fs::write(dir.path().join(&response.tracks[1].filename), [255; 257]).unwrap();
        assert!(reviewed(&c, &request, &keys, &Overrides::new()).is_err());
        let banned = Overrides::from([(request.stop_keys[2].clone(), (true, Some(80)))]);
        let result = query(&c, true, &request, &banned).unwrap();
        assert!(!result.complete);
        assert!(!result.stops_ready[2]);
        assert!(result.tracks.is_empty());
        let mut filtered = request.clone();
        filtered.minimum_rating = Some(100);
        assert!(
            !query(&c, true, &filtered, &Overrides::new())
                .unwrap()
                .complete
        );
        filtered.stop_keys[1] = filtered.stop_keys[0].clone();
        assert!(query(&c, true, &filtered, &Overrides::new()).is_err());
    }
    #[test]
    fn endpoint_search_uses_literal_text_and_ignores_invalid_profiles_and_vectors() {
        let (_dir, c, request) = fixture();
        c.execute("UPDATE tracks SET title='Song %_One' WHERE id=1", [])
            .unwrap();
        let results = search(&c, true, "%_", &Overrides::new()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].track_key, request.stop_keys[0]);
        assert_eq!(
            search(&c, true, "Singer", &Overrides::new()).unwrap().len(),
            12
        );
        assert!(
            search(&c, false, "Singer", &Overrides::new())
                .unwrap()
                .is_empty()
        );
    }
}
