//! Explicit catalog boundaries for whole-track sonic challenges.
use serde::{Deserialize, Serialize};

pub const VERSION: &str = "sonic-worlds-v1";
/// A conservative product heuristic in this profile's units, not a probability.
pub const MAX_DISTANCE: f64 = 0.25;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Boundary {
    Decade,
    AlbumArtist,
    Genre,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    Stay,
    Either,
    Cross,
}

pub struct Metadata<'a> {
    /// Catalog album year. Never substitute the release/reissue year.
    pub year: Option<i64>,
    pub album_artist: &'a str,
    pub genre: Option<&'a str>,
}

impl Boundary {
    pub fn value(self, metadata: &Metadata<'_>) -> Option<String> {
        match self {
            Self::Decade => metadata
                .year
                .filter(|y| (1000..=9999).contains(y))
                .map(|y| format!("{}s", y / 10 * 10)),
            Self::AlbumArtist => known(metadata.album_artist),
            Self::Genre => metadata.genre.and_then(known),
        }
    }
}

fn known(value: &str) -> Option<String> {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    (!value.is_empty()
        && !matches!(
            value.to_lowercase().as_str(),
            "unknown" | "unknown artist" | "unknown genre" | "various artists"
        ))
    .then_some(value)
}

/// Missing metadata never proves equality or crossing. Either remains acoustic-only.
pub fn accepts(direction: Direction, seed: Option<&str>, candidate: Option<&str>) -> bool {
    if direction == Direction::Either {
        return true;
    }
    let (Some(seed), Some(candidate)) = (seed, candidate) else {
        return false;
    };
    let same = seed.to_lowercase() == candidate.to_lowercase();
    match direction {
        Direction::Stay => same,
        Direction::Cross => !same,
        Direction::Either => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundaries_use_known_album_metadata_and_support_both_directions() {
        let metadata = Metadata {
            year: Some(1989),
            album_artist: "  Björk  ",
            genre: Some(" Art   Pop "),
        };
        assert_eq!(Boundary::Decade.value(&metadata).as_deref(), Some("1980s"));
        assert_eq!(Boundary::Genre.value(&metadata).as_deref(), Some("Art Pop"));
        let artist = Boundary::AlbumArtist.value(&metadata);
        assert!(accepts(Direction::Stay, artist.as_deref(), Some("BJÖRK")));
        assert!(!accepts(Direction::Cross, artist.as_deref(), Some("BJÖRK")));
        assert!(accepts(Direction::Cross, Some("1980s"), Some("1990s")));
        for missing in [None, Some(0), Some(-1980), Some(10000)] {
            assert!(Boundary::Decade
                .value(&Metadata {
                    year: missing,
                    ..metadata
                })
                .is_none());
        }
        assert!(Boundary::AlbumArtist
            .value(&Metadata {
                album_artist: "Various Artists",
                ..metadata
            })
            .is_none());
        assert!(Boundary::Genre
            .value(&Metadata {
                genre: Some("  "),
                ..metadata
            })
            .is_none());
        for direction in [Direction::Stay, Direction::Cross] {
            assert!(!accepts(direction, None, Some("1990s")));
            assert!(!accepts(direction, Some("1990s"), None));
        }
        assert!(accepts(Direction::Either, None, None));
    }
}
