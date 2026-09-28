//! Song identity fallback shared by Music Library and Aurora (keep copies aligned).
use std::collections::{HashMap, HashSet};

/// Remove balanced parenthetical groups only; malformed or empty titles have no fallback.
pub(crate) fn without_parentheses(value: &str) -> Option<String> {
    let mut depth = 0usize;
    let mut removed = false;
    let mut result = String::new();
    for c in value.chars() {
        match c {
            '(' => {
                depth += 1;
                removed = true;
                if depth == 1 {
                    result.push(' ');
                }
            }
            ')' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
            }
            _ if depth == 0 => result.push(c),
            _ => {}
        }
    }
    let result = result.split_whitespace().collect::<Vec<_>>().join(" ");
    (removed && depth == 0 && !result.is_empty()).then_some(result)
}

type Candidates = HashMap<(String, String), Vec<(usize, String)>>;
#[derive(Default)]
pub(crate) struct SongIndex {
    exact: Candidates,
    aliases: Candidates,
    bases: Candidates,
}
impl SongIndex {
    pub(crate) fn insert(
        &mut self,
        id: usize,
        artists: &[String],
        full: &str,
        aliases: &[String],
        base: Option<&str>,
    ) {
        if full.is_empty() {
            return;
        }
        for artist in artists.iter().filter(|a| !a.is_empty()) {
            self.exact
                .entry((artist.clone(), full.into()))
                .or_default()
                .push((id, full.into()));
            for title in aliases {
                self.aliases
                    .entry((artist.clone(), title.clone()))
                    .or_default()
                    .push((id, full.into()));
            }
            if let Some(base) = base.filter(|b| !b.is_empty()) {
                self.bases
                    .entry((artist.clone(), base.into()))
                    .or_default()
                    .push((id, full.into()));
            }
        }
    }

    pub(crate) fn resolve(
        &self,
        artists: &[String],
        full: &str,
        aliases: &[String],
        base: Option<&str>,
    ) -> Vec<usize> {
        if full.is_empty() {
            return Vec::new();
        }
        let collect = |map: &Candidates, titles: &[String]| {
            artists
                .iter()
                .flat_map(|artist| {
                    titles
                        .iter()
                        .filter_map(|title| map.get(&(artist.clone(), title.clone())))
                })
                .flatten()
                .cloned()
                .collect::<Vec<_>>()
        };
        let exact = collect(&self.exact, &[full.into()]);
        let candidates = if !exact.is_empty() {
            exact
        } else {
            let aliases = collect(&self.aliases, aliases);
            if !aliases.is_empty() {
                aliases
            } else {
                // Only one side may lose parentheses: never equate two differing versions.
                let mut fallback = collect(&self.bases, &[full.into()]);
                if let Some(base) = base.filter(|b| !b.is_empty()) {
                    fallback.extend(collect(&self.exact, &[base.into()]));
                }
                // Multiple copies of one title are fine; different titles are ambiguous.
                if fallback
                    .iter()
                    .map(|(_, title)| title)
                    .collect::<HashSet<_>>()
                    .len()
                    > 1
                {
                    return Vec::new();
                }
                fallback
            }
        };
        let mut ids = candidates.into_iter().map(|(id, _)| id).collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parentheses_are_balanced_nonempty_and_word_separated() {
        assert_eq!(
            without_parentheses("Song (Single (US) Mix) (Edit)"),
            Some("Song".into())
        );
        assert_eq!(without_parentheses("A(Mix)B"), Some("A B".into()));
        for title in ["(Only Title)", "Song (Edit", "Song )Edit(", "Song"] {
            assert_eq!(without_parentheses(title), None);
        }
    }
    #[test]
    fn exact_first_artist_scoped_and_ambiguous_fallback_rejected() {
        let artists = vec!["artist".into()];
        let mut index = SongIndex::default();
        index.insert(
            1,
            &artists,
            "song live",
            &["song live".into()],
            Some("song"),
        );
        assert_eq!(
            index.resolve(&artists, "song", &["song".into()], None),
            vec![1]
        );
        index.insert(
            2,
            &artists,
            "song remix",
            &["song remix".into()],
            Some("song"),
        );
        assert!(
            index
                .resolve(&artists, "song", &["song".into()], None)
                .is_empty()
        );
        assert_eq!(
            index.resolve(&artists, "song live", &["song live".into()], Some("song")),
            vec![1]
        );
        assert!(
            index
                .resolve(&["other".into()], "song live", &[], None)
                .is_empty()
        );
        assert!(
            index
                .resolve(&artists, "song edit", &[], Some("song"))
                .is_empty()
        );
        index.insert(3, &artists, "song", &["song".into()], None);
        assert_eq!(
            index.resolve(&artists, "song edit", &[], Some("song")),
            vec![3]
        );
        assert_eq!(
            index.resolve(&artists, "song", &["song".into()], None),
            vec![3]
        );
    }
}
