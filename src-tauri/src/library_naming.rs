//! Checks whether a library album is named the way Aurora renames albums: the folder as
//! `Album Artist - Album (Year)` and each track as `1-01 - Artist - Title` (`01 - …` without
//! a disc number). The Albums view shows the result as an icon, read from the catalog so a
//! page of albums is checked without opening any file.

use crate::album_naming::{
    NamingTrack, folder_label, misnamed_tracks, name_matches, sanitize_component,
};
use rusqlite::{Connection, params_from_iter, types::Value};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// The naming state of one album.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub(crate) enum AlbumNamingCheck {
    /// The folder and every track file follow the tags.
    Organized,
    /// A rename would change the folder or some track files.
    Misnamed { issues: Vec<String> },
    /// A tag the names come from is missing or differs between tracks.
    Unchecked { reason: String },
}

/// One catalog track, as the check needs it.
#[derive(Clone, Debug, Default)]
pub(crate) struct CatalogNamingTrack {
    pub(crate) directory: String,
    pub(crate) filename: String,
    pub(crate) album_artist: Option<String>,
    pub(crate) album: Option<String>,
    pub(crate) year: Option<i32>,
    pub(crate) artist: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) track_number: Option<u32>,
    pub(crate) disc_number: Option<u32>,
}

/// Judges an album with the rename's rules: one Album Artist (or Artist), Album and Year
/// shared by every track, and a number, artist and title on each track.
pub(crate) fn check_album(tracks: &[CatalogNamingTrack]) -> AlbumNamingCheck {
    if tracks.is_empty() {
        return unchecked("This album has no tracks to check.");
    }
    let Some(album_artist) = shared(tracks.iter().map(|track| {
        text(track.album_artist.as_deref()).or_else(|| text(track.artist.as_deref()))
    })) else {
        return unchecked("The tracks have different Album Artist tags.");
    };
    let Some(album) = shared(tracks.iter().map(|track| text(track.album.as_deref()))) else {
        return unchecked("The tracks have different Album tags.");
    };
    let Some(year) = shared(
        tracks
            .iter()
            .map(|track| track.year.filter(|year| *year > 0)),
    ) else {
        return unchecked("The tracks have different Year tags.");
    };
    let Some(label) = folder_label(album_artist, album, year) else {
        let missing = [
            album_artist.is_none().then_some("Album Artist"),
            album.is_none().then_some("Album"),
            year.is_none().then_some("Year"),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ");
        return unchecked(&format!("Add {missing} tags to check the album's naming."));
    };
    let untagged = tracks
        .iter()
        .filter(|track| {
            track.track_number.is_none()
                || text(track.artist.as_deref()).is_none()
                || text(track.title.as_deref()).is_none()
        })
        .count();
    if untagged > 0 {
        return unchecked(&format!(
            "Add Track Number, Artist and Title tags to {} to check its file name{}.",
            count(untagged, "track"),
            if untagged == 1 { "" } else { "s" }
        ));
    }

    let mut issues = Vec::new();
    let mut directories = HashSet::new();
    // Disc subfolders are named by their parent, as the rename treats them.
    let folder_ok = tracks
        .iter()
        .filter(|track| directories.insert(track.directory.to_lowercase()))
        .all(|track| {
            track
                .directory
                .split(['\\', '/'])
                .any(|component| name_matches(component, &label))
        });
    if !folder_ok {
        let expected = sanitize_component(&label).unwrap_or(label);
        issues.push(format!("Folder should be “{expected}”"));
    }
    let naming = tracks
        .iter()
        .map(|track| NamingTrack {
            file_name: &track.filename,
            artist: track.artist.as_deref(),
            title: track.title.as_deref(),
            track_number: track.track_number,
            track_total: None,
            disc_number: track.disc_number,
        })
        .collect::<Vec<_>>();
    // Music Library's tag sync treats no disc, disc 0 and disc 1 as the same disc and keeps
    // the catalog's old value, so a track tagged `1/1` can be cataloged without a disc number.
    // Without one, the disc-1 spelling `1-01 - …` passes as well as `01 - …`.
    let as_disc_one = naming
        .iter()
        .map(|track| NamingTrack {
            disc_number: track.disc_number.filter(|disc| *disc > 0).or(Some(1)),
            ..*track
        })
        .collect::<Vec<_>>();
    let misnamed_as_disc_one = misnamed_tracks(&as_disc_one)
        .into_iter()
        .map(|(file_name, _)| file_name)
        .collect::<HashSet<_>>();
    let misnamed = misnamed_tracks(&naming)
        .into_iter()
        .filter(|(file_name, _)| misnamed_as_disc_one.contains(file_name))
        .collect::<Vec<_>>();
    if let Some((first, expected)) = misnamed.first() {
        issues.push(format!(
            "{} of {} not named Disc-Track - Artist - Title",
            count(misnamed.len(), "track file"),
            tracks.len()
        ));
        issues.push(format!("“{first}” should be “{expected}”"));
    }
    if issues.is_empty() {
        AlbumNamingCheck::Organized
    } else {
        AlbumNamingCheck::Misnamed { issues }
    }
}

fn unchecked(reason: &str) -> AlbumNamingCheck {
    AlbumNamingCheck::Unchecked {
        reason: reason.to_owned(),
    }
}

fn count(value: usize, noun: &str) -> String {
    if value == 1 {
        format!("1 {noun}")
    } else {
        format!("{value} {noun}s")
    }
}

fn text(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

/// `Some(value)` when every track agrees (an empty value included), `None` when they differ.
fn shared<T: Eq + std::hash::Hash + Copy>(
    values: impl Iterator<Item = Option<T>>,
) -> Option<Option<T>> {
    let values = values.collect::<HashSet<_>>();
    match values.len() {
        0 => Some(None),
        1 => values.into_iter().next(),
        _ => None,
    }
}

/// Reads the page's tracks in one query and checks each album. A catalog without the
/// columns this needs leaves every album unchecked instead of failing the page.
pub(crate) fn check_albums(
    connection: &Connection,
    album_ids: &[&str],
) -> HashMap<String, AlbumNamingCheck> {
    if album_ids.is_empty() {
        return HashMap::new();
    }
    match read_tracks(connection, album_ids) {
        Ok(tracks) => tracks
            .into_iter()
            .map(|(album_id, tracks)| (album_id, check_album(&tracks)))
            .collect(),
        Err(error) => {
            eprintln!("Aurora could not check album naming: {error}");
            HashMap::new()
        }
    }
}

fn read_tracks(
    connection: &Connection,
    album_ids: &[&str],
) -> rusqlite::Result<HashMap<String, Vec<CatalogNamingTrack>>> {
    let placeholders = vec!["?"; album_ids.len()].join(", ");
    let sql = format!(
        "SELECT CAST(album_id AS TEXT), COALESCE(file_path, ''), COALESCE(filename, ''),
                album_artist_display, album, year, display_artist, title, track_number, disc_number
         FROM tracks WHERE album_id IN ({placeholders})"
    );
    let mut statement = connection.prepare(&sql)?;
    let params = album_ids
        .iter()
        .map(|id| Value::Text((*id).to_owned()))
        .collect::<Vec<_>>();
    let mut rows = statement.query(params_from_iter(params.iter()))?;
    let mut albums = HashMap::<String, Vec<CatalogNamingTrack>>::new();
    while let Some(row) = rows.next()? {
        let album_id: String = row.get(0)?;
        albums
            .entry(album_id)
            .or_default()
            .push(CatalogNamingTrack {
                directory: row.get(1)?,
                filename: row.get(2)?,
                album_artist: row.get(3)?,
                album: row.get(4)?,
                year: row
                    .get::<_, Option<i64>>(5)?
                    .and_then(|year| i32::try_from(year).ok()),
                artist: row.get(6)?,
                title: row.get(7)?,
                track_number: row
                    .get::<_, Option<i64>>(8)?
                    .and_then(|value| u32::try_from(value).ok()),
                disc_number: row
                    .get::<_, Option<i64>>(9)?
                    .and_then(|value| u32::try_from(value).ok()),
            });
    }
    Ok(albums)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(directory: &str, filename: &str, number: u32, title: &str) -> CatalogNamingTrack {
        CatalogNamingTrack {
            directory: directory.to_owned(),
            filename: filename.to_owned(),
            album_artist: Some("Pink Floyd".to_owned()),
            album: Some("Animals".to_owned()),
            year: Some(1977),
            artist: Some("Pink Floyd".to_owned()),
            title: Some(title.to_owned()),
            track_number: Some(number),
            disc_number: Some(1),
        }
    }

    const FOLDER: &str = r"M:\Music\Pink Floyd - Animals (1977)";

    #[test]
    fn organized_album_passes_with_disc_prefixed_tracks() {
        let tracks = [
            track(FOLDER, "1-01 - Pink Floyd - Dogs.mp3", 1, "Dogs"),
            track(FOLDER, "1-02 - Pink Floyd - Sheep.mp3", 2, "Sheep"),
        ];
        assert_eq!(check_album(&tracks), AlbumNamingCheck::Organized);
    }

    #[test]
    fn tracks_without_a_disc_number_use_the_plain_position() {
        let mut plain = track(FOLDER, "01 - Pink Floyd - Dogs.mp3", 1, "Dogs");
        plain.disc_number = None;
        assert_eq!(check_album(&[plain.clone()]), AlbumNamingCheck::Organized);
        plain.filename = "Dogs.mp3".to_owned();
        assert_eq!(
            check_album(&[plain]),
            AlbumNamingCheck::Misnamed {
                issues: vec![
                    "1 track file of 1 not named Disc-Track - Artist - Title".to_owned(),
                    "“Dogs.mp3” should be “01 - Pink Floyd - Dogs”".to_owned(),
                ]
            }
        );
    }

    #[test]
    fn tracks_without_a_disc_number_may_use_the_disc_one_position() {
        // A `1/1` disc tag that Music Library's sync left out of the catalog.
        let mut tracks = [
            track(FOLDER, "1-01 - Pink Floyd - Dogs.mp3", 1, "Dogs"),
            track(FOLDER, "1-02 - Pink Floyd - Sheep.mp3", 2, "Sheep"),
        ];
        for track in &mut tracks {
            track.disc_number = None;
        }
        assert_eq!(check_album(&tracks), AlbumNamingCheck::Organized);
        tracks[1].filename = "2-02 - Pink Floyd - Sheep.mp3".to_owned();
        assert!(matches!(
            check_album(&tracks),
            AlbumNamingCheck::Misnamed { .. }
        ));
    }

    #[test]
    fn reports_the_folder_and_the_first_misnamed_track() {
        let tracks = [
            track(r"M:\Music\animals", "dogs.mp3", 1, "Dogs"),
            track(
                r"M:\Music\animals",
                "1-02 - Pink Floyd - Sheep.mp3",
                2,
                "Sheep",
            ),
        ];
        assert_eq!(
            check_album(&tracks),
            AlbumNamingCheck::Misnamed {
                issues: vec![
                    "Folder should be “Pink Floyd - Animals (1977)”".to_owned(),
                    "1 track file of 2 not named Disc-Track - Artist - Title".to_owned(),
                    "“dogs.mp3” should be “1-01 - Pink Floyd - Dogs”".to_owned(),
                ]
            }
        );
    }

    #[test]
    fn accepts_disc_subfolders_case_and_the_legacy_colon_spelling() {
        let mut live = track(
            r"M:\Music\pink floyd - pulse_ live (1995)\CD1",
            "1-01 - Pink Floyd - Shine On.flac",
            1,
            "Shine On",
        );
        live.album = Some("Pulse: Live".to_owned());
        live.year = Some(1995);
        assert_eq!(check_album(&[live]), AlbumNamingCheck::Organized);
    }

    #[test]
    fn missing_or_mixed_tags_leave_the_album_unchecked() {
        let mut no_year = track(FOLDER, "1-01 - Pink Floyd - Dogs.mp3", 1, "Dogs");
        no_year.year = None;
        assert_eq!(
            check_album(&[no_year]),
            unchecked("Add Year tags to check the album's naming.")
        );
        let mut other_year = track(FOLDER, "1-02 - Pink Floyd - Sheep.mp3", 2, "Sheep");
        other_year.year = Some(1978);
        let tracks = [
            track(FOLDER, "1-01 - Pink Floyd - Dogs.mp3", 1, "Dogs"),
            other_year,
        ];
        assert_eq!(
            check_album(&tracks),
            unchecked("The tracks have different Year tags.")
        );
        let mut untitled = track(FOLDER, "1-01.mp3", 1, "");
        untitled.title = None;
        assert_eq!(
            check_album(&[untitled]),
            unchecked("Add Track Number, Artist and Title tags to 1 track to check its file name.")
        );
    }

    #[test]
    fn wide_albums_use_three_digit_positions() {
        let mut tracks = (1..=100)
            .map(|number| {
                track(
                    FOLDER,
                    &format!("1-{number:03} - Pink Floyd - Song {number}.mp3"),
                    number,
                    &format!("Song {number}"),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(check_album(&tracks), AlbumNamingCheck::Organized);
        tracks[0].filename = "1-01 - Pink Floyd - Song 1.mp3".to_owned();
        assert!(matches!(
            check_album(&tracks),
            AlbumNamingCheck::Misnamed { .. }
        ));
    }

    #[test]
    fn checks_a_page_of_albums_from_the_catalog() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE tracks (album_id INTEGER, file_path TEXT, filename TEXT,
                   album_artist_display TEXT, album TEXT, year INTEGER, display_artist TEXT,
                   title TEXT, track_number INTEGER, disc_number INTEGER);
                 INSERT INTO tracks VALUES
                   (7, 'M:\\Music\\Pink Floyd - Animals (1977)', '01 - Pink Floyd - Dogs.mp3',
                    'Pink Floyd', 'Animals', 1977, 'Pink Floyd', 'Dogs', 1, NULL),
                   (8, 'M:\\Music\\Untagged', 'a.mp3', NULL, NULL, NULL, NULL, NULL, NULL, NULL);",
            )
            .unwrap();
        let checks = check_albums(&connection, &["7", "8", "9"]);
        assert_eq!(checks["7"], AlbumNamingCheck::Organized);
        assert!(matches!(checks["8"], AlbumNamingCheck::Unchecked { .. }));
        assert!(!checks.contains_key("9"));
        let bare = Connection::open_in_memory().unwrap();
        bare.execute_batch("CREATE TABLE tracks (album_id INTEGER);")
            .unwrap();
        assert!(check_albums(&bare, &["7"]).is_empty());
    }
}
