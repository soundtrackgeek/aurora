# Aurora improvement and feature roadmap

*Assessment of Aurora 0.28.16 · 3 October 2026*

This document reviews the whole repository: the Tauri 2/Rust backend (about 50,000 lines in 48 modules), the React 19 frontend (about 23,000 lines), the behavior contracts in `docs/`, the CI and release workflows, and a walkthrough of the browser preview. It also includes online research into services, APIs, and libraries available as of October 2026.

It has three parts:

1. [20 existing features that can be improved, and how](#part-1--20-improvements-to-existing-features)
2. [20 features worth adding](#part-2--20-features-worth-adding)
3. [Online services, APIs, and libraries that could improve Aurora](#part-3--online-services-apis-and-libraries)

It closes with a [suggested sequence](#suggested-sequencing) and [sources](#sources).

---

## Snapshot: what already works well

Aurora is unusually disciplined for a personal app. Every recommendation below keeps its existing rules:

- **Authority boundaries.** The shared catalog is opened read-only (`query_only`). The MP3 file is authoritative for tag edits. Music Library is the only process that writes the catalog and moves files. Aurora's own state, history, waveform, and curation data live in separate databases.
- **Safe writes.** Tag edits are written to a same-folder working copy, verified (including a SHA-256 of the audio bytes), atomically replaced, and recovered after a crash. Album batches roll back as a unit.
- **Bounded payloads.** Pages hold at most 100 rows, the queue holds at most 200 tracks, startup sends a bounded payload, and keyset cursors replace offsets.
- **Engineering hygiene.** There are about 350 Rust tests and 340 frontend tests, no `any` types, a strict CSP, minimal Tauri capabilities, pinned toolchains, signed Windows updates, and a notarized universal macOS build.
- **Audio engine.** Decoding runs on producer threads that feed lock-free PCM rings, so the real-time callback never decodes or allocates. Gapless successors are prepared ahead of time, and a failed output device falls back to the Windows default.

Every online integration proposed below follows the pattern already used for Last.fm, Discogs, fanart.tv, and OpenRouter: it is **opt-in**, stores its key in the **OS credential vault**, sends **bounded requests**, **caches locally**, and **falls back to offline behavior**.

### Numbers that shaped these recommendations

| Signal | Value | Where it comes from |
| --- | --- | --- |
| Tracks / albums / album artists | 1,096,288 / 72,012 / ~20,000 | README, data model |
| **Unrated tracks** | **947,794 (~86 %)** | Ratings Studio |
| MP3s carrying ReplayGain tags | **7 of 250 sampled (~3 %)** | `docs/audio-output-contract.md` |
| Album-cover archive coverage | 98.09 % | README |
| `src/App.tsx` | 4,202 lines; 145 `useState`, 71 `useRef`, 38 `useEffect` | source |
| Native state polled by WebView timers | 2 s, 5 s (×4), 15 s (×2), 60 s | source |
| CSS color literals vs. custom properties | 1,210 hex literals vs. 50 tokens | source |
| Commits that only stabilize tests on release runners | 9 or more | `git log` |

---

## Part 1 — 20 improvements to existing features

| # | Area | Improvement | Effort | Impact |
| --- | --- | --- | --- | --- |
| 1 | Architecture | Split `App.tsx` into domain modules | L | High |
| 2 | Architecture | Push native events instead of polling; schedule sync in Rust | M | High |
| 3 | Performance | Make the cover protocol async and cap its disk cache | S | Med–High |
| 4 | Performance | Virtualize long result lists | M | Medium |
| 5 | UI | Move colors into design tokens (themes, high contrast) | M | Medium |
| 6 | Robustness | Allow only one running instance | S | High |
| 7 | Playback | Normalize loudness when ReplayGain tags are missing | M | High |
| 8 | Playback | Make the queue more capable | M | High |
| 9 | Interaction | Add keyboard control inside the app | S | Med–High |
| 10 | Search | Generate syntax help from one source, validate live, run sooner | M | High |
| 11 | Search | Search personal data and tolerate typos | M–L | High |
| 12 | Home | Make Universe a living home screen | M | Med–High |
| 13 | History | Add flexible Listening Report periods | S | Medium |
| 14 | Configuration | Replace hardcoded paths with a locations model | M | Med–High |
| 15 | Sync | Resolve state-sync conflicts in the app | M | Medium |
| 16 | Network Mode | Keep the Mac catalog fresh automatically | M–L | Med–High |
| 17 | Inbox | Make conversion safer and scanning event-driven | M | Med–High |
| 18 | Artwork | Close the remaining cover-art gap | M | Medium |
| 19 | Engineering | Make tests deterministic and match CI to release platforms | M | Med–High |
| 20 | Documentation | Restructure the README and app brief | S–M | Medium |

### 1. Split `App.tsx` into domain modules

**Implemented in 0.28.18.** Explorer/workspace, inspector/artist, catalog sync, tag mutations, settings, and destination data now live in typed domain hooks under `src/app/`. An app-scoped `useSyncExternalStore` store shares navigation, track selection, and catalog invalidation; lazy route containers retain the existing `RememberedPage` boundaries. Scroll restoration also waits for lazy route content. See [Frontend domain architecture](frontend-domains.md) for module ownership and regression contracts. The assessment below records the original baseline.

**Today.** [`src/App.tsx`](../src/App.tsx) is 4,202 lines. The `App` component holds 145 `useState`, 71 `useRef`, and 38 `useEffect` hooks. Explorer state, inspector selection, catalog-sync status, Inbox and Charts handoffs, playback glue, and workspace restoration all live in one closure, next to the inline `Universe` and `UpdateDialog` components.

**Why it matters.** Almost every feature edits the same file. A state change in one domain re-renders the whole shell. Tests such as `App.chartRefresh.test.tsx` and `App.navigation.test.tsx` have to mount the entire application, which is a major source of the runner-speed flakiness described in [#19](#19-make-tests-deterministic-and-match-ci-to-release-platforms). Startup cost grows with it too: `npm run build` produces one **666 kB** entry chunk and warns about chunks over 500 kB, because only Inbox, Add Music, Artist page, and Listening Report are lazy-loaded.

**How to improve.**

- Extract domain hooks with explicit inputs and outputs, for example `useCatalogRevision`, `usePendingLibrarySync`, `useExplorerWorkspace`, `useInspectorSelection`, and `useArtistNavigation`.
- Keep shared state (selected track, active destination, catalog revision) in a small external store (`useSyncExternalStore` or Zustand), so each page subscribes only to the slices it uses.
- Give each destination its own route container (`<LibraryRoute>`, `<ChartsRoute>`, and so on). `RememberedPage` already provides the mount and retention boundary. Lazy-load these containers (Charts, Ratings, Genres, Years, Publishers, Observatory, Playlists) to shrink the entry chunk.
- Migrate one hook per change, using the existing App tests as the regression net.

### 2. Push native events instead of polling; schedule sync in Rust

**Implemented in 0.28.20.** Rust owns independent schedules for state publication, catalog detection, library retries, pending-tag reconciliation, playback advancement, and history revisions. React listens for changes, with an initial read and a 15-second playback recovery heartbeat. Automatic updates use startup, hourly focus eligibility, six-hour checks, and bounded exponential backoff. See [Native background events](native-events.md). Inbox filesystem watching remains improvement 17. The assessment below records the original baseline.

**Today.** WebView timers drive most background refreshes:

- playback snapshot every 2 s ([`playback.ts:349`](../src/playback.ts));
- Laptop Mode/sync status every 5 s ([`App.tsx:865`](../src/App.tsx));
- catalog revision every 5 s ([`App.tsx:1233`](../src/App.tsx));
- external tag refresh and pending-sync retry every 5 s;
- track history insight and the History page every 15 s;
- Inbox every 15 s;
- update checks every **60 s** ([`updater.ts:7`](../src/updater.ts)).

Rust emits events in only a few places (`library-intake-progress` and the shortcut results).

The most important case: while the app is running, **OneDrive state-snapshot publication is triggered only by that 5-second frontend poll.** `laptop_mode_status` → `LaptopModeRuntime::status(false)` → `StateSyncService::sync_now` ([`lib.rs:1524`](../src-tauri/src/lib.rs), [`laptop_mode.rs:39`](../src-tauri/src/laptop_mode.rs)). The only native trigger is window close ([`lib.rs:1839`](../src-tauri/src/lib.rs)). Chromium-based WebViews throttle timers in hidden or minimized windows, so how often sync runs depends on whether the window is visible.

**Why it matters.** Polling costs IPC calls and SQLite work even when nothing is happening, adds 2–5 s of latency, and spreads stale-response guard logic across React. Checking for updates every 60 s sends about 1,440 GitHub requests per device per day.

**How to improve.**

- Run state-sync publication, catalog-revision detection, and library-sync retries on **native background threads with their own schedule**. They should emit `sync://status`, `catalog://revision`, and `library-sync://status` events, and React should only listen.
- Emit `playback://state` from the playback runtime on every transition (track change, play/pause, seek, queue edit, device fallback). Keep the 250 ms local playhead clock and keep the 2 s poll only as a slow heartbeat (for example, every 15 s). Use `tauri::ipc::Channel` for high-frequency streams.
- Check for updates at startup, on focus when at least an hour has passed, and every 6 hours, with exponential backoff after failures. Keep the manual **Check for updates** button.

### 3. Make the cover protocol async and cap its disk cache - DONE

**Implemented in 0.28.23.** The asynchronous `aurora-cover` responder dispatches to three workers with at most 256 pending requests. All four cover-thumbnail folders share a 1 GiB LRU budget, swept at startup and enforced before atomic writes. Cache reads persist their access timestamps for restart ordering; generated covers still display when caching fails. Source fingerprints and embedded fallbacks are retained. See [Cover protocol and cache](cover-cache.md) for the contract and regression checks. The assessment below records the original baseline.

**Today.** [`lib.rs:1735`](../src-tauri/src/lib.rs) registers `aurora-cover` with the **synchronous** `register_uri_scheme_protocol`. On a cache miss, the protocol thread decodes a source of up to 32 MiB / 100 MP, resizes it with Lanczos3, and encodes WebP. The `aurora-artist` protocol was already moved to the asynchronous responder for this exact reason (see the README note on Artists-page portraits). Thumbnails are written to `covers/` and `embedded-album-covers/` under names derived from a source fingerprint, and nothing ever evicts them. Replaced covers therefore leave orphans, and 76k covers in several sizes (64–512 px) can grow without limit.

**How to improve.**

- Switch to `register_asynchronous_uri_scheme_protocol` with a small bounded worker pool (2–4 threads), so a fast-scrolled cover grid doesn't decode covers one at a time.
- Sweep the cache at startup or when idle with a size-capped LRU policy (for example, 1–2 GB, evicting the least recently accessed first).
- Optionally pre-generate thumbnails for the next keyset page while the user scrolls.

### 4. Virtualize long result lists

**Today.** Songs, Albums, and Artists load 50-row keyset pages through an `IntersectionObserver`. Every loaded page stays mounted, including while the page is hidden, which is intentional so returning is instant. `content-visibility: auto` is applied only to the whole table wrapper ([`DeepExplorer.css:90`](../src/components/explorer/DeepExplorer.css)).

**Why it matters.** In a long session, thousands of rows and cover cards with inline rating controls accumulate. DOM size and reconciliation cost grow linearly, and every background catalog refresh re-renders all loaded rows.

**How to improve.** Use [TanStack Virtual](https://tanstack.com/virtual/latest) for the track table and a lane-based virtualizer for the cover grid. Keep keyset paging. Render the inline album panel as a variable-height virtual row. For restoration, persist the first visible item key plus its offset instead of a raw pixel scroll position. Set `useFlushSync: false` to avoid React 19 scroll warnings.

### 5. Move colors into design tokens

**Today.** The CSS contains 1,210 hardcoded hex colors and only 50 custom properties. For example, [`ChartStudio.css`](../src/components/charts/ChartStudio.css) repeats `#44364e` and `#0d0b13` for its inputs. `color-scheme: dark` is fixed.

**How to improve.** Define a token layer (surface, raised, border, text, text-muted, accent, danger, success, and the rating-constellation ramp) and replace the literals with tokens using a codemod. That unlocks:

- a **high-contrast** variant for accessibility;
- an optional light theme;
- **an accent color taken from the now-playing cover.** Rust already decodes covers to make thumbnails, so it can extract dominant colors at the same time and expose them as CSS variables.

### 6. Allow only one running instance - DONE

**Implemented in 0.28.22.** The single-instance plugin is registered first, before other plugins and application setup. Later launches show, unminimize, and focus the existing main window and forward their arguments and working directory through `app://second-instance`. See [Single-instance startup](single-instance.md) for the contract and native checks. The assessment below records the original baseline.

**Today.** Aurora uses neither `tauri-plugin-single-instance` nor an OS mutex. A second launch (a double-clicked shortcut, or autostart plus a manual launch) starts a second playback engine, a second writer for `aurora-history.sqlite3`, a second state-sync publisher, and competing global-shortcut registrations.

**How to improve.** Add [`tauri-plugin-single-instance`](https://v2.tauri.app/plugin/single-instance/) as the first plugin in the builder. Its callback should focus and restore the existing window and forward any arguments, which also prepares for `aurora://` deep links or "Open with" later. This is cheap protection against corrupting the data Aurora writes.

### 7. Normalize loudness when ReplayGain tags are missing

**Today.** The Off, Track, and Album ReplayGain modes read only existing `REPLAYGAIN_*` frames. The library audit found those frames in just 7 of 250 MP3s, so normalization does nothing for about 97 % of the library. Meanwhile, [`decode_mp3_waveform`](../src-tauri/src/waveform.rs) (line 394) already decodes **every frame** of each played track to build the 640-peak overview.

**How to improve.**

- Feed the same decoded samples into [`ebur128`](https://github.com/sdroege/ebur128) to measure integrated loudness and true peak during waveform analysis.
- Store the results in a device-local `track_loudness` table keyed by the same size and modification-time signature. Keep it separate from the waveform cache, which keeps at most 2,000 entries, so loudness results are never evicted.
- Add a ReplayGain mode **"Tags, then Aurora analysis"** with a −18 LUFS reference (the ReplayGain 2.0 level), reusing the current clipping prevention for true-peak limiting.
- Analyze the prepared successor during its final 15-second preparation window so even a first play is normalized. Optionally, add a low-priority idle worker that analyzes loved and 5★ tracks first.
- The contract stays intact: Aurora never writes tags or audio bytes.

### 8. Make the queue more capable

**Today.** [`QueuePanel.tsx`](../src/components/QueuePanel.tsx) offers play, one-step up/down buttons, remove, and clear, with a 200-track cap. The native commands are replace, append, move, remove, and clear; there is no way to insert at a position. Chart playback stops after the first 100 matched songs.

**How to improve.**

- Reorder by drag and drop and from the keyboard (Alt+↑/↓), and allow selecting several entries to remove.
- Add **Play next** and **Play after this album** through a new `playback_insert_queue` command that uses the same identity validation as append.
- Show a **Previously played** section; the runtime already keeps 20 earlier entries.
- Make **Clear** and **Remove** undoable.
- Add **Save queue as playlist** (see [Feature 2](#2-playlist-authoring)).
- Reuse the Genre Radio refill rule (fewer than 20 left → fetch the next 100) for Charts, Playlists, Years, and Publishers, so long contexts no longer stop silently at the cap.

### 9. Add keyboard control inside the app

**Today.** Inside the window, `Ctrl+K` is the only app-wide key binding ([`App.tsx:970`](../src/App.tsx)). Transport and rating need the mouse or the system-wide `Ctrl+Alt` chords, which are meant for use while another app is focused.

**How to improve.** Add focus-aware bindings that are ignored while a text field has focus (reusing the `searchFocusGuard` logic):

- **Space** — play/pause
- **←/→** — seek ±10 s
- **Shift+←/→** — previous/next track
- **0–5** — rate the *selected* row
- **L** — Love
- **Q** — toggle the queue
- **I** — toggle the inspector
- **Alt+←** — go back (navigation history already exists)
- **?** — show a cheat sheet

Make them rebindable in the existing Shortcuts settings tab.

### 10. Generate syntax help from one source, validate live, run sooner

**Today.** The tooltip text `trackSearchHelp` ([`App.tsx:285`](../src/App.tsx)) lists 11 fields. It omits `born:`, `dead:`, `founded:`, `dissolved:`, `minutes:`, and `ar:`, and the chart fields `bb:`, `uk:`, `vg:`, `ti:`, and `nt:`, even though [`docs/search.md`](search.md) documents all of them. Search runs only after a fixed **2-second** pause ([`App.tsx:286`](../src/App.tsx)), so a malformed query is reported only after that wait.

**How to improve.**

- Expose a field registry from Rust (name, aliases, value type, scope, example) through one command. Generate the tooltip, the autocomplete, and the field table in `docs/search.md` from it so they can't drift apart.
- **Parse while the user types.** Add a cheap `validate_search` command, or a TypeScript mirror of the Rust parser, to highlight fields and flag errors inline, such as a reversed range or an unbalanced quote.
- **Run immediately on Enter.** When the query already parses as complete, shorten the debounce to about 500 ms. The existing stale-response guards already make cancellation safe.
- Autocomplete values for `genre:`, `publisher:`, and `country:` from the existing indexes.

### 11. Search personal data and tolerate typos

**Today.** The query language has no `rating:` or unrated field, and no `plays:`, `lastplayed:`, `added:`, `bitrate:`, or `skips:` fields (`docs/search.md` says so explicitly). Matching is word-prefix FTS only, so `madona` finds nothing.

**How to improve.**

- Build per-query temporary tables on Aurora's side (the same pattern as `temp.aurora_live_album_genres`) for history aggregates such as plays, last played, and skip rate, combining the local and peer history snapshots.
- Add `rating:`, `added:` (album-added timestamps already exist), and `bitrate:` (the Music Doctor quality tables already exist).
- For typos, keep an Aurora-owned [FTS5 `trigram`](https://www.sqlite.org/fts5.html#the_trigram_tokenizer) index of distinct artist, album, and title strings and offer "Did you mean **Madonna**?". The catalog is never modified. [Tantivy](https://docs.rs/tantivy/latest/tantivy/query/struct.FuzzyTermQuery.html) is the next step if FTS5 isn't enough.

### 12. Make Universe a living home screen

**Today.** [`Universe`](../src/App.tsx) (line 433) shows a hardcoded "Welcome back, Jørn." (the name is also hardcoded in the sidebar at line 3569), a fixed Tolstoy quote, and eight planets taken from the "high-volume artists" in the startup payload: the artists with the most *tracks*, not the ones you listen to.

**How to improve.**

- Size and order the planets by registered plays in the last 90 days, with a recent/all-time toggle. History already aggregates top artists.
- Add shelves:
  - **Continue listening** — the restored queue;
  - **Recently added** — from album-added timestamps;
  - **Rediscover** — loved or 5★ tracks not played for a year;
  - **Almost complete** — the Ratings shelf;
  - **On this day** — see [Feature 13](#13-year-in-review-and-on-this-day).
- Read the display name from settings or the OS account, and rotate through a short list of quotes.

### 13. Add flexible Listening Report periods

**Today.** The report offers only `7 | 30 | 90 | all` ([`ListeningReport.tsx:33`](../src/components/history/ListeningReport.tsx)).

**How to improve.** Add calendar month and year, a custom date range, comparison with the same period last year, and a per-device split view. The backend already accepts explicit millisecond bounds and a timezone offset, so this is mostly UI work. Add **Export report** (PNG or JSON).

### 14. Replace hardcoded paths with a locations model

**Today.** Several environment paths are compiled into the app:

- `COVER_ROOT = C:\_code\music_backup_v5\AlbumCovers` ([`catalog.rs:17`](../src-tauri/src/catalog.rs));
- three fixed Laptop Mode mappings such as `D:\MUSIC → Y:\MUSIC` ([`device_mode.rs:14`](../src-tauri/src/device_mode.rs));
- the album-removal destination `D:\MUSIC_NOT_ALBUMS`, validated in [`library_bridge.rs:533`](../src-tauri/src/library_bridge.rs);
- an FFmpeg fallback at `C:\ffmpeg\bin`.

macOS Network Mode, by contrast, already has a settings-backed connections model ([`connections.rs`](../src-tauri/src/connections.rs)).

**Why it matters.** A new drive letter, a new PC, or a moved cover archive currently needs a code change and a release. The app brief lists "editable drive mappings" as a non-goal, but that predates Network Mode, which already solved the problem on macOS.

**How to improve.** Add one device-local **Library locations** settings panel used by Desktop, Laptop, and Network modes. It should cover the catalog path, cover archive, removal destination, per-root mappings, and FFmpeg path, each with a live status indicator (reusing `path_mapping_statuses`) and validation that every mapping translates a complete root. Default to today's values so existing machines see no change.

### 15. Resolve state-sync conflicts in the app

**Today.** When both computers have changed their state, Aurora keeps both files and tells the user to "close Aurora on one machine and resolve which state to keep" ([`state_sync.rs:350`](../src-tauri/src/state_sync.rs)). The app offers no way to compare the two states or choose one.

**How to improve.** Add a conflict panel to the Laptop/Network popover:

- Show a count of differences per category: queue, tag overlays, tag-journal entries, MusicBrainz decisions, and album-added records.
- Offer **Keep this computer**, **Keep the other computer**, and **Merge append-only records**. Album additions are already merged narrowly today, and the decision log is append-only.
- Always keep a timestamped safety copy and record which choice was made.

### 16. Keep the Mac catalog fresh automatically

**Today.** In Network Mode, the Mac catalog is a manually copied snapshot that, per the README, "is not automatically refreshed from the PC". Remote rating and Love edits are mirrored locally, but new albums and other imports on the PC require a manual copy with both apps closed.

**How to improve.**

- After each completed import, the PC publishes a consistent catalog snapshot (SQLite backup API or `VACUUM INTO`, written to a temporary file and then atomically renamed, the same sealing pattern state snapshots use) together with a revision marker in the sync folder.
- At startup or on demand, the Mac validates the snapshot (`quick_check` and schema), swaps it in, and keeps the previous copy for rollback.
- The Network popover shows how far behind the catalog is, for example "Catalog is 3 imports behind".

### 17. Make Inbox conversion safer and scanning event-driven

**Today.** FLAC/APE conversion depends on finding FFmpeg next to the executable, on `PATH`, or at `C:\ffmpeg`. It always produces 320 kbps CBR and then **deletes the lossless source** after verification ([`inbox-contract.md`](inbox-contract.md)). Monitored folders are rescanned recursively every 15 s while Inbox is visible.

**How to improve.**

- **Archive lossless originals by default.** Offer "Move lossless originals to an archive folder" instead of deleting them, because a lost lossless master can't be recreated.
- **Find FFmpeg up front.** Detect it before conversion and show a setup hint, or ship it as a Tauri sidecar.
- **Offer LAME V0** as an optional output setting.
- **Watch instead of rescanning.** Replace the 15-second rescan with [`notify`](https://github.com/notify-rs/notify) (ReadDirectoryChangesW) plus a debouncer for the at most 10 monitored folders, and keep focus and periodic rescans as a fallback. The app brief's no-watcher rule targets the million-track library, not Inbox staging.

### 18. Close the remaining cover-art gap

**Today.** 98.09 % of albums map to the cover archive; the rest fall back to generated artwork. Aurora uses the Cover Art Archive only for release thumbnails inside Inbox.

**How to improve.** Add a **Missing covers** queue:

- For albums with a *verified* MusicBrainz release-group link (from the overlay or an Aurora decision), fetch [`coverartarchive.org/release-group/{mbid}/front-500`](https://musicbrainz.org/doc/Cover_Art_Archive/API) (or `-1200`).
- Fall back to fanart.tv album art; its credentials already exist.
- Show the candidates side by side. After the user confirms, reuse the existing verified "embed cover and update the archive" transaction.
- Never apply a cover automatically or by title matching, to respect the exact-album-identity rule.

### 19. Make tests deterministic and match CI to release platforms

**Today.** At least nine commits exist only to stabilize tests on release runners: Charts scroll and refresh, Inbox rename readiness, PCM ordering, and storage locks. The `verify` job runs only on `windows-latest`, but releases also build for macOS, so macOS-only breakage first appears at release time (for example `39fa8f9` and `51382c0`).

**How to improve.**

- Add a `macos-latest` leg to `verify`, at least running `cargo clippy` and `cargo test` for `cfg(target_os = "macos")` code.
- Standardize on fake timers and an injectable clock in hooks; several tests have already moved to deterministic timer control.
- Add a few Playwright smoke flows against the browser preview, in line with AGENTS.md's preference for browser-based testing.
- After [#1](#1-split-apptsx-into-domain-modules), test domain hooks in isolation instead of mounting all of `App`.

### 20. Restructure the README and app brief

**Today.** `README.md` is about 100 KB. It opens with release notes (0.28.16, 0.28.15, …) and catalogs features as a list of about 280 bullets under "Current 0.24.41 slice". `docs/app-brief.md` ends with "Planned sections after 0.14.0".

**How to improve.**

- Turn the README into what Aurora is, requirements, installation, a page-by-page feature tour, and links to the contracts. Keep per-version narrative in `CHANGELOG.md` only. AGENTS.md is still satisfied by updating both files, with README edits limited to the relevant section.
- Refresh the app brief with the current authority rules and a "non-goals as of 0.28" list, and link it to this roadmap.
- Optionally, add `docs/user-guide/` with one page per area (Inbox, Charts, Ratings, …).

---

## Part 2 — 20 features worth adding

| # | Feature | Builds on | Effort |
| --- | --- | --- | --- |
| 1 | Smart playlists and saved views | Query language, app brief "Planned #2" | M |
| 2 | Playlist authoring | Music Library bridge, playlists | M |
| 3 | Context menus with Play next / Add to queue | Queue insert (Improvement 8) | S–M |
| 4 | Rating Sprint | 947,794 unrated tracks, verified tag writes | M |
| 5 | Suggested ratings from listening behavior | Session outcomes in history | M |
| 6 | "More like this" sonic radio | bliss-audio, analysis worker | L |
| 7 | Natural-language search | OpenRouter/Ollama, query language | S–M |
| 8 | Lyrics panel | USLT/SYLT frames, LRCLIB | M |
| 9 | Opt-in scrobbling | History journal, Last.fm credentials | M |
| 10 | New-release radar and discography gaps | MusicBrainz overlay, ListenBrainz | M |
| 11 | Library Health dashboard | Existing quality/sync/curation data | M |
| 12 | Fingerprint duplicate finder and identification | Chromaprint, AcoustID | L |
| 13 | Year in Review and On This Day | Listening report engine | M |
| 14 | Command palette | Bounded search commands | S–M |
| 15 | Tray, mini player, and taskbar buttons | Tauri tray, `windows` crate | M |
| 16 | Sleep timer and stop-after | Playback producer gain stage | S |
| 17 | Setlists and concerts | setlist.fm, chart song matcher | M |
| 18 | Chart previews and wishlist | Charts "Not in library", Deezer | M |
| 19 | Personal album notes and listening journal | Aurora state DB | S–M |
| 20 | Local read-only MCP server | Existing bounded commands, `rmcp` | M |

### 1. Smart playlists and saved views

**What.** Live, rule-based playlists such as "5★ synthwave from 1984–1988 not played in six months", plus saved explorer views pinned in the sidebar.

**Why for Aurora.** Smart playlists are item 2 of the app brief's planned sections and haven't been built yet. The query language is already powerful enough, especially with the personal fields from [Improvement 11](#11-search-personal-data-and-tolerate-typos). Today, playlists come only from Music Library and are static.

**How it fits.** Store each definition (name, query, sort, limit, refresh policy) in `aurora-state.sqlite3`, so it syncs between devices through the existing snapshots. Evaluate definitions with keyset pages and play them through bounded queue refills. Add them to the Playlists flyout.

### 2. Playlist authoring

**What.** Create, rename, reorder, and delete playlists; **Add to playlist** from any row; **Save queue as playlist**; import and export M3U8.

**How it fits.** `saved_playlists` lives in the Music Library catalog, which Aurora only reads. Send writes through the existing versioned file bridge so Music Library stays the only catalog writer, or start with Aurora-owned playlists in the state database plus an **Export to Music Library** action.

### 3. Context menus with Play next / Add to queue

**What.** Right-click (or Shift+F10 / the Menu key) on any track, album, artist, chart entry, or history row to get: Play, **Play next**, **Add to queue**, Add to playlist, Go to album/artist, Show in Explorer/Finder, Copy "Artist – Title", Open Tags, and Rate.

**Why.** Aurora has no context menus today, so these actions are spread across pages or missing. Build them as an accessible in-WebView menu or with Tauri's native `Menu` API.

### 4. Rating Sprint

**What.** A keyboard-driven mode for rating quickly. Aurora plays a short excerpt from a "hook" point, either about 35–40 % into the track or the loudest section according to the cached waveform peaks. Number keys rate the track and advance to the next one; `S` skips and `L` loves. The session shows a goal and progress ("+120 ratings today"). A sprint can target an album, an artist, a genre, or a smart playlist.

**Why for Aurora.** About 86 % of the library is unrated. Ratings drive Album Score, Charts, Tonight's Album, and Ratings Studio, so every rating improves the rest of the app. "Finish what you love" works per album; a sprint works across the library.

**How it fits.** It uses the existing verified MP3 write path and overlay. Writes are queued in the background, as global shortcuts already do, so the sprint never waits for disk I/O.

### 5. Suggested ratings from listening behavior

**What.** Infer a suggested rating from the history journal (completed vs. skipped sessions, replays, listening time). For example, three or more complete plays and no skips could suggest ★4, and repeated skips within 30 s could suggest ★2. Suggestions appear as ghost stars and need one click to accept.

**Guardrails.** Nothing is ever written automatically, and the rule behind each suggestion is visible. Suggestions also order the Rating Sprint queue, so the user confirms likely favorites first.

### 6. "More like this" sonic radio

**What.** Build a queue of tracks that *sound* similar to a seed track, album, or the currently playing track, filtered by Ban and optionally by rating or genre.

**How it fits.** [`bliss-audio`](https://github.com/Polochon-street/bliss-rs), using its Symphonia decoder so no FFmpeg is needed, computes a short feature vector per track (tempo, timbre, loudness, chroma). Store the vectors in a device-local analysis database, with [`sqlite-vec`](https://alexgarcia.xyz/sqlite-vec/rust.html) for nearest-neighbor search if needed. Analyzing 1.1M tracks takes days of CPU time, so do it incrementally: played, loved, and 5★ tracks first, then an idle background worker with a CPU cap. This complements Genre Radio, which is based on metadata, with continuity based on sound.

### 7. Natural-language search

**What.** An **Ask** mode turns a sentence such as "melancholic 80s synth ballads by Norwegian artists I've loved" into Aurora's query language, for example `genre:synthpop AND year:1980..1989 AND country:NO AND love:1..`. Aurora shows the generated query so the user can edit it before running it.

**Why.** This makes the full search language usable without learning it, and it costs little. Only the sentence and the field registry from [Improvement 10](#10-generate-syntax-help-from-one-source-validate-live-run-sooner) leave the device; no catalog data does.

**How it fits.** Use the existing OpenRouter connection (its key is already in the vault), or a local [Ollama](https://docs.ollama.com/capabilities/structured-outputs) model constrained by a JSON schema for full privacy. The same local option could serve Tonight's Album.

### 8. Lyrics panel

**What.** A lyrics panel that highlights the current line of synced lyrics, driven by the player clock.

**How it fits.** Read embedded USLT/SYLT frames first; `id3_write.rs` already preserves them. Otherwise, query [LRCLIB](https://lrclib.net/docs) `/api/get` with artist, title, album, and duration. It needs no key, matches durations within ±2 s, and publishes SQLite dumps that would allow a fully offline mirror. Cache results in an Aurora database. Network lookups are opt-in, and lyrics are written into MP3s only if the user explicitly saves them through the Tags pipeline.

### 9. Opt-in scrobbling

**What.** Per-device, explicitly enabled submission of plays to Last.fm and/or ListenBrainz.

**Why.** The history contract deliberately keeps plays local, so this must be opt-in. Many users still want a Last.fm profile, and ListenBrainz scrobbling also unlocks its recommendations and personal fresh-release feeds ([Feature 10](#10-new-release-radar-and-discography-gaps)).

**How it fits.**

- Scrobble from the history journal: finished sessions that meet Last.fm's rule (the track is longer than 30 s and was played for at least half its length or 4 minutes), sent in [batches of up to 50](https://www.last.fm/api/show/track.scrobble).
- Keep a durable outbox for offline periods and peer-device sessions, and send now-playing updates.
- Store the Last.fm session key in the vault; the API key and secret are already configured in Metadata settings. ListenBrainz uses a user token and the same threshold rule.

### 10. New-release radar and discography gaps

**What.** For artists with verified MBIDs:

- **Missing from your library** — release groups in the curated overlay or cache that have no linked local album, filtered by type (Album or EP). Observatory already models these links.
- **New and upcoming releases** — from ListenBrainz [fresh releases](https://listenbrainz.readthedocs.io/en/latest/users/api/misc.html). The personal feed needs scrobbling; the global feed can be filtered by your MBIDs.

Both feed the wishlist in [Feature 18](#18-chart-previews-and-wishlist).

### 11. Library Health dashboard

**What.** One page that counts fixable problems and links each count to the tool that fixes it:

- albums without covers (about 1.9 %);
- missing genre, publisher, or year;
- albums with mixed bitrates or under 192 kbps (the quality tables already exist);
- pending or blocked Music Library syncs;
- unverified artist identities;
- ReplayGain/loudness coverage;
- sync conflicts.

Each count opens a filtered Albums view or the Observatory.

### 12. Fingerprint duplicate finder and identification

**What.** Compute audio fingerprints and use them to find the same recording across compilations, remasters, and duplicate rips, and to identify poorly tagged Inbox rips.

**How it fits.**

- Compute Chromaprint fingerprints with [`chromaprint-next`](https://github.com/attilagyorffy/chromaprint-next), a pure-Rust port whose fingerprints are bit-identical to AcoustID's, from the same decoded PCM used for loudness and similarity analysis.
- Show duplicate groups with bitrate and rating side by side, and let the user remove copies through the existing verified deletion workflow ("keep the best").
- In Inbox, add [AcoustID](https://acoustid.org/webservice) lookup (`meta=recordings releasegroups`) as a third candidate source next to MusicBrainz and Discogs text search. Results remain proposals; the user still chooses.

### 13. Year in Review and On This Day

**What.**

- **Year in Review** — an annual summary built from all devices' history: top artists, albums, and genres; biggest discoveries; longest streak; listening clock; most-skipped tracks; and albums added and completed that year. It can be exported as an image.
- **On This Day** — a shelf showing what you played on this date in earlier years, plus albums released on this date (the catalog has Release Time).

### 14. Command palette

**What.** `Ctrl+Shift+P` opens a palette that fuzzy-jumps to any page, artist, album, genre, publisher, or playlist, and runs actions such as "Toggle shuffle", "Rate current track 5", "Open Inbox", "Check for updates", or "Export MusicBrainz curation". It uses the existing bounded search commands and pairs naturally with [Improvement 9](#9-add-keyboard-control-inside-the-app).

### 15. Tray, mini player, and taskbar buttons

**What.**

- **Tray icon** (Tauri's `tray-icon` feature): now playing, play/pause, next, rate/Love, and quit, with an optional close-to-tray setting.
- **Mini player**: a compact, always-on-top window with cover, title, waveform, and stars.
- **Windows taskbar**: a [thumbnail toolbar](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-itaskbarlist3-thumbbaraddbuttons) with previous, play/pause, and next buttons, plus taskbar progress. Both use `ITaskbarList3`, and the `windows` crate is already a dependency.
- Optionally, start with Windows through `tauri-plugin-autostart`.

### 16. Sleep timer and stop-after

**What.** Stop after the current track, after the current album, after N minutes, or at the end of the queue, with a 10-second fade-out. The fade is a simple gain ramp in the existing producer gain stage, not DSP crossfading. "Stop after" survives queue edits, and history records the outcome as usual.

### 17. Setlists and concerts

**What.**

- **Setlists** — on the artist page, show recent setlists from [setlist.fm](https://api.setlist.fm/docs/1.0/resource__1.0_artist__mbid__setlists.html) (free for non-commercial use, looked up by artist MBID). A **Play this setlist from my library** button resolves each song with the existing conservative matcher (`chart_song_match.rs`) and lists the songs it couldn't find.
- **Concerts** — optionally, use the [Ticketmaster Discovery API](https://developer.ticketmaster.com/products-and-docs/apis/getting-started/) (5,000 calls per day) to list upcoming shows near a configured city for artists you play often.

setlist.fm's terms require an attribution link.

### 18. Chart previews and wishlist

**What.** Charts can already filter to **Not in library**. For those entries, add:

- **30-second previews** from Deezer's public API, which needs no authentication. Fetch preview URLs just before use, because they expire after about 15 minutes.
- **A wishlist** of songs and albums with their source chart and date, stored in Aurora state so it syncs across devices. A wishlist entry resolves automatically when an Inbox intake adds a matching album, using the existing chart-identity matching.

### 19. Personal album notes and listening journal

**What.** Timestamped free-text notes and personal tags on albums and artists, such as "first heard at…" or "the 1999 remaster is brighter". Notes are stored in Aurora state, so they sync through snapshots and never touch MP3s or the catalog. They are searchable through a `note:` field and shown in the Album inspector. Optionally, finishing an album prompts for a note and an album rating.

### 20. Local read-only MCP server

**What.** Expose library tools to Claude Desktop and Claude Code through the official Rust MCP SDK ([`rmcp`](https://github.com/modelcontextprotocol/rust-sdk)), over stdio or localhost Streamable HTTP. Possible tools: `search_catalog` (using the query language), `album_detail`, `listening_report`, `ratings_overview`, and optionally `queue_tracks`.

**Guardrails.** Tools reuse the existing bounded commands. The server is read-only by default and enabled only by an explicit Settings toggle, and it never exposes file paths. It would let an assistant answer questions like "What did I listen to most in August?" or "Which 1985 albums am I closest to finishing?".

---

## Part 3 — Online services, APIs, and libraries

Aurora already integrates MusicBrainz, Discogs, Last.fm (metadata only: `track.getInfo` and `artist.getinfo`), fanart.tv, the Cover Art Archive (Inbox thumbnails only), OpenRouter (Jev), and Tonehavn. The options below add new value without replacing any of them.

### A. Metadata, identification, and artwork

| Service / library | What it gives Aurora | Access and cost | Caveats | Maps to |
| --- | --- | --- | --- | --- |
| [AcoustID web service](https://acoustid.org/webservice) | Fingerprint → MusicBrainz recording and release-group IDs | Free client API key; compressed POST preferred | Identifies whole songs, not short snippets | Feature 12, Inbox |
| [chromaprint-next](https://github.com/attilagyorffy/chromaprint-next) / [rusty-chromaprint](https://github.com/darksv/rusty-chromaprint) | Pure-Rust Chromaprint; next is bit-identical to C, with base64 encode/decode helpers | MIT, crates.io | rusty-chromaprint covers only the default algorithm well | Feature 12 |
| [Cover Art Archive](https://musicbrainz.org/doc/Cover_Art_Archive/API) | `front-250/500/1200` per release or release group | Free; currently no rate limit | Follow 307 redirects; handle 404/503 | Improvement 18 |
| [Wikipedia REST summary](https://en.wikipedia.org/api/rest_v1/) and [Wikidata REST](https://www.wikidata.org/wiki/Wikidata:REST_API) | Biographies when Last.fm has none; label logos (P154 → Commons) | Free; User-Agent required | Commons files have per-file licenses | Implements the route in [`publisher-logos.md`](publisher-logos.md) |
| [TheAudioDB](https://www.theaudiodb.com/free_music_api) | Transparent artist logos, clearart, album descriptions in many languages | Free key `123` with limited methods; paid premium | Free tier is rate-limited | Artist page |
| [ListenBrainz fresh releases](https://listenbrainz.readthedocs.io/en/latest/users/api/misc.html) | New and upcoming releases, personal or global | Free | Global 90-day feed is about 10 MB, so filter and cache | Feature 10 |
| [setlist.fm API](https://api.setlist.fm/docs/1.0/resource__1.0_artist__mbid__setlists.html) | Setlists by artist MBID | Free non-commercial key | Attribution link required | Feature 17 |
| [Ticketmaster Discovery v2](https://developer.ticketmaster.com/products-and-docs/apis/discovery-manual/v2/) | Upcoming events by artist | Free; 5,000 calls/day, 5 req/s | Two calls per artist (attraction, then events) | Feature 17 |
| [Deezer public API](https://support.deezer.com/hc/en-gb/articles/360011538897-Deezer-FAQs-For-Developers) | 30-second previews; per-track BPM and gain | No authentication for catalog data | Preview URLs expire; BPM is sometimes halved or doubled | Feature 18 |

### B. Listening services

| Service / library | What it gives Aurora | Notes | Maps to |
| --- | --- | --- | --- |
| [Last.fm Scrobbling 2.0](https://www.last.fm/api/scrobbling) | `track.updateNowPlaying` and `track.scrobble` | Up to 50 per batch; signature parameters sorted in ASCII order; retry only error codes 11 and 16 | Feature 9 |
| [ListenBrainz submit-listens](https://listenbrainz.readthedocs.io/en/latest/users/api/core.html) and the [`listenbrainz`](https://docs.rs/listenbrainz) crate | Open scrobbling; `import` type for backfill; [collaborative-filtering recommendations](https://listenbrainz.readthedocs.io/en/latest/users/api/recommendation.html) | Token auth; same half-or-4-minutes rule | Features 9, 10 |
| [`discord-rich-presence`](https://github.com/vionya/discord-rich-presence) 1.1 | "Listening to …" status via `ActivityType::Listening` | Small, opt-in, nice to have | — |

### C. Lyrics

| Service | What it gives Aurora | Notes | Maps to |
| --- | --- | --- | --- |
| [LRCLIB](https://lrclib.net/docs) | Synced LRC and plain lyrics, plus an `instrumental` flag | Free, no key; duration must match within ±2 s; 429 responses include `Retry-After`; **SQLite dumps** allow a fully offline mirror | Feature 8 |

### D. Offline audio analysis (Rust)

| Library | What it gives Aurora | Notes | Maps to |
| --- | --- | --- | --- |
| [`ebur128`](https://github.com/sdroege/ebur128) | Integrated loudness, loudness range (LRA), true peak | Pure-Rust port of libebur128; passes EBU Tech 3341/3342. [`ebur128-stream`](https://github.com/KitaitiMakoto/ebur128-stream) is a newer zero-allocation alternative | Improvement 7 |
| [`bliss-audio`](https://github.com/Polochon-street/bliss-rs) | Similarity vectors (tempo, timbre, loudness, chroma) | Symphonia decoder option avoids FFmpeg | Feature 6 |
| [`stratum-dsp`](https://docs.rs/stratum-dsp) | BPM, musical key (Krumhansl-Kessler), beat grid | Would enable `bpm:` and `key:` search fields and energy-aware radio | Optional |
| CLAP via [`ort`](https://rlupi.com/i-built-a-music-intelligence-app-in-five-days-with-two-ai-coding-agents) and [`sqlite-vec`](https://alexgarcia.xyz/sqlite-vec/rust.html) | Fully local text-to-music "vibe" search | About 78 MB quantized ONNX model and about 0.3 s per track; a stretch goal | After Features 6 and 7 |

> **Cross-cutting recommendation: one analysis worker.** Waveform peaks, EBU R128 loudness, Chromaprint fingerprints, and bliss vectors (plus BPM/key) all need the same decoded PCM. Design one throttled, resumable worker that decodes each file once and fans the samples out to every analyzer. Results go into a device-local `aurora-analysis.sqlite3` keyed by the file's size and modification time. Otherwise, adding each analysis feature separately would mean decoding 1.1M files several times.

### E. Search and data

| Library | What it gives Aurora | Notes | Maps to |
| --- | --- | --- | --- |
| [SQLite FTS5 `trigram`](https://www.sqlite.org/fts5.html#the_trigram_tokenizer) | Substring and "did you mean" matching in an Aurora-owned index | Already compiled into the bundled SQLite; no new dependency | Improvement 11 |
| [Tantivy](https://docs.rs/tantivy/latest/tantivy/query/struct.FuzzyTermQuery.html) | BM25 ranking and Levenshtein fuzzy queries | Fuzzy queries score every match equally; expand terms first, then run a TermQuery | Improvement 11 (if FTS5 isn't enough) |
| [`sqlite-vec`](https://alexgarcia.xyz/blog/2024/sqlite-vec-stable-release/index.html) | Vector KNN search inside SQLite | Brute-force SIMD, fine up to a few hundred thousand vectors; pre-1.0 | Features 6, 12 |

### F. Desktop platform

| Library / API | What it gives Aurora | Maps to |
| --- | --- | --- |
| [`tauri-plugin-single-instance`](https://v2.tauri.app/plugin/single-instance/) | One running process; arguments forwarded to it | Improvement 6 |
| Tauri [`tray-icon`](https://v2.tauri.app/learn/system-tray/), [`autostart`](https://v2.tauri.app/plugin/autostart/), and notification plugins | Tray controls, start with Windows, "intake finished" and "new release" notifications | Features 15, 10, Inbox |
| [`ITaskbarList3`](https://learn.microsoft.com/en-us/windows/win32/shell/taskbar-extensions) through the existing `windows` crate | Taskbar thumbnail buttons and progress | Feature 15 |
| [`notify`](https://github.com/notify-rs/notify) and `notify-debouncer-full` | ReadDirectoryChangesW watcher with rename tracking | Improvement 17 |

### G. Frontend

| Library | What it gives Aurora | Notes | Maps to |
| --- | --- | --- | --- |
| [TanStack Virtual v3](https://tanstack.com/virtual/latest/docs/introduction) | Virtualized lists and grids with dynamic row heights | Supports React 19; use `useFlushSync: false`; the React Compiler lint flags `useVirtualizer` | Improvement 4 |

### H. AI

| Service / library | What it gives Aurora | Notes | Maps to |
| --- | --- | --- | --- |
| [Ollama](https://docs.ollama.com/capabilities/structured-outputs) | Local, private backend constrained by a JSON schema for Tonight's Album and Ask | OpenAI-compatible `/v1` endpoint; a recently reported bug changes JSON key order on that path | Feature 7 |
| [`rmcp`](https://github.com/modelcontextprotocol/rust-sdk) | Official Rust MCP SDK (2026-07-28 specification) | Recent major versions changed the API; check the migration guide | Feature 20 |

### I. Avoid or use with care

- **Spotify Web API audio features and recommendations.** Closed to new applications since late 2024. Analyze audio locally instead (section D).
- **AcousticBrainz.** Shut down in 2022, and its data dumps are static. Use local analysis.
- **Unofficial Musixmatch endpoints.** Their terms and rate limits are problems. Use LRCLIB.

---

## Suggested sequencing

| Phase | Theme | Items |
| --- | --- | --- |
| 1 · Quick wins (days) | Safety and speed | Improvement 6 (single instance), 3 (async covers and cache cap), 10 (one syntax registry, Enter to search), 9 (keyboard), 13 (report periods); Feature 16 (sleep timer) |
| 2 · Listening quality (1–3 weeks) | Sound and flow | Improvement 7 (loudness), 8 (queue) together with Feature 3 (context menus), Improvement 2 (native scheduling and events); Feature 8 (lyrics) |
| 3 · Curation (weeks) | The unrated 86 % | Features 4 (Rating Sprint) and 5 (suggested ratings), 1 (smart playlists), 2 (playlist authoring), 11 (Library Health); Improvement 12 (Universe) |
| 4 · Connected, all opt-in (weeks) | Discovery | Features 9 (scrobbling), 10 (release radar), 18 (wishlist and previews), 17 (setlists), 7 (Ask) |
| 5 · Analysis platform (longer) | One decode pass, many insights | Shared analysis worker → Feature 6 (sonic radio), Feature 12 (fingerprints), BPM/key fields |
| Continuous | Engineering health | Improvements 1 (split `App.tsx`), 19 (tests and CI), 5 (tokens), 14 (locations), 20 (documentation) |

Each phase keeps Aurora's contracts: the shared catalog stays read-only, MP3s remain authoritative and change only through verified transactions, payloads stay bounded, and every network service is optional.

---

## Sources

- ListenBrainz: [Core API](https://listenbrainz.readthedocs.io/en/latest/users/api/core.html) · [Recommendations](https://listenbrainz.readthedocs.io/en/latest/users/api/recommendation.html) · [Miscellaneous / fresh releases](https://listenbrainz.readthedocs.io/en/latest/users/api/misc.html) · [`listenbrainz` crate](https://docs.rs/listenbrainz)
- Last.fm: [Scrobbling 2.0](https://www.last.fm/api/scrobbling) · [track.scrobble](https://www.last.fm/api/show/track.scrobble) · [track.updateNowPlaying](https://www.last.fm/api/show/track.updateNowPlaying)
- AcoustID and Chromaprint: [AcoustID web service](https://acoustid.org/webservice) · [chromaprint-next](https://github.com/attilagyorffy/chromaprint-next) · [rusty-chromaprint](https://github.com/darksv/rusty-chromaprint)
- Lyrics: [LRCLIB API documentation](https://lrclib.net/docs)
- Artwork and metadata: [Cover Art Archive API](https://musicbrainz.org/doc/Cover_Art_Archive/API) · [Wikidata REST API](https://www.wikidata.org/wiki/Wikidata:REST_API) · [Wikidata WikiProject Music](https://www.wikidata.org/wiki/Wikidata:WikiProject_Music) · [TheAudioDB free API](https://www.theaudiodb.com/free_music_api)
- Live music: [setlist.fm artist setlists](https://api.setlist.fm/docs/1.0/resource__1.0_artist__mbid__setlists.html) · [Ticketmaster getting started](https://developer.ticketmaster.com/products-and-docs/apis/getting-started/) · [Discovery API v2](https://developer.ticketmaster.com/products-and-docs/apis/discovery-manual/v2/)
- Previews: [Deezer developer FAQ](https://support.deezer.com/hc/en-gb/articles/360011538897-Deezer-FAQs-For-Developers)
- Audio analysis: [ebur128](https://github.com/sdroege/ebur128) · [ebur128-stream](https://github.com/KitaitiMakoto/ebur128-stream) · [bliss-rs](https://github.com/Polochon-street/bliss-rs) · [stratum-dsp](https://docs.rs/stratum-dsp) · [CLAP in Rust with ort (Roberto Lupi)](https://rlupi.com/i-built-a-music-intelligence-app-in-five-days-with-two-ai-coding-agents) · [Xenova CLAP ONNX export](https://huggingface.co/Xenova/larger_clap_music_and_speech)
- Search and vectors: [Tantivy FuzzyTermQuery](https://docs.rs/tantivy/latest/tantivy/query/struct.FuzzyTermQuery.html) · [sqlite-vec in Rust](https://alexgarcia.xyz/sqlite-vec/rust.html) · [SQLite FTS5](https://www.sqlite.org/fts5.html)
- Tauri and Windows: [Tauri plugins](https://v2.tauri.app/plugin/) · [Single Instance](https://v2.tauri.app/plugin/single-instance/) · [System Tray](https://v2.tauri.app/learn/system-tray/) · [Autostart](https://v2.tauri.app/plugin/autostart/) · [ITaskbarList3::ThumbBarAddButtons](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-itaskbarlist3-thumbbaraddbuttons) · [Taskbar extensions](https://learn.microsoft.com/en-us/windows/win32/shell/taskbar-extensions) · [notify](https://github.com/notify-rs/notify)
- Frontend: [TanStack Virtual](https://tanstack.com/virtual/latest/docs/introduction) · [React adapter notes](https://tanstack.com/virtual/latest/docs/framework/react/react-virtual)
- AI: [Ollama structured outputs](https://docs.ollama.com/capabilities/structured-outputs) · [Official MCP Rust SDK](https://github.com/modelcontextprotocol/rust-sdk) · [rmcp docs](https://docs.rs/rmcp)
- Social: [discord-rich-presence](https://github.com/vionya/discord-rich-presence)
