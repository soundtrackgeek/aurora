# Aurora

Aurora is a fast, local-first Windows 11 explorer and player for a personal music universe. Version 0.24.6 keeps the Inbox extra-track confirmation fully visible above the Auto-Tagger on short screens.

![Aurora design reference](Aurora.png)

## Current 0.24.6 slice

- The unmatched-track recovery confirmation now occupies a dedicated overlay above the complete Auto-Tagger panel. Sticky reconciliation and track headers cannot obscure it, its content scrolls when vertical space is limited, and Escape dismisses only the confirmation instead of closing the Auto-Tagger.

- Album Auto-Tagger canonicalizes dotted and spaced initialisms before reconciling release tracks, so `Lust`, `L.U.S.T`, `L. U. S. T.`, and `L U S T` resolve to the same exact match without weakening unrelated title comparisons.
- When titles really differ because of a typo or provider error, an unresolved release row offers the unmatched local files and a reversible **Confirm match** action. Confirmed pairs receive the release title and track number during tagging and count toward safe extra-track cleanup.

- Album Auto-Tagger uses one vertically scrollable content area for search, release matches, editable metadata, and track reconciliation while keeping its action footer visible. The release table reserves room for at least five results, and responsive grids keep search, field-selection, metadata, and footer text inside the dialog at lower resolutions and Maximum text size.

- Album Auto-Tagger now defaults to **Prefer the original edition**. MusicBrainz results use the release group's earliest date, concrete editions show separate Original and Edition years plus track-count fit, and the selected values write Aurora's distinct Year and Release Year tags correctly.
- Auto-Tagger reconciles files to release tracks by normalized title, duration, and numbering instead of array position. Confidently unmatched local bonus tracks can be selected for removal; Aurora copies and SHA-256 verifies them in app-managed Inbox recovery before changing the album, then rolls tags and files back together if removal or renaming fails.

- A complete album selection in the Albums or Inbox **Tags** editor shows its cover above Album Artist. Clicking the cover opens an image picker in that album's folder; the chosen JPG, PNG, GIF, BMP, or WebP remains a draft until **Save**, which embeds one normalized front cover in every album MP3.
- Cover-only and combined tag/cover saves use the existing verified batch pipelines. Library saves also replace the exact `C:\_code\music_backup_v5\AlbumCovers` file indexed for the album, transcoding only when needed to preserve its indexed image format. Aurora preserves non-front pictures, other unselected frames, and audio bytes; if the archive swap fails, it restores the old archive image and every changed MP3. In Inbox, the cover always applies to the complete album even when tag fields are scoped to selected tracks.

- Opened albums show each track's duration in a compact **Time** column between **Track** and **Rating**.

- Inbox discovers FLAC- and APE-only album folders and blocks Ready until their lossless tracks are converted. **Convert to 320 kbps MP3** writes each MP3 beside its source with FFmpeg, verifies the output format, duration, and 320 kbps bitrate, and only then deletes that source file. Existing same-name MP3s are never overwritten. Aurora finds FFmpeg beside the app, on `PATH`, or at `C:\ffmpeg\bin\ffmpeg.exe`.

- Inbox scans staged MP3 format, size, bitrate, and duration, caches unchanged probes, and blocks intake when audio properties cannot be read. The selected-album inspector shows the resulting aggregate before the move.
- Successful reviewed intake writes the final published files' quality into Music Library's existing Music Doctor tables. The completed historical Music Doctor scan remains the baseline; Aurora owns new additions from this release onward.
- The album sidebar inspector now displays the database-backed format aggregate, including mixed-format albums, without adding density beneath Album Detail covers.

- The tag editor now accepts MusicBee `LOVE RATING`, Release Time, and display-artist user-text values that end with an ID3 null terminator. Valid loved tracks no longer make the containing album fail to open.

- Inbox `Ctrl+R` verification now waits for the asynchronously loaded album selection to make Rename available before exercising the shortcut, eliminating a runner-speed-dependent false failure.

- Available album chart rankings now sit between the track-duration line and rating line on album cards, keeping ratings and scores visible even when all three rankings exist.
- Album chart ranks use the shared United States, United Kingdom, and Norway SVG flags followed by their numeric position. Single chart ranks retain the `BB`, `UK`, `VG`, `TI`, and `NT` abbreviations.

- Albums now show Music Library's bundled SVG origin-country flag beside the Album Artist on each album card, in opened album detail, and in the Album inspector. Track rows remain deliberately unchanged.
- Catalog search now supports `country:` by imported origin-country name or two-letter code, including boolean and inherited alternatives such as `country:norway OR sweden`, exact values, `NOT`, and leading `-` exclusions.

- Album cards and detail show only their available US, UK, and Norway chart ranks. Track rows and the Track inspector show only the available Billboard, Official UK, VG Lista, Ti i Skuddet, and Norsktoppen abbreviations, such as `BB:#37`; missing charts take no space.
- Aurora reads Music Library's materialized rank columns directly, preserving its one canonical rank per album or song instead of recalculating weekly chart history.

- Inbox album intake now retries a bounded number of times when Music Library reports that an apply-time plan became stale. Every retry rebuilds the plan and verifies that the reviewed albums, actions, and destinations are still identical before applying it.

- Album Auto-Tagger gives Genre the same wide field length as Album. Disc total and Disc # override now use compact neighboring inputs on the top metadata row.

- Opening an album immediately uses cached Last.fm evidence, then fills missing album tracks through `track.getInfo` in the background. Aurora ranks positive listener counts, breaks ties by global play count, and marks the top three with 🔥 without writing to Music Library's catalog.

- Listening Report shows small Last.fm portraits for top artists and local album covers for top tracks. The Artists page uses the same portrait component by default and retains its initials fallback when Last.fm has no usable image.
- Metadata Settings accepts a Last.fm API key and shared secret. Aurora keeps both in the operating-system credential vault, uses only the key for unsigned artist lookups, proxies bounded Last.fm images through a local protocol, and caches resized WebP portraits without exposing credentials to React.

- Inbox `Ctrl+R` now coordinates selected sibling discs as one filesystem transaction. `CD1`, `CD2`, and a partially renamed canonical child are flattened into `Album Artist - Album (Year)` with `1-01`, `2-01`, and later disc prefixes, cover files are preserved, empty disc folders are removed, and any failure rolls the staged moves back.

- Global rating and Love shortcuts now project the chosen state into Aurora immediately, then persist MP3 and catalog work through an ordered background queue. Global playback shortcuts deliver their native snapshot directly to the player UI, and the next track is prepared asynchronously so background synchronization cannot leave stale player chrome or impose avoidable decoder startup delay.

- Inbox **Rename from tags** and `Ctrl+R` now rename every selected album, aggregate the renamed track and folder counts, and identify any album that could not be renamed while continuing the remaining selection.

- Inbox destination previews discard only Aurora-owned `.tmp.mp3` tag-staging copies that are at least five minutes old before Music Library scans the selected folder. A recent staging file blocks preview until the edit finishes; recovery backups and ordinary MP3s are preserved.
- Inbox albums cannot become ready for Add to Library until Year is valid, the folder is `Album Artist - Album (Year)`, and track filenames match the tag-derived naming rule. **Rename from tags** fixes the organization and the next scan unlocks intake.
- A same-named destination is reviewed as a replacement only when Album Artist, Album, and Year match. Aurora displays track/rating/Love comparison counts and requires explicit confirmation; Music Library keeps the stable album identity and retains the verified old release in a hidden recovery folder.
- Album detail includes **Move to Inbox**. The destination must be one of Aurora's monitored Inbox folders; Music Library copies and verifies it first, commits the catalog removal through its snapshot importer, and removes the old library folder only after commit.

- The Inbox Tags editor combines tracks from every selected album, shows the complete MP3 and album counts, and saves the shared field changes safely as one native batch per album folder.

- Inbox album rows support plain-click replacement, Ctrl-click toggling, Shift-click ranges, and Ctrl+Shift additive ranges without selecting webpage text. The active album remains clearly marked for the inspector, while selection-wide actions apply to every selected album.

- Genre fields in the vertical Tags editor and Album Auto-Tagger suggest the catalog's existing canonical genres as you type while still allowing a new free-form value.

- Clicking an already-open album cover in Albums closes its track detail, so the cover acts as a direct open/close toggle.

- Tracks, Albums, and Artists automatically load the next bounded 50-item page near the bottom of their result scroller; there is no separate load-more button.
- Background catalog refreshes retain the continuation cursor for every already-loaded page, preventing an earlier alphabetical page from being appended after an open album detail.

- The Tags inspector scrolls vertically only; long values remain contained and horizontally navigable inside their own text inputs without moving the entire sidebar.
- Every Tags editor grid track, including Reset and Save, is capped to the card's available width.

- Long Tags text inputs keep their own horizontal viewport, so arrow-key caret movement and mouse drag-selection reveal the full value without moving the sidebar or hiding its checkboxes.
- Tags Reset and Save actions stay stacked at every application width, keeping the complete Save control reachable inside the narrow inspector.

- Tauri 2, Rust, React, TypeScript, and Vite Windows application.
- Selecting another **Finish what you love** album no longer redirects an open track Tags editor to the album batch. The Tags sidebar stays on the current song until **Play unrated tracks** starts the newly selected album queue.
- History discovers sanitized Tonehavn peer journals from `%APPDATA%\com.local.musiclibrary\tonehavn-history`, validates and atomically backs them up to OneDrive's `_musicbackup\tonehavn-history` directory, and prefers the live local copy over its backup. Registered plays appear as **Tonehavn local** or **Tonehavn iOS** in the existing device dropdown and roll into **All devices** Listening report totals.
- Ratings **Play unrated tracks** now replaces the Tags inspector with the playing unrated track even when an album selection remains stored from the Albums page. Explorer selections still control Tags while their own pages are visible.
- Long Tags values once again autoscroll while they are selected with a left-mouse drag, without allowing the editor to grow beyond the right sidebar. At narrow window widths, Reset and Save stack so their full labels remain visible.
- **Finish what you love** now advances the selected track with the unrated playback queue, keeping the Tags inspector on the song that is actually playing while preserving deliberate album or unrelated track selections.
- Exact **Finish what you love** filters with zero live matches now say so directly and suggest another count or a later Refresh, while continuing to exclude completed ratings and confirmed-missing MP3s.
- Listening Report resolves every distinct catalog track in safe bounded batches, so reports with more than 200 plays retain their complete Year-based decade distribution.
- Listening Rhythm uses a compact 24-hour activity ribbon, and Music by Decade always uses Aurora's **Year** field rather than **Release Year**.
- CI enforces warning-free Rust with Clippy; the Listening Report history ordering uses the preferred keyed descending sort.
- History now opens with a dedicated **Listening report** page for 7, 30, or 90 days and all time. It aggregates every matching session across all available device-history databases—not the 50 visible timeline rows—and reports period comparisons, daily activity, top artists/albums/tracks, listening hour, a personal listening fingerprint, release decades, discovery, and quick facts. The original searchable timeline remains available as the adjacent **History** page.
- A dedicated [search guide](docs/search.md) covers every search surface, field, operator, range, exclusion, exact match, shortcut, limitation, and practical query recipe.
- Library search waits for a two-second pause after typing before filtering, so complete expressions such as `aartist:dolly parton` run once instead of repeatedly filtering partial input. Clearing search remains immediate.
- Music Library folder retries run in their own single-flight loop, continue while Aurora is unfocused, and select the most recently edited folder first. Pending-tag reconciliation remains focus-aware but can no longer starve a newly rated album's companion import.
- Opened albums show every track in the library's natural page scroll instead of trapping the first rows in a second scrollbar. Their compact table replaces the repeated Year column with track number and constrains the delete action so title and per-track Artist use the remaining width.
- Background Music Library imports refresh metadata without replacing the visible Library list: the active view, loaded row order, open album, selected track, and scroll position stay in place while newly discovered items join the loaded results.
- The finite PCM ordering test now waits for its background producer to finish filling before consuming faster than real-time, eliminating a Windows CI race that caused three consecutive otherwise-clean releases to fail verification.
- Top-bar sync, intake, and catalog status messages now use a dedicated line beneath search instead of competing with the search field beside **Add music**.
- **Finish what you love** now has one **Partially rated** shelf for every incomplete album with at least one rating. Filter it to any exact positive number of tracks left, with quick choices for 1, 2, and 3 plus a custom value such as 7.
- Library track rows, album cards, and opened-album track rows follow Windows selection conventions: plain click replaces, Ctrl-click toggles, and Shift-click selects a contiguous range without browser-style text highlighting. Multi-track and multi-album selections flow into the Tags sidebar for verified batch editing of up to 500 MP3 files.
- Album Auto-Tagger replaces Discogs track positions outside the release's track total with their sequential release-order number, so hidden-track positions such as `41` on a 13-track CD become track 13.
- Album Auto-Tagger preserves MusicBrainz artist-credit join phrases such as `featuring`, `&`, and commas when applying Artist metadata and building renamed track filenames.
- The Tags editor cannot horizontally scroll or grow beyond the right sidebar; long values retain native scrolling inside their fixed-width inputs.
- Album Auto-Tagger places Genre in a wide metadata slot matching Album, while Disc total and Disc # override use compact neighboring inputs on the top row with matching visual and keyboard order.
- The **Finish what you love** Refresh button now reacts immediately, spins while its overview and active shelf reload, and prevents duplicate refresh requests until the cycle finishes.
- The Album Auto-Tagger grows up to 1100 px tall when the window allows it, giving release matches and track comparisons more visible room without sacrificing the viewport margin or fixed action footer.
- All-folders Inbox intake re-previews each monitored root immediately before its apply, preventing an earlier folder's catalog update from making the next Music Library plan stale. Aurora compares that fresh plan with the reviewed album and destination list and stops for a new review if the source contents actually changed.
- Ratings completion details reflow the selected album above its tracks at a wider content-pane threshold, so Maximum (+8 px) text remains readable with the right inspector open in smaller windows.
- Every monitored Inbox folder now has an **Add to Library** action, as does **All folders**. A single reviewed dialog can assign General, Scores, or Synthwave independently to each non-empty monitored folder, refuses scopes containing albums with unresolved readiness issues, previews exact destinations, and then delegates moving, embedded-cover archiving, catalog updates, and added-date recording to the existing Music Library bridge.
- Inbox's selected-album inspector now has Album and Tags tabs. The Tags tab reuses Aurora's vertical field-selecting editor for one selected track or a multi-track/whole-album batch, while keeping writes in Inbox's verified, rollback-safe MP3 pipeline and outside the Music Library catalog.
- The Album Auto-Tagger keeps its Rename after tagging, Cancel, and Apply & rename footer visible while release and track results scroll within the available dialog height.
- Whole-album Tags saves immediately project the verified Album, Album Artist, Year, Release Year, Genre, and Publisher values into both the Album card and Album Detail. Partial or mixed track results never overwrite an album summary by guesswork.
- Album covers use five compact text lines: title, artist, `Year — Genre — Publisher`, `track count — album length`, and Album Rating/Score. Album Detail presents Genre and Publisher together with explicit unknown-value fallbacks.
- While an album is open, its card and Album Detail derive a rolling Album Rating and Album Score immediately from the complete track list already on screen. Partial results remain view-local and provisional; global album-rating filters and charts continue to use Music Library-compatible effective ratings.
- A dedicated **Inbox** between Universe and Observatory. Up to ten device-local folders are scanned every 15 seconds and whenever Aurora regains focus; folders containing MP3s become staged albums without entering the Music Library catalog.
- Inbox album rows and the selected-album inspector show the first usable embedded front cover found in track order, serve bounded WebP thumbnails through the local cover protocol, and fall back to the existing disc mark when the album has no usable embedded image.
- Inbox readiness reports missing or inconsistent album identity, track titles, track/disc numbering, genre, publisher context, and embedded front art before promotion. Every MP3 must contain exactly one valid front cover with image bytes matching the rest of the album. The inspector reports embedded coverage and can propagate the displayed cover across every track or let the user choose a JPG, PNG, GIF, BMP, or WebP when no embedded source exists.
- `Ctrl+Shift+T` opens a dense Album Auto-Tagger that searches concrete MusicBrainz and Discogs releases, compares their track lists, allows per-field inclusion and manual Genre/title correction, and applies one verified album batch. Background Inbox rescans preserve the open tagger's selected release and comparison state.
- Auto-Tagger renaming is enabled by default for a full album, and `Ctrl+R` applies the same rename rules to manually tagged Inbox albums. Album folders become `Album Artist - Album (Year)`; tracks become `Disc-01 - Artist - Title.mp3` when a disc tag exists or `01 - Artist - Title.mp3` when it does not. Inbox track checkboxes can scope Auto-Tagger to one CD at a time, with Disc # and total overrides for separate CD1/CD2 releases. Windows-invalid characters and collisions are handled safely, and Discogs vinyl positions such as A1/A2/B1 are normalized to continuous 01/02/03 numbering.
- MusicBrainz requests use Aurora's identifying User-Agent and a process-wide one-request-per-second gate. Discogs accepts either a personal access token or a consumer key plus secret stored only in the operating-system credential vault; saved values are never returned to React or written to Aurora JSON/SQLite state. Debug builds may read `DISCOGS` and `DISCOGS_SECRET` (or `DISCOGS_TOKEN`) from the ignored `.env.local` for local endpoint testing.
- Inbox tag and artwork writes stay outside the catalog workflow: Rust canonicalizes every selected album path, stages same-folder MP3 copies, verifies parsed tags or the exact front-cover digest plus preserved non-cover frames and post-ID3 audio SHA-256, creates safety backups, installs atomically, and restores earlier tracks if a later write fails.
- Album-level **Move to library** and folder-level **Add to Library** reuse Add Music's exact preview/apply bridge and its General, Scores, and Synthwave roots. Music Library remains the sole filesystem mover, cover archiver, and catalog writer; Inbox albums disappear from staging only after that reviewed bridge operation completes.
- Ratings **Play unrated tracks** scopes pending-deletion reconciliation to the selected album instead of rescanning the million-track catalog for every pending Music Library target before playback.
- Rust's strict warning-as-error CI lint passes for the optimized album snapshot path without changing its deletion projection or runtime behavior.
- Album detail reads pending deletion state once, scopes missing-file verification to the selected album, and reuses that result for its rows, rating, Love, duration, and track counts. It never runs Ratings' global deletion scan or opens the state database once per track during an ordinary album open.
- Album detail reads Aurora's local pending-deletion queue before checking an MP3 path, so ordinary album opens avoid synchronous probes across sleeping, remote, or unavailable music drives while queued-and-missing deleted tracks remain hidden.
- Albums detail refreshes its selected track rows after a synchronized Tags save, so the displayed per-track Artist credit matches both the verified MP3 and the tag editor without collapsing the selected album.
- Universe's compact Listening Memory strip shows the last-heard song's exact per-track Artist credit, album title, and small cover image, with historical metadata retained when its live catalog track cannot be resolved.
- Ratings completion counts, **Finish what you love** shelves and details, rating-band totals, and **Play unrated tracks** queues exclude verified-missing MP3s covered by Aurora's durable Music Library synchronization queue. An album whose only unrated track was deleted becomes complete immediately instead of retaining an unplayable card.
- Inline rating, Love, and Release Year updates refresh only those tag fields in the native playback queue, so a later star click cannot restore stale Artist, title, album, or other metadata after a vertical Tags editor save.
- Album and Ratings album-detail track lists show the exact per-track Artist credit as a muted, MusicBee-style suffix beside the title. `DISPLAY ARTIST` overrides remain preferred, so Various Artists compilations identify every performer without sacrificing the compact album layout.
- Album detail supports standard click, Ctrl+click, and Shift+click track selection, a bulk **Delete selected** action, and an explicit permanent-deletion confirmation. Aurora re-resolves every bounded catalog identity before deleting only regular MP3 files, durably queues the exact affected files, and asks Music Library to rescan immediately so its catalog and Updates deletion count reflect the removed tracks. While a failed or locked bridge update remains queued, stale catalog reads cannot restore a verified-missing deleted row, and whole-album Tags safely excludes only that queued missing file.
- A top-bar **Add music** workflow for one already-tagged album folder or a parent containing many album folders. Choose General music, Movie / TV / game music, or Synthwave; preview every unchanged folder name and exact destination in a bounded, keyboard-scrollable plan before one explicit batch apply. Apply closes the modal and continues under a persistent top-bar status so browsing and playback remain available; the intake action stays disabled until completion, and a synchronous guard prevents duplicate requests. Music Library `0.144.5` then runs its cover-archive/embedded-art workflow only for the added albums, writing embedded front art to the configured `AlbumCovers` folder without a manual full-library Cover add.
- A dedicated **Tags** tab in the right inspector for album artwork, Album Artist, Artist, Album, Track Title, Genre, Publisher, Track Rating, Year, Release Year, track number/total, and disc number/total. Album artwork appears only for one complete album selection.
- Player stars, global rating/Love shortcuts, inline edits, inspector saves, and undo no longer queue behind slow Music Library bridge work. Background reconciliation reserves an older projection token before it starts and conditionally updates only the exact pending-overlay revision it inspected, so foreground changes appear promptly and always win stale-result races.
- The playbar and Track inspector prefer the selected track's Artist credit, and the Artist tab follows that credit instead of the album-level Album Artist. Track Publisher and Artist metadata remain available through library snapshots, restored queues, and specialized Genres, Publishers, Years, Ratings, and Charts playback routes.
- Track and album selection semantics model MusicBee's vertical editor: common values are shown once, differing values are labelled **Mixed**, and only checked or edited fields are written. A checked blank value is an explicit clear except for Album Artist, Album, and Track Title, which Music Library requires for safe catalog identity.
- Artist edits MusicBee's `DISPLAY ARTIST` override while preserving the underlying multi-value performer credits; Album Artist retains semicolon-separated multi-value `TPE2` credits.
- Album saves preflight every selected MP3 and its revision before the first write. Aurora then performs verified same-folder atomic writes, rolls back earlier completed files if a later file fails, and retains recovery evidence for ambiguous Windows replacement failures.
- Aurora invokes Music Library `0.144.2` or newer through a versioned, file-based local bridge. Music Library remains the sole filesystem mover and catalog writer; Aurora never opens the shared catalog for writes.
- After a verified inline, inspector, global-shortcut, or undo tag edit, Aurora durably queues the exact MP3 and returns the interaction result without waiting for Music Library. An independent background retry asks Music Library `0.144.2` to scan only that file for an ordinary rating, Love/Ban, or Release Year edit, then applies a guarded album-only transaction; broader identity or text edits and multiple pending files in one album retain the safe complete-folder/full-catalog fallback. Aurora first requires the companion's explicit legacy-`Default` POPM preservation capability, so an older helper leaves the durable update pending instead of rebuilding an album unsafely.
- Folder synchronization and Add Music intake share one bridge coordinator, so preview or apply waits behind active background work instead of racing Music Library's Windows workflow lock. Folder synchronization is token-protected: one invalid old folder cannot poison a new edit, an older receipt cannot erase a newer edit queued for the same folder, and neither a delayed edit response nor external-tag reconciliation can project over newer tag state. Pending overlays are reconciled per live track, so a targeted album import does not hide edits still awaiting synchronization in another album. The independent retry loop selects the newest pending folder every five seconds even while Aurora is unfocused, pauses a folder after three consecutive failures, and resets its retry budget when a later MP3 edit queues new work; the Ratings completion shelf deliberately waits for its explicit Refresh action.
- The top-bar Add Music flow still assumes already-prepared folders. Identification and metadata preparation now live deliberately in Inbox; Aurora does not perform audio-fingerprint matching.
- Native folder selection, strict bridge/category/receipt validation, bounded helper timeouts, and clear update guidance when the installed Music Library does not yet support album intake. Source paths are passed in private request files rather than command-line arguments, and post-edit bridge work never blocks the rating or tag interaction that queued it.
- Truthful batch completion distinguishes fully moved albums from verified catalog copies whose source cleanup needs attention. A successful import triggers Aurora's existing revision check, stable queue rebind, and bounded view refresh immediately.
- A lightweight completed-import revision check every five seconds and whenever Aurora regains focus remains as a fallback. Its opaque completion-order token changes even when imports finish out of ID order. Successful edit and recovery receipts request the same guarded catalog refresh immediately. Queue rebinding and the base view each use one consistent SQLite read snapshot, and Aurora refreshes only after their reported revisions match the detected completed import. Stable normalized path keys keep replaced catalog row IDs from becoming playback identity.
- Catalog refreshes preserve the playing source, current track, and preloaded successor when stable queue order is unchanged. Removed queue entries are dropped; a removed current track stops safely and selects the next surviving entry in a paused state.
- Import-time rating/tag completions update only tag fields on the freshly rebound queue row, selected tracks follow their stable file key, and an unsaved inspector draft remains mounted across transient catalog-ID changes.
- Device-local Windows output selection using stable endpoint IDs, with automatic continuation on the Windows default when the preferred device is missing, cannot open, or disconnects.
- A two-entry, signature-checked encoded-MP3 read-ahead cache loads ordinary current and prepared-next tracks sequentially before the real-time callback can request them. Files above the 96 MiB admission cap or allocation failures retain a 1 MiB buffered-file fallback.
- ReplayGain Off, Track, and Album modes based on MusicBee-compatible `REPLAYGAIN_*` ID3 text frames. Album mode falls back to Track tags, positive gain is capped by the tagged peak, and MP3 files are never modified.
- One stable shared Windows output stream at the endpoint format, with CPAL real-time callback scheduling and a stability-focused 4,096-frame device buffer. Dedicated producer threads decode, apply ReplayGain, and run Rodio's balanced Rubato sinc/FFT resampler into per-track lock-free PCM rings holding at most three seconds; playback pre-fills up to 500 ms and reports observed starvation, device underruns, or denied real-time scheduling in the player output readout.
- Gapless-capable queue transitions: Aurora opens and appends the next resolved MP3 to the same native player during the final 15 seconds, so audio handoff does not wait for React polling. Missing, invalid, or unknown-duration files retain the safe ordinary transition.
- An Audio Settings tab beside Global Shortcuts, atomic per-computer persistence in `%APPDATA%\com.soundtrackgeek.aurora\aurora-audio.json`, and a compact player readout for the active output and applied gain.
- A Windows media session for physical Play/Pause, Stop, Previous, and Next keyboard buttons, with native now-playing metadata and Windows arbitration between active players. This is independent of audio shared/exclusive mode.
- Windows global shortcuts for play/pause, next, whole-star ratings 0–5, and Love. The rating defaults use `Ctrl+Alt+Numpad0` through `Ctrl+Alt+Numpad5` so number-row AltGr characters remain available; playback, next, and Love remain `Ctrl+Alt+P`, `Ctrl+Alt+N`, and `Ctrl+Alt+L`.
- A native Settings editor that captures replacement key combinations, rejects duplicates and modifierless keys, restores defaults, and enables or disables the complete shortcut set.
- A Display Settings tab with global Compact through Maximum text presets, readable minimum sizes, and Compact through Extra Large library-cover presets.
- Independent text and cover overrides for Universe, every Library destination, Observatory, Charts, and History. Views inherit the global choices until explicitly overridden; Charts starts at the larger text preset, and cover controls are disabled where a view has no adjustable artwork.
- Device-local display preferences in versioned browser storage, restored before Aurora renders and kept outside MP3s, the read-only catalog, and shared OneDrive state.
- Shortcut actions always resolve Aurora's now-playing track from the Rust playback runtime. Explore selection never becomes the rating or Love target, and tag shortcuts return after the same verified MP3 plus optimistic Aurora-state update as the player while catalog synchronization continues in the background.
- Device-local shortcut persistence in `%APPDATA%\com.soundtrackgeek.aurora\aurora-shortcuts.json`; these Windows bindings are intentionally excluded from Laptop Mode and OneDrive state synchronization.
- Aurora unregisters all active shortcuts during both window close and application exit. Another running app must still retry or restart if its own registration previously failed while Aurora owned the same binding.
- A persistent left-sidebar cycle with expanded, icon-only, and fully collapsed modes, plus an independently collapsible right inspector. Layout choices stay local to each computer and restore before the first rendered frame.
- A separate device-local **last view** snapshot restores the active destination, explorer mode, exact search and filter expression, sort direction, right-inspector section, and selected album context after quitting and reopening Aurora. Invalid or obsolete snapshots fall back safely to Universe.
- A collapsible Library tree containing Songs, Albums, Artists, Publishers, Genres, Years, Ratings, and Tags. Opening a closed Library enters Songs by default; Library and Playlists disclosure choices persist per computer.
- A dedicated Publisher Signal Timeline with bounded case-insensitive catalog rollups, Release activity, Original-year activity, and Catalog share lenses, publisher search, selected-publisher metrics, decade highlights, publisher playback, and exact handoff into a publisher-filtered Albums collection. Every signal uses its activity years on the same responsive axis, including the final 2026 interval.
- Publisher metadata in Songs, Albums, Genre, Years, Ratings, Publishers, and the right-side track or album inspector. Album-level views read `albums.publisher`; track views retain `tracks.publisher` without schema mutation.
- Offline-safe publisher identity slots use distinctive deterministic Aurora monograms. A selected publisher can use a bounded device-local PNG, JPEG, or WebP override and return to its generated monogram at any time; optional future enrichment retains the documented MusicBrainz → Wikidata → Wikimedia Commons provenance and licensing route.
- Compact Library and pinned-playlist flyouts in icon-only mode, with active nested destinations still visible on the parent Library icon.
- A paired-clock Years explorer that preserves Music Library's distinct Original Year and Release Year fields, with clickable album-level histograms and aggregated edition flows between them.
- Release, Original, and Two Clocks modes; exact missing-year lenses; previous/next year movement; and bounded edition shelves grouped by the counterpart decade.
- A dedicated Album inspector for selected editions, exact Original/Release Year handoff into Songs, and bounded year or album playback without exposing file paths to React.
- Lazy, stale-safe Years queries: overview payloads contain roughly one row per year, year details return at most 100 representative albums, and playback returns at most 100 tracks.
- A dedicated Ratings Studio with separate track and effective-album constellations, clickable whole- and half-star bands, an exact 5 Star Collection, and tall real-cover pyramids with a silver-to-magenta constellation palette.
- Selected albums in Ratings completion details include a **Go to Album** action that opens Albums with that exact album selected, even when it is outside the first album page.
- Almost Complete, Partially Rated, and Unrated Album lanes with mutually exclusive catalog counts, at most 14 album candidates per request, bounded track details, and Play Unrated Tracks.
- Ratings completion details respond to the available content-pane width, keeping the cover and album metadata together above the track list while placing Play Unrated Tracks beneath large cover art.
- Music Library-compatible effective album ratings: explicit MusicBee Album Rating wins; otherwise a rounded normalized track mean becomes valid only after every track is rated. Partial means are labelled provisional and never enter album-rating counts.
- Music Library's exact unbounded Album Score formula, kept numeric rather than converted to stars. Fully track-rated albums show the current score in Ratings and ordinary Album detail; future Charts can rank by the same value without changing its meaning.
- A dedicated Charts page above History with Singles and Albums modes, direct weekly drill-down, named period presets, editable custom week ranges, and one-click full-year charts.
- Historical Official UK, VG Lista, Ti i Skuddet, and Norsktoppen weekly charts plus the catalog's annual Billboard singles and album charts. Unsupported source/type combinations are never presented as data.
- Calculated period charts rank by number of #1 finishes, then #2 finishes, then each lower position in order; chart points and appearances provide deterministic final tie-breaks.
- A first-class Aurora Album Score chart and year shelf reuse Music Library's exact numeric formula without converting it to stars, use `Year` by default, and can switch explicitly to `Release Year`.
- Library-matched chart entries expose real cover art, rating, Love, movement, peak, source history, direct playback, chart-queue playback, and handoff into the ordinary library inspector. Requests and playback queues remain capped at 100 items.
- Instant Ratings Studio star and Love controls reuse the verified MP3 transaction and Aurora overlay while keeping the visible completion candidates stable. The explicit Refresh action reloads the Ratings overview and completion shelf when the rating pass is finished; switching completion lanes still avoids rerunning the full overview.
- Persistent icon-only device mode: a monitor identifies Desktop Mode, a laptop identifies Laptop Mode, and each computer remembers its own choice in `aurora-device.json` outside the shared state database.
- Exact runtime-only drive translation from `D:\MUSIC`, `G:\_BACKUP\SCORES`, and `H:\Synthwave` to `Y:\MUSIC`, `V:\_BACKUP\SCORES`, and `U:\Synthwave`; the catalog and stable track identities remain unchanged.
- Verified SQLite state snapshots at `%USERPROFILE%\OneDrive\_musicbackup\aurora-state.sqlite3`, published at most once per minute and once more on clean shutdown.
- Per-device listening journals in local `aurora-history.sqlite3` databases, mirrored as separately named, validated OneDrive snapshots so Desktop and Laptop sessions can be combined without creating shared-state conflicts.
- A configurable 1–3600 second played threshold, defaulting to 30 seconds. Only observed forward playback counts; seeking does not inflate listening time, a shorter track counts when it naturally finishes, and active listening is checkpointed in 30-second buckets rather than every 10 seconds.
- A bounded History timeline with outcome, device, date, and text filters; registered-play, listening-time, unique-track, skip, and most-played summaries; and direct replay/inspection actions.
- Personal registered plays, listening time, and last-listened time in the selected-track inspector, kept distinct from imported Last.fm popularity.
- First-run laptop recovery copies a valid OneDrive snapshot into Aurora app data before SQLite opens. Newer clean snapshots are also applied only before open, with a retained local safety copy.
- Sync lineage, generations, and logical revisions detect two-computer divergence. Aurora reports a conflict and preserves both files instead of using unsafe newest-file-wins behavior.
- Equivalent OneDrive branches reconcile automatically when only transient catalog IDs, playback position, import-run markers, or retry timestamps differ. Stable queue identity and user-authored tag, journal, playback-setting, and curation differences still block automatic replacement.
- Strictly read-only access to `%APPDATA%\com.local.musiclibrary\music-library.sqlite3`.
- A dedicated Genre Atlas over all canonical catalog genres, with search and sorts for scale, rating, Love, recent listening, unexplored worlds, and name.
- Bounded genre details with representative album covers, `Year` decades, personal listening memory, top albums and artists, shared-artist connections, and editable track highlights.
- Genre Radio, Shuffle, Loved, Highest Rated, Rediscover, and Unrated Expedition actions that load at most 100 tracks per batch, auto-refill below 20 remaining tracks, and never exceed the 200-track queue.
- Bounded startup payload: summary, eight high-volume artists, and 50 five-star tracks.
- Keyset-paged Tracks, Albums, and Artists views that request 50 rows at a time and never hold a million-row result in the WebView.
- The top search reports the exact filtered Songs, Albums, or Artists total even when only the current 50-row page is loaded.
- Songs, Albums, and Artists keep a compact Sort and Reset row while catalog filtering happens through the persistent top search. Songs and Albums can sort by when they were added. Successful Add Music batches record a durable, synchronized album-added timestamp; older albums fall back to the newest catalog track's insertion order. The active sort remains an enabled menu choice, so re-selecting it always reverses newest/oldest or A–Z/Z–A direction even after moving across other choices. Existing collection handoffs can still apply exact rating, Love, year, genre, and artist scopes, and Reset clears them.
- Field-aware search supports `artist:` (Display Artist), `aartist:` (Album Artist display), `album:`, `genre:`, `year:` (Year), `ryear:` (Release Year), `publisher:`, `country:` (Album Artist origin), and `title:`. Country accepts imported names or two-letter codes. Year fields accept exact years and inclusive closed or open ranges such as `year:1985..1987`, `year:1985..`, and `ryear:..1987`. Commas or uppercase `AND` combine groups; uppercase `OR` adds alternatives and inherits the preceding field; `NOT` or a leading `-` excludes a group. A complete quoted value is exact, while unquoted text remains prefix-based. `genre:scores` expands to the Music Library film, TV, animation, anime, and game-score genres.
- Validated sorts for newest, title, artist, album, year, release year, rating, and artist track count; opaque cursors cannot be reused with a different sort.
- Clickable artist planets open an exact artist focus in Songs, while artist results open the artist's Albums by default; both retain an exact artist scope that can be switched between Songs and Albums.
- A functional Constellations artist inspector opened from universe planets, Artist results, the selected track, or the Observatory review queue.
- A bounded, searchable Observatory for candidate-bearing artists, with Needs review, Conflicts, Unconfirmed, Aurora decisions, and All candidates filters.
- Explicit artist candidate confirmation, ignore, and clear actions. Aurora decisions are durable, undoable, and take presentation precedence without hiding disagreements in the imported sources.
- Local release-group curation for linking a visible MusicBrainz group to an album from the same artist, marking it not in scope, ignoring it, or clearing the decision.
- Lazy local MusicBrainz identity resolution with verified, unconfirmed, conflict, ignored, and unmatched states; verified external overlay links and explicit Aurora confirmations are labeled with their exact provenance.
- MBID-gated artist type, active dates, area, birthplace, and origin-country context from the existing catalog import.
- Source-precedence release-group discographies capped at 100 rows: curated overlay first for verified identities, catalog mirror fallback, then the broad cache without mixing stale and refreshed sources.
- Visible local provenance and source availability for the catalog, curated overlay, and broad cache; missing optional databases never block normal library browsing.
- Explicit overlay export creates a new, complete Music Library-compatible SQLite snapshot in Aurora's app-data `exports` folder. Aurora never mutates the live shared overlay; publishing the exported file remains a deliberate user step.
- Album cover grids with a MusicBee-style inline track panel directly beneath the selected cover row. Clicking the same cover or the close control collapses the panel without replacing the grid or losing the browsing position.
- Every album card and expanded detail shows the stored effective Album Rating as five stars with half-star support alongside the numeric Album Score. The currently playing track has its own animated left signal, independent from the selected row used for inspection or editing.
- Bounded album track details retain playback activation, keyboard row navigation, inline tag controls, and a right inspector whose Album, Track, and Artist tabs stay scoped to the selected album.
- A vertical multi-file inspector tag editor plus existing inline half-star rating and Love/Neutral/Ban controls, with read-only duration and optional Last.fm popularity in the Track view.
- Direct Explore-row rating and Love controls: click either half of a star for an exact 0.5 step or click the heart to toggle Love, and Aurora saves to the MP3 immediately with per-row verification feedback.
- Native MP3 playback with play/pause, seek, previous/next, volume, shuffle, and repeat-one/repeat-all controls.
- A real MP3-derived, purple-to-cyan waveform timeline. Native builds decode every audio frame into a 640-peak whole-song overview, cache it in device-local `aurora-waveforms.sqlite3`, and never accept an arbitrary WebView path. The continuous envelope distinguishes played from upcoming audio and remains directly seekable. Cache misses use one cancellable sequential decode slot, so rapid skips do not multiply competing analysis jobs; older 320-peak sparse cache entries are discarded automatically.
- Race-safe seeking and a local player clock: the exact released range value is committed, older overlapping seek responses cannot replace newer state, and the progress line continues at 250 ms cadence between two-second native snapshots.
- Bottom-player half-star rating (including clear-to-unrated) and Love controls that reuse Aurora's verified instant MP3 tag-write and optimistic state-overlay workflow.
- A clickable end-time readout that toggles between total duration and a live negative remaining-time display.
- A bounded 200-track queue with play-now, reorder, remove, and clear actions.
- Durable queue, current track, position, volume, shuffle, and repeat state in Aurora's own SQLite database.
- Stable queue identity based on the normalized MP3 path, verified alongside every transient track ID so queue items survive Music Library catalog imports without being retargeted.
- Transactional same-folder MP3 writes using standard ID3 text/position frames, MusicBee's exact POPM byte map, legacy whole-star `Default` POPM fallback, `LOVE RATING`, Release Time, and existing `DISPLAY ARTIST` conventions. A rating edit replaces either recognized owner with one canonical `MusicBee` frame while preserving unrelated POPM owners.
- Conflict detection, post-write tag/audio verification, Windows atomic replacement, and crash recovery. Original backups remain available while a single-file or album operation is in progress, then are deleted after the complete operation verifies; ambiguous or failed recovery files remain untouched for manual recovery.
- Aurora-owned tag overlays update rating, Love, and Release Year immediately while the verified save asks Music Library to reimport the existing album folder. The normal catalog-revision watcher then refreshes every affected view.
- Focus-time reconciliation remains as a recovery path for pending overlays: it reads only bounded pending MP3s, treats their tags as authoritative, clears caught-up overlays, and rotates unavailable files so they cannot starve later work.
- Half-star track reconciliation reads Music Library's raw rating when its older normalized field is null; removed-track overlays are excluded from library totals.
- Album covers served through a narrow Rust protocol that resolves exact album IDs, contains canonical paths to the configured archive, rejects oversized sources, and caches 64–512 px WebP thumbnails.
- Packaged-app update checks at startup and every 60 seconds, with an Aurora-styled install prompt.
- Windows NSIS release workflow with mandatory Tauri updater signatures.

The MP3 is authoritative for Aurora tag edits. Aurora never writes the shared Music Library SQLite database; the companion performs a guarded existing-folder reimport after verified saves, while Aurora's private overlay covers the short interval before the catalog catches up. Aurora's MusicBrainz decisions are stored in Aurora's app-owned database, but they are independent of MP3 tags and the imported catalog. Listening events deliberately use a separate per-device database instead of the single shared state snapshot. All OneDrive copies are consistent SQLite snapshots rather than copies of live WAL-backed files. See [docs/audio-output-contract.md](docs/audio-output-contract.md), [docs/charts-contract.md](docs/charts-contract.md), [docs/genre-atlas-contract.md](docs/genre-atlas-contract.md), [docs/global-shortcuts-contract.md](docs/global-shortcuts-contract.md), [docs/inbox-contract.md](docs/inbox-contract.md), [docs/listening-history-contract.md](docs/listening-history-contract.md), [docs/laptop-mode-contract.md](docs/laptop-mode-contract.md), [docs/publisher-logos.md](docs/publisher-logos.md), [docs/ratings-studio-contract.md](docs/ratings-studio-contract.md), [docs/sidebar-navigation-contract.md](docs/sidebar-navigation-contract.md), [docs/tag-editing-contract.md](docs/tag-editing-contract.md), [docs/musicbee-tags.md](docs/musicbee-tags.md), [docs/playback-contract.md](docs/playback-contract.md), and [docs/years-explorer-contract.md](docs/years-explorer-contract.md).

## Data model

The primary catalog currently contains 1,096,288 MP3 tracks, 72,012 albums, and approximately 20,000 album artists across 687 canonical genres. Of those albums, 12,434 have an effective album rating, 678 need only 1–3 more track ratings, 5,723 are partially rated, and 59,578 are unrated. The live full Ratings overview, including overlay-aware album recalculation and bounded shelves, measured about 1.7 seconds; completion-lane changes reuse that overview. Aurora opens the active WAL-backed database with SQLite read-only flags and `query_only`; it does not use immutable mode and does not write ratings back into this imported catalog. Common bounded explorer paths remain approximately 26–84 ms including SQLite process startup. Global title A–Z is the known slower path at approximately 120 ms because the shared catalog has no title-only index.

The broad MusicBrainz cache and curated overlay are deferred from startup and opened independently only when the Artist inspector or Observatory requests context. The audited cache contains 20,208 artist-name rows and 483,675 release groups; the curated overlay contains 493 verified artist links and 9,658 release groups. Cache-only identities remain unconfirmed because 44 audited exact-name candidates conflict with verified links and many MBIDs are shared by multiple names. Observatory pages are capped at 100 rows and intentionally cover candidate-bearing artists already present in the imported MusicBrainz artist-info table; they are not a claim to enumerate all 20,392 catalog artists. See [docs/database-contract.md](docs/database-contract.md) for verified responsibilities, limits, and authority rules.

The album-cover archive at `C:\_code\music_backup_v5\AlbumCovers` contains 76,329 images and maps to current albums through `album_covers.album_id` with 98.09% coverage. Rust now resolves and decodes those images outside the WebView; missing or invalid art falls back to Aurora's generated artwork.

## Requirements

- Windows 11 with WebView2
- Node.js 22+
- Rust stable with the MSVC target and Windows C++ build tools
- The music catalog at the default `%APPDATA%` path above
- Music Library `0.144.2` or newer installed at `%LOCALAPPDATA%\Music Library\music-library.exe` for album intake and background post-edit catalog synchronization
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
