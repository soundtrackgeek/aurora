//! `sort:` and `group:` in the catalog search box.
//!
//! They arrange Songs and Albums and filter nothing, so they are taken out of
//! the query before the catalog search parser sees it. Aurora NG has the same
//! clauses for its cover wall (`src/search.rs` there). `docs/search.md`
//! describes them.

use crate::explorer::{AlbumSort, ArtistSort, TrackSort};

/// How `sort:` orders the results. Each view maps it to one of its own sorts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ViewSort {
    Random,
    Year {
        descending: bool,
    },
    ReleaseYear {
        descending: bool,
    },
    Added {
        descending: bool,
    },
    Rating {
        descending: bool,
    },
    Minutes {
        descending: bool,
    },
    Plays {
        descending: bool,
    },
    Love {
        descending: bool,
    },
    /// The item's own title: the song title in Songs, the album title in Albums.
    Title {
        descending: bool,
    },
    Album {
        descending: bool,
    },
    AlbumArtist {
        descending: bool,
    },
    Genre {
        descending: bool,
    },
    Publisher {
        descending: bool,
    },
}

/// What `group:` puts results together by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GroupField {
    Genre,
    AlbumArtist,
    /// The track's display artist in Songs; the album artist in Albums.
    Artist,
    /// The album title; in Albums each album is its own group.
    Album,
    Publisher,
    Country,
    Year,
    ReleaseYear,
    Decade,
    Love,
    AlbumRating,
}

impl GroupField {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "genre" => Self::Genre,
            "aartist" => Self::AlbumArtist,
            "artist" => Self::Artist,
            "album" => Self::Album,
            "publisher" => Self::Publisher,
            "country" => Self::Country,
            "year" => Self::Year,
            "ryear" => Self::ReleaseYear,
            "decade" => Self::Decade,
            "love" => Self::Love,
            "ar" => Self::AlbumRating,
            _ => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            Self::Genre => "genre",
            Self::AlbumArtist => "aartist",
            Self::Artist => "artist",
            Self::Album => "album",
            Self::Publisher => "publisher",
            Self::Country => "country",
            Self::Year => "year",
            Self::ReleaseYear => "ryear",
            Self::Decade => "decade",
            Self::Love => "love",
            Self::AlbumRating => "ar",
        }
    }

    fn is_text(self) -> bool {
        matches!(
            self,
            Self::Genre
                | Self::AlbumArtist
                | Self::Artist
                | Self::Album
                | Self::Publisher
                | Self::Country
        )
    }

    /// Text A–Z and years oldest first; Love and album rating highest first.
    fn descending_by_default(self) -> bool {
        matches!(self, Self::Love | Self::AlbumRating)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CatalogGroup {
    pub(crate) field: GroupField,
    pub(crate) descending: bool,
}

/// The `sort:` and `group:` of a query.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CatalogView {
    pub(crate) sort: Option<ViewSort>,
    pub(crate) group: Option<CatalogGroup>,
}

const SORT_HELP: &str = "Use sort:random, sort:year.., sort:..year, sort:ryear.., sort:added.., sort:rating.., sort:az, sort:za, sort:title az, sort:album az or sort:aartist za.";
const GROUP_HELP: &str = "Use group:genre, group:aartist, group:artist, group:album, group:publisher, group:country, group:year, group:ryear, group:decade, group:love or group:ar.";

/// Fields that order by a number and take `name..` / `..name`.
fn numeric_sort(name: &str, descending: bool) -> Option<ViewSort> {
    Some(match name {
        "year" => ViewSort::Year { descending },
        "ryear" => ViewSort::ReleaseYear { descending },
        "added" => ViewSort::Added { descending },
        "rating" => ViewSort::Rating { descending },
        "minutes" => ViewSort::Minutes { descending },
        "plays" => ViewSort::Plays { descending },
        "love" => ViewSort::Love { descending },
        _ => return None,
    })
}

/// Fields that order by text and take `az` / `za` after a space.
fn text_sort(name: &str, descending: bool) -> Option<ViewSort> {
    Some(match name {
        "title" => ViewSort::Title { descending },
        "album" | "albums" => ViewSort::Album { descending },
        "aartist" => ViewSort::AlbumArtist { descending },
        "genre" => ViewSort::Genre { descending },
        "publisher" => ViewSort::Publisher { descending },
        _ => return None,
    })
}

fn parse_sort(value: &str, direction: Option<bool>) -> Result<ViewSort, String> {
    let unknown = || format!("Unknown sort \"{value}\". {SORT_HELP}");
    match value {
        "" => return Err(format!("sort: needs a value. {SORT_HELP}")),
        "random" => return Ok(ViewSort::Random),
        "az" => return Ok(ViewSort::Title { descending: false }),
        "za" => return Ok(ViewSort::Title { descending: true }),
        _ => {}
    }
    if let Some(name) = value.strip_suffix("..") {
        return numeric_sort(name, false).ok_or_else(unknown);
    }
    if let Some(name) = value.strip_prefix("..") {
        return numeric_sort(name, true).ok_or_else(unknown);
    }
    text_sort(value, direction.unwrap_or(false)).ok_or_else(unknown)
}

fn parse_group(value: &str, direction: Option<bool>) -> Result<CatalogGroup, String> {
    if value.is_empty() {
        return Err(format!("group: needs a value. {GROUP_HELP}"));
    }
    let unknown = || format!("Unknown group \"{value}\". {GROUP_HELP}");
    let (name, descending) = if let Some(name) = value.strip_suffix("..") {
        (name, Some(false))
    } else if let Some(name) = value.strip_prefix("..") {
        (name, Some(true))
    } else {
        (value, direction)
    };
    let field = GroupField::parse(name).ok_or_else(unknown)?;
    // `..` orders numbers; `az`/`za` order text.
    if value.contains("..") && field.is_text() {
        return Err(unknown());
    }
    Ok(CatalogGroup {
        field,
        descending: descending.unwrap_or(field.descending_by_default()),
    })
}

/// Takes the `sort:` and `group:` clauses out of a query, wherever they are,
/// and returns the rest of the query with them. Text fields may be followed
/// by `az` or `za` after a space; `=` works in place of `:`.
pub(crate) fn split_catalog_view(input: &str) -> Result<(String, CatalogView), String> {
    let is_break = |ch: char| ch.is_whitespace() || ch == ',';
    let mut rest = String::new();
    let mut view = CatalogView::default();
    let mut quoted = false;
    let mut ix = 0;
    while let Some(ch) = input[ix..].chars().next() {
        let tail = &input[ix..];
        let at_word_start = input[..ix].chars().next_back().is_none_or(is_break);
        let keyword = ["sort", "group"].into_iter().find(|keyword| {
            tail.get(..keyword.len())
                .is_some_and(|word| word.eq_ignore_ascii_case(keyword))
                && matches!(tail[keyword.len()..].chars().next(), Some(':' | '='))
        });
        let Some(keyword) = keyword.filter(|_| !quoted && at_word_start) else {
            if ch == '"' {
                quoted = !quoted;
            }
            rest.push(ch);
            ix += ch.len_utf8();
            continue;
        };

        let after = &tail[keyword.len() + 1..];
        let end = after.find(is_break).unwrap_or(after.len());
        let value = after[..end].to_ascii_lowercase();
        let mut consumed = keyword.len() + 1 + end;
        let takes_direction = if keyword == "sort" {
            text_sort(&value, false).is_some()
        } else {
            GroupField::parse(&value).is_some_and(GroupField::is_text)
        };
        let mut direction = None;
        if takes_direction {
            let next = &after[end..];
            let spaces = next.len() - next.trim_start_matches(char::is_whitespace).len();
            let word_end = next[spaces..]
                .find(is_break)
                .map_or(next.len(), |end| spaces + end);
            direction = match next[spaces..word_end].to_ascii_lowercase().as_str() {
                "az" => Some(false),
                "za" => Some(true),
                _ => None,
            };
            if direction.is_some() {
                consumed += word_end;
            }
        }
        if keyword == "sort" {
            if view.sort.replace(parse_sort(&value, direction)?).is_some() {
                return Err("Use one sort: per search.".to_owned());
            }
        } else if view
            .group
            .replace(parse_group(&value, direction)?)
            .is_some()
        {
            return Err("Use one group: per search.".to_owned());
        }
        ix += consumed;
    }
    if view == CatalogView::default() {
        // Nothing was taken out; the query stays exactly as typed.
        return Ok((rest, view));
    }
    Ok((strip_dangling_operators(&rest), view))
}

/// `genre:rock AND ` once `sort:` is gone: drops the operators and commas
/// left at either end or doubled where a clause was, so the rest still parses.
fn strip_dangling_operators(rest: &str) -> String {
    // `NOT` belongs to the clause after it, so it is never dropped in front.
    let joiner = |word: &str| matches!(word, "AND" | "OR" | ",");
    let mut words: Vec<&str> = Vec::new();
    for word in rest.split_whitespace() {
        if joiner(word)
            && words
                .last()
                .is_none_or(|last| joiner(last) || *last == "NOT")
        {
            continue;
        }
        words.push(word);
    }
    while words
        .last()
        .is_some_and(|word| joiner(word) || *word == "NOT")
    {
        words.pop();
    }
    words
        .join(" ")
        .trim_matches(|ch: char| ch == ',' || ch.is_whitespace())
        .to_owned()
}

fn unavailable(view: &str, sort: ViewSort) -> String {
    format!(
        "sort:{} is not available in {view}. {SORT_HELP}",
        sort_name(sort)
    )
}

fn sort_name(sort: ViewSort) -> &'static str {
    match sort {
        ViewSort::Random => "random",
        ViewSort::Year { .. } => "year",
        ViewSort::ReleaseYear { .. } => "ryear",
        ViewSort::Added { .. } => "added",
        ViewSort::Rating { .. } => "rating",
        ViewSort::Minutes { .. } => "minutes",
        ViewSort::Plays { .. } => "plays",
        ViewSort::Love { .. } => "love",
        ViewSort::Title { .. } => "title",
        ViewSort::Album { .. } => "album",
        ViewSort::AlbumArtist { .. } => "aartist",
        ViewSort::Genre { .. } => "genre",
        ViewSort::Publisher { .. } => "publisher",
    }
}

pub(crate) fn track_sort(sort: ViewSort) -> TrackSort {
    let pick = |descending: bool, ascending_sort, descending_sort| {
        if descending {
            descending_sort
        } else {
            ascending_sort
        }
    };
    match sort {
        ViewSort::Random => TrackSort::Random,
        ViewSort::Year { descending } => pick(descending, TrackSort::YearAsc, TrackSort::YearDesc),
        ViewSort::ReleaseYear { descending } => pick(
            descending,
            TrackSort::ReleaseYearAsc,
            TrackSort::ReleaseYearDesc,
        ),
        ViewSort::Added { descending } => pick(descending, TrackSort::Oldest, TrackSort::Newest),
        ViewSort::Rating { descending } => {
            pick(descending, TrackSort::RatingAsc, TrackSort::RatingDesc)
        }
        ViewSort::Minutes { descending } => {
            pick(descending, TrackSort::DurationAsc, TrackSort::DurationDesc)
        }
        ViewSort::Plays { descending } => {
            pick(descending, TrackSort::PlaysAsc, TrackSort::PlaysDesc)
        }
        ViewSort::Love { descending } => pick(descending, TrackSort::LoveAsc, TrackSort::LoveDesc),
        ViewSort::Title { descending } => {
            pick(descending, TrackSort::TitleAsc, TrackSort::TitleDesc)
        }
        ViewSort::Album { descending } => {
            pick(descending, TrackSort::AlbumAsc, TrackSort::AlbumDesc)
        }
        ViewSort::AlbumArtist { descending } => {
            pick(descending, TrackSort::ArtistAsc, TrackSort::ArtistDesc)
        }
        ViewSort::Genre { descending } => {
            pick(descending, TrackSort::GenreAsc, TrackSort::GenreDesc)
        }
        ViewSort::Publisher { descending } => pick(
            descending,
            TrackSort::PublisherAsc,
            TrackSort::PublisherDesc,
        ),
    }
}

pub(crate) fn album_sort(sort: ViewSort) -> Result<AlbumSort, String> {
    let pick = |descending: bool, ascending_sort, descending_sort| {
        if descending {
            descending_sort
        } else {
            ascending_sort
        }
    };
    Ok(match sort {
        ViewSort::Random => AlbumSort::Random,
        ViewSort::Year { descending } => pick(descending, AlbumSort::YearAsc, AlbumSort::YearDesc),
        ViewSort::ReleaseYear { descending } => pick(
            descending,
            AlbumSort::ReleaseYearAsc,
            AlbumSort::ReleaseYearDesc,
        ),
        ViewSort::Added { descending } => pick(descending, AlbumSort::Oldest, AlbumSort::Newest),
        ViewSort::Rating { descending } => {
            pick(descending, AlbumSort::RatingAsc, AlbumSort::RatingDesc)
        }
        ViewSort::Title { descending } | ViewSort::Album { descending } => {
            pick(descending, AlbumSort::TitleAsc, AlbumSort::TitleDesc)
        }
        ViewSort::AlbumArtist { descending } => {
            pick(descending, AlbumSort::ArtistAsc, AlbumSort::ArtistDesc)
        }
        other => return Err(unavailable("Albums", other)),
    })
}

pub(crate) fn artist_sort(sort: ViewSort) -> Result<ArtistSort, String> {
    Ok(match sort {
        ViewSort::Title { descending: false } | ViewSort::AlbumArtist { descending: false } => {
            ArtistSort::NameAsc
        }
        ViewSort::Title { descending: true } | ViewSort::AlbumArtist { descending: true } => {
            ArtistSort::NameDesc
        }
        other => return Err(unavailable("Artists", other)),
    })
}

/// Which explorer a group is built for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GroupTarget {
    Tracks,
    Albums,
}

/// The SQL that groups one explorer's rows.
pub(crate) struct GroupSql {
    /// Never NULL; ordered ascending or descending as the group asks, with
    /// rows that have no value last either way.
    pub(crate) key: String,
    /// The heading shown above the group.
    pub(crate) label: String,
    pub(crate) descending: bool,
    /// Tells cursors from different groupings apart.
    pub(crate) cursor_tag: String,
}

fn text_value(expression: &str) -> String {
    format!("NULLIF(trim({expression}), '')")
}

fn valid_year(expression: &str) -> String {
    format!("CASE WHEN {expression} BETWEEN 1000 AND 2999 THEN {expression} END")
}

/// The group's key and label for Songs (`t`) or Albums (`a`). `artist_key`
/// builds Aurora's artist identity key for a column, for country lookups.
pub(crate) fn group_sql(
    group: CatalogGroup,
    target: GroupTarget,
    artist_key: impl Fn(&str) -> String,
) -> GroupSql {
    let albums = target == GroupTarget::Albums;
    let alias = if albums { "a" } else { "t" };
    let album_rating = if albums {
        "a.effective_album_rating".to_owned()
    } else {
        "(SELECT rated_album.effective_album_rating FROM albums AS rated_album WHERE rated_album.id = t.album_id)".to_owned()
    };
    // (value to order by, label, whether the value is a number)
    let (value, label, numeric) = match group.field {
        GroupField::Genre => {
            let genre = if albums {
                crate::live_genres::album_sql("a")
            } else {
                crate::live_genres::track_sql("t")
            };
            (
                format!("lower({})", text_value(&genre)),
                text_value(&genre),
                false,
            )
        }
        GroupField::AlbumArtist => {
            let column = format!("{alias}.album_artist_display");
            (
                format!("lower({})", text_value(&column)),
                text_value(&column),
                false,
            )
        }
        GroupField::Artist if albums => {
            let column = "a.album_artist_display";
            (
                format!("lower({})", text_value(column)),
                text_value(column),
                false,
            )
        }
        GroupField::Artist => {
            let artist = format!(
                "COALESCE({}, {})",
                text_value("t.display_artist"),
                text_value("t.album_artist_display")
            );
            (format!("lower({artist})"), artist, false)
        }
        GroupField::Album => {
            let column = format!("{alias}.album");
            (
                format!("lower({})", text_value(&column)),
                text_value(&column),
                false,
            )
        }
        GroupField::Publisher => {
            let column = format!("{alias}.publisher");
            (
                format!("lower({})", text_value(&column)),
                text_value(&column),
                false,
            )
        }
        GroupField::Country => {
            let country = format!(
                "(SELECT COALESCE(NULLIF(trim(origin_group.country_name), ''), NULLIF(trim(origin_group.country_code), '')) FROM musicbrainz_artist_origin_countries AS origin_group WHERE origin_group.local_artist_key = {})",
                artist_key(&format!("{alias}.album_artist_display"))
            );
            (format!("lower({country})"), country, false)
        }
        GroupField::Year => {
            let year = valid_year(&format!("{alias}.year"));
            (year.clone(), format!("CAST({year} AS TEXT)"), true)
        }
        GroupField::ReleaseYear => {
            let year = valid_year(&format!("{alias}.release_year"));
            (year.clone(), format!("CAST({year} AS TEXT)"), true)
        }
        GroupField::Decade => {
            let decade = format!("({} / 10) * 10", valid_year(&format!("{alias}.year")));
            (
                decade.clone(),
                format!("CAST({decade} AS TEXT) || 's'"),
                true,
            )
        }
        GroupField::Love if albums => {
            let loved = "MAX(0, COALESCE(CASE WHEN live.album_id IS NOT NULL THEN live.loved_tracks ELSE a.loved_tracks END, 0))";
            (
                loved.to_owned(),
                format!(
                    "CASE {loved} WHEN 0 THEN 'No loved tracks' WHEN 1 THEN '1 loved track' ELSE CAST({loved} AS TEXT) || ' loved tracks' END"
                ),
                true,
            )
        }
        GroupField::Love => {
            let loved = "CASE WHEN t.love = 'L' THEN 1 ELSE 0 END";
            (
                loved.to_owned(),
                format!("CASE {loved} WHEN 1 THEN 'Loved' ELSE 'Not loved' END"),
                true,
            )
        }
        GroupField::AlbumRating => {
            // Half stars: ratings run 0–100, 20 to a star.
            let half_stars = format!(
                "CASE WHEN {album_rating} > 0 THEN MAX(1, (MIN({album_rating}, 100) + 5) / 10) END"
            );
            (
                half_stars.clone(),
                format!(
                    "CASE WHEN {half_stars} = 2 THEN '1 star' ELSE CAST({half_stars} / 2 AS TEXT) || CASE WHEN {half_stars} % 2 = 1 THEN '.5' ELSE '' END || ' stars' END"
                ),
                true,
            )
        }
    };
    let sortable = if numeric {
        format!("printf('%08d', {value})")
    } else {
        value
    };
    // Rows without a value come last in either direction.
    let (present, missing) = if group.descending {
        ("'1'", "'0'")
    } else {
        ("'0'", "'1'")
    };
    let key =
        format!("CASE WHEN ({sortable}) IS NULL THEN {missing} ELSE {present} || ({sortable}) END");
    let missing_label = match group.field {
        GroupField::Genre => "No genre",
        GroupField::AlbumArtist | GroupField::Artist => "No artist",
        GroupField::Album => "No album",
        GroupField::Publisher => "No publisher",
        GroupField::Country => "No country",
        GroupField::Year | GroupField::ReleaseYear | GroupField::Decade => "No year",
        GroupField::Love => "No loved tracks",
        GroupField::AlbumRating => "Not rated",
    };
    GroupSql {
        key,
        label: format!("COALESCE({label}, '{missing_label}')"),
        descending: group.descending,
        cursor_tag: format!(
            "group-{}-{}",
            group.field.name(),
            if group.descending { "desc" } else { "asc" }
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(query: &str) -> (String, CatalogView) {
        split_catalog_view(query).unwrap()
    }

    #[test]
    fn takes_sort_and_group_out_of_a_query() {
        assert_eq!(
            split("sort:random"),
            (
                String::new(),
                CatalogView {
                    sort: Some(ViewSort::Random),
                    group: None
                }
            )
        );
        assert_eq!(
            split("genre:rock AND sort=..year"),
            (
                "genre:rock".to_owned(),
                CatalogView {
                    sort: Some(ViewSort::Year { descending: true }),
                    group: None
                }
            )
        );
        assert_eq!(
            split("sort:aartist za, genre:scores group:country"),
            (
                "genre:scores".to_owned(),
                CatalogView {
                    sort: Some(ViewSort::AlbumArtist { descending: true }),
                    group: Some(CatalogGroup {
                        field: GroupField::Country,
                        descending: false
                    })
                }
            )
        );
        assert_eq!(
            split("year:1980..1989 AND group:genre za AND cr:..99").0,
            "year:1980..1989 AND cr:..99"
        );
        let group = |query: &str| split(query).1.group.unwrap();
        assert!(!group("group:year").descending);
        assert!(group("group:..year").descending);
        assert_eq!(group("GROUP=Decade").field, GroupField::Decade);
        assert!(group("group:love").descending);
        assert!(!group("group:love..").descending);
        assert_eq!(group("group:ar").field, GroupField::AlbumRating);
        assert_eq!(
            split("sort:az").1.sort,
            Some(ViewSort::Title { descending: false })
        );
        assert_eq!(
            split("sort:title za").1.sort,
            Some(ViewSort::Title { descending: true })
        );
        assert_eq!(
            split("sort:..rating").1.sort,
            Some(ViewSort::Rating { descending: true })
        );
        // A NOT keeps its clause; a query without sort: or group: is untouched.
        assert_eq!(
            split("genre:rock AND sort:az AND NOT year:1997").0,
            "genre:rock AND NOT year:1997"
        );
        assert_eq!(split("genre:rock OR").0, "genre:rock OR");
        // Not a clause: inside quotes, or in the middle of a word.
        assert_eq!(split("title:\"sort:az\"").1, CatalogView::default());
        assert_eq!(split("aartist:resort:az").1, CatalogView::default());

        for bad in [
            "sort:",
            "sort:sideways",
            "sort:az sort:za",
            "group:",
            "group:mood",
            "group:genre..",
            "group:genre group:year",
            "sort:title..",
        ] {
            assert!(split_catalog_view(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn maps_sorts_to_each_view() {
        assert_eq!(
            album_sort(ViewSort::Year { descending: true }).unwrap(),
            AlbumSort::YearDesc
        );
        assert_eq!(
            album_sort(ViewSort::Title { descending: false }).unwrap(),
            AlbumSort::TitleAsc
        );
        assert!(album_sort(ViewSort::Genre { descending: false }).is_err());
        assert_eq!(
            track_sort(ViewSort::Added { descending: true }),
            TrackSort::Newest
        );
        assert_eq!(
            artist_sort(ViewSort::Title { descending: true }).unwrap(),
            ArtistSort::NameDesc
        );
        assert!(artist_sort(ViewSort::Random).is_err());
    }
}
