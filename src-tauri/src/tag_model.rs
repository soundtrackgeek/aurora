use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum TagEditorTarget {
    Track {
        track_id: String,
        track_key: String,
        #[serde(default)]
        label: Option<String>,
    },
    Album {
        album_id: String,
        #[serde(default)]
        label: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum EditableTagField {
    AlbumArtist,
    Artist,
    Album,
    Title,
    Genre,
    Publisher,
    Rating,
    Year,
    ReleaseYear,
    TrackNumber,
    TrackTotal,
    DiscNumber,
    DiscTotal,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct EditableTagValues {
    pub(crate) album_artist: Option<String>,
    pub(crate) artist: Option<String>,
    pub(crate) album: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) genre: Option<String>,
    pub(crate) publisher: Option<String>,
    pub(crate) rating: Option<f64>,
    pub(crate) year: Option<i32>,
    pub(crate) release_year: Option<i32>,
    pub(crate) track_number: Option<u32>,
    pub(crate) track_total: Option<u32>,
    pub(crate) disc_number: Option<u32>,
    pub(crate) disc_total: Option<u32>,
}

impl EditableTagValues {
    pub(crate) fn validate(&self) -> Result<(), String> {
        for (label, value, max_chars) in [
            ("Album Artist", self.album_artist.as_deref(), 512),
            ("Artist", self.artist.as_deref(), 512),
            ("Album", self.album.as_deref(), 512),
            ("Track Title", self.title.as_deref(), 1024),
            ("Genre", self.genre.as_deref(), 256),
            ("Publisher", self.publisher.as_deref(), 512),
        ] {
            if value.is_some_and(|value| value.chars().count() > max_chars) {
                return Err(format!("{label} is too long."));
            }
            if value.is_some_and(|value| value.chars().any(char::is_control)) {
                return Err(format!(
                    "{label} contains an unsupported control character."
                ));
            }
        }
        if self.album_artist.as_deref().is_some_and(|value| {
            let credits = value.split(';').collect::<Vec<_>>();
            credits.len() > 64 || credits.iter().any(|credit| credit.trim().is_empty())
        }) {
            return Err("Album Artist contains an invalid or empty credit.".to_owned());
        }
        if let Some(rating) = self.rating {
            let doubled = rating * 2.0;
            if !rating.is_finite()
                || !(0.5..=5.0).contains(&rating)
                || (doubled.round() - doubled).abs() > f64::EPSILON
            {
                return Err("Rating must be unrated or a half-star value from 0.5 to 5.".to_owned());
            }
        }
        for (label, value) in [("Year", self.year), ("Release Year", self.release_year)] {
            if value.is_some_and(|year| !(1000..=2999).contains(&year)) {
                return Err(format!("{label} must be between 1000 and 2999."));
            }
        }
        for (label, value) in [
            ("Track Number", self.track_number),
            ("Track Total", self.track_total),
            ("Disc Number", self.disc_number),
            ("Disc Total", self.disc_total),
        ] {
            if value.is_some_and(|number| number == 0 || number > 9_999) {
                return Err(format!("{label} must be between 1 and 9999."));
            }
        }
        Ok(())
    }

    pub(crate) fn normalize(mut self) -> Self {
        for value in [
            &mut self.album_artist,
            &mut self.artist,
            &mut self.album,
            &mut self.title,
            &mut self.genre,
            &mut self.publisher,
        ] {
            *value = value
                .take()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty());
        }
        self.album_artist = self.album_artist.map(|value| {
            value
                .split(';')
                .map(str::trim)
                .collect::<Vec<_>>()
                .join("; ")
        });
        self
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TagEditorTrackState {
    pub(crate) track_id: String,
    pub(crate) track_key: String,
    pub(crate) revision: String,
    pub(crate) values: EditableTagValues,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TagEditorSnapshot {
    pub(crate) tracks: Vec<TagEditorTrackState>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TagEditorUpdateRequest {
    pub(crate) target: TagEditorTarget,
    pub(crate) expected: TagEditorSnapshot,
    pub(crate) fields: Vec<EditableTagField>,
    pub(crate) values: EditableTagValues,
}

impl TagEditorUpdateRequest {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.fields.is_empty() {
            return Err("Choose at least one tag field before saving.".to_owned());
        }
        let unique = self.fields.iter().copied().collect::<HashSet<_>>();
        if unique.len() != self.fields.len() {
            return Err("The tag edit contains duplicate fields.".to_owned());
        }
        for (field, value, label) in [
            (
                EditableTagField::AlbumArtist,
                self.values.album_artist.as_deref(),
                "Album Artist",
            ),
            (
                EditableTagField::Album,
                self.values.album.as_deref(),
                "Album",
            ),
            (
                EditableTagField::Title,
                self.values.title.as_deref(),
                "Track Title",
            ),
        ] {
            if self.fields.contains(&field) && value.is_none_or(|value| value.trim().is_empty()) {
                return Err(format!(
                    "{label} is required by Music Library and cannot be cleared."
                ));
            }
        }
        self.values.validate()
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TagEditorUpdateResult {
    pub(crate) state: TagEditorSnapshot,
    pub(crate) tracks: Vec<crate::catalog::TrackSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) catalog_sync: Option<TagEditorCatalogSync>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TagEditorCatalogSyncStatus {
    Synced,
    Pending,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TagEditorCatalogSync {
    pub(crate) status: TagEditorCatalogSyncStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) message: Option<String>,
}

impl TagEditorCatalogSync {
    pub(crate) fn synced() -> Self {
        Self {
            status: TagEditorCatalogSyncStatus::Synced,
            message: None,
        }
    }

    pub(crate) fn pending(message: String) -> Self {
        Self {
            status: TagEditorCatalogSyncStatus::Pending,
            message: Some(message),
        }
    }
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
