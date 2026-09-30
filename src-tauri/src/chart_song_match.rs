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
    performers: Candidates,
}
impl SongIndex {
    /// `credits` are the library track's chart_identity::main_performers.
    pub(crate) fn insert(
        &mut self,
        id: usize,
        artists: &[String],
        credits: &[String],
        full: &str,
        aliases: &[String],
        base: Option<&str>,
    ) {
        if full.is_empty() {
            return;
        }
        for title in std::iter::once(full).chain(aliases.iter().map(String::as_str)) {
            for credit in credits {
                self.performers
                    .entry((credit.clone(), title.to_string()))
                    .or_default()
                    .push((id, full.into()));
            }
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

    /// `credits` are the chart entry's chart_identity::main_performers, lead first.
    pub(crate) fn resolve(
        &self,
        artists: &[String],
        credits: &[String],
        full: &str,
        aliases: &[String],
        base: Option<&str>,
    ) -> Vec<usize> {
        if full.is_empty() {
            return Vec::new();
        }
        let collect_for = |map: &Candidates, artists: &[String], titles: &[String]| {
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
        let collect = |map: &Candidates, titles: &[String]| collect_for(map, artists, titles);
        let exact = collect(&self.exact, &[full.into()]);
        let candidates = if !exact.is_empty() {
            exact
        } else {
            let aliased = collect(&self.aliases, aliases);
            if !aliased.is_empty() {
                aliased
            } else {
                // Only one side may lose parentheses: never equate two differing versions.
                let mut fallback = collect(&self.bases, &[full.into()]);
                if let Some(base) = base.filter(|b| !b.is_empty()) {
                    fallback.extend(collect(&self.exact, &[base.into()]));
                }
                // Last resort for differently printed collaborations: the chart's
                // lead is a main performer of the library credit ("JOHN LENNON" vs
                // "John Lennon & Yoko Ono"). The reverse mostly pairs remakes with
                // originals ("Kygo & Tina Turner" vs "Tina Turner"), so it is not used.
                if fallback.is_empty() {
                    let mut titles = aliases.to_vec();
                    titles.push(full.into());
                    fallback =
                        collect_for(&self.performers, &credits[..credits.len().min(1)], &titles);
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
            &[],
            "song live",
            &["song live".into()],
            Some("song"),
        );
        assert_eq!(
            index.resolve(&artists, &[], "song", &["song".into()], None),
            vec![1]
        );
        index.insert(
            2,
            &artists,
            &[],
            "song remix",
            &["song remix".into()],
            Some("song"),
        );
        assert!(
            index
                .resolve(&artists, &[], "song", &["song".into()], None)
                .is_empty()
        );
        assert_eq!(
            index.resolve(
                &artists,
                &[],
                "song live",
                &["song live".into()],
                Some("song")
            ),
            vec![1]
        );
        assert!(
            index
                .resolve(&["other".into()], &[], "song live", &[], None)
                .is_empty()
        );
        assert!(
            index
                .resolve(&artists, &[], "song edit", &[], Some("song"))
                .is_empty()
        );
        index.insert(3, &artists, &[], "song", &["song".into()], None);
        assert_eq!(
            index.resolve(&artists, &[], "song edit", &[], Some("song")),
            vec![3]
        );
        assert_eq!(
            index.resolve(&artists, &[], "song", &["song".into()], None),
            vec![3]
        );
    }
    #[test]
    fn collaborations_match_by_lead_performer_only_as_a_last_resort() {
        let credits = |value: &str| crate::chart_identity::main_performers(value);
        let key = |value: &str| vec![crate::chart_identity::artist_group_key(value)];
        let mut index = SongIndex::default();
        for (id, artist, title) in [
            (1, "John Lennon & Yoko Ono", "woman"),
            (2, "Dave Stewart & Barbara Gaskin", "it s my party"),
            (3, "Percy Faith & His Orchestra", "moon river"),
            (4, "Yoko Ono", "walking on thin ice"),
        ] {
            index.insert(id, &key(artist), &credits(artist), title, &[], None);
        }
        let resolve = |index: &SongIndex, artist: &str, title: &str| {
            index.resolve(&key(artist), &credits(artist), title, &[], None)
        };
        // The chart lead is one of the library track's main performers.
        assert_eq!(resolve(&index, "JOHN LENNON", "woman"), vec![1]);
        assert_eq!(
            resolve(&index, "DAVE STEWART WITH BARBARA GASKIN", "it s my party"),
            vec![2]
        );
        assert_eq!(
            resolve(&index, "Yoko Ono feat. John Lennon", "woman"),
            vec![1]
        );
        assert_eq!(resolve(&index, "Barbara Gaskin", "it s my party"), vec![2]);
        // A library lead that is only a co-credit on the chart is not enough.
        assert!(resolve(&index, "Kygo & Yoko Ono", "walking on thin ice").is_empty());
        // Remakes featuring the original artist are different recordings.
        index.insert(
            8,
            &key("Rod Stewart"),
            &credits("Rod Stewart"),
            "sexy",
            &[],
            None,
        );
        index.insert(
            9,
            &key("N-Trance feat. Rod Stewart"),
            &credits("N-Trance feat. Rod Stewart"),
            "remake",
            &[],
            None,
        );
        assert!(resolve(&index, "N-TRANCE FT ROD STEWART", "sexy").is_empty());
        assert!(resolve(&index, "ROD STEWART", "remake").is_empty());
        // A shared supporting credit, or a different title, is not a match.
        assert!(resolve(&index, "Ray Conniff & His Orchestra", "moon river").is_empty());
        assert!(resolve(&index, "John Lennon", "walking on thin ice").is_empty());
        // Exact credits still win over performer matches.
        index.insert(
            5,
            &key("John Lennon"),
            &credits("John Lennon"),
            "woman",
            &[],
            None,
        );
        assert_eq!(resolve(&index, "John Lennon", "woman"), vec![5]);
        // Performer matches to different titles are ambiguous.
        index.insert(
            6,
            &key("Yoko Ono"),
            &credits("Yoko Ono"),
            "woman",
            &["woman live".into()],
            None,
        );
        index.insert(
            7,
            &key("Yoko Ono & Friends"),
            &credits("Yoko Ono & Friends"),
            "woman live",
            &[],
            None,
        );
        assert!(resolve(&index, "Yoko Ono with Band", "woman live").is_empty());
    }
}
