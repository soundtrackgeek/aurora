# Aurora

Aurora is a fast, local-first Windows 11 explorer and player for a personal music universe. Version 0.15.7 adds boolean, negative, exact, and inherited-field catalog search plus the Music Library `scores` genre umbrella.

![Aurora design reference](Aurora.png)

## Current 0.15.7 slice

- Tauri 2, Rust, React, TypeScript, and Vite Windows application.
- Device-local Windows output selection using stable endpoint IDs, with automatic continuation on the Windows default when the preferred device is missing, cannot open, or disconnects.
- ReplayGain Off, Track, and Album modes based on MusicBee-compatible `REPLAYGAIN_*` ID3 text frames. Album mode falls back to Track tags, positive gain is capped by the tagged peak, and MP3 files are never modified.
- Gapless-capable queue transitions: Aurora opens and appends the next resolved MP3 to the same native player during the final 15 seconds, so audio handoff does not wait for React polling. Missing, invalid, or unknown-duration files retain the safe ordinary transition.
- An Audio Settings tab beside Global Shortcuts, atomic per-computer persistence in `%APPDATA%\com.soundtrackgeek.aurora\aurora-audio.json`, and a compact player readout for the active output and applied gain.
- Windows global shortcuts for play/pause, next, whole-star ratings 0–5, and Love. The rating defaults use `Ctrl+Alt+Numpad0` through `Ctrl+Alt+Numpad5` so number-row AltGr characters remain available; playback, next, and Love remain `Ctrl+Alt+P`, `Ctrl+Alt+N`, and `Ctrl+Alt+L`.
- A native Settings editor that captures replacement key combinations, rejects duplicates and modifierless keys, restores defaults, and enables or disables the complete shortcut set.
- A Display Settings tab with global Compact through Maximum text presets, readable minimum sizes, and Compact through Extra Large library-cover presets.
- Independent text and cover overrides for Universe, every Library destination, Observatory, Charts, and History. Views inherit the global choices until explicitly overridden; Charts starts at the larger text preset, and cover controls are disabled where a view has no adjustable artwork.
- Device-local display preferences in versioned browser storage, restored before Aurora renders and kept outside MP3s, the read-only catalog, and shared OneDrive state.
- Shortcut actions always resolve Aurora's now-playing track from the Rust playback runtime. Explore selection never becomes the rating or Love target, and tag shortcuts use the same immediate verified MP3 plus optimistic Aurora-state pipeline as the player.
- Device-local shortcut persistence in `%APPDATA%\com.soundtrackgeek.aurora\aurora-shortcuts.json`; these Windows bindings are intentionally excluded from Laptop Mode and OneDrive state synchronization.
- Aurora unregisters all active shortcuts during both window close and application exit. Another running app must still retry or restart if its own registration previously failed while Aurora owned the same binding.
- A persistent left-sidebar cycle with expanded, icon-only, and fully collapsed modes, plus an independently collapsible right inspector. Layout choices stay local to each computer and restore before the first rendered frame.
- A collapsible Library tree containing Songs, Albums, Artists, Genres, Years, Ratings, and Tags. Opening a closed Library enters Songs by default; Library and Playlists disclosure choices persist per computer.
- Compact Library and pinned-playlist flyouts in icon-only mode, with active nested destinations still visible on the parent Library icon.
- A paired-clock Years explorer that preserves Music Library's distinct Original Year and Release Year fields, with clickable album-level histograms and aggregated edition flows between them.
- Release, Original, and Two Clocks modes; exact missing-year lenses; previous/next year movement; and bounded edition shelves grouped by the counterpart decade.
- A dedicated Album inspector for selected editions, exact Original/Release Year handoff into Songs, and bounded year or album playback without exposing file paths to React.
- Lazy, stale-safe Years queries: overview payloads contain roughly one row per year, year details return at most 100 representative albums, and playback returns at most 100 tracks.
- A dedicated Ratings Studio with separate track and effective-album constellations, clickable whole- and half-star bands, an exact 5 Star Collection, and tall real-cover pyramids with a silver-to-magenta constellation palette.
- Almost Complete, Partially Rated, and Unrated Album lanes with mutually exclusive catalog counts, at most 14 album candidates per request, bounded track details, and Play Unrated Tracks.
- Music Library-compatible effective album ratings: explicit MusicBee Album Rating wins; otherwise a rounded normalized track mean becomes valid only after every track is rated. Partial means are labelled provisional and never enter album-rating counts.
- Music Library's exact unbounded Album Score formula, kept numeric rather than converted to stars. Fully track-rated albums show the current score in Ratings and ordinary Album detail; future Charts can rank by the same value without changing its meaning.
- A dedicated Charts page above History with Singles and Albums modes, direct weekly drill-down, named period presets, editable custom week ranges, and one-click full-year charts.
- Historical Official UK, VG Lista, Ti i Skuddet, and Norsktoppen weekly charts plus the catalog's annual Billboard singles and album charts. Unsupported source/type combinations are never presented as data.
- Calculated period charts rank by number of #1 finishes, then #2 finishes, then each lower position in order; chart points and appearances provide deterministic final tie-breaks.
- A first-class Aurora Album Score chart and year shelf reuse Music Library's exact numeric formula without converting it to stars, use `Year` by default, and can switch explicitly to `Release Year`.
- Library-matched chart entries expose real cover art, rating, Love, movement, peak, source history, direct playback, chart-queue playback, and handoff into the ordinary library inspector. Requests and playback queues remain capped at 100 items.
- Instant Ratings Studio star and Love controls reuse the verified MP3 transaction and Aurora overlay, then refresh only the affected bounded UI. Switching completion lanes does not rerun the full ratings overview.
- Persistent icon-only device mode: a monitor identifies Desktop Mode, a laptop identifies Laptop Mode, and each computer remembers its own choice in `aurora-device.json` outside the shared state database.
- Exact runtime-only drive translation from `D:\MUSIC`, `G:\_BACKUP\SCORES`, and `H:\Synthwave` to `Y:\MUSIC`, `V:\_BACKUP\SCORES`, and `U:\Synthwave`; the catalog and stable track identities remain unchanged.
- Verified SQLite state snapshots at `%USERPROFILE%\OneDrive\_musicbackup\aurora-state.sqlite3`, published at most once per minute and once more on clean shutdown.
- Per-device listening journals in local `aurora-history.sqlite3` databases, mirrored as separately named, validated OneDrive snapshots so Desktop and Laptop sessions can be combined without creating shared-state conflicts.
- A configurable 1–3600 second played threshold, defaulting to 30 seconds. Only observed forward playback counts; seeking does not inflate listening time, and a shorter track counts when it naturally finishes.
- A bounded History timeline with outcome, device, date, and text filters; registered-play, listening-time, unique-track, skip, and most-played summaries; and direct replay/inspection actions.
- Personal registered plays, listening time, and last-listened time in the selected-track inspector, kept distinct from imported Last.fm popularity.
- First-run laptop recovery copies a valid OneDrive snapshot into Aurora app data before SQLite opens. Newer clean snapshots are also applied only before open, with a retained local safety copy.
- Sync lineage, generations, and logical revisions detect two-computer divergence. Aurora reports a conflict and preserves both files instead of using unsafe newest-file-wins behavior.
- Equivalent OneDrive branches reconcile automatically when only transient catalog IDs, playback position, import-run markers, or retry timestamps differ. Stable queue identity and user-authored tag, journal, playback-setting, and curation differences still block automatic replacement.
- Strictly read-only access to `%APPDATA%\com.local.musiclibrary\music-library.sqlite3`.
- A dedicated Genre Atlas over all canonical catalog genres, with search and sorts for scale, rating, Love, recent listening, unexplored worlds, and name.
- Bounded genre details with representative album covers, release decades, personal listening memory, top albums and artists, shared-artist connections, and editable track highlights.
- Genre Radio, Shuffle, Loved, Highest Rated, Rediscover, and Unrated Expedition actions that load at most 100 tracks per batch, auto-refill below 20 remaining tracks, and never exceed the 200-track queue.
- Bounded startup payload: summary, eight high-volume artists, and 50 five-star tracks.
- Keyset-paged Tracks, Albums, and Artists views that request 50 rows at a time and never hold a million-row result in the WebView.
- Exact half-star/unrated, Love/Neutral/Ban, release-year, genre, and artist filters, plus safely quoted FTS5 prefix search across the entire catalog.
- Field-aware search supports `artist:` (Display Artist), `aartist:` (Album Artist display), `album:`, `genre:`, `year:` (Year), `ryear:` (Release Year), `publisher:`, and `title:`. Commas or uppercase `AND` combine groups; uppercase `OR` adds alternatives and inherits the preceding field; `NOT` or a leading `-` excludes a group. A complete quoted value is exact, while unquoted text remains prefix-based. `genre:scores` expands to the Music Library film, TV, animation, anime, and game-score genres.
- Validated sorts for newest, title, artist, album, release year, rating, and artist track count; opaque cursors cannot be reused with a different sort.
- Clickable artist planets and artist results that open an exact artist focus which can be switched between tracks and albums.
- A functional Constellations artist inspector opened from universe planets, Artist results, the selected track, or the Observatory review queue.
- A bounded, searchable Observatory for candidate-bearing artists, with Needs review, Conflicts, Unconfirmed, Aurora decisions, and All candidates filters.
- Explicit artist candidate confirmation, ignore, and clear actions. Aurora decisions are durable, undoable, and take presentation precedence without hiding disagreements in the imported sources.
- Local release-group curation for linking a visible MusicBrainz group to an album from the same artist, marking it not in scope, ignoring it, or clearing the decision.
- Lazy local MusicBrainz identity resolution with verified, unconfirmed, conflict, ignored, and unmatched states; verified external overlay links and explicit Aurora confirmations are labeled with their exact provenance.
- MBID-gated artist type, active dates, area, birthplace, and origin-country context from the existing catalog import.
- Source-precedence release-group discographies capped at 100 rows: curated overlay first for verified identities, catalog mirror fallback, then the broad cache without mixing stale and refreshed sources.
- Visible local provenance and source availability for the catalog, curated overlay, and broad cache; missing optional databases never block normal library browsing.
- Explicit overlay export creates a new, complete Music Library-compatible SQLite snapshot in Aurora's app-data `exports` folder. Aurora never mutates the live shared overlay; publishing the exported file remains a deliberate user step.
- Album cover grids with bounded album track details, playback activation, keyboard row navigation, and inline tag controls.
- Inspector editor for half-star ratings, Love/Neutral/Ban, and Release Year, plus read-only genre, duration, and optional Last.fm popularity.
- Direct Explore-row rating and Love controls: click either half of a star for an exact 0.5 step or click the heart to toggle Love, and Aurora saves to the MP3 immediately with per-row verification feedback.
- Native MP3 playback with play/pause, seek, previous/next, volume, shuffle, and repeat-one/repeat-all controls.
- A real MP3-derived, purple-to-cyan waveform timeline. Native builds sample 64 evenly spaced decoded windows into 320 peaks, cache them in device-local `aurora-waveforms.sqlite3`, and never accept an arbitrary WebView path.
- Race-safe seeking: the exact released range value is committed, older overlapping seek responses cannot replace newer state, and the live playback clock retakes the playhead after the latest seek finishes.
- Bottom-player half-star rating (including clear-to-unrated) and Love controls that reuse Aurora's verified instant MP3 tag-write and optimistic state-overlay workflow.
- A clickable end-time readout that toggles between total duration and a live negative remaining-time display.
- A bounded 200-track queue with play-now, reorder, remove, and clear actions.
- Durable queue, current track, position, volume, shuffle, and repeat state in Aurora's own SQLite database.
- Stable queue identity based on the normalized MP3 path, verified alongside every transient track ID so queue items survive Music Library TSV imports without being retargeted.
- Transactional same-folder MP3 writes using MusicBee's exact POPM byte map, `LOVE RATING`, and Release Time conventions.
- Conflict detection, post-write tag/audio verification, Windows atomic replacement, retained rollback copies, crash recovery, and one-step undo. Ambiguous or externally changed files are never auto-overwritten; Aurora retains both versions for manual recovery.
- Aurora-owned tag overlays that update the UI immediately and reconcile automatically after a later MusicBee TSV import updates Music Library.
- Focus-time MusicBee reconciliation that reads only pending-overlay MP3s in bounded batches, treats their tags as authoritative, clears caught-up overlays, and rotates unavailable files so they cannot starve later work.
- Half-star track reconciliation reads Music Library's raw rating when its older normalized field is null; removed-track overlays are excluded from library totals.
- Album covers served through a narrow Rust protocol that resolves exact album IDs, contains canonical paths to the configured archive, rejects oversized sources, and caches 64–512 px WebP thumbnails.
- Packaged-app update checks at startup and every 60 seconds, with an Aurora-styled install prompt.
- Windows NSIS release workflow with mandatory Tauri updater signatures.

The MP3 is authoritative for Aurora tag edits. Aurora never writes the shared Music Library SQLite database; it records a small optimistic tag overlay in `aurora-state.sqlite3` until the normal MusicBee TSV export and Music Library import catch up. Aurora's MusicBrainz decisions are also stored in that app-owned database, but they are independent of MP3 tags and the imported catalog. Listening events deliberately use a separate per-device database instead of the single shared state snapshot. All OneDrive copies are consistent SQLite snapshots rather than copies of live WAL-backed files. See [docs/audio-output-contract.md](docs/audio-output-contract.md), [docs/charts-contract.md](docs/charts-contract.md), [docs/genre-atlas-contract.md](docs/genre-atlas-contract.md), [docs/global-shortcuts-contract.md](docs/global-shortcuts-contract.md), [docs/listening-history-contract.md](docs/listening-history-contract.md), [docs/laptop-mode-contract.md](docs/laptop-mode-contract.md), [docs/ratings-studio-contract.md](docs/ratings-studio-contract.md), [docs/sidebar-navigation-contract.md](docs/sidebar-navigation-contract.md), [docs/tag-editing-contract.md](docs/tag-editing-contract.md), [docs/musicbee-tags.md](docs/musicbee-tags.md), [docs/playback-contract.md](docs/playback-contract.md), and [docs/years-explorer-contract.md](docs/years-explorer-contract.md).

## Data model

The primary catalog currently contains 1,096,288 MP3 tracks, 72,012 albums, and approximately 20,000 album artists across 687 canonical genres. Of those albums, 12,434 have an effective album rating, 678 need only 1–3 more track ratings, 5,723 are partially rated, and 59,578 are unrated. The live full Ratings overview, including overlay-aware album recalculation and bounded shelves, measured about 1.7 seconds; completion-lane changes reuse that overview. Aurora opens the active WAL-backed database with SQLite read-only flags and `query_only`; it does not use immutable mode and does not write ratings back into this imported catalog. Common bounded explorer paths remain approximately 26–84 ms including SQLite process startup. Global title A–Z is the known slower path at approximately 120 ms because the shared catalog has no title-only index.

The broad MusicBrainz cache and curated overlay are deferred from startup and opened independently only when the Artist inspector or Observatory requests context. The audited cache contains 20,208 artist-name rows and 483,675 release groups; the curated overlay contains 493 verified artist links and 9,658 release groups. Cache-only identities remain unconfirmed because 44 audited exact-name candidates conflict with verified links and many MBIDs are shared by multiple names. Observatory pages are capped at 100 rows and intentionally cover candidate-bearing artists already present in the imported MusicBrainz artist-info table; they are not a claim to enumerate all 20,392 catalog artists. See [docs/database-contract.md](docs/database-contract.md) for verified responsibilities, limits, and authority rules.

The album-cover archive at `C:\_code\music_backup_v5\AlbumCovers` contains 76,329 images and maps to current albums through `album_covers.album_id` with 98.09% coverage. Rust now resolves and decodes those images outside the WebView; missing or invalid art falls back to Aurora's generated artwork.

## Requirements

- Windows 11 with WebView2
- Node.js 22+
- Rust stable with the MSVC target and Windows C++ build tools
- The music catalog at the default `%APPDATA%` path above
- The referenced MP3 files and album-cover archive mounted at their cataloged paths for playback and real artwork
- For Laptop Mode, the equivalent library roots mounted at `Y:\MUSIC`, `V:\_BACKUP\SCORES`, and `U:\Synthwave`
- A locally available `%USERPROFILE%\OneDrive\_musicbackup` directory for Aurora state and per-device history mirroring; catalog browsing and local history still work and report a sync warning when it is unavailable
- Optional local MusicBrainz sources under `%USERPROFILE%\OneDrive\_musicbackup` for Constellations enrichment; Aurora remains usable when either source is missing

## Develop and verify

```powershell
npm ci
npm run check:version
npm run lint
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri -- dev
```

Browser development uses clearly labelled preview records and serves matching read-only artwork directly from the configured local cover archive. Prefer the Browser plugin for UI testing and visual inspection; reserve Computer Use for native-only behavior that Browser cannot exercise. Only the Tauri runtime exercises the real SQLite and verified MP3-write boundaries.

Build a local NSIS installer with:

```powershell
npm run tauri -- build
```

## Releases and in-app updates

Every successful push to `master` runs verification first, then builds a Windows NSIS setup executable, signs the updater artifact, creates the matching SemVer tag and GitHub Release, and uploads `latest.json`. The workflow can also be started manually to retry publication of the current version.

Before pushing a new version:

1. Update every manifest, lockfile, and user-facing version label to the same version.
2. Move the relevant changelog notes from `Unreleased` into a dated version section.
3. Run `npm run check:version` and the full verification commands above.
4. Commit and push to `master`. CI verifies and publishes the release autonomously; no manual tag or post-push monitoring is required.

The repository already has `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` Actions secrets. The local encrypted key material is under `%USERPROFILE%\.tauri`:

- `aurora-updater.key` — encrypted private updater key
- `aurora-updater.key.pub` — public updater key
- `aurora-updater-password.dpapi.xml` — passphrase protected for the current Windows account

Back up the private key and passphrase separately and securely. Losing either prevents installed Aurora copies from accepting future updates. This updater signature proves artifact integrity; it is separate from optional Windows Authenticode signing, so installers may still show an Unknown publisher/SmartScreen warning.

## Architecture brief

The behavioral scope, performance target, source-of-truth decisions, and next sections are captured in [docs/app-brief.md](docs/app-brief.md).
