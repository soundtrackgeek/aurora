# Years explorer contract

Aurora 0.13.0 turns Years into a paired-clock explorer over Music Library's distinct `year` (Original Year) and `release_year` (Release Year) fields.

## Visual target

The release must combine both selected references:

- `docs/design/years-0.13.0-release-lens.png` — Release Year is active and Original Year explains the editions collected in that release year.
- `docs/design/years-0.13.0-original-lens.png` — Original Year is active and Release Year explains original editions, reissues, and remasters.

Both clocks are peers. Selecting a year on either clock makes that field authoritative for the current view; Aurora never substitutes one date for the other. Cyan represents Original Year and violet represents Release Year.

## Authority and identity

- `albums.year` and `tracks.year` are Original Year.
- `albums.release_year` and `tracks.release_year` are Release Year.
- `albums.id` is the stable album identity used for artwork and bounded track detail.
- The imported Music Library database stays read-only and is never indexed or migrated by Aurora.
- Missing Original Year and Missing Release Year remain separate selections.

## Query and scale boundaries

- Opening Years reads two album-level year histograms and summary facts from the existing album aggregates. It does not scan the million-row track table, and the WebView receives roughly one row per year rather than album or track inventories.
- Flow ribbons aggregate by year-to-year counts. Aurora never draws one ribbon per album or track.
- A selected year returns at most 100 representative albums, ranked within counterpart decades so older editions are not crowded out by the largest decade.
- Album inspection reuses the existing bounded album-detail command and returns at most 100 tracks.
- Playing a year requests at most 100 tracks through a Rust-owned selection; the WebView never supplies file paths.
- Exploring a year opens Songs with the chosen Original/Release basis and exact year filter, retaining normal 50-row keyset paging.
- Late year-detail results must not replace a newer clock selection.

## Interaction states

- **Release landscape:** Release Year is the active single-clock lens.
- **Original landscape:** Original Year is the active single-clock lens.
- **Two clocks:** both timelines remain visible; clicking either one changes the active lens and redraws aggregated counterpart flows.
- Selecting an album opens the existing right-side Album tab with both dates, cover, rating, Love count, track count, duration, and a bounded Play action.
- Loading, unavailable, empty, missing-year, queue-working, and queue-result states are explicit.
- Keyboard users can focus and activate timeline years, modes, missing-year choices, album cards, and actions.

## Acceptance checks

- Selecting Release Year 2025 shows editions released in 2025 grouped by Original Year/decade.
- Selecting Original Year 1982 shows music originating in 1982 grouped by its Release Year/decade.
- Both selections show the same pair of date fields on every visible album and in the Album inspector.
- Release and Original year clicks update without stale responses or unbounded payloads.
- Play loads no more than 100 tracks; Explore opens the matching Songs filter and keeps keyset paging.
- The browser preview exercises both lens directions; the packaged Tauri build exercises the real read-only database, artwork protocol, album detail, and playback boundary.
