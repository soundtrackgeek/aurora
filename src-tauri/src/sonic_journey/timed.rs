use super::*;
use music_sonic_core::{
    Metric,
    timed_journey::{self, TimedTrack},
};

fn point(e: Entry) -> Result<TimedTrack<Entry>, String> {
    let seconds = u32::try_from(e.track.seconds)
        .ok()
        .filter(|s| *s > 0 && *s <= 43_200)
        .ok_or("Every journey track needs a known duration up to 720 minutes.")?;
    Ok(TimedTrack {
        key: e.track.track_key.clone(),
        recording: e.audio_hash.clone(),
        features: e.features.clone(),
        seconds,
        bytes: e.size,
        data: e,
    })
}
fn required(
    c: &Connection,
    request: &JourneyRequest,
    overrides: &Overrides,
) -> Result<Vec<Vec<TimedTrack<Entry>>>, String> {
    let mut groups = Vec::new();
    for key in &request.stop_keys {
        let stop =
            load(c, key, overrides)?.ok_or("A required stop needs current audio analysis.")?;
        let entries = if request.album_stop_key.as_ref() == Some(key) {
            if stop.track.album_id.is_empty() {
                return Err("This stop has no catalog album.".into());
            }
            let mut q = c.prepare("SELECT file_path,filename FROM tracks WHERE album_id=?1 ORDER BY COALESCE(disc_number,1),COALESCE(track_number,2147483647),filename,id LIMIT 121").map_err(|e| e.to_string())?;
            let keys = q
                .query_map([&stop.track.album_id], |r| {
                    Ok(track_key(&r.get::<_, String>(0)?, &r.get::<_, String>(1)?))
                })
                .map_err(|e| e.to_string())?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|e| e.to_string())?;
            if keys.is_empty() || keys.len() > 120 {
                return Err("Choose a complete album with 1–120 tracks.".into());
            }
            keys.iter()
                .map(|key| {
                    load(c, key, overrides)?.ok_or(
                        "Every track in the whole album needs current audio analysis.".into(),
                    )
                })
                .collect::<Result<Vec<_>, String>>()?
        } else {
            vec![stop]
        };
        if entries.iter().any(|e| e.banned || !fresh(e)) {
            return Err("A required stop or album track is unavailable, changed or banned. Review the stops and analyze them again.".into());
        }
        groups.push(
            entries
                .into_iter()
                .map(point)
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    Ok(groups)
}
fn eligible(e: &Entry, request: &JourneyRequest, genre: Option<&String>) -> bool {
    !e.banned
        && request
            .minimum_rating
            .is_none_or(|r| e.track.rating.is_some_and(|v| v >= r))
        && (!request.same_genre || genre.is_some_and(|g| e.track.genre.as_ref() == Some(g)))
}
pub(super) fn query(
    c: &Connection,
    has: bool,
    request: &JourneyRequest,
    overrides: &Overrides,
    indexed: bool,
) -> Result<JourneyResponse, String> {
    if !has {
        return Err("Analyze your chosen stops and more music in Music Library first.".into());
    }
    let tx = c.unchecked_transaction().map_err(|e| e.to_string())?;
    let groups = required(&tx, request, overrides)?;
    let weights =
        analysis(&tx, groups[0][0].features.clone())?.ok_or("Incompatible audio analysis")?;
    // Validate required recordings and budgets before retrieving the library.
    timed_journey::plan(
        &weights,
        &groups,
        vec![],
        request.target_seconds.unwrap(),
        request.max_mib.map(|v| u64::from(v) * 1_048_576),
    )?;
    let targets = timed_journey::targets(&groups);
    let metric = Metric::new(&weights).ok_or("Incompatible audio analysis")?;
    let candidates = if indexed {
        crate::sonic_index::candidates(
            &tx,
            &weights,
            &targets,
            false,
            128,
            None,
            &Default::default(),
        )
    } else {
        None
    };
    let selection = if candidates.is_some() {
        " AND s.track_key IN (SELECT value FROM json_each(?2))"
    } else {
        ""
    };
    let mut params = vec![rusqlite::types::Value::Text(PROFILE.into())];
    if let Some(candidates) = &candidates {
        params.push(rusqlite::types::Value::Text(candidates.keys.clone()));
    }
    let mut q = tx.prepare(&format!("SELECT {SELECT} {JOIN} WHERE s.profile=?1 AND lower(t.filename) LIKE '%.mp3'{selection}")).map_err(|e|e.to_string())?;
    // Bounded streaming fallback also works while the derived index is rebuilding.
    let mut pools = vec![Vec::<(f64, TimedTrack<Entry>)>::new(); targets.len()];
    let mut analyzed = 0;
    let required_keys = groups
        .iter()
        .flatten()
        .map(|t| t.key.as_str())
        .collect::<HashSet<_>>();
    let required_recordings = groups
        .iter()
        .flatten()
        .map(|t| t.recording.as_str())
        .collect::<HashSet<_>>();
    for row in q
        .query_map(rusqlite::params_from_iter(params), row)
        .map_err(|e| e.to_string())?
    {
        let mut e = row.map_err(|e| e.to_string())?;
        analyzed += 1;
        apply(&mut e, overrides);
        if !eligible(&e, request, groups[0][0].data.track.genre.as_ref())
            || required_keys.contains(e.track.track_key.as_str())
            || required_recordings.contains(e.audio_hash.as_str())
        {
            continue;
        }
        let Ok(t) = point(e) else { continue };
        let Some((slot, cost)) = targets
            .iter()
            .enumerate()
            .filter_map(|(i, target)| metric.between(target, &t.features).map(|d| (i, d)))
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
        else {
            continue;
        };
        let pool = &mut pools[slot];
        if pool.len() == 64
            && pool
                .last()
                .is_some_and(|last| (cost, &t.key) >= (last.0, &last.1.key))
        {
            continue;
        }
        pool.push((cost, t));
        pool.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.key.cmp(&b.1.key)));
        pool.truncate(64);
    }
    let pool = pools
        .into_iter()
        .flatten()
        .map(|(_, t)| t)
        .filter(|t| fresh(&t.data))
        .collect();
    let entries = timed_journey::plan(
        &weights,
        &groups,
        pool,
        request.target_seconds.unwrap(),
        request.max_mib.map(|v| u64::from(v) * 1_048_576),
    )?;
    if entries.iter().any(|e| !fresh(e)) {
        return Err("A journey file changed. Build and review again.".into());
    }
    let total_seconds = entries.iter().map(|e| e.track.seconds as u32).sum();
    let total_bytes = entries.iter().map(|e| e.size).sum();
    let album_track_keys = request
        .album_stop_key
        .as_ref()
        .and_then(|key| request.stop_keys.iter().position(|k| k == key))
        .map(|i| groups[i].iter().map(|t| t.key.clone()).collect())
        .unwrap_or_default();
    Ok(JourneyResponse {
        stops_ready: vec![true; request.stop_keys.len()],
        analyzed,
        complete: true,
        tracks: entries.into_iter().map(|e| e.track).collect(),
        timing: Some(JourneyTiming {
            total_seconds,
            total_bytes,
            shortfall_seconds: request.target_seconds.unwrap() - total_seconds,
            album_track_keys,
        }),
    })
}
pub(super) fn reviewed(
    c: &Connection,
    request: &JourneyRequest,
    keys: &[String],
    overrides: &Overrides,
) -> Result<Vec<JourneyTrack>, String> {
    if keys.len() > 1456
        || keys.len() < request.stop_keys.len()
        || keys.iter().any(|k| k.is_empty() || k.len() > 4096)
    {
        return Err("Invalid reviewed journey.".into());
    }
    let groups = required(c, request, overrides)?;
    let weights =
        analysis(c, groups[0][0].features.clone())?.ok_or("Incompatible audio analysis")?;
    let metric = Metric::new(&weights).ok_or("Incompatible audio analysis")?;
    let required_keys = groups
        .iter()
        .flatten()
        .map(|t| t.key.as_str())
        .collect::<HashSet<_>>();
    let mut cursor = 0;
    for (i, group) in groups.iter().enumerate() {
        let pos = keys[cursor..]
            .iter()
            .position(|k| k == &group[0].key)
            .map(|p| p + cursor)
            .ok_or("A required stop is missing or reordered.")?;
        if i == 0 && pos != 0
            || i == groups.len() - 1 && pos + group.len() != keys.len()
            || keys
                .get(pos..pos + group.len())
                .is_none_or(|slice| !slice.iter().eq(group.iter().map(|t| &t.key)))
        {
            return Err(
                "Keep the starting track, ending track and complete album in the reviewed order."
                    .into(),
            );
        }
        cursor = pos + group.len();
    }
    let mut recordings = HashSet::new();
    let mut identities = HashSet::new();
    let mut seconds = 0u64;
    let mut bytes = 0u64;
    let mut result = Vec::new();
    for key in keys {
        let e = load(c, key, overrides)?.ok_or("A reviewed track is no longer analyzed.")?;
        if !identities.insert(key)
            || e.audio_hash.is_empty()
            || !recordings.insert(e.audio_hash.clone())
            || !fresh(&e)
            || e.banned
            || e.track.seconds <= 0
            || metric.distance(&e.features).is_none()
            || !required_keys.contains(key.as_str())
                && !eligible(&e, request, groups[0][0].data.track.genre.as_ref())
        {
            return Err("A journey track changed, repeats a recording or no longer qualifies. Build and review again.".into());
        }
        seconds = seconds
            .checked_add(e.track.seconds as u64)
            .ok_or("Duration overflow")?;
        bytes = bytes.checked_add(e.size).ok_or("File size overflow")?;
        result.push(e.track);
    }
    if seconds > u64::from(request.target_seconds.unwrap())
        || request
            .max_mib
            .is_some_and(|cap| bytes > u64::from(cap) * 1_048_576)
    {
        return Err(
            "The reviewed journey exceeds the time or audio storage limit. Build again.".into(),
        );
    }
    Ok(result)
}
