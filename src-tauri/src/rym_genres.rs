//! Rate Your Music's genre list (`data/rym-genres.csv`: main genre, subgenre,
//! description). Main genres are broad; every subgenre is a genre in its own
//! right, however few albums of the library carry it.
use std::{collections::HashMap, sync::OnceLock};

const CSV: &str = include_str!("../data/rym-genres.csv");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Level {
    /// A top-level genre such as Rock or New Age.
    Main,
    /// A subgenre such as Neoclassical New Age.
    Sub,
}

/// Lower case, `&` as "and", letters and digits only: "Rock & Roll" and
/// "Rock And Roll" fold to the same key.
pub(crate) fn fold(genre: &str) -> String {
    genre
        .to_lowercase()
        .replace('&', "and")
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// The fields of one CSV record, with quoted fields unquoted. Returns the
/// record and the rest of the input.
fn record(input: &str) -> (Vec<String>, &str) {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = input.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        match c {
            '"' if quoted && chars.peek().is_some_and(|(_, next)| *next == '"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => fields.push(std::mem::take(&mut field)),
            '\n' if !quoted => {
                fields.push(field.trim_end_matches('\r').to_owned());
                return (fields, &input[at + 1..]);
            }
            _ => field.push(c),
        }
    }
    fields.push(field.trim_end_matches('\r').to_owned());
    (fields, "")
}

fn parse(csv: &str) -> HashMap<String, Level> {
    let mut rest = csv.trim_start_matches('\u{feff}');
    // The header.
    (_, rest) = record(rest);
    let mut levels = HashMap::new();
    while !rest.trim().is_empty() {
        let fields;
        (fields, rest) = record(rest);
        if let [main, sub, ..] = fields.as_slice() {
            // A main genre stays main even where it is also listed as a subgenre.
            levels.insert(fold(main), Level::Main);
            levels.entry(fold(sub)).or_insert(Level::Sub);
        }
    }
    levels
}

/// The genre's place in Rate Your Music's list, or `None` when it is not a
/// Rate Your Music genre.
pub(crate) fn level(genre: &str) -> Option<Level> {
    static LEVELS: OnceLock<HashMap<String, Level>> = OnceLock::new();
    LEVELS.get_or_init(|| parse(CSV)).get(&fold(genre)).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_quoted_fields() {
        let levels = parse(
            "\u{feff}Main Genre,Subgenre,Description\r\nNew Age,Neoclassical New Age,\"Early music, \"\"Baroque\"\"\nand more.\"\r\nRock,Metal,x\nMetal,Doom Metal,y",
        );
        assert_eq!(levels.get("newage"), Some(&Level::Main));
        assert_eq!(levels.get("neoclassicalnewage"), Some(&Level::Sub));
        assert_eq!(levels.get("metal"), Some(&Level::Main));
        assert_eq!(levels.get("doommetal"), Some(&Level::Sub));
        assert_eq!(levels.len(), 5);
    }

    #[test]
    fn knows_the_real_list() {
        assert_eq!(level("Neoclassical New Age"), Some(Level::Sub));
        assert_eq!(level("New Age"), Some(Level::Main));
        assert_eq!(level("Ambient"), Some(Level::Main));
        assert_eq!(level("Dark Ambient"), Some(Level::Sub));
        assert_eq!(level("Hip-Hop"), Some(Level::Main));
        assert_eq!(level("Metal"), Some(Level::Main));
        assert_eq!(level("Drama"), None);
    }
}
