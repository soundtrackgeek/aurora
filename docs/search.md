# Search in Aurora

Aurora has one persistent search box at the top of the window, but the search behavior follows the destination you are viewing. Songs, Albums, and Artists use Aurora's full catalog query language. Genres, Publishers, History, and Observatory use focused text search for that destination.

## Deep Catalog performance

Deep Catalog reads the local catalog plus saved Aurora corrections before checking pending music files in the background. The rating and genre projections use the small set of changed albums/tracks to drive indexed lookups. This is especially important for temporary correction tables without catalog statistics; an unrestricted join could otherwise scan every track even for an empty correction set. SQLite documents this control of join order in its [query optimizer overview](https://www.sqlite.org/optoverview.html#manual_control_of_query_plans_using_cross_join).

Album searches build one temporary matching-ID set for the count and result page. Display metadata joins and keyset sorting then read that set, and `scores` checks use the candidate album's tracks. Restoring a saved list still loads its complete pagination and keeps its selection and scroll position.

The opt-in native benchmark uses the catalog read-only and consistent disposable copies of Aurora state, observations, and popularity cache:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml benchmark_deep_catalog_startup -- --ignored --nocapture --test-threads=1
```

It measures library initialization, correction preparation, the first page for `year:1980..1989 AND CR:1..99 NOT genre:soundtrack OR scores`, complete result restoration, and local album detail including cached popularity. It compares all matching IDs and their order with the previous score predicate. Timings depend on catalog size, disk cache, and other activity; these are native data-path measurements, not installed-app startup or WebView rendering measurements. Run `cargo clean` from `src-tauri` afterward.

On the local 1.1-million-track / 72,600-album catalog, the October 6 benchmark found the same 534 matches in the same order. Warm first-page preparation and retrieval fell from roughly 3.4–4.1 seconds to 0.28–0.29 seconds. Sampled local album details fell from 0.47–0.62 seconds to 0.011–0.022 seconds, including cached popularity in the final run. Restoring every matching album across 11 pages took 3.4–3.6 seconds. The first search after compilation still took about 8 seconds, and library initialization took 2.7 seconds; warm measurements do not imply immediate results after uncached disk reads.

## Quick start

- Press `Ctrl+K` to focus the top search box.
- Start typing. In Songs, Albums, and Artists, Aurora waits for a two-second pause before running the query so a complete expression runs once. Clearing the box is immediate.
- The number beside the box is the exact total for the filtered Songs, Albums, or Artists result set, even though Aurora loads only one bounded page at a time.
- Search is case-insensitive.
- Initial catalog results use local SQLite metadata and saved corrections without waiting for MP3 or folder inspection. Pending file changes reconcile in the background. Album and artist track filters collect matching album IDs once, including combined queries such as `genre:soundtrack AND year:1997`.
- Use **Reset** in Songs, Albums, or Artists to clear both the query and filters inherited from another Aurora view.
- Aurora restores the last catalog query and filters after a restart.
- Add `sort:` to order results, or `group:` to show them under headings by genre, artist, country, year, decade, Love, album rating and more. See [Sort and group in the search](#sort-and-group-in-the-search-sort-and-group).

## Catalog search: Songs, Albums, and Artists

An ordinary query searches across title, Display Artist, Album Artist, album, genre, and publisher metadata. Each unquoted word is a word prefix, not an arbitrary substring. Origin country is available through `country:`. Album rating completeness and album Love status are available through `cr:` and `love:`, official singles through `single:`, and the number of genres in the genre tags through `genres:`.

Every named catalog field accepts either `:` or `=` as its separator. This guide uses `:` as Aurora's conventional form, but `artist:cher` and `artist=cher`, or `cr:80` and `cr=80`, are equivalent.

| Query | What it finds |
| --- | --- |
| `dolly parton` | Items containing words that begin with `dolly` and `parton` in the indexed catalog metadata. |
| `electric` | `Electric`, `Electricity`, and other words beginning with `electric`. |
| `title:hallelujah` | Titles containing a word beginning with `hallelujah`. |
| `album:"Purple Rain"` | Albums whose complete album value is exactly `Purple Rain`. |

Punctuation in an unquoted value separates words. Use quotes when the entire stored value must match exactly.

In Charts, an album's **Open in Library** action opens a fresh exact album search, such as `album:"Sports"`. Aurora uses the stored title of the already-matched library album, so a chart title of `Sports` can open `album:"Sports (Expanded Edition)"` when that is the matched release's library title. If the catalog title is unavailable, the chart title is used. Results include every album with that exact title and do not match artists or longer album names. This reuses existing chart-to-library matches; it does not guess new fuzzy matches. Embedded quotation marks are escaped by doubling them.

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
| `country:` | Imported Album Artist origin-country name or two-letter code | `country:norway OR sweden` |
| `title:` | Track title | `title:"Running Up That Hill"` |
| `cr:` | Album rating-completeness percentage or range | `cr:50..80` |
| `love:` | Loved-track count or range for the album | `love:1..3` |
| `single:` | Official-single count or range: per album in Albums and Artists, per track in Songs (`singles:` is an alias) | `single:2..5` |
| `genres:` | Number of genres in the genre tags as written: per album in Albums and Artists, per track in Songs. A single number is exact | `genres:2..` |
| `sonic:` | Saved compatible audio analysis on a track | `sonic:yes`, `sonic:no` |
| `sound:` | A sound descriptor heard in the audio: per track in Songs; in Albums and Artists, on at least a third of an album's tracks | `sound:choir`, `sound:synth OR chiptune` |
| `vibe:` | A description of the sound in your own words: the best-fitting 2% of tracks in Songs, of albums in Albums and Artists | `vibe:"dark ambient drone"`, `vibe:"heroic brass fanfare" sort:vibe` |

`artist:` and `aartist:` are intentionally different. Use `artist:` for a track's credited performer and `aartist:` for the artist used to group an album.

Artist links open a fresh Albums search such as `aartist:"Bunny X"`, replacing the previous filters and results. Pagination waits for that search to load; background refreshes preserve the loaded list only for the same view and filters.

`country:` uses Music Library's imported artist-origin record for the Album Artist. It accepts country names or two-letter codes, so `country:norway`, `country:NO`, and exact `country:"Norway"` all target the same origin data. Artists without an imported origin do not match.

`cr:80` includes albums from 0% through 80% rating completeness, preserving the original maximum shorthand. `cr:50..80` includes albums from 50% through 80%, inclusive. Aurora calculates rated tracks divided by total tracks from the Music Library catalog plus Aurora's verified pending rating changes and queued missing-track deletions, so search results do not wait for Music Library synchronization. Bounds must be whole percentages from 0 through 100.

`love:1` includes an album when at least one of its tracks is marked Love, preserving the original shorthand. `love:0` includes albums with no loved tracks. Use a range to filter the album's loved-track count: `love:1..3` means one through three loved tracks, inclusive. These are album-wide filters even though they use the same conventional separator as text fields.

`single:` filters by official singles, using the singles Music Library finds from MusicBrainz (**Settings → Providers → Album singles** in Music Library 0.210.0 or newer). In **Albums** and **Artists** it counts the singles on each album: `single:1` means at least one single, `single:0` means none, `single:3..` means three or more, `single:2..5` means two through five, and `single:..3` means at most three. In **Songs** it checks each track, so `single:1` lists the singles themselves and `single:0` every other song; combine it with other fields, for example `aartist:"Bob Seger" AND single:1`. `singles:` is accepted as an alias and `=` may replace `:`. Only singles MusicBrainz confirms are counted, and MusicBrainz coverage is incomplete, so a missing single is not proof that a song was never released as one. A catalog without Music Library's singles data matches nothing for `single:1` and everything for `single:0`.

There are no `rating:` or `unrated:` search fields. Those remain Aurora collection filters and handoffs rather than query-language keywords.

`sonic:yes` selects current catalog paths with saved compatible audio analysis;
`sonic:no` selects paths without it. These work during a partial Music Library
analysis and combine with other fields/boolean operators. Albums and Artists
qualify through matching tracks, so partly analyzed albums can match either
value. `sonic=` is accepted too. Searches report saved coverage; similarity
validates shortlisted files before using them. See [sonic analysis](sonic-analysis.md).

`sound:` finds music by what it sounds like rather than by its tags, which helps
most with film, TV and game scores, whose genre is only "Sci-Fi" or "Video Game".
The descriptors are `orchestral`, `choir`, `strings`, `piano`, `guitar`, `synth`,
`hybrid` (orchestra with electronics), `ambient`, `percussion`, `chiptune`, `jazz`
and `vocals`; `orchestra`, `choral`, `string`, `synthesizer`, `synths`, `drums`,
`chip`, `vocal` and `voice` also work. A track can carry several. In **Songs**,
`sound:choir` lists the tracks that carry it. In **Albums** and **Artists**, an
album qualifies when at least a third of its described tracks do, so one choral
cue does not make a 40-track score a choir album. Combine it like any field:
`genre:scores AND sound:synth AND year:1980..1989`, `sound:choir OR orchestral`,
`genre:"video game" NOT sound:chiptune`.

Descriptors are estimated from Music Library's sound search results (its
**Describe a sound** step, Music Library 0.216.0 or newer), so only tracks with
those results have them, and they are estimates, not tags: expect some mistakes,
especially between neighbors such as orchestral, strings and choir. Aurora
works them out in the background; the first pass over a million tracks takes a
few minutes, and later passes only read new results. See
[sound descriptors](sound-descriptors.md).

`vibe:` goes past the twelve descriptors: describe the sound in your own words
and Aurora finds what sounds most like it. Quote descriptions of more than one
word, as in `vibe:"dark ambient drone"`. It keeps the best-fitting **2%**: 2% of
the analyzed tracks in **Songs**, and 2% of the albums in **Albums** and
**Artists**, where an album is compared as the average of its tracks. Combine it
with any other field to narrow those down, and add `sort:vibe` to put the best
fit first:

```text
vibe:"dark ambient drone" AND year:1990..1999 AND cr:..50
vibe:"heroic orchestral fanfare" AND genre:scores sort:vibe
vibe:"lonely solo piano" NOT sound:strings
```

Descriptions are case-insensitive and at most 200 characters; spacing does not
matter. The first search with a new description takes a few seconds while Music
Library's CLAP model reads it (Aurora runs `music-clap-worker.exe`, installed
with Music Library 0.216.0 or newer); Aurora saves the result, so the same words
are quick afterwards. On the reference library (1.1 million tracks, 73,000
albums), an Albums search took about 0.2 s and a Songs search about 0.7 s, plus
1.6 s the first time to score every track; scores are kept for half an hour. `vibe:` needs the same sound search results as `sound:`, and in
Albums it waits for Aurora's background pass to average each album, a few minutes
after a new version starts. Like `sound:`, it is an estimate: CLAP matches broad
character such as instruments, mood and texture well, and exact facts such as a
composer or a year not at all, so leave those to the other fields. See
[vibe search](vibe.md) for example descriptions and how it works.

### Sort and group in the search: `sort:` and `group:`

`sort:` orders results and `group:` puts them together under headings. Neither filters anything. Each can appear once, anywhere in a Songs, Albums, or Artists query, either alone or next to a search. Separate it with a space, a comma, or `AND`, and use `=` in place of `:` if you prefer:

```text
sort:random
genre:scores sort:..year
year:1980..1989, group:country
aartist:"Kate Bush" AND group:year sort:..rating
group:genre za sort:az
```

A `sort:` in the search replaces the **Sort** menu's choice while it is in the query. Remove it to go back to the menu's sort. Aurora NG has the same clauses for its cover wall.

#### `sort:`

| Sort | Order | Songs | Albums | Artists |
| --- | --- | --- | --- | --- |
| `sort:random` | Shuffled; pick **Random** in the Sort menu again for a new draw | ✓ | ✓ | — |
| `sort:year..` / `sort:..year` | Original Year, oldest / newest first | ✓ | ✓ | — |
| `sort:ryear..` / `sort:..ryear` | Release Year, oldest / newest first | ✓ | ✓ | — |
| `sort:added..` / `sort:..added` | Order added to the library, oldest / newest first | ✓ | ✓ | — |
| `sort:rating..` / `sort:..rating` | Rating, lowest / highest first (track rating in Songs, album rating in Albums) | ✓ | ✓ | — |
| `sort:minutes..` / `sort:..minutes` | Track duration, shortest / longest first | ✓ | — | — |
| `sort:plays..` / `sort:..plays` | Plays, fewest / most first | ✓ | — | — |
| `sort:love..` / `sort:..love` | Love state | ✓ | — | — |
| `sort:az` / `sort:za` | Title A–Z / Z–A: the song title in Songs, the album title in Albums, the artist name in Artists | ✓ | ✓ | ✓ |
| `sort:title az` / `sort:title za` | Same as `sort:az` / `sort:za` | ✓ | ✓ | ✓ |
| `sort:album az` / `sort:album za` | Album title (`sort:albums` works too) | ✓ | ✓ | — |
| `sort:aartist az` / `sort:aartist za` | Album artist, then title | ✓ | ✓ | ✓ |
| `sort:genre az` / `sort:genre za` | Genre | ✓ | — | — |
| `sort:publisher az` / `sort:publisher za` | Publisher | ✓ | — | — |
| `sort:vibe` | Best fit to the query's first `vibe:` description first; needs a `vibe:` in the query | ✓ | ✓ | — |

Numeric sorts use the same `..` notation as ranges. `year..` reads as "from the earliest year onward", so oldest first, and `..year` is newest first. Text sorts take `az` or `za` after a space, and without either they sort A–Z. Items without a year or rating come last. A sort that a view does not offer, such as `sort:genre` in Albums, is reported as an error instead of being ignored.

#### `group:`

With `group:`, results are ordered by the group first. Each group starts under a heading, and inside a group the `sort:` order applies, or the Sort menu's when the query has no `sort:`. In Albums, each group starts on a new row of covers. In Songs, a heading row appears where the group changes. Headings stay correct across pages: if a group continues on the next page of 50, it does not get a second heading.

| Group | Groups by | Default order | Songs | Albums |
| --- | --- | --- | --- | --- |
| `group:genre` | Genre, including pending genre edits | A–Z | ✓ | ✓ |
| `group:aartist` | Album Artist | A–Z | ✓ | ✓ |
| `group:artist` | Display Artist in Songs; Album Artist in Albums | A–Z | ✓ | ✓ |
| `group:album` | Album title | A–Z | ✓ | ✓ |
| `group:publisher` | Publisher or label | A–Z | ✓ | ✓ |
| `group:country` | Album Artist origin country, as imported from MusicBrainz (the same data as `country:` and the flags) | A–Z | ✓ | ✓ |
| `group:year` | Original Year | Oldest first | ✓ | ✓ |
| `group:ryear` | Release Year | Oldest first | ✓ | ✓ |
| `group:decade` | Decade of the Original Year, such as "1980s" | Oldest first | ✓ | ✓ |
| `group:love` | Albums: loved-track count, such as "3 loved tracks". Songs: "Loved" or "Not loved" | Most first | ✓ | ✓ |
| `group:ar` | Album rating in half stars, such as "4.5 stars"; in Songs the containing album's rating | Highest first | ✓ | ✓ |
| `group:sound` | The track's most likely sound descriptor, such as "Choir"; in Albums the descriptor the most tracks carry | A–Z | ✓ | ✓ |

Text groups take `za` (or `az`) after a space to reverse them: `group:genre za`, `group:country za`. Numeric groups use `..`: `group:year..` / `group:..year` for oldest / newest first, and `group:love..` or `group:ar..` for lowest first. Items without a value come last in either direction, under headings such as "No genre", "No country", "No year", "Not rated" and "No sound descriptor". Grouping is not available in Artists. Group names are compared without regard to case, so "Synthpop" and "synthpop" share a heading.

Album ratings are grouped to the nearest half star of the catalog's album rating (effective, then calculated, then stored). An album rated 4.34 stars is grouped under "4.5 stars"; use `ar:` ranges for exact bounds.

Grouping by country or rating in Songs looks up the album artist or album for every matching song. On a very large unfiltered library, the first page can take longer to load than an ungrouped one. Adding a filter keeps it fast.

### Artist lifespans

These fields use the imported MusicBrainz record for the **Album Artist**, on both Songs and Albums (and matching catalog results in Artists). `born:` and `dead:` require a Person; `founded:` and `dissolved:` require a Group. Missing artist records never match, including `dead:no` and `dissolved:no`.

| Field | Closed range | Upper bound | Lower bound | Status |
| --- | --- | --- | --- | --- |
| Band dissolved | `dissolved:1999..2004` | `dissolved:..2004` | `dissolved:2004..` | `dissolved:yes`, `dissolved:no` |
| Band founded | `founded:1999..2004` | `founded:..2004` | `founded:2004..` | — |
| Person died | `dead:1999..2004` | `dead:..2004` | `dead:2004..` | `dead:yes`, `dead:no` |
| Person born | `born:1966..1968` | `born:..2004` | `born:2004..` | — |

All range boundaries are **inclusive**, like `year:`: `..2004` means through 2004 and `2004..` means from 2004 onward. Use `..2003` for strictly before 2004, or `2005..` for strictly after. A single year also works, e.g. `born:1966`. Partial dates with a known year work; absent or malformed years do not match year ranges. `yes` means an ended flag or a recorded end date; `no` requires an explicit non-ended flag and no end date. An ended artist without a known end year can match `yes` but cannot match a year range. The negative status spelling is `dead:no`.

### Duration and album rating

| Field | Closed range | Upper bound | Lower bound |
| --- | --- | --- | --- |
| Minutes | `minutes:33..44` | `minutes:..44` | `minutes:33..` |
| Album rating (0–5 stars) | `ar:3..4` | `ar:..4.34` | `ar:4.34..` |

`minutes:` uses total album duration on Albums and individual track duration on Songs. Stored seconds are compared without rounding to whole minutes; decimal bounds are supported. `ar:` always uses the containing album's catalog rating, including on Songs, using effective, calculated, then stored album rating in that order. It does not round ratings to whole or half stars. Pending edits may need Music Library synchronization before this catalog rating changes. Missing durations or ratings do not match ranges. Bounds are inclusive: `ar:..4.34` includes 4.34 and `minutes:33..` includes exactly 33 minutes. Single values such as `ar:4.34` are also accepted.

### Chart status and ranks

Chart searches use the imported catalog rank shown on the album or track, not a selected Charts period. Positive ranks indicate chart presence; missing, zero, or negative ranks match `no`.

| Chart | Presence | Absence | Closed range | Lower bound | Upper bound |
| --- | --- | --- | --- | --- | --- |
| Billboard | `bb:yes` | `bb:no` | `bb:1..5` | `bb:5..` | `bb:..5` |
| Official UK | `uk:yes` | `uk:no` | `uk:1..5` | `uk:5..` | `uk:..5` |
| VG-lista | `vg:yes` | `vg:no` | `vg:1..5` | `vg:5..` | `vg:..5` |
| Ti i skuddet (Songs only) | `ti:yes` | `ti:no` | `ti:1..5` | `ti:5..` | `ti:..5` |
| Norsktoppen (Songs only) | `nt:yes` | `nt:no` | `nt:1..5` | `nt:5..` | `nt:..5` |

On Albums, `bb:`, `uk:`, and `vg:` use album ranks; on Songs they use singles ranks. `ti:` and `nt:` have no album equivalent and return no Albums matches. Rank bounds must be positive whole numbers. `bb:5..` means rank 5 or numerically higher (a worse chart position); `bb:..5` means the top five. All boundaries are inclusive. `bb:1` finds rank 1 exactly. Unknown ranks never match numeric ranges.

Combine new fields using the usual `AND`, comma, `OR`, and `NOT` operators; both `:` and `=` work:

```text
founded:1999..2004 AND dissolved:no
born:1966..1968 AND dead:no
minutes:33..44 AND ar:4.34..
bb:..5 AND uk:yes AND vg:no
ti:yes OR nt:yes
year:1990 AND sti:5..10
dead:1999..2004 OR 2010..
ar=3..4 AND NOT bb:yes
```

Reversed ranges, empty ranges (`..`), non-finite numbers, out-of-range ratings, and fractional years or ranks are rejected.

### Singles charts on Albums

`sbb:`, `suk:`, `svg:`, `sti:`, and `snt:` search the singles charts in Albums too. An album matches when at least one of its tracks charted as a single in the range:

| Singles chart | Presence | Absence | Closed range | Lower bound | Upper bound |
| --- | --- | --- | --- | --- | --- |
| Billboard | `sbb:yes` | `sbb:no` | `sbb:5..10` | `sbb:5..` | `sbb:..10` |
| Official UK | `suk:yes` | `suk:no` | `suk:5..10` | `suk:5..` | `suk:..10` |
| VG-lista | `svg:yes` | `svg:no` | `svg:5..10` | `svg:5..` | `svg:..10` |
| Ti i skuddet | `sti:yes` | `sti:no` | `sti:5..10` | `sti:5..` | `sti:..10` |
| Norsktoppen | `snt:yes` | `snt:no` | `snt:5..10` | `snt:5..` | `snt:..10` |

`year:1990 AND sti:5..10` finds albums from 1990 with a track that peaked between 5th and 10th place on Ti i skuddet. `snt:no` finds albums none of whose tracks charted on Norsktoppen. On Songs, the fields are the same as `bb:`, `uk:`, `vg:`, `ti:`, and `nt:`: the song's own singles rank. On Albums, `vg:` still reads the album's own VG-lista album rank, and `svg:` its tracks' VG-lista singles ranks. The ranks follow the same rules as the other chart fields.

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
country:norway OR sweden
```

Combine an alternative group with another requirement by using a comma or `AND`:

```text
aartist:bon jovi OR def leppard,year:1985..1992
```

This means “Bon Jovi or Def Leppard, and an Original Year from 1985 through 1992.”

Operators are recognized only as uppercase standalone words. Lowercase `and`, `or`, and `not` are treated as ordinary search words.

Pending MP3 genre edits are included in catalog search before Music Library finishes importing them. Genre prefixes, exact genres, and score groups use the same pending values as album details; filtered counts and pagination follow those values. Unreadable files retain catalog metadata until they can be read again.

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

### Completeness, Love and single ranges

### Genre count

`genres:` counts genres in the raw genre tag, which Music Library joins with `;` (`Electro; Dance` holds two). An album's count is the larger of the most genres on one track and the number of different genre tags on its tracks, compared without case, so an album whose tracks say `Rock` and `Pop` also counts two. Tracks without a genre add nothing, and an album with no genre at all counts zero.

Unlike `love:` and `single:`, a single number is exact: `genres:1` finds albums that follow a one-genre-per-album rule, `genres:2..` finds albums that break it, and `genres:0` finds albums without a genre. `genre:` still matches the first (canonical) genre, so `genre:electro AND genres:2..` lists Electro albums that carry a second genre.

`cr:`, `love:`, `single:` and `genres:` use the same inclusive `..` notation as years:

```text
cr:50..80
cr:80..
love:1..3
love:2..
single:3..
single:2..5
single:..3
```

Either range boundary can be omitted. `cr:..80` means at most 80% complete, while `cr:80..` means at least 80%. `love:..3` means at most three loved tracks, while `love:2..` means at least two. A range must contain at least one boundary and its starting value cannot exceed its ending value.

The single-value shorthands keep their established meanings: `cr:80` is equivalent to `cr:0..80`, `love:1` means one or more loved tracks, and `love:0` means no loved tracks. To require an exact positive loved-track count, repeat it as both bounds, such as `love:3..3`. `single:` follows the same rules: `single:1` means one or more, `single:0` means none, and an exact count such as `single:3..3` repeats the bound.

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
| Artists originating in Norway or Sweden | `country:norway OR sweden` |
| Albums no more than 80% rated | `cr:80` |
| Albums from 50% through 80% rated | `cr:50..80` |
| Incomplete albums with one to three loved tracks | `cr:99,love:1..3` |
| Albums with at least one official single | `single:1` |
| Albums with more than one genre | `genres:2..` |
| Electro albums that also carry another genre | `genre:electro AND genres:2..` |
| Albums with three or more singles | `single:3..` |
| Albums with two to five singles | `single:2..5` |
| Albums with at most three singles | `single:..3` |
| Only the singles from one artist, in Songs | `aartist:"Bob Seger" AND single:1` |
| 1980s albums by country, newest first in each | `year:1980..1989 group:country sort:..year` |
| Score albums by decade, best rated first | `genre:scores group:decade sort:..rating` |
| A random draw of loved songs | `love:1 sort:random` |
| One artist's songs by album rating | `aartist:"Kate Bush" group:ar` |
| 1980s synth scores | `genre:scores AND sound:synth AND year:1980..1989` |
| Game music that is not chiptune | `genre:"video game" NOT sound:chiptune` |
| Sci-Fi scores by how they sound | `genre:sci-fi group:sound` |
| Cheaper 1990s drone, best fit first | `vibe:"dark ambient drone" AND year:1990..1999 AND cr:..50 sort:vibe` |
| Songs that sound like a description | `vibe:"warm analog synth arpeggios" sort:vibe` |

### Differences between Songs, Albums, and Artists

- **Songs** returns matching tracks. An album-wide `cr:` clause returns every matching track from qualifying albums; `love:`, `single:`, `sound:` and `vibe:` check each track itself.
- **Albums** returns an album when its indexed album metadata or one of its tracks satisfies the query. `cr:`, `love:`, `single:`, `sound:` and `vibe:` evaluate the album itself.
- **Artists** returns Album Artist groups. A plain artist-name search is a case-insensitive contains match on Album Artist; fielded and boolean queries are evaluated against the artists' albums and tracks, including album-wide `cr:`, `love:` and `single:` clauses.

The same query can therefore produce different result counts in each view.

## Destination-specific search

The following searches do not use the catalog query language. Field separators, quotes, and boolean operators have no special meaning there.

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
- If `cr` or `love` is rejected, use whole, non-negative bounds in ascending order. Completeness bounds are limited to 0–100; a single `love` value remains limited to the `0` and `1` shorthands, while positive exact counts use a repeated range such as `love:3..3`. Either `:` or `=` is accepted.
- If `genres:` is rejected, use a whole, non-negative count or an ascending range, such as `genres:1` or `genres:2..`.
- If `single:` is rejected, use `0`, `1`, or a range with whole, non-negative bounds in ascending order, such as `single:2..5`; an exact count repeats the bound (`single:3..3`). If it finds nothing, update Music Library to 0.210.0 or newer and run **Settings → Providers → Album singles** so the catalog has singles to search.
- If `genre:scores` is too broad, search one exact genre such as `genre:"video game"`.
- If `sort:` or `group:` is rejected, check that the view offers it: `sort:genre`, `sort:publisher`, `sort:minutes`, `sort:plays` and `sort:love` are Songs only, and `group:` is not available in Artists. Use one of each per query, `az`/`za` after text fields, and `..` before or after numeric ones. A `sort:` or `group:` inside quotes, such as `title:"sort:az"`, is ordinary search text.
- Catalog queries are limited to 256 characters, 32 search words, and 32 alternatives. Aurora reports malformed quotes, operators, fields without values, and invalid year ranges instead of silently changing the query.

Album Love filters (`love:1`, `love:0`, and ranges; `=` remains compatible) use verified surviving tracks and pending Love edits while Music Library sync is pending. Deleting an album's only loved track immediately excludes it from `love=1 AND cr=99 NOT genre:scores OR soundtrack`, including filtered counts and pagination. Unavailable folders are not treated as deleted tracks.
