# Search in Aurora

Aurora has one persistent search box at the top of the window, but the search behavior follows the destination you are viewing. Songs, Albums, and Artists use Aurora's full catalog query language. Genres, Publishers, History, and Observatory use focused text search for that destination.

## Quick start

- Press `Ctrl+K` to focus the top search box.
- Start typing. In Songs, Albums, and Artists, Aurora waits for a two-second pause before running the query so a complete expression runs once. Clearing the box is immediate.
- The number beside the box is the exact total for the filtered Songs, Albums, or Artists result set, even though Aurora loads only one bounded page at a time.
- Search is case-insensitive.
- Use **Reset** in Songs, Albums, or Artists to clear both the query and filters inherited from another Aurora view.
- Aurora restores the last catalog query and filters after a restart.

## Catalog search: Songs, Albums, and Artists

An ordinary query searches across title, Display Artist, Album Artist, album, genre, and publisher metadata. Each unquoted word is a word prefix, not an arbitrary substring.

| Query | What it finds |
| --- | --- |
| `dolly parton` | Items containing words that begin with `dolly` and `parton` in the indexed catalog metadata. |
| `electric` | `Electric`, `Electricity`, and other words beginning with `electric`. |
| `title:hallelujah` | Titles containing a word beginning with `hallelujah`. |
| `album:"Purple Rain"` | Albums whose complete album value is exactly `Purple Rain`. |

Punctuation in an unquoted value separates words. Use quotes when the entire stored value must match exactly.

### Search fields

| Field | Searches | Example |
| --- | --- | --- |
| `artist:` | Per-track Display Artist | `artist:cher` |
| `aartist:` | Album Artist display value | `aartist:"Dolly Parton"` |
| `album:` | Album title | `album:rumours` |
| `genre:` | Canonical genre | `genre:synthpop` |
| `year:` | Original Year | `year:1985` |
| `ryear:` | Release Year | `ryear:2004` |
| `publisher:` | Publisher or label | `publisher:varèse` |
| `title:` | Track title | `title:"Running Up That Hill"` |

`artist:` and `aartist:` are intentionally different. Use `artist:` for a track's credited performer and `aartist:` for the artist used to group an album.

There are no `rating:`, `love:`, or `unrated:` search fields. Those are Aurora collection filters and handoffs, not query-language keywords.

### Prefix and exact matching

Unquoted text uses word-prefix matching:

```text
artist:mad
```

This can match a word such as `Madonna`. It does not mean “contains these characters anywhere inside a word.”

Quotes make one complete value exact, ignoring case and surrounding whitespace:

```text
artist:"Madonna"
album:"Like a Prayer"
```

Quotes must wrap the complete value after a field. Partial or unclosed quotes are rejected.

### AND: require every group

Separate groups with a comma or uppercase `AND`. Both forms mean the same thing:

```text
aartist:def leppard,genre:hard rock
aartist:def leppard AND genre:hard rock
```

Within one unquoted value, every word is also required. `genre:hard rock` requires word prefixes for both `hard` and `rock` in the genre field.

### OR: accept alternatives

Use uppercase `OR` between alternatives. Alternatives after `OR` inherit the preceding field, so the field does not need to be repeated:

```text
aartist:bon jovi OR def leppard OR kiss
year:1985 OR 1987 OR 1991
genre:synthpop OR new wave
```

Combine an alternative group with another requirement by using a comma or `AND`:

```text
aartist:bon jovi OR def leppard,year:1985..1992
```

This means “Bon Jovi or Def Leppard, and an Original Year from 1985 through 1992.”

Operators are recognized only as uppercase standalone words. Lowercase `and`, `or`, and `not` are treated as ordinary search words.

### NOT and exclusions

Use uppercase `NOT` or a leading hyphen to exclude a group:

```text
genre:rock NOT artist:"Various Artists"
genre:rock,-year:1990..1999
aartist:kate bush AND -title:live
```

A leading `-` starts a new negative clause, so place a comma or `AND` before it when another clause comes first.

### Years and ranges

`year:` searches Original Year. `ryear:` searches Release Year. Both accept a single year, an inclusive closed range, or an inclusive open-ended range:

```text
year:1985
year:1985..1987
year:1985..
ryear:..1987
```

Ranges include their endpoints. Years must be from 1000 through 2999, the start cannot be later than the end, and `..` by itself is invalid.

Year alternatives inherit the field:

```text
year:1985..1987 OR 1990..1992
```

### The Scores umbrella

The unquoted query `genre:scores` (or `genre:score`) expands to Aurora's score-related canonical genres:

```text
action, animation, comedy, documentary, drama, fantasy, horror,
sci-fi, thriller, tv, video game, western, anime
```

It can be combined like any other alternative:

```text
genre:scores OR synthpop
genre:scores NOT year:..1979
```

Quoted `genre:"scores"` is different: it looks only for the exact canonical genre `scores` and does not expand the umbrella.

### Practical recipes

| Goal | Query |
| --- | --- |
| Exact artist, any title | `aartist:"Kate Bush"` |
| Two possible album artists | `aartist:bon jovi OR def leppard` |
| Hard rock by either artist | `aartist:bon jovi OR def leppard,genre:hard rock` |
| Original 1980s releases | `year:1980..1989` |
| Reissues released since 2000 with older originals | `ryear:2000.. AND year:..1999` |
| Score genres excluding games | `genre:scores NOT genre:"video game"` |
| A song title prefix, excluding live albums | `title:heroes NOT album:live` |
| Everything from a publisher except one artist | `publisher:decca,-aartist:"Various Artists"` |

### Differences between Songs, Albums, and Artists

- **Songs** returns matching tracks.
- **Albums** returns an album when its indexed album metadata or one of its tracks satisfies the query.
- **Artists** returns Album Artist groups. A plain artist-name search is a case-insensitive contains match on Album Artist; fielded and boolean queries are evaluated against the artists' albums and tracks.

The same query can therefore produce different result counts in each view.

## Destination-specific search

The following searches do not use the catalog query language. Colons, quotes, and boolean operators have no special meaning there.

| Destination | Search behavior | Useful with |
| --- | --- | --- |
| Genres | Case-insensitive contains match on canonical genre names. | The genre sort menu for size, rating, Love, listening, and exploration. |
| Publishers | Case-insensitive contains match on normalized publisher names. | Publisher activity lenses and the selected publisher detail. |
| History | Case-insensitive contiguous text match across track title, artist, album, and genre. | Outcome, device, and date filters; all active filters combine. |
| Observatory | Case-insensitive contains match on artist display names. | Needs review, Conflicts, Unconfirmed, Aurora decisions, and All candidates filters. |

Examples:

```text
Genres:      ambient
Publishers:  records
History:     kate bush hounds
Observatory: carpenter
```

History searches one continuous string assembled from title, artist, album, and genre. A query such as `kate bush hounds` can cross adjacent fields, but it is still literal contains matching rather than catalog boolean syntax.

## Searches outside the top bar

The Inbox Album Auto-Tagger has its own online release lookup. Open it with `Ctrl+Shift+T`, enter an Album artist and Album, and choose **Find**. Aurora searches MusicBrainz and Discogs for concrete releases and compares candidate track lists. This lookup is separate from local catalog search and requires the relevant service connectivity and, for Discogs, configured credentials.

The Genre Atlas also repeats its genre-name search inside the atlas for convenience. It controls the same genre search value shown in the top bar.

## Where search is not available

- **Inbox:** the top search box is disabled. Use the Album Auto-Tagger's release lookup for a selected staged album.
- **Years:** the top search box is disabled; use the timeline, clock mode, and year selections.
- **Ratings, Tags, and Charts:** use their purpose-built shelves, filters, and controls. They do not add search-only keywords to the catalog query language.

## Troubleshooting

- If a query behaves like plain text, check that operators are uppercase and separated by spaces.
- If an exact query returns nothing, remove the quotes to try word-prefix matching and confirm the stored spelling.
- If an artist result looks too broad, use `aartist:` instead of unscoped text; use `artist:` only when you mean per-track Display Artist.
- If a year looks wrong, confirm whether you need Original Year (`year:`) or Release Year (`ryear:`).
- If `genre:scores` is too broad, search one exact genre such as `genre:"video game"`.
- Catalog queries are limited to 256 characters, 32 search words, and 32 alternatives. Aurora reports malformed quotes, operators, fields without values, and invalid year ranges instead of silently changing the query.
