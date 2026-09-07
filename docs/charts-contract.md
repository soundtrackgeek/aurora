# Charts contract

Aurora 0.15.0 exposes the historical chart tables already imported into the read-only Music Library catalog. It does not scrape charts, change chart history, or write calculated rankings back to the catalog.

## Sources

| Mode | Weekly sources | Annual or score sources |
| --- | --- | --- |
| Singles | Official UK, VG Lista, Ti i Skuddet, Norsktoppen | Billboard |
| Albums | Official UK, VG Lista | Billboard, Aurora Album Score |

Billboard is annual-only in the current catalog. Aurora therefore disables exact-week controls for Billboard instead of presenting annual records as weekly results. Aurora Album Score ranks fully rated catalog albums with Music Library's existing numeric formula and never converts that score into stars.

## Time lenses

- Exact week selects one ISO year/week from a weekly source.
- A named preset or custom range includes every source row from its inclusive first ISO week through its inclusive last ISO week.
- Clicking the displayed year builds an inclusive week 1–53 period for that year.
- Aurora Album Score filters the catalog's distinct `year` field by default. Its explicit Release Year option filters `release_year` instead; neither basis falls back to the other when its selected field is missing.
- Requests may span at most 20 years and return at most 100 entries.

## Period ranking

Each distinct normalized artist/title pair receives a histogram of chart finishes. Aurora sorts that histogram lexicographically: most #1 finishes, then most #2 finishes, then most #3 finishes, continuing through position 100. If those are identical, total position points (`101 - position`) and then total appearances break the tie. This makes “most number ones, then number twos” exact rather than approximating it with a single points total.

## Library matching and playback

Chart history remains useful when an item is absent from the library. A matched single exposes one catalog track; a matched album exposes its catalog album. The row, inspector, and chart Play action pass only validated catalog identities to Rust. Playback queues are capped at 100 tracks, and unmatched chart items are skipped without exposing filesystem paths to the WebView.

## State and authority

The chart tables and Album Score are read-only catalog data. Track ratings and Love shown beside matched entries use the existing MP3-authoritative tag workflow and Aurora overlay reconciliation. Changing them never mutates a historical chart row or the shared Music Library database.

## Period controls and artist filters (0.24.40)

The single preset picker offers Spring, Summer, Fall, Winter, Christmas, New Year, and Full year for a chosen year. Winter ends in the following February. The custom row accepts inclusive year/week or year/month ranges; months map to touching ISO weeks within the selected years, with January/December boundaries clamped to that year's week range. Annual sources continue to use years only. Applying a range opens its period chart.

Advanced artist filters combine country (exact name or code, case-insensitive), Person/Group, and current stored lifespan status. Country comes from `musicbrainz_artist_origin_countries`; type and lifespan come from `musicbrainz_artist_infos`. Alive and Active require an explicit `life_ended = 0` and no end date; Dead and Disbanded require an ended flag or end date for the corresponding artist type. Missing metadata is excluded when its filter is active. Joint credits require an exact artist-key record; individual members are not guessed. Missing metadata tables produce an actionable error only when needed.

Filtering happens before ranking limits and counts. Playback uses the same filtered page. The score shelf also applies artist filters before choosing its five albums. Chart history and the catalog remain read-only. Status is current metadata, not reconstructed historical status at the selected chart date.
