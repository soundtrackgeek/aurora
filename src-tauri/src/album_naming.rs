//! The names Aurora gives album folders and tracks: `Album Artist - Album (Year)` and
//! `1-01 - Artist - Title.mp3`. Inbox renames and library renames share these rules so an
//! album gets the same names wherever it is renamed.

pub(crate) fn required_component(value: Option<&str>, label: &str) -> Result<String, String> {
    let value = value.map(str::trim).filter(|value| !value.is_empty());
    value
        .map(str::to_owned)
        .ok_or_else(|| format!("Add a {label} tag before renaming this album."))
}

/// `Album Artist - Album (Year)`, sanitized for Windows.
pub(crate) fn album_folder_name(
    album_artist: Option<&str>,
    album: Option<&str>,
    year: Option<i32>,
) -> Result<String, String> {
    let album_artist = required_component(album_artist, "Album Artist")?;
    let album_title = required_component(album, "Album")?;
    let year = year
        .filter(|value| *value > 0)
        .ok_or_else(|| "Add a Year tag before renaming this album.".to_owned())?;
    sanitize_component(&format!("{album_artist} - {album_title} ({year})"))
}

/// Digits used for track numbers: the largest track number (or total), at least two.
pub(crate) fn track_number_width(numbers: impl Iterator<Item = Option<u32>>) -> usize {
    numbers.flatten().max().map_or(2, decimal_width).max(2)
}

/// `1-01` with a disc number, `01` without. Disc 0 (an empty `0/0` disc tag) counts as no
/// disc number, as Music Library's catalog stores it.
pub(crate) fn track_position(disc_number: Option<u32>, track_number: u32, width: usize) -> String {
    match disc_number.filter(|disc| *disc > 0) {
        Some(disc) => format!("{disc}-{track_number:0width$}"),
        None => format!("{track_number:0width$}"),
    }
}

/// `01 - Artist - Title` without the extension, sanitized for Windows.
pub(crate) fn track_file_stem(
    position: &str,
    artist: Option<&str>,
    title: Option<&str>,
) -> Result<String, String> {
    let artist = required_component(artist, "Artist")?;
    let title = required_component(title, "Title")?;
    sanitize_component(&format!("{position} - {artist} - {title}"))
}

/// Names written for renamed albums and tracks. A colon becomes a space, as in the
/// library's existing folders (`Tail: Fievel` is `Tail  Fievel`); other characters
/// Windows rejects become `_`.
pub(crate) fn sanitize_component(value: &str) -> Result<String, String> {
    sanitize_component_with_colon(value, ' ')
}

/// The spelling Aurora wrote before colons became spaces. Albums renamed then stay ready.
pub(crate) fn legacy_sanitize_component(value: &str) -> Result<String, String> {
    sanitize_component_with_colon(value, '_')
}

fn sanitize_component_with_colon(value: &str, colon: char) -> Result<String, String> {
    let mut sanitized = value
        .chars()
        .map(|character| {
            if character == ':' {
                colon
            } else if character.is_control()
                || matches!(character, '<' | '>' | '"' | '/' | '\\' | '|' | '?' | '*')
            {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    sanitized = sanitized.trim().trim_end_matches(['.', ' ']).to_owned();
    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        return Err("The tags do not produce a valid Windows file name.".to_owned());
    }
    let stem = sanitized.split('.').next().unwrap_or_default();
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if reserved.iter().any(|name| stem.eq_ignore_ascii_case(name)) {
        sanitized.insert(0, '_');
    }
    Ok(sanitized)
}

pub(crate) fn decimal_width(value: u32) -> usize {
    value.max(1).ilog10() as usize + 1
}

/// `Album Artist - Album (Year)` before sanitizing, or `None` when a part is missing.
pub(crate) fn folder_label(
    album_artist: Option<&str>,
    album: Option<&str>,
    year: Option<i32>,
) -> Option<String> {
    let album_artist = album_artist
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let album = album.map(str::trim).filter(|value| !value.is_empty())?;
    let year = year.filter(|value| *value > 0)?;
    Some(format!("{album_artist} - {album} ({year})"))
}

/// Whether `actual` is `label` as Aurora writes it, in today's spelling or the legacy one
/// where a colon became `_`. Case is ignored, as Windows ignores it.
pub(crate) fn name_matches(actual: &str, label: &str) -> bool {
    [sanitize_component(label), legacy_sanitize_component(label)]
        .into_iter()
        .flatten()
        .any(|expected| actual.eq_ignore_ascii_case(&expected))
}

/// A file name without its extension.
pub(crate) fn file_stem(file_name: &str) -> &str {
    file_name
        .rsplit_once('.')
        .map_or(file_name, |(stem, _)| stem)
}

/// The tags a track file is named from.
#[derive(Clone, Copy, Debug)]
pub(crate) struct NamingTrack<'a> {
    pub(crate) file_name: &'a str,
    pub(crate) artist: Option<&'a str>,
    pub(crate) title: Option<&'a str>,
    pub(crate) track_number: Option<u32>,
    pub(crate) track_total: Option<u32>,
    pub(crate) disc_number: Option<u32>,
}

impl NamingTrack<'_> {
    /// `1-01 - Artist - Title` before sanitizing, or `None` when a tag is missing.
    fn label(&self, width: usize) -> Option<String> {
        let number = self.track_number?;
        let artist = self
            .artist
            .map(str::trim)
            .filter(|value| !value.is_empty())?;
        let title = self
            .title
            .map(str::trim)
            .filter(|value| !value.is_empty())?;
        let position = track_position(self.disc_number, number, width);
        Some(format!("{position} - {artist} - {title}"))
    }
}

/// The tracks whose file is not named `1-01 - Artist - Title` (or `01 - …` without a disc
/// number), with the name each should have. Tracks missing a number, artist or title are
/// left out: their name cannot be judged. Any extension is accepted.
pub(crate) fn misnamed_tracks<'a>(tracks: &[NamingTrack<'a>]) -> Vec<(&'a str, String)> {
    let width = track_number_width(
        tracks
            .iter()
            .map(|track| track.track_number.or(track.track_total)),
    );
    tracks
        .iter()
        .filter_map(|track| {
            let label = track.label(width)?;
            (!name_matches(file_stem(track.file_name), &label)).then(|| {
                let expected = sanitize_component(&label).unwrap_or(label);
                (track.file_name, expected)
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disc_zero_counts_as_no_disc_number() {
        assert_eq!(track_position(Some(0), 1, 2), "01");
        assert_eq!(track_position(None, 1, 2), "01");
        assert_eq!(track_position(Some(1), 1, 2), "1-01");
        let zero_disc = NamingTrack {
            file_name: "0-01 - Gloria Gaynor - Ain't No Bigger Fool.mp3",
            artist: Some("Gloria Gaynor"),
            title: Some("Ain't No Bigger Fool"),
            track_number: Some(1),
            track_total: Some(8),
            disc_number: Some(0),
        };
        assert_eq!(
            misnamed_tracks(&[zero_disc]),
            vec![(
                zero_disc.file_name,
                "01 - Gloria Gaynor - Ain't No Bigger Fool".to_owned()
            )]
        );
        let renamed = NamingTrack {
            file_name: "01 - Gloria Gaynor - Ain't No Bigger Fool.mp3",
            ..zero_disc
        };
        assert!(misnamed_tracks(&[renamed]).is_empty());
    }

    #[test]
    fn folder_and_track_names_follow_the_inbox_pattern() {
        assert_eq!(
            album_folder_name(Some("Artist"), Some("Album: Live"), Some(1976)).unwrap(),
            "Artist - Album  Live (1976)"
        );
        assert_eq!(
            album_folder_name(Some("Artist"), Some("Album"), None).unwrap_err(),
            "Add a Year tag before renaming this album."
        );
        assert_eq!(
            album_folder_name(Some(" "), Some("Album"), Some(1976)).unwrap_err(),
            "Add a Album Artist tag before renaming this album."
        );
        let width = track_number_width([Some(9), Some(11), None].into_iter());
        assert_eq!(width, 2);
        assert_eq!(track_number_width([Some(101)].into_iter()), 3);
        assert_eq!(track_position(Some(1), 3, width), "1-03");
        assert_eq!(track_position(None, 3, width), "03");
        assert_eq!(
            track_file_stem("03", Some("AC/DC"), Some("T.N.T.")).unwrap(),
            "03 - AC_DC - T.N.T"
        );
    }
}
