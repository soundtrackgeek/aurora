use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum LoveState {
    Neutral,
    Loved,
    Banned,
}

impl LoveState {
    pub(crate) fn from_catalog(value: Option<&str>) -> Self {
        match value {
            Some("L") => Self::Loved,
            Some("B") => Self::Banned,
            _ => Self::Neutral,
        }
    }

    pub(crate) fn as_db(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Loved => "loved",
            Self::Banned => "banned",
        }
    }

    pub(crate) fn from_db(value: &str) -> Result<Self, String> {
        match value {
            "neutral" => Ok(Self::Neutral),
            "loved" => Ok(Self::Loved),
            "banned" => Ok(Self::Banned),
            _ => Err("Aurora's saved Love state is invalid.".to_owned()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TagValues {
    pub(crate) rating: Option<f64>,
    pub(crate) love_state: LoveState,
    pub(crate) release_year: Option<i32>,
}

impl TagValues {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if let Some(rating) = self.rating {
            let doubled = rating * 2.0;
            if !rating.is_finite()
                || !(0.5..=5.0).contains(&rating)
                || (doubled.round() - doubled).abs() > f64::EPSILON
            {
                return Err("Rating must be unrated or a half-star value from 0.5 to 5.".to_owned());
            }
        }
        if let Some(year) = self.release_year
            && !(1000..=2999).contains(&year)
        {
            return Err("Release Year must be between 1000 and 2999.".to_owned());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TagSyncState {
    PendingImport,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TagEditRequest {
    pub(crate) track_id: String,
    pub(crate) track_key: String,
    pub(crate) expected: TagValues,
    pub(crate) desired: TagValues,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrackTagState {
    pub(crate) values: TagValues,
    pub(crate) sync_state: Option<TagSyncState>,
    pub(crate) can_undo: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_musicbee_half_star_scale() {
        for rating in [0.5, 1.0, 2.5, 4.5, 5.0] {
            assert!(
                TagValues {
                    rating: Some(rating),
                    love_state: LoveState::Neutral,
                    release_year: Some(2026),
                }
                .validate()
                .is_ok()
            );
        }
        for rating in [0.0, 3.25, 5.5] {
            assert!(
                TagValues {
                    rating: Some(rating),
                    love_state: LoveState::Neutral,
                    release_year: None,
                }
                .validate()
                .is_err()
            );
        }
    }
}
