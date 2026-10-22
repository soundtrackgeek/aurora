//! Read-only, on-demand album structure. Keep the companion adapter aligned.
use crate::sonic_albums::SonicAlbum;
use music_sonic_core::{Analysis, Metric, PROFILE, album_sides, file_is_current, track_key};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter, types::Value};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AlbumSidesRequest {
    pub album_id: String,
    pub strand_key: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SideTrack {
    pub track_key: String,
    pub album_id: String,
    pub title: String,
    pub artist: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AlbumSide {
    pub representative: SideTrack,
    pub tracks: Vec<SideTrack>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SideAlbumMatch {
    pub album: SonicAlbum,
    pub matching_track: SideTrack,
    pub distance: f64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AlbumSidesResponse {
    pub version: String,
    pub total_tracks: usize,
    pub analyzed_tracks: usize,
    pub ready: bool,
    pub sides: Vec<AlbumSide>,
    pub contrasting_track: Option<SideTrack>,
    pub selected_key: Option<String>,
    pub matches: Vec<SideAlbumMatch>,
    pub search_limited: bool,
}
struct UsableTrack {
    track: SideTrack,
    features: Vec<f32>,
    hash: String,
}
fn load(
    c: &Connection,
    album: &str,
    has: bool,
    overrides: &HashMap<String, bool>,
) -> Result<(usize, Vec<UsableTrack>), String> {
    let total: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM tracks WHERE album_id=?1",
            [album],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if total > 5000 {
        return Ok((total as usize, vec![]));
    }
    if !has {
        return Ok((total as usize, vec![]));
    }
    let mut q=c.prepare("SELECT t.title,COALESCE(t.display_artist,t.album_artist_display,''),t.file_path,t.filename,COALESCE(t.love,''),a.features,s.size,s.modified,s.audio_hash
        FROM tracks t LEFT JOIN sonic.sonic_tracks s ON s.directory=t.file_path AND s.filename=t.filename AND s.profile=?2
        LEFT JOIN sonic.sonic_audio a ON a.audio_hash=s.audio_hash AND a.profile=s.profile
        WHERE t.album_id=?1 ORDER BY t.id").map_err(|e|e.to_string())?;
    let rows = q
        .query_map(params![album, PROFILE], |r| {
            Ok((
                r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<i64>>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<String>>(8)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut tracks = vec![];
    for row in rows {
        let (title, artist, directory, filename, love, json, size, modified, hash) =
            row.map_err(|e| e.to_string())?;
        let key = track_key(&directory, &filename);
        if *overrides.get(&key).unwrap_or(&(love == "B"))
            || !filename.to_ascii_lowercase().ends_with(".mp3")
        {
            continue;
        }
        let Some(features) = json
            .and_then(|j| serde_json::from_str::<Vec<f32>>(&j).ok())
            .filter(|f| f.len() == music_sonic_core::DIMENSIONS && f.iter().all(|v| v.is_finite()))
        else {
            continue;
        };
        if !size
            .zip(modified.as_deref())
            .is_some_and(|(size, modified)| {
                size >= 0 && file_is_current(&directory, &filename, size as u64, modified)
            })
        {
            continue;
        }
        tracks.push(UsableTrack {
            track: SideTrack {
                track_key: key,
                album_id: album.into(),
                title,
                artist,
            },
            features,
            hash: hash.unwrap_or_default(),
        });
    }
    Ok((total as usize, tracks))
}

pub(crate) fn query(
    source: &Connection,
    has: bool,
    request: &AlbumSidesRequest,
    overrides: &HashMap<String, bool>,
) -> Result<AlbumSidesResponse, String> {
    query_indexed(source, has, request, overrides, true)
}
// The streamed baseline is retained for deterministic regression comparisons.
// Interactive requests require the shared index so a cold store never causes
// an uncancellable million-track scan on a listener's discovery request.
pub(crate) fn query_indexed(
    source: &Connection,
    has: bool,
    request: &AlbumSidesRequest,
    overrides: &HashMap<String, bool>,
    indexed: bool,
) -> Result<AlbumSidesResponse, String> {
    if request.album_id.is_empty()
        || request.album_id.len() > 4096
        || request.strand_key.as_ref().is_some_and(|s| s.len() > 4096)
    {
        return Err("Invalid album sound request".into());
    }
    let tx = source.unchecked_transaction().map_err(|e| e.to_string())?;
    let c = &*tx;
    let (total, tracks) = load(c, &request.album_id, has, overrides)?;
    if total > 5000 {
        return Err("Explore its sound supports albums of up to 5,000 tracks.".into());
    }
    let mut result = AlbumSidesResponse {
        version: album_sides::VERSION.into(),
        total_tracks: total,
        analyzed_tracks: tracks.len(),
        ready: false,
        sides: vec![],
        contrasting_track: None,
        selected_key: None,
        matches: vec![],
        search_limited: false,
    };
    if tracks.is_empty() || tracks.len() != total {
        return Ok(result);
    }
    let weights: Option<String> = c
        .query_row(
            "SELECT weights FROM sonic.sonic_profiles WHERE profile=?1",
            [PROFILE],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(weights) = weights.and_then(|j| serde_json::from_str(&j).ok()) else {
        return Ok(result);
    };
    let mut analysis = Analysis {
        profile: PROFILE.into(),
        features: tracks[0].features.clone(),
        weights,
    };
    let features: Vec<_> = tracks.iter().map(|t| t.features.clone()).collect();
    let shape = album_sides::describe(&analysis, &features).ok_or(
        "Unsupported album sound profile. Update Music Library and check analysis coverage.",
    )?;
    result.ready = true;
    result.sides = shape
        .groups
        .iter()
        .map(|g| AlbumSide {
            representative: tracks[g.representative].track.clone(),
            tracks: g.members.iter().map(|&i| tracks[i].track.clone()).collect(),
        })
        .collect();
    result.contrasting_track = shape.contrast.map(|i| tracks[i].track.clone());
    let Some(key) = &request.strand_key else {
        return Ok(result);
    };
    let seed = tracks
        .iter()
        .find(|t| &t.track.track_key == key)
        .ok_or("This strand is no longer available. Explore the album again.")?;
    analysis.features = seed.features.clone();
    let metric = Metric::new(&analysis).ok_or("Invalid sonic metric")?;
    result.selected_key = Some(key.clone());
    // Retrieve tracks, never album means. Bound both the indexed neighborhood
    // and file verification work; expose that boundary to the listener.
    let candidates = indexed
        .then(|| {
            crate::sonic_index::candidates(
                c,
                &analysis,
                std::slice::from_ref(&analysis.features),
                false,
                4096,
                None,
                &overrides.keys().cloned().collect(),
            )
        })
        .flatten();
    if indexed && candidates.is_none() {
        return Err("The sound search index is not ready yet. Open Music Library to prepare it, then try this sound again shortly.".into());
    }
    let selection = if candidates.is_some() {
        " AND s.track_key IN (SELECT value FROM json_each(?3))"
    } else {
        ""
    };
    let mut values = vec![
        Value::Text(PROFILE.into()),
        Value::Text(request.album_id.clone()),
    ];
    if let Some(ref candidates) = candidates {
        values.push(Value::Text(candidates.keys.clone()));
    }
    let mut q=c.prepare(&format!("SELECT t.album_id,t.file_path,t.filename,COALESCE(t.love,''),a.features,s.audio_hash
        FROM sonic.sonic_tracks s CROSS JOIN tracks t ON t.file_path=s.directory AND t.filename=s.filename
        JOIN sonic.sonic_audio a ON a.audio_hash=s.audio_hash AND a.profile=s.profile
        WHERE s.profile=?1 AND t.album_id<>?2 AND lower(t.filename) LIKE '%.mp3'{selection}")).map_err(|e|e.to_string())?;
    let rows = q
        .query_map(params_from_iter(values), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut ranked: HashMap<String, f64> = HashMap::new();
    let mut truncated = false;
    for row in rows {
        let (id, dir, file, love, json, hash) = row.map_err(|e| e.to_string())?;
        if hash == seed.hash
            || *overrides
                .get(&track_key(&dir, &file))
                .unwrap_or(&(love == "B"))
        {
            continue;
        }
        let Some(distance) = serde_json::from_str::<Vec<f32>>(&json)
            .ok()
            .and_then(|f| metric.distance(&f))
        else {
            continue;
        };
        ranked
            .entry(id)
            .and_modify(|d| *d = d.min(distance))
            .or_insert(distance);
        if ranked.len() > 240 {
            truncated = true;
            let mut sorted: Vec<_> = ranked.into_iter().collect();
            sorted.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
            sorted.truncate(120);
            ranked = sorted.into_iter().collect();
        }
    }
    let mut ranked: Vec<_> = ranked.into_iter().collect();
    ranked.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    result.search_limited = truncated || candidates.is_some() || ranked.len() > 120;
    for (id, _) in ranked.into_iter().take(120) {
        let (total, usable) = load(c, &id, has, overrides)?;
        if total == 0 || usable.len() != total {
            continue;
        }
        let nearest = usable
            .iter()
            .filter(|t| t.hash != seed.hash)
            .filter_map(|t| metric.distance(&t.features).map(|d| (t, d)))
            .min_by(|a, b| {
                a.1.total_cmp(&b.1)
                    .then(a.0.track.track_key.cmp(&b.0.track.track_key))
            });
        let Some((nearest, distance)) = nearest else {
            continue;
        };
        let album=c.query_row("SELECT COALESCE(album,''),COALESCE(album_artist_display,''),canonical_genre FROM tracks WHERE album_id=?1 ORDER BY id LIMIT 1",[&id],|r|Ok(SonicAlbum { album_id:id.clone(),title:r.get(0)?,album_artist:r.get(1)?,genre:r.get(2)?,total_tracks:total,analyzed_tracks:usable.len(),distance:Some(distance) })).map_err(|e|e.to_string())?;
        result.matches.push(SideAlbumMatch {
            album,
            matching_track: nearest.track.clone(),
            distance,
        });
        // Candidates are visited by their best track distance. Complete fresh
        // albums preserve that ordering; stop once the requested shelf is full.
        if result.matches.len() == 12 {
            break;
        }
    }
    result.matches.sort_by(|a, b| {
        a.distance
            .total_cmp(&b.distance)
            .then(a.album.album_id.cmp(&b.album.album_id))
    });
    result.matches.truncate(12);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use music_sonic_core::{DIMENSIONS, file_signature};
    fn baseline(
        c: &Connection,
        has: bool,
        request: &AlbumSidesRequest,
        overrides: &HashMap<String, bool>,
    ) -> Result<AlbumSidesResponse, String> {
        query_indexed(c, has, request, overrides, false)
    }
    fn fixture() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE tracks(id INTEGER PRIMARY KEY,title TEXT,display_artist TEXT,album_artist_display TEXT,album TEXT,canonical_genre TEXT,album_id TEXT,file_path TEXT,filename TEXT,love TEXT);
            CREATE INDEX albums ON tracks(album_id); CREATE INDEX paths ON tracks(file_path,filename);
            ATTACH DATABASE ':memory:' AS sonic;
            CREATE TABLE sonic.sonic_profiles(profile TEXT PRIMARY KEY,weights TEXT);
            CREATE TABLE sonic.sonic_audio(audio_hash TEXT,profile TEXT,features TEXT,PRIMARY KEY(audio_hash,profile));
            CREATE TABLE sonic.sonic_tracks(track_key TEXT PRIMARY KEY,directory TEXT,filename TEXT,audio_hash TEXT,profile TEXT,size INTEGER,modified TEXT);").unwrap();
        let mut weights = vec![0.; DIMENSIONS * DIMENSIONS];
        weights[0] = 1.;
        c.execute(
            "INSERT INTO sonic.sonic_profiles VALUES(?1,?2)",
            params![PROFILE, serde_json::to_string(&weights).unwrap()],
        )
        .unwrap();
        // 'contains' has the closest cue but a very distant album mean.
        for (album, values) in [
            ("seed", vec![0., 0.01, 0.02, 1., 1.01, 1.02, 2., 2.01, 2.02]),
            ("contains", vec![0.03, 9., 9.]),
            ("mean", vec![0.5, 0.5, 0.5]),
        ] {
            for (i, value) in values.iter().enumerate() {
                let filename = format!("{album}-{i}.mp3");
                let directory = dir.path().to_string_lossy();
                std::fs::write(dir.path().join(&filename), [9; 256]).unwrap();
                let (size, modified) = file_signature(&dir.path().join(&filename)).unwrap();
                c.execute("INSERT INTO tracks(title,display_artist,album_artist_display,album,canonical_genre,album_id,file_path,filename,love) VALUES(?1,'Performer','Album artist',?2,'Score',?2,?3,?1,'')",params![filename,album,directory]).unwrap();
                let mut features = vec![0.; DIMENSIONS];
                features[0] = *value;
                c.execute(
                    "INSERT INTO sonic.sonic_audio VALUES(?1,?2,?3)",
                    params![filename, PROFILE, serde_json::to_string(&features).unwrap()],
                )
                .unwrap();
                c.execute(
                    "INSERT INTO sonic.sonic_tracks VALUES(?1,?2,?3,?3,?4,?5,?6)",
                    params![
                        track_key(&directory, &filename),
                        directory,
                        filename,
                        PROFILE,
                        size as i64,
                        modified
                    ],
                )
                .unwrap();
            }
        }
        (dir, c)
    }
    fn request() -> AlbumSidesRequest {
        AlbumSidesRequest {
            album_id: "seed".into(),
            strand_key: None,
        }
    }
    #[test]
    fn cold_indexes_keep_groups_available_and_request_a_discovery_retry() {
        let (_dir, c) = fixture();
        let mut request = request();
        let shape = query(&c, true, &request, &Default::default()).unwrap();
        assert!(shape.ready);
        request.strand_key = Some(shape.sides[0].representative.track_key.clone());
        let error = query(&c, true, &request, &Default::default()).unwrap_err();
        assert!(error.contains("index is not ready"));
    }
    #[test]
    fn indexed_strands_revalidate_current_candidates_and_pending_bans() {
        let (_dir, c, _) = crate::sonic_index::tests::fixture();
        let mut request = AlbumSidesRequest {
            album_id: "album-000".into(),
            strand_key: None,
        };
        let shape = query(&c, true, &request, &Default::default()).unwrap();
        assert!(shape.ready);
        request.strand_key = Some(shape.sides[0].representative.track_key.clone());
        let found = query(&c, true, &request, &Default::default()).unwrap();
        assert!(!found.matches.is_empty());
        assert!(found.search_limited);
        let first = &found.matches[0];
        let overrides = HashMap::from([(first.matching_track.track_key.clone(), true)]);
        let next = query(&c, true, &request, &overrides).unwrap();
        assert!(
            next.matches
                .iter()
                .all(|m| m.album.album_id != first.album.album_id)
        );
    }
    #[test]
    fn discovery_finds_albums_containing_the_strand_instead_of_similar_means() {
        let (_dir, c) = fixture();
        c.execute_batch("PRAGMA query_only=ON").unwrap();
        let shape = baseline(&c, true, &request(), &Default::default()).unwrap();
        assert!(shape.ready);
        assert_eq!(shape.sides.len(), 3);
        assert!(shape.sides.iter().all(|s| s.tracks.len() == 3));
        let mut r = request();
        r.strand_key = Some(shape.sides[0].representative.track_key.clone());
        let result = baseline(&c, true, &r, &Default::default()).unwrap();
        assert_eq!(result.matches[0].album.album_id, "contains");
        assert_eq!(result.matches[0].matching_track.title, "contains-0.mp3");
        assert_eq!(result.matches[0].matching_track.artist, "Performer");
        assert!(c.execute("DELETE FROM tracks", []).is_err());
    }
    #[test]
    fn incomplete_stale_banned_missing_and_wrong_profile_tracks_never_form_sides() {
        for change in [
            "DELETE FROM sonic.sonic_tracks WHERE filename='seed-0.mp3'",
            "UPDATE sonic.sonic_tracks SET profile='old' WHERE filename='seed-0.mp3'",
            "UPDATE tracks SET love='B' WHERE filename='seed-0.mp3'",
            "UPDATE sonic.sonic_audio SET features='[0]' WHERE audio_hash='seed-0.mp3'",
            "UPDATE tracks SET filename='seed-0.flac' WHERE filename='seed-0.mp3'",
        ] {
            let (_dir, c) = fixture();
            c.execute_batch(change).unwrap();
            let r = baseline(&c, true, &request(), &Default::default()).unwrap();
            assert!(!r.ready);
            assert_eq!(r.analyzed_tracks, 8);
            assert!(r.sides.is_empty());
        }
        let (dir, c) = fixture();
        std::fs::remove_file(dir.path().join("seed-0.mp3")).unwrap();
        assert!(
            !baseline(&c, true, &request(), &Default::default())
                .unwrap()
                .ready
        );
        let (dir, c) = fixture();
        std::fs::write(dir.path().join("seed-0.mp3"), [1; 257]).unwrap();
        assert!(
            !baseline(&c, true, &request(), &Default::default())
                .unwrap()
                .ready
        );
        let (dir, c) = fixture();
        let overrides =
            HashMap::from([(track_key(&dir.path().to_string_lossy(), "seed-0.mp3"), true)]);
        assert!(!baseline(&c, true, &request(), &overrides).unwrap().ready);
        assert_eq!(
            baseline(&c, false, &request(), &Default::default())
                .unwrap()
                .analyzed_tracks,
            0
        );
    }
    #[test]
    fn stale_candidates_and_foreign_strands_are_rejected() {
        let (dir, c) = fixture();
        let mut r = request();
        r.strand_key = Some(track_key(&dir.path().to_string_lossy(), "mean-0.mp3"));
        assert!(baseline(&c, true, &r, &Default::default()).is_err());
        r.strand_key = Some(track_key(&dir.path().to_string_lossy(), "seed-1.mp3"));
        std::fs::remove_file(dir.path().join("contains-2.mp3")).unwrap();
        let result = baseline(&c, true, &r, &Default::default()).unwrap();
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].album.album_id, "mean");
    }
}
