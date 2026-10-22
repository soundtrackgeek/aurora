//! Deterministic whole-track representatives, versioned independently of audio.
//! Groups describe acoustic neighborhoods, never moods, instruments or sections.
use crate::{AlbumAccumulator, Analysis, Metric};

pub const VERSION: &str = "album-sides-v1";
pub struct Side {
    pub representative: usize,
    pub members: Vec<usize>,
}
pub struct Sides {
    pub groups: Vec<Side>,
    pub contrast: Option<usize>,
}

/// Bounded O(3 * 24 * 3 * tracks * dimensions) refinement, with actual tracks as
/// centers. Input order breaks ties; callers supply stable catalog identity order.
/// A split needs two tracks per group, >=25% explained squared spread, and
/// representatives >=0.15 profile-distance apart. A third needs another 20%
/// reduction. These are conservative product heuristics, not genre detection.
pub fn describe(analysis: &Analysis, tracks: &[Vec<f32>]) -> Option<Sides> {
    if tracks.is_empty() || tracks.len() > 5000 {
        return None;
    }
    // Reject unsupported metrics rather than making centroid assumptions.
    if analysis.weights.iter().enumerate().any(|(i, w)| {
        if i / crate::DIMENSIONS == i % crate::DIMENSIONS {
            *w < 0.
        } else {
            *w != 0.
        }
    }) {
        return None;
    }
    let metric = Metric::new(analysis)?;
    let center = |members: &[usize]| -> Option<usize> {
        let mut sum = AlbumAccumulator::default();
        for &i in members {
            if !sum.add(&tracks[i]) {
                return None;
            }
        }
        let mean = sum.mean()?;
        members.iter().copied().min_by(|&a, &b| {
            metric
                .between(&mean, &tracks[a])
                .unwrap()
                .total_cmp(&metric.between(&mean, &tracks[b]).unwrap())
                .then(a.cmp(&b))
        })
    };
    let all: Vec<_> = (0..tracks.len()).collect();
    let first = center(&all)?;
    let assign = |centers: &[usize]| -> (Vec<Side>, f64) {
        let mut groups: Vec<_> = centers
            .iter()
            .map(|&representative| Side {
                representative,
                members: vec![],
            })
            .collect();
        let mut cost = 0.;
        for (i, features) in tracks.iter().enumerate() {
            let (group, distance) = centers
                .iter()
                .enumerate()
                .map(|(g, &c)| (g, metric.between(features, &tracks[c]).unwrap()))
                .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
                .unwrap();
            cost += distance * distance;
            groups[group].members.push(i);
        }
        (groups, cost)
    };
    let (mut groups, mut cost) = assign(&[first]);
    for k in 2..=3 {
        if tracks.len() < k * 3 || cost < 1e-12 {
            break;
        }
        let mut rejected = std::collections::HashSet::new();
        let mut accepted = None;
        // A spoken bonus track or isolated stinger must not prevent the rest
        // of a varied score from getting useful representatives. Retry after
        // excluding unsupported singleton seeds; all tracks still get assigned.
        for _ in 0..3 {
            let mut centers: Vec<_> = groups.iter().map(|g| g.representative).collect();
            let farthest = all
                .iter()
                .copied()
                .filter(|i| !rejected.contains(i))
                .max_by(|&a, &b| {
                    let nearest = |i: usize| {
                        centers
                            .iter()
                            .map(|&c| metric.between(&tracks[i], &tracks[c]).unwrap())
                            .min_by(f64::total_cmp)
                            .unwrap()
                    };
                    nearest(a).total_cmp(&nearest(b)).then(b.cmp(&a))
                })?;
            centers.push(farthest);
            for _ in 0..24 {
                let (next, _) = assign(&centers);
                let Some(updated) = next
                    .iter()
                    .map(|g| center(&g.members))
                    .collect::<Option<Vec<_>>>()
                else {
                    break;
                };
                if updated == centers {
                    break;
                }
                centers = updated;
            }
            let (next, next_cost) = assign(&centers);
            for group in next.iter().filter(|g| g.members.len() < 2) {
                rejected.extend(group.members.iter().copied());
            }
            rejected.insert(farthest);
            let separated = centers.iter().enumerate().all(|(i, &a)| {
                centers[..i]
                    .iter()
                    .all(|&b| metric.between(&tracks[a], &tracks[b]).unwrap() >= 0.15)
            });
            if !separated
                || next.iter().any(|g| g.members.len() < 2)
                || next_cost > cost * if k == 2 { 0.75 } else { 0.8 }
            {
                continue;
            }
            accepted = Some((next, next_cost));
            break;
        }
        let Some((next, next_cost)) = accepted else {
            break;
        };
        groups = next;
        cost = next_cost;
    }
    groups.sort_by(|a, b| {
        b.members
            .len()
            .cmp(&a.members.len())
            .then(a.representative.cmp(&b.representative))
    });
    let contrast = all
        .into_iter()
        .filter(|i| *i != first)
        .max_by(|&a, &b| {
            metric
                .between(&tracks[first], &tracks[a])
                .unwrap()
                .total_cmp(&metric.between(&tracks[first], &tracks[b]).unwrap())
                .then(b.cmp(&a))
        })
        .filter(|&i| metric.between(&tracks[first], &tracks[i]).unwrap() >= 0.15);
    Some(Sides { groups, contrast })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DIMENSIONS, PROFILE};
    fn analysis() -> Analysis {
        let mut weights = vec![0.; DIMENSIONS * DIMENSIONS];
        weights[0] = 1.;
        Analysis {
            profile: PROFILE.into(),
            features: vec![0.; DIMENSIONS],
            weights,
        }
    }
    fn tracks(values: &[f32]) -> Vec<Vec<f32>> {
        values
            .iter()
            .map(|v| {
                let mut f = vec![0.; DIMENSIONS];
                f[0] = *v;
                f
            })
            .collect()
    }
    #[test]
    fn varied_albums_keep_real_representatives_and_every_member_once() {
        let t = tracks(&[0., 0.01, 0.02, 1., 1.01, 1.02, 2., 2.01, 2.02]);
        let result = describe(&analysis(), &t).unwrap();
        assert_eq!(result.groups.len(), 3);
        assert!(result
            .groups
            .iter()
            .all(|g| g.members.len() == 3 && g.members.contains(&g.representative)));
        let mut members: Vec<_> = result
            .groups
            .iter()
            .flat_map(|g| g.members.clone())
            .collect();
        members.sort();
        assert_eq!(members, (0..9).collect::<Vec<_>>());
        assert_eq!(
            result
                .groups
                .iter()
                .map(|g| g.representative)
                .collect::<Vec<_>>(),
            describe(&analysis(), &t)
                .unwrap()
                .groups
                .iter()
                .map(|g| g.representative)
                .collect::<Vec<_>>()
        );
    }
    #[test]
    fn uniform_short_and_outlier_only_albums_do_not_invent_sides() {
        for values in [
            vec![0.; 10],
            vec![0., 1.],
            vec![0., 0., 0., 0., 0., 0., 0., 0., 2.],
            vec![0., 0.01, 0.02, 0.03, 0.04, 0.05],
        ] {
            assert_eq!(
                describe(&analysis(), &tracks(&values))
                    .unwrap()
                    .groups
                    .len(),
                1
            );
        }
        let result = describe(&analysis(), &tracks(&[0., 0., 0., 0., 0., 2.])).unwrap();
        assert_eq!(result.contrast, Some(5));
        assert!(describe(&analysis(), &tracks(&[0.; 6]))
            .unwrap()
            .contrast
            .is_none());
    }
    #[test]
    fn invalid_features_and_unsupported_weights_are_rejected() {
        assert!(describe(&analysis(), &tracks(&[f32::NAN])).is_none());
        let mut a = analysis();
        a.weights[1] = 1.;
        assert!(describe(&a, &tracks(&[0., 1.])).is_none());
        assert!(describe(&analysis(), &[]).is_none());
    }
    #[test]
    fn a_single_outlier_does_not_hide_supported_groups() {
        let t = tracks(&[
            0., 0.01, 0.02, 0.03, 0.04, 0.05, 1., 1.01, 1.02, 1.03, 1.04, 1.05, 3.,
        ]);
        let result = describe(&analysis(), &t).unwrap();
        assert!(result.groups.len() >= 2);
        assert!(result.groups.iter().all(|g| g.members.len() >= 2));
        assert_eq!(result.contrast, Some(12));
    }
}
