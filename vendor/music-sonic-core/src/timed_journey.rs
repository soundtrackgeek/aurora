//! Duration-constrained journeys over a bounded sonic shortlist. Whole tracks,
//! ordered required groups and byte-identical recording exclusion are hard rules.
//! This is exact duration packing within the supplied shortlist, not a global
//! shortest path or a claim about intros, beats, keys or semantic mood.
use crate::{Analysis, Metric, DIMENSIONS};
use std::collections::HashSet;

#[derive(Clone)]
pub struct TimedTrack<T> {
    pub key: String,
    pub recording: String,
    pub features: Vec<f32>,
    pub seconds: u32,
    pub bytes: u64,
    pub data: T,
}

pub fn targets<T>(groups: &[Vec<TimedTrack<T>>]) -> Vec<Vec<f32>> {
    groups
        .windows(2)
        .flat_map(|pair| {
            let a = &pair[0].last().unwrap().features;
            let b = &pair[1][0].features;
            (1..=9).map(move |i| {
                (0..DIMENSIONS)
                    .map(|d| a[d] + (b[d] - a[d]) * i as f32 / 10.)
                    .collect()
            })
        })
        .collect()
}

/// Keep the closest candidates to each part of the arc before packing. Callers
/// retrieve and freshness-check a bounded neighborhood around `targets`.
pub fn plan<T: Clone>(
    analysis: &Analysis,
    groups: &[Vec<TimedTrack<T>>],
    candidates: Vec<TimedTrack<T>>,
    seconds: u32,
    max_bytes: Option<u64>,
) -> Result<Vec<T>, String> {
    if !(2..=10).contains(&groups.len())
        || groups.iter().any(|g| g.is_empty() || g.len() > 120)
        || !(60..=43_200).contains(&seconds)
    {
        return Err("Choose 2–10 stops and a time limit up to 720 minutes.".into());
    }
    let metric = Metric::new(analysis).ok_or("Incompatible audio analysis")?;
    let mut keys = HashSet::new();
    let mut recordings = HashSet::new();
    let mut required_seconds = 0u32;
    let mut required_bytes = 0u64;
    for t in groups.iter().flatten() {
        if t.seconds == 0
            || t.seconds > seconds
            || t.bytes == 0
            || metric.distance(&t.features).is_none()
        {
            return Err("Every required track needs current analysis, a known duration and an available file that fits the time limit.".into());
        }
        if !keys.insert(t.key.clone())
            || t.recording.is_empty()
            || !recordings.insert(t.recording.clone())
        {
            return Err("Required stops or album blocks repeat the same recording. Choose a different stop or album.".into());
        }
        required_seconds = required_seconds
            .checked_add(t.seconds)
            .ok_or("Duration overflow")?;
        required_bytes = required_bytes
            .checked_add(t.bytes)
            .ok_or("File size overflow")?;
    }
    if required_seconds > seconds || max_bytes.is_some_and(|cap| required_bytes > cap) {
        return Err("The required stops and whole album exceed the time or audio storage limit. Increase the limit or remove a stop.".into());
    }
    let targets = targets(groups);
    let mut ranked = candidates
        .into_iter()
        .filter_map(|t| {
            if t.seconds == 0
                || t.seconds > seconds - required_seconds
                || t.bytes == 0
                || t.recording.is_empty()
                || keys.contains(&t.key)
                || recordings.contains(&t.recording)
            {
                return None;
            }
            let (slot, cost) = targets
                .iter()
                .enumerate()
                .filter_map(|(i, target)| metric.between(target, &t.features).map(|d| (i, d)))
                .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))?;
            Some((t, slot, cost))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|a, b| a.2.total_cmp(&b.2).then(a.0.key.cmp(&b.0.key)));
    let mut per_slot = vec![0; targets.len()];
    ranked.retain(|(t, slot, _)| {
        if per_slot[*slot] >= 8 || keys.contains(&t.key) || recordings.contains(&t.recording) {
            return false;
        }
        per_slot[*slot] += 1;
        keys.insert(t.key.clone());
        recordings.insert(t.recording.clone());
        true
    });
    ranked.truncate(256);
    // A 0/1 knapsack with one state per exact second; reverse iteration prevents
    // reuse. With a storage cap, lowest bytes dominates at each duration; without
    // one, minimize sonic cost. Decision rows reconstruct immutable membership.
    let remaining = (seconds - required_seconds) as usize;
    let byte_limit = max_bytes.map(|v| v - required_bytes).unwrap_or(u64::MAX);
    let mut dp = vec![None::<(u64, f64)>; remaining + 1];
    dp[0] = Some((0, 0.));
    let mut choices = Vec::with_capacity(ranked.len());
    for (track, _, cost) in &ranked {
        let duration = track.seconds as usize;
        let mut chosen = vec![false; remaining + 1];
        for s in (duration..=remaining).rev() {
            let Some((bytes, sound)) = dp[s - duration] else {
                continue;
            };
            let Some(bytes) = bytes.checked_add(track.bytes).filter(|b| *b <= byte_limit) else {
                continue;
            };
            let next = (bytes, sound + cost);
            let better = dp[s].is_none_or(|old| {
                if max_bytes.is_some() {
                    next.0 < old.0 || next.0 == old.0 && next.1 < old.1
                } else {
                    next.1 < old.1
                }
            });
            if better {
                dp[s] = Some(next);
                chosen[s] = true;
            }
        }
        choices.push(chosen);
    }
    let mut duration = (0..=remaining)
        .rev()
        .find(|s| dp[*s].is_some())
        .unwrap_or(0);
    let mut selected = Vec::new();
    for i in (0..ranked.len()).rev() {
        if choices[i][duration] {
            duration -= ranked[i].0.seconds as usize;
            selected.push(i);
        }
    }
    selected.sort_by(|a, b| {
        ranked[*a]
            .1
            .cmp(&ranked[*b].1)
            .then(ranked[*a].0.key.cmp(&ranked[*b].0.key))
    });
    let mut result = Vec::new();
    for (leg, group) in groups.iter().enumerate() {
        result.extend(group.iter().map(|t| t.data.clone()));
        result.extend(
            selected
                .iter()
                .filter(|i| ranked[**i].1 / 9 == leg)
                .map(|i| ranked[*i].0.data.clone()),
        );
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn track(id: usize, seconds: u32, bytes: u64) -> TimedTrack<usize> {
        TimedTrack {
            key: id.to_string(),
            recording: id.to_string(),
            features: vec![id as f32; DIMENSIONS],
            seconds,
            bytes,
            data: id,
        }
    }
    fn analysis() -> Analysis {
        let mut weights = vec![0.; DIMENSIONS * DIMENSIONS];
        weights[0] = 1.;
        Analysis {
            profile: crate::PROFILE.into(),
            features: vec![0.; DIMENSIONS],
            weights,
        }
    }
    #[test]
    fn packs_exact_seconds_and_preserves_stops_and_indivisible_album() {
        let groups = vec![
            vec![track(0, 60, 10)],
            vec![track(4, 60, 10), track(5, 60, 10)],
            vec![track(9, 60, 10)],
        ];
        let candidates = vec![
            track(1, 73, 10),
            track(2, 77, 10),
            track(6, 91, 10),
            track(7, 60, 10),
        ];
        let result = plan(&analysis(), &groups, candidates, 390, None).unwrap();
        assert_eq!(result, vec![0, 1, 2, 4, 5, 9]);
        assert!(plan(&analysis(), &groups, vec![], 239, None).is_err());
        assert!(plan(&analysis(), &groups, vec![], 400, Some(39)).is_err());
    }
    #[test]
    fn excludes_identical_recordings_and_respects_storage() {
        let groups = vec![vec![track(0, 60, 10)], vec![track(9, 60, 10)]];
        let mut twin = track(2, 60, 1);
        twin.recording = "0".into();
        let result = plan(
            &analysis(),
            &groups,
            vec![track(1, 80, 20), track(3, 70, 2), twin],
            200,
            Some(25),
        )
        .unwrap();
        assert_eq!(result, vec![0, 3, 9]);
        let mut duplicate = groups;
        duplicate[1][0].recording = "0".into();
        assert!(plan(&analysis(), &duplicate, vec![], 200, None).is_err());
    }
    #[test]
    fn duration_packing_matches_exhaustive_small_pools() {
        for cap in 120..220 {
            let pool = (1..=6)
                .map(|i| track(i, 9 + i as u32 * 7, i as u64 * 3))
                .collect::<Vec<_>>();
            let groups = vec![vec![track(0, 60, 1)], vec![track(9, 60, 1)]];
            let result = plan(&analysis(), &groups, pool.clone(), cap, Some(47)).unwrap();
            let actual: u32 = result
                .iter()
                .filter_map(|id| pool.iter().find(|t| t.data == *id))
                .map(|t| t.seconds)
                .sum();
            let expected = (0u32..64)
                .filter_map(|mask| {
                    let chosen = pool
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| mask & (1 << i) != 0)
                        .map(|(_, t)| t)
                        .collect::<Vec<_>>();
                    let s: u32 = chosen.iter().map(|t| t.seconds).sum();
                    (s <= cap - 120 && chosen.iter().map(|t| t.bytes).sum::<u64>() <= 45)
                        .then_some(s)
                })
                .max()
                .unwrap();
            assert_eq!(actual, expected);
        }
    }
}
