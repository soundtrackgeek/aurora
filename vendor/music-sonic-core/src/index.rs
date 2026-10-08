//! Persistent exact k-d tree for the current diagonal weighted metric.
//! The producer supplies weights; unsupported matrices use the caller's scan.
use crate::{Analysis, DIMENSIONS};
use std::{
    cmp::Ordering,
    collections::BinaryHeap,
    io::{self, Read, Write},
};

const LEAF: usize = 32;
const MAX_POINTS: usize = 3_000_000;
const MAX_KEY: usize = 4096;

#[derive(Clone)]
pub struct Point {
    pub key: String,
    pub features: [f32; DIMENSIONS],
    axis: u8,
}
impl Point {
    pub fn new(key: String, features: &[f32]) -> Option<Self> {
        if key.is_empty() || key.len() > MAX_KEY || features.iter().any(|v| !v.is_finite()) {
            return None;
        }
        Some(Self {
            key,
            features: features.try_into().ok()?,
            axis: 255,
        })
    }
}

pub struct Index {
    points: Vec<Point>,
    weights: [f64; DIMENSIONS],
}
pub struct Neighbors<'a> {
    pub points: Vec<(&'a Point, f64)>,
    pub visited: usize,
}
struct Hit<'a> {
    point: &'a Point,
    squared: f64,
}
impl PartialEq for Hit<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Hit<'_> {}
impl PartialOrd for Hit<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Hit<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.squared
            .total_cmp(&other.squared)
            .then(self.point.key.cmp(&other.point.key))
    }
}
impl Index {
    pub fn build(analysis: &Analysis, mut points: Vec<Point>) -> Option<Self> {
        if !analysis.valid() || points.len() > MAX_POINTS {
            return None;
        }
        let mut weights = [0.; DIMENSIONS];
        for (i, w) in analysis.weights.iter().enumerate() {
            if i / DIMENSIONS == i % DIMENSIONS {
                if *w < 0. {
                    return None;
                }
                weights[i / DIMENSIONS] = f64::from(*w);
            } else if *w != 0. {
                return None;
            }
        }
        fn partition(points: &mut [Point], weights: &[f64; DIMENSIONS]) {
            if points.len() <= LEAF {
                return;
            }
            let mut low = [f32::INFINITY; DIMENSIONS];
            let mut high = [f32::NEG_INFINITY; DIMENSIONS];
            for point in points.iter() {
                for i in 0..DIMENSIONS {
                    low[i] = low[i].min(point.features[i]);
                    high[i] = high[i].max(point.features[i]);
                }
            }
            let axis = (0..DIMENSIONS)
                .max_by(|&a, &b| {
                    let span =
                        |i: usize| (f64::from(high[i]) - f64::from(low[i])).powi(2) * weights[i];
                    span(a).total_cmp(&span(b)).then(a.cmp(&b))
                })
                .unwrap();
            let mid = points.len() / 2;
            points.select_nth_unstable_by(mid, |a, b| {
                a.features[axis]
                    .total_cmp(&b.features[axis])
                    .then(a.key.cmp(&b.key))
            });
            let (left, rest) = points.split_at_mut(mid);
            rest[0].axis = axis as u8;
            partition(left, weights);
            partition(&mut rest[1..], weights);
        }
        partition(&mut points, &weights);
        Some(Self { points, weights })
    }
    pub fn len(&self) -> usize {
        self.points.len()
    }
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
    pub fn points(&self) -> &[Point] {
        &self.points
    }
    pub fn compatible(&self, analysis: &Analysis) -> bool {
        analysis.valid()
            && analysis.weights.iter().enumerate().all(|(i, w)| {
                if i / DIMENSIONS == i % DIMENSIONS {
                    f64::from(*w) == self.weights[i / DIMENSIONS]
                } else {
                    *w == 0.
                }
            })
    }
    /// Exact branch-and-bound. Equal distances use the same identity tie-break
    /// as the streamed baseline. A dirty identity is excluded before selection.
    pub fn nearest(
        &self,
        target: &[f32],
        limit: usize,
        usable: impl Fn(&str) -> bool,
    ) -> Neighbors<'_> {
        if target.len() != DIMENSIONS || target.iter().any(|v| !v.is_finite()) || limit == 0 {
            return Neighbors {
                points: vec![],
                visited: 0,
            };
        }
        struct Search<'a, F> {
            target: [f32; DIMENSIONS],
            weights: &'a [f64; DIMENSIONS],
            limit: usize,
            heap: BinaryHeap<Hit<'a>>,
            usable: F,
            visited: usize,
        }
        impl<'a, F: Fn(&str) -> bool> Search<'a, F> {
            fn offer(&mut self, point: &'a Point) {
                self.visited += 1;
                if !(self.usable)(&point.key) {
                    return;
                }
                let squared = (0..DIMENSIONS)
                    .map(|i| {
                        let delta = f64::from(self.target[i]) - f64::from(point.features[i]);
                        delta * self.weights[i] * delta
                    })
                    .sum();
                let hit = Hit { point, squared };
                if self.heap.len() < self.limit {
                    self.heap.push(hit);
                } else if self.heap.peek().is_some_and(|worst| hit < *worst) {
                    self.heap.pop();
                    self.heap.push(hit);
                }
            }
            fn walk(&mut self, points: &'a [Point], bound: f64, bounds: &mut [f64; DIMENSIONS]) {
                // A small roundoff margin only causes extra visits, never lost neighbors.
                if self.heap.len() == self.limit
                    && self
                        .heap
                        .peek()
                        .is_some_and(|h| bound > h.squared + 1e-10 * (1. + h.squared))
                {
                    return;
                }
                if points.len() <= LEAF {
                    for point in points {
                        self.offer(point);
                    }
                    return;
                }
                let mid = points.len() / 2;
                let point = &points[mid];
                self.offer(point);
                let axis = point.axis as usize;
                let delta = f64::from(self.target[axis]) - f64::from(point.features[axis]);
                let (near, far) = if delta <= 0. {
                    (&points[..mid], &points[mid + 1..])
                } else {
                    (&points[mid + 1..], &points[..mid])
                };
                self.walk(near, bound, bounds);
                let previous = bounds[axis];
                let next = (delta * delta * self.weights[axis]).max(previous);
                bounds[axis] = next;
                self.walk(far, (bound - previous + next).max(0.), bounds);
                bounds[axis] = previous;
            }
        }
        let mut search = Search {
            target: target.try_into().unwrap(),
            weights: &self.weights,
            limit,
            heap: BinaryHeap::new(),
            usable,
            visited: 0,
        };
        search.walk(&self.points, 0., &mut [0.; DIMENSIONS]);
        let visited = search.visited;
        let points = search
            .heap
            .into_sorted_vec()
            .into_iter()
            .map(|h| (h.point, h.squared.sqrt()))
            .collect();
        Neighbors { points, visited }
    }
    pub fn write(&self, mut out: impl Write) -> io::Result<()> {
        out.write_all(b"SONICKD1")?;
        out.write_all(&(self.len() as u32).to_le_bytes())?;
        for w in self.weights {
            out.write_all(&w.to_le_bytes())?;
        }
        for point in &self.points {
            out.write_all(&(point.key.len() as u32).to_le_bytes())?;
            out.write_all(point.key.as_bytes())?;
            for feature in point.features {
                out.write_all(&feature.to_le_bytes())?;
            }
            out.write_all(&[point.axis])?;
        }
        Ok(())
    }
    /// The outer artifact must verify its checksum before calling this reader.
    pub fn read(mut input: impl Read) -> io::Result<Self> {
        fn invalid() -> io::Error {
            io::Error::new(io::ErrorKind::InvalidData, "Invalid sonic index")
        }
        fn bytes<const N: usize>(input: &mut impl Read) -> io::Result<[u8; N]> {
            let mut b = [0; N];
            input.read_exact(&mut b)?;
            Ok(b)
        }
        if &bytes::<8>(&mut input)? != b"SONICKD1" {
            return Err(invalid());
        }
        let count = u32::from_le_bytes(bytes(&mut input)?) as usize;
        if count > MAX_POINTS {
            return Err(invalid());
        }
        let mut weights = [0.; DIMENSIONS];
        for w in &mut weights {
            *w = f64::from_le_bytes(bytes(&mut input)?);
            if !w.is_finite() || *w < 0. {
                return Err(invalid());
            }
        }
        let mut points = Vec::with_capacity(count);
        let mut key_bytes = 0usize;
        for _ in 0..count {
            let size = u32::from_le_bytes(bytes(&mut input)?) as usize;
            key_bytes = key_bytes.saturating_add(size);
            if size == 0 || size > MAX_KEY || key_bytes > 512 * 1024 * 1024 {
                return Err(invalid());
            }
            let mut key = vec![0; size];
            input.read_exact(&mut key)?;
            let key = String::from_utf8(key).map_err(|_| invalid())?;
            let mut features = [0.; DIMENSIONS];
            for v in &mut features {
                *v = f32::from_le_bytes(bytes(&mut input)?);
                if !v.is_finite() {
                    return Err(invalid());
                }
            }
            let axis = bytes::<1>(&mut input)?[0];
            points.push(Point {
                key,
                features,
                axis,
            });
        }
        fn valid(points: &[Point]) -> bool {
            if points.len() <= LEAF {
                return points.iter().all(|p| p.axis == 255);
            }
            let mid = points.len() / 2;
            let pivot = &points[mid];
            let axis = pivot.axis as usize;
            axis < DIMENSIONS
                && points[..mid]
                    .iter()
                    .all(|p| p.features[axis] <= pivot.features[axis])
                && points[mid + 1..]
                    .iter()
                    .all(|p| p.features[axis] >= pivot.features[axis])
                && valid(&points[..mid])
                && valid(&points[mid + 1..])
        }
        if !valid(&points) {
            return Err(invalid());
        }
        Ok(Self { points, weights })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Metric, PROFILE};
    fn fixture() -> (Analysis, Vec<Point>) {
        let mut weights = vec![0.; DIMENSIONS * DIMENSIONS];
        for i in 0..DIMENSIONS {
            weights[i * DIMENSIONS + i] = (i % 4) as f32 * 0.25;
        }
        let analysis = Analysis {
            profile: PROFILE.into(),
            features: vec![0.; DIMENSIONS],
            weights,
        };
        let mut state = 1337u64;
        let points = (0..4096)
            .map(|i| {
                let features = (0..DIMENSIONS)
                    .map(|_| {
                        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                        (state >> 32) as u32 as f32 / u32::MAX as f32
                    })
                    .collect::<Vec<_>>();
                Point::new(format!("{i:06}"), &features).unwrap()
            })
            .collect();
        (analysis, points)
    }
    #[test]
    fn index_round_trip_matches_weighted_exact_with_exclusions_and_ties() {
        let (analysis, mut points) = fixture();
        points.push(Point::new("duplicate".into(), &points[0].features).unwrap());
        let index = Index::build(&analysis, points.clone()).unwrap();
        let mut data = vec![];
        index.write(&mut data).unwrap();
        let restored = Index::read(&data[..]).unwrap();
        for target in points.iter().step_by(151).map(|p| p.features) {
            let mut seed = analysis.clone();
            seed.features = target.to_vec();
            let metric = Metric::new(&seed).unwrap();
            let mut exact = points
                .iter()
                .filter(|p| p.key != "000001")
                .map(|p| (&p.key, metric.distance(&p.features).unwrap()))
                .collect::<Vec<_>>();
            exact.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(b.0)));
            exact.truncate(50);
            let found = restored.nearest(&target, 50, |key| key != "000001");
            assert_eq!(
                found
                    .points
                    .iter()
                    .map(|(p, d)| (&p.key, *d))
                    .collect::<Vec<_>>(),
                exact
            );
        }
        data[0] = 0;
        assert!(Index::read(&data[..]).is_err());
        assert!(Index::read(&data[..10]).is_err());
    }
    #[test]
    fn unsupported_weights_and_invalid_points_are_rejected() {
        let (mut analysis, points) = fixture();
        analysis.weights[1] = 0.2;
        assert!(Index::build(&analysis, points).is_none());
        assert!(Point::new("bad".into(), &[f32::NAN; DIMENSIONS]).is_none());
    }
}
