//! Chart identity keys shared by Music Library and Aurora (keep copies aligned).
use unicode_normalization::UnicodeNormalization;

/// Fold case, accents, punctuation, and `&`. Stored chart keys depend on this
/// contract; changing it requires rebuilding every stored key in the catalog.
pub(crate) fn text_key(value: &str) -> String {
    let lowercased = value.replace('&', " and ").to_lowercase();
    let folded = lowercased
        .nfd()
        .filter(|character| !unicode_normalization::char::is_combining_mark(*character))
        .fold(String::new(), |mut normalized, character| {
            match character {
                'æ' => normalized.push_str("ae"),
                'œ' => normalized.push_str("oe"),
                'ø' => normalized.push('o'),
                'ð' => normalized.push('d'),
                'þ' => normalized.push_str("th"),
                'ł' => normalized.push('l'),
                'ß' => normalized.push_str("ss"),
                _ => normalized.push(character),
            }
            normalized
        });

    folded
        .split(|character: char| !character.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Group printed spellings of one artist credit. Archives print the same duo
/// with "&", "and", "/" or "+", and bands with or without a leading "The".
/// Different names (ELO vs Electric Light Orchestra) need explicit aliases.
pub(crate) fn artist_group_key(value: &str) -> String {
    let key = text_key(value);
    let mut words = key
        .split(' ')
        .filter(|word| *word != "and")
        .collect::<Vec<_>>();
    if words.len() > 1 && words[0] == "the" {
        words.remove(0);
    }
    if words.is_empty() {
        return key;
    }
    words.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_keys_fold_case_accents_punctuation_and_ampersands() {
        for (value, expected) in [
            ("  BÉYONCÉ & JAY-Z  ", "beyonce and jay z"),
            ("Æ Œ Ø Ð Þ Ł ß", "ae oe o d th l ss"),
            ("Don't  Stop… (Live)", "don t stop live"),
            ("P!nk", "p nk"),
            ("", ""),
        ] {
            assert_eq!(text_key(value), expected, "{value}");
        }
    }

    #[test]
    fn artist_group_keys_merge_printed_credit_variants_only() {
        for spellings in [
            &[
                "Daryl Hall & John Oates",
                "Daryl Hall John Oates",
                "Daryl Hall / John Oates",
                "DARYL HALL AND JOHN OATES",
            ][..],
            &["Prince And The Revolution", "Prince & The Revolution"],
            &["The Clovers", "Clovers", "CLOVERS"],
            &["Queen & David Bowie", "Queen + David Bowie"],
            &["Beyoncé", "BEYONCE"],
        ] {
            let expected = artist_group_key(spellings[0]);
            for spelling in spellings {
                assert_eq!(artist_group_key(spelling), expected, "{spelling}");
            }
        }
        for (left, right) in [
            ("ELO", "Electric Light Orchestra"),
            (
                "Electric Light Orchestra",
                "Electric Light Orchestra Part Two",
            ),
            ("P!nk", "P!nk featuring Nate Ruess"),
            ("Pink", "P!nk"),
        ] {
            assert_ne!(artist_group_key(left), artist_group_key(right));
        }
        assert_eq!(artist_group_key("The The"), "the");
        assert_eq!(artist_group_key("And"), "and");
        assert_eq!(artist_group_key(""), "");
    }
}
