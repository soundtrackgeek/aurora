# Changelog

All notable Aurora changes are recorded here.

## [Unreleased]

## [0.24.9] - 2026-08-31

### Fixed

- Kept the Artists page responsive while uncached Last.fm portraits are fetched, decoded, and cached. Artist-image protocol requests now resolve asynchronously instead of blocking WebView resource handling during searches and artist selection.

## [0.24.8] - 2026-08-31

### Fixed

- Replaced the Artists page's hard-coded empty history state with registered-play totals and latest-play timestamps aggregated across Aurora's available device histories.

## [0.24.7] - 2026-08-31

### Fixed

- Prevented Windows WebView2 from locking Aurora when a populated top-bar search first receives focus. Pointer clicks and `Ctrl+K` now focus an empty native control before restoring the controlled query and caret on the next frame.

## [0.24.6] - 2026-08-30

### Fixed

- Kept the unmatched-track recovery confirmation centered and fully visible above sticky Auto-Tagger table headers on short screens. The confirmation now has a bounded scrolling overlay, and Escape returns to release review without closing the Auto-Tagger.

## [0.24.5] - 2026-08-30

### Added

- Added a reversible **Confirm match** override for misspellings and provider errors. Each unresolved release track can be assigned to one unmatched local file; Aurora then uses the release title and track position for tagging and treats the user-confirmed pair as safe reconciliation evidence.

### Fixed

- Album Auto-Tagger now treats dotted and spaced initialisms such as `L.U.S.T`, `L. U. S. T.`, and `L U S T` as the same title as `Lust`, preventing an obvious match from appearing as one missing and one extra track.

## [0.24.4] - 2026-08-30

### Changed

- Made the Album Auto-Tagger's complete working content vertically scrollable behind its fixed action footer, enlarged release matches to show at least five rows, and constrained responsive search, metadata, field-selection, track, and footer layouts so Maximum text and lower-resolution windows no longer clip the right edge.

## [0.24.3] - 2026-08-30

### Added

- Added a default-on **Prefer the original edition** Auto-Tagger search mode with separate Original and Edition years, visible release track counts, and original-release prioritization that no longer hides a canonical release merely because the Inbox folder contains bonus tracks.
- Added one-to-one track reconciliation for exact, likely, extra, missing, and ambiguous tracks. Automatic extra-track removal is available only after every release track has a confident match and a dedicated confirmation names the affected MP3s.
- Added verified Inbox recovery for removed bonus tracks. Aurora copies each selected extra outside monitored folders, verifies the complete file hash, applies tags, removes extras, and renames as one rollback-aware workflow while retaining successful recovery copies.

### Fixed

- Auto-Tagger now writes the release group's earliest date to Aurora's Original Year and the selected concrete edition date to Release Year instead of collapsing the edition date into Year.

## [0.24.2] - 2026-08-30

### Changed

- Library cover replacements now also replace the exact `C:\_code\music_backup_v5\AlbumCovers` file mapped by `album_covers.album_id`. Aurora stages and validates a format-compatible archive image, installs it only after every MP3 verifies, and restores both the old archive image and the MP3 batch if the archive replacement fails.

## [0.24.1] - 2026-08-30

### Changed

- Open album cards and Album Detail now project rolling Album Rating and Album Score values immediately from the visible complete track list when a track rating or Love changes, without waiting for tag import, cover work, or catalog refresh. Partial values stay provisional and do not enter global album-rating filters or charts.

## [0.24.0] - 2026-08-30

### Added

- Added album-cover replacement to the Albums and Inbox **Tags** editors. Clicking the cover opens the native image picker in the album folder and stages a preview until the shared **Save** action embeds it in every album MP3.
- Added cover-only and combined tag/cover transactions with bounded image validation, exact post-write artwork checks, preserved non-front pictures and audio bytes, complete-album rollback, and artwork-aware startup recovery.
- Added a contained selected-artwork preview route backed by short-lived opaque tokens rather than frontend filesystem paths.

### Changed

- Inbox cover changes now apply to every MP3 in the selected album even when ordinary tag edits are scoped to only some tracks.

## [0.23.13] - 2026-08-30

### Added

- Opened albums now show each track's duration in a **Time** column between **Track** and **Rating**.

## [0.23.12] - 2026-08-29

### Added

- Inbox now discovers FLAC and APE album folders and offers one fixed **Convert to 320 kbps MP3** action before tagging or intake.
- Each conversion uses a same-folder temporary MP3, preserves source metadata and attached artwork through FFmpeg, verifies a readable 320 kbps output with duration, installs it without overwriting an existing MP3, and deletes the source only after verification succeeds.

## [0.23.11] - 2026-08-29

### Added

- Inbox scans each staged MP3 for format, size, bitrate, and duration, caches unchanged results between refreshes, reports unreadable audio as a readiness issue, and shows the album aggregate in the inspector.
- Album inspectors now show the real format aggregate already stored in Music Library's shared quality tables.

### Changed

- Aurora Inbox intake now completes the Music Doctor workflow for new releases by validating published MP3 audio and persisting its quality data during the reviewed Music Library catalog transaction.

## [0.23.10] - 2026-08-29

### Fixed

- Accepted MusicBee `LOVE RATING` and other compatible user-text values with a trailing ID3 null terminator, so albums containing those valid frames open in the tag editor instead of reporting an unsupported Love value.

## [0.23.9] - 2026-08-29

### Added

- Made embedded front-cover consistency part of Inbox readiness. Every MP3 must contain exactly one valid, identical front cover before album- or folder-level intake can proceed.
- Added an Inbox artwork repair action that reuses the first usable embedded album image or a user-selected JPG, PNG, GIF, BMP, or WebP, stages same-folder MP3 copies, preserves non-cover frames and audio bytes, verifies every installed cover, and rolls the complete album back if any replacement fails.
- Added per-album embedded-artwork coverage to the Inbox inspector and made Inbox thumbnails prefer the standard front-cover frame.

## [0.23.8] - 2026-08-29

### Fixed

- Stabilized the Inbox `Ctrl+R` rename verification by waiting for the asynchronously loaded selection to enable Rename before sending the shortcut, preventing runner-speed-dependent CI failures.

## [0.23.7] - 2026-08-29

### Fixed

- Moved available album chart rankings onto their own card row between duration and rating so dense chart data cannot hide the album rating or score.
- Replaced `US`, `UK`, and `NO` album-chart abbreviations with the shared bundled country SVG flags while retaining compact numeric ranks and leaving single-chart abbreviations unchanged.

## [0.23.6] - 2026-08-29

### Added

- Added Music Library's bundled SVG origin-country flags beside Album Artist in album cards, opened album detail, and the Album inspector without adding flags to individual track rows.
- Added `country:` catalog search by imported origin-country name or two-letter code, with exact matching, exclusions, and inherited boolean alternatives such as `country:norway OR sweden` across Songs, Albums, and Artists.

## [0.23.5] - 2026-08-29

### Added

- Added Music Library's materialized album chart ranks to album cards, Album Detail, and the Album inspector, using compact `US:#14`, `UK:#4`, and `NO:#1` labels only when the corresponding rank exists.
- Added materialized single chart ranks to Library and opened-album track rows plus the Track inspector. Aurora shows only available `BB`, `UK`, `VG`, `TI`, and `NT` values and never fills missing charts with placeholders.

## [0.23.4] - 2026-08-29

### Fixed

- Fixed Inbox album intake failing when a pending Music Library catalog synchronization landed between Aurora's apply-time preview and apply request. Aurora now retries only `stalePlan` failures, revalidating the exact reviewed albums and destinations before each bounded retry.

## [0.23.3] - 2026-08-29

### Fixed

- Reworked Album Auto-Tagger metadata layout so Genre uses the same wide editor width as Album, while the compact Disc total and Disc # override inputs sit together on the top row. Named grid areas now keep the layout stable when hidden suggestion elements are present.

## [0.23.2] - 2026-08-29

### Fixed

- Album detail no longer treats Music Library's sparse artist-popularity snapshot as a complete album ranking. Aurora now reuses read-only catalog evidence, refreshes missing tracks from Last.fm in the background, caches results in device-local state, ranks by listeners then global play count, and consistently marks the top three tracks.

## [0.23.1] - 2026-08-29

### Added

- Album detail now marks the three tracks with the highest available global Last.fm popularity using a small 🔥 indicator. Tracks without imported Last.fm play counts remain unmarked.

## [0.23.0] - 2026-08-28

### Added

- Added cached Last.fm artist portraits to Listening Report and the Artists page, with initials retained as the safe fallback.
- Added album cover images to Listening Report's top track rows.
- Added Last.fm API key and shared-secret management to Metadata Settings. Credentials stay in the operating-system vault, while artist images are downloaded, validated, resized, and served through Aurora's local image protocol.

## [0.22.1] - 2026-08-28

### Fixed

- Fixed multi-disc Inbox renames competing for the same destination and nesting one renamed disc inside the old release folder. Selected sibling discs now merge atomically into one flat `Album Artist - Album (Year)` folder with disc-prefixed track filenames, including recovery from the partially renamed `CD1` plus canonical-child state.

## [0.22.0] - 2026-08-28

### Changed

- Global rating and Love shortcuts now update Aurora optimistically at keypress time and persist their ordered MP3 edits in the background, so slow hashing, drive flushes, and Music Library synchronization no longer delay visible feedback.
- Aurora prepares the next playback source asynchronously from the start of the current track and applies global playback shortcut snapshots directly to the player UI, eliminating the two-second stale-title polling window and using the prepared source for immediate next-track transitions.

## [0.21.1] - 2026-08-28

### Fixed

- Fixed Inbox **Rename from tags** and `Ctrl+R` processing only the active album. Rename now processes every selected album, reports aggregate track/folder results, and continues past an individual album failure while identifying it.

## [0.21.0] - 2026-08-28

### Added

- Added one reviewed workflow for replacing an existing same-named release. Aurora shows old/new track counts, matched tracks, and existing rated/loved counts, requires explicit confirmation, preserves the album identity, and reports Music Library's retained recovery folder.
- Added **Move to Inbox** to album detail. Aurora targets only a configured monitored Inbox folder, previews the exact catalog removal, and delegates the verified copy, commit, and post-commit source cleanup to Music Library.

### Changed

- Inbox readiness now requires a valid year plus the canonical `Album Artist - Album (Year)` folder and tag-derived track filenames. **Rename from tags** remains the one-step repair action before Add to Library becomes available.

## [0.20.10] - 2026-08-28

### Fixed

- Inbox destination previews now remove abandoned Aurora `.tmp.mp3` tag-staging copies older than five minutes before handing the selected folder to Music Library. Recent staging files block preview instead of racing an active edit, while recovery backups and ordinary MP3s remain untouched.

## [0.20.9] - 2026-08-28

### Fixed

- Fixed the Inbox Tags editor so a multi-album selection includes every selected album's tracks and saves the complete edit as per-album native tag batches instead of editing only the active album.

## [0.20.8] - 2026-08-28

### Added

- Added Windows-style plain, Ctrl, Shift, and Ctrl+Shift album selection to Inbox, with an explicit selected count and active inspector row.

### Fixed

- Prevented Shift-clicking Inbox albums from selecting page text.

## [0.20.7] - 2026-08-27

### Added

- Added lightweight, catalog-backed genre suggestions to the vertical Tags editor and Album Auto-Tagger while preserving free-form genre entry.

## [0.20.6] - 2026-08-27

### Changed

- Made album covers in Albums toggle their track detail closed when the open cover is clicked again.

## [0.20.5] - 2026-08-27

### Fixed

- Preserved the true continuation cursor when a background catalog refresh updates only the first Explorer page, so opening an album and later loading more results cannot append an earlier alphabetical page.

### Changed

- Replaced the Tracks, Albums, and Artists **Load 50 more** button with automatic 50-item lazy loading near the bottom of the result scroller.

## [0.20.4] - 2026-08-27

### Fixed

- Discovered Tonehavn's live local listening-history snapshots from the Music Library AppData directory instead of requiring Tonehavn to publish directly into OneDrive.

### Added

- Added validated atomic backups of the two known Tonehavn device snapshots under OneDrive's `_musicbackup\tonehavn-history` directory, while preferring local journals to prevent duplicate report rows.

## [0.20.3] - 2026-08-27

### Fixed

- Constrained every Tags editor grid track to the inspector's available width, so long Save labels and metadata values cannot enlarge the card.
- Locked the Tags inspector to vertical scrolling while preserving each text input's native horizontal viewport, preventing the entire sidebar from sliding right.

## [0.20.2] - 2026-08-27

### Fixed

- Kept long Tags text inputs on their native horizontal viewport so keyboard caret movement and mouse drag-selection can reveal the full value without shifting or clipping the sidebar.
- Stacked the Tags Reset and Save actions at every sidebar width, keeping the complete Save control reachable in wide application windows with a narrow inspector.

## [0.20.1] - 2026-08-27

### Fixed

- Kept an explicit track Tags context unchanged while browsing other **Finish what you love** albums, preventing album tag-read warnings from replacing the current song before **Play unrated tracks** is pressed.

## [0.20.0] - 2026-08-27

### Added

- Added Tonehavn as a first-class listening-history source. Aurora discovers the sanitized `Tonehavn local` and `Tonehavn iOS` device snapshots, includes their registered plays in Listening report totals, and exposes each source in the existing device dropdown.

## [0.19.7] - 2026-08-27

### Fixed

- Scoped Songs, Albums, Artists, and Tags explorer selections to their visible pages, so a stored Albums selection can no longer override the track started by Ratings **Play unrated tracks** in the Tags inspector.

## [0.19.6] - 2026-08-27

### Fixed

- Restored native mouse drag-selection autoscrolling inside long Tags inputs by moving sidebar containment into a non-scrolling inner clip instead of clipping the input's editor container.
- Stacked the Tags Reset and Save actions at narrow application widths so long album-batch labels remain fully visible inside the sidebar.

## [0.19.5] - 2026-08-27

### Fixed

- **Finish what you love** playback now advances the selected track with the unrated queue, so the Tags inspector changes to the second unrated song instead of remaining on the first. Deliberate album or unrelated track selections remain unchanged.

## [0.19.4] - 2026-08-27

### Fixed

- **Finish what you love** now explains when an exact tracks-left filter has no albums in Aurora's live rating and available-file state, instead of showing unexplained blank space that makes Refresh appear broken.

## [0.19.3] - 2026-08-26

### Fixed

- Listening Report now deduplicates catalog references and resolves them in safe 200-track batches, preventing reports with more than 200 plays from collapsing Music by Decade into a single **Unknown** row.
- Music by Decade remains strictly based on **Year**; the fix does not fall back to **Release Year**.

## [0.19.2] - 2026-08-26

### Changed

- Replaced Listening Rhythm's radial clock with a clearer 24-hour segmented activity ribbon that highlights the busiest hour while preserving every hourly value.

### Fixed

- Music by Decade now always groups listening history by Aurora's **Year** field instead of **Release Year**, including the browser preview path.

## [0.19.1] - 2026-08-26

### Fixed

- Updated Listening Report history ordering to use Clippy's preferred keyed descending sort, restoring the warning-free Rust CI gate.

## [0.19.0] - 2026-08-26

### Added

- Added a dedicated **Listening report** page inside History with 7-, 30-, and 90-day periods plus all-time reporting, previous-period navigation, device filtering, comparative activity bars, top artists/albums/tracks, a 24-hour listening clock, listening fingerprint, release-decade distribution, discovery ratios, and quick facts.
- Added a non-paginated Rust reporting command that aggregates the complete matching listening record across every available device-history database. The existing 50-row History page limit affects only the visible timeline and never report totals.

### Changed

- History now opens on Listening report while keeping the original searchable, filterable session timeline as a peer page.

## [0.18.23] - 2026-08-26

### Documentation

- Added a dedicated search guide covering Aurora's catalog query language, destination-specific searches, fields, boolean operators, exclusions, exact values, year ranges, the Scores umbrella, shortcuts, examples, limitations, and troubleshooting.

## [0.18.22] - 2026-08-26

### Changed

- Library search now waits for a two-second pause after typing before filtering tracks, albums, or artists, while clearing the search remains immediate.

## [0.18.21] - 2026-08-26

### Fixed

- Music Library folder retries now run independently from slower pending-tag reconciliation and continue while Aurora is unfocused, so a large overlay backlog cannot leave newly rated albums at **Pending tag import** without attempting the companion update.
- Automatic synchronization prioritizes the most recently edited folder while retaining the existing three-attempt safety limit and durable retry queue.

## [0.18.20] - 2026-08-26

### Changed

- Expanded opened-album track lists into the main library scroll, replacing the nested five-row scrollbar with one natural page scroll.
- Replaced the redundant album-detail Year column with track number and tightened the delete column so long track titles and per-track Artist credits use the remaining width.
- Preserved the active Library list, loaded order, open album, selected track, and scroll position across automatic Music Library catalog refreshes instead of replacing the view with a loading state.

### Fixed

- Made the finite PCM buffer ordering test wait for producer completion before its artificial faster-than-real-time drain, removing the Windows CI underrun race behind three consecutive failed release runs.

## [0.18.19] - 2026-08-26

### Changed

- Moved top-bar sync, intake, and catalog status messages to a dedicated line beneath search so they no longer reduce the space available to the search field.

## [0.18.18] - 2026-08-26

### Changed

- Consolidated **Finish what you love** into one **Partially rated** shelf for every incomplete album with at least one rating, with exact remaining-track filters for 1, 2, 3, or any custom positive count such as 7.

## [0.18.17] - 2026-08-26

### Changed

- Added Windows-style selection to Library track rows, album cards, and opened-album track rows: a plain click replaces the selection, Ctrl-click toggles one item, and Shift-click selects an inclusive range without selecting page text.
- Connected multi-track and multi-album selections to the Tags sidebar so a selected batch can be edited together, with the existing verified batch writer limiting a tag operation to 500 MP3 files.

## [0.18.16] - 2026-08-26

### Fixed

- Normalized Discogs track positions outside the release's track total to sequential release order in Album Auto-Tagger, so values such as track `41` on a 13-track CD are applied as track 13 instead of failing validation.

## [0.18.15] - 2026-08-26

### Fixed

- Preserved MusicBrainz artist-credit join phrases in Album Auto-Tagger metadata and renamed track filenames, including credits such as `X‐Ecutioners featuring Large Professor` instead of replacing `featuring` with a semicolon.

## [0.18.14] - 2026-08-26

### Fixed

- Kept the Tags editor within the right sidebar when long values are selected or scrolled, while preserving native horizontal scrolling inside each fixed-width input.

## [0.18.13] - 2026-08-26

### Changed

- Reorganized Album Auto-Tagger metadata fields so Genre uses the wide lower-left editor slot and Disc total uses the compact lower-right numeric slot, including an intentional two-column arrangement for narrower windows.

## [0.18.12] - 2026-08-26

### Changed

- The Ratings **Finish what you love** Refresh button now gives immediate pressed feedback, disables itself, changes to **Refreshing…**, and spins its icon until both the overview and active completion shelf finish reloading.

## [0.18.11] - 2026-08-26

### Changed

- Increased the Album Auto-Tagger's maximum height from 720 px to 1100 px so release matches and track comparisons use more of tall application windows while retaining the existing viewport margin and fixed footer.

## [0.18.10] - 2026-08-26

### Fixed

- Fixed multi-folder Inbox intake failing with Music Library's `stalePlan` response. Aurora now re-previews each monitored root immediately before applying it, after any earlier root has updated the catalog, and refuses the apply if that fresh plan no longer matches the destinations and albums the user reviewed.

## [0.18.9] - 2026-08-26

### Fixed

- Ratings completion details now place album information above the track list sooner in narrow content panes, preventing maximum-size text from crowding the tracks when the right inspector is open.

## [0.18.8] - 2026-08-26

### Added

- Added an **Add to Library** action to every monitored Inbox folder and to **All folders**. The reviewed batch dialog assigns a General, Scores, or Synthwave root per monitored folder, blocks folders containing unready albums, previews every destination, then uses Music Library's existing mover, embedded-cover archive, catalog update, and Aurora added-date workflow.

## [0.18.7] - 2026-08-25

### Added

- Added an Album/Tags switch to the Inbox inspector. The Tags view exposes the same vertical manual editor used by Albums, scopes edits through the existing track checkboxes for one track or an entire album, and saves through Inbox's verified rollback-safe writer without cataloging staged files.

## [0.18.6] - 2026-08-25

### Fixed

- Kept the Album Auto-Tagger footer inside the visible dialog so Rename after tagging, Cancel, and Apply & rename remain reachable regardless of optional provider notices or search warnings.

## [0.18.5] - 2026-08-25

### Fixed

- Kept the Album Auto-Tagger's selected release and track comparison stable during periodic or focus-triggered Inbox rescans, eliminating the repeated loading flash and alternating match indicators.

## [0.18.4] - 2026-08-25

### Fixed

- Whole-album tag edits now refresh Album and Album Detail metadata immediately from the complete verified MP3 result, including Album, Album Artist, Year, Release Year, Genre, and Publisher. Partial or mixed track results leave existing album-level values intact.

## [0.18.3] - 2026-08-25

### Changed

- Album covers now use five compact text lines, adding `Year — Genre — Publisher` and `track count — album length` above Album Rating/Score. Album Detail also shows `Genre — Publisher` beneath the artist, with clear unknown-value fallbacks.

## [0.18.2] - 2026-08-25

### Added

- Inbox album rows and the selected-album inspector now show embedded cover art from the first sorted track only. Aurora validates the MP3 beneath a monitored root, decodes and bounds the embedded image natively, caches WebP thumbnails, and retains the existing disc fallback when track 1 has no usable art.

## [0.18.1] - 2026-08-25

### Added

- Added default-on folder and track renaming to Inbox Auto-Tagger plus `Ctrl+R` renaming for manually tagged albums. Renames stay within the album's current parent folder, use `Album Artist - Album (Year)` and optional-disc/two-digit-track filenames, reject collisions, and roll back partial file renames.
- Discogs vinyl positions such as A1, A2, and B1 now become continuous numeric track numbers instead of duplicate side-local numbers.
- Inbox albums now expose per-track selection for `Ctrl+Shift+T`. Separate CD1/CD2 provider matches can tag only their selected tracks, restart track numbering at 01, and override Disc # / Disc total while retaining one album folder.

## [0.18.0] - 2026-08-25

### Added

- Added **Inbox** between Universe and Observatory as a catalog-external staging surface for up to ten monitored folders, with bounded recursive MP3 scanning, 15-second and focus refreshes, album readiness issues, and device-local persistence.
- Added `Ctrl+Shift+T` **Album Auto-Tagger** search across MusicBrainz and Discogs releases, editable album/track metadata, per-field write intent, provider track-list comparison, and verified batch application.
- Added OS-credential-vault Discogs settings for either a personal token or consumer key plus secret. MusicBrainz networking identifies Aurora and observes the service's one-request-per-second limit.
- Inbox metadata writes now stage same-folder copies, preserve and verify the MP3 audio payload, retain rollback backups until the complete batch succeeds, and restore earlier files when a later install fails.
- Inbox promotion reuses Add Music's reviewed Music Library preview/apply bridge for General, Scores, and Synthwave so staged albums remain outside the catalog until the selected move succeeds.

## [0.17.32] - 2026-08-25

### Added

- Albums can now be sorted by **Added · newest** or **Added · oldest**. Successful Add Music batches persist a synchronized album-added timestamp, while older albums fall back to their newest catalog track's insertion order.

## [0.17.31] - 2026-08-25

### Fixed

- **Play unrated tracks** in Ratings now checks pending deletions only for the selected album, removing the multi-second full-catalog rescan before playback while keeping queued missing tracks out of the queue.

## [0.17.30] - 2026-08-25

### Fixed

- Removed a redundant borrow from the optimized album snapshot path so Aurora's strict Rust lint passes and the Windows release can proceed without changing album loading behavior.

## [0.17.29] - 2026-08-25

### Fixed

- Album detail no longer invokes Ratings' global pending-deletion scan or opens Aurora's state database once per track. It loads the durable deletion queue once, scopes missing-file checks to the selected album, and reuses those keys for both visible rows and live album counts, removing the remaining five-second delay without restoring deleted tracks.

## [0.17.28] - 2026-08-25

### Fixed

- Verified rating, Love, Release Year, and full Tags editor operations now delete Aurora's same-folder `.original.backup` files as soon as the complete single-track or album operation succeeds. Album backups remain available until every track verifies so mid-batch failures can still roll back safely, and startup removes completed backups left by older Aurora versions.

## [0.17.27] - 2026-08-25

### Fixed

- Opening an album no longer probes every MP3 on the music drive before showing its tracks. Aurora checks its local pending-deletion queue first and touches the filesystem only for a queued row, preserving deleted-track filtering without delaying ordinary album details on sleeping, remote, or unavailable drives.

## [0.17.26] - 2026-08-25

### Fixed

- Albums detail now preserves and refreshes the selected album after a synchronized Tags save, so a changed per-track Artist credit appears in the track row instead of leaving the pre-edit artist visible. A superseded optimistic projection also triggers the authoritative catalog refresh rather than updating only the tag editor.

## [0.17.25] - 2026-08-25

### Added

- Universe's **Last heard** summary now shows a small album cover and the album title alongside the song, and prefers the track's exact Artist credit over its Album Artist while retaining historical metadata when the catalog track is unavailable.

## [0.17.24] - 2026-08-25

### Fixed

- Ratings completion now removes queued-and-missing deleted MP3s from its counts, **Finish what you love** shelves and album details, track-rating totals, and playback queues. Albums whose only unrated track was deleted become complete immediately, and **Play unrated tracks** can no longer enqueue that unavailable catalog row while Music Library synchronization is pending.

## [0.17.23] - 2026-08-25

### Fixed

- Rating, Love, and Release Year changes no longer restore stale Artist, title, album, or other editable metadata in the player after a vertical Tags editor save.

## [0.17.22] - 2026-08-25

### Added

- Album detail and Ratings completion now show each track's exact Artist credit beside its title, including `DISPLAY ARTIST` overrides on Various Artists releases.

## [0.17.21] - 2026-08-25

### Fixed

- A successfully deleted album track no longer reappears when an in-flight album or catalog response still contains the stale Music Library row. Aurora keeps the verified-missing file hidden while its durable update is pending and corrects the album track, rating, love, and duration counts.
- Whole-album Tags now reloads after deletion and excludes only a missing MP3 covered by Aurora's durable Music Library queue. Unrelated unavailable files still stop a batch edit instead of silently editing an incomplete album.
- New deletions queue exact filenames before filesystem removal, retaining whole-folder fallback only when several changed files in the same album require it.

## [0.17.20] - 2026-08-25

### Added

- Ratings completion details now include a **Go to Album** action that opens Albums with the chosen album loaded and selected.

## [0.17.19] - 2026-08-25

### Fixed

- Rating, Love/Ban, Release Year, inspector, and undo writes no longer wait behind Music Library's slow background bridge or pending-tag reconciliation. Foreground MP3 edits and their player projections keep their own short serialization path, while background reconciliation uses revision-guarded updates so a stale result cannot replace a newer edit.

## [0.17.18] - 2026-08-25

### Fixed

- Add Music review plans with more than two albums now use a bounded, keyboard-accessible album list, so every source-to-destination row remains scrollable inside the Verified Plan card at constrained window heights.
- Starting **Move and catalog** immediately closes the modal and continues the intake in the background, leaving browsing and playback usable while a persistent top-bar status reports completion or failure. The Add Music action remains disabled during that one active intake, and a synchronous in-flight guard prevents rapid clicks from submitting the same locked plan twice.
- The progress copy now distinguishes fast file copying from Music Library's reviewed whole-catalog snapshot apply and index rebuild instead of making a multi-minute catalog phase look like a stalled file move.
- Music Library `0.144.5` automatically archives and links embedded cover art for newly added Aurora intake albums, removing the separate manual Cover add step.

## [0.17.17] - 2026-08-25

### Added

- Album detail can permanently delete one MP3 or a Ctrl/Shift multi-selection after an explicit confirmation.
- Successful deletions durably queue complete-folder Music Library synchronization before filesystem removal, run the bridge immediately, and retain automatic retry state so the shared catalog and Music Library Updates deletion count catch up safely.

### Security

- Native deletion accepts at most 100 catalog track references from one open album, re-resolves every transient ID against its stable path key, rejects duplicates and non-regular/non-MP3 targets, and never accepts an arbitrary path from React.

## [0.17.16] - 2026-08-25

### Fixed

- The Ratings completion detail now responds to its content-pane width, keeping the cover and album metadata together above the track list while placing Play Unrated Tracks beneath large cover art on constrained screens.

## [0.17.15] - 2026-08-25

### Fixed

- Add Music preview and apply now share Aurora's Music Library bridge coordinator with background tag synchronization, so intake waits for active work instead of racing the helper's Windows workflow lock.
- Folder synchronization pauses after three consecutive failures instead of retrying every five seconds forever. Aurora reports the blocked folders clearly, and a later MP3 edit resets that folder's retry budget.

## [0.17.14] - 2026-08-24

### Fixed

- MP3 decoding, ReplayGain, and sample-rate conversion now run on dedicated producer threads that prefill bounded lock-free PCM ring buffers. The Windows audio callback only consumes ready endpoint-format samples instead of decoding or resampling under its real-time deadline.
- Current and gapless-prepared tracks each keep at most three seconds of PCM, prefill up to 500 ms before becoming playable, preserve seeking by invalidating stale generations, and count buffer starvation through the existing audio-underrun diagnostic.

## [0.17.13] - 2026-08-24

### Fixed

- Native playback now keeps one stable endpoint-format WASAPI stream across mixed-rate tracks, explicitly uses Rodio's balanced Rubato sinc/FFT resampler instead of its previous linear conversion, raises the CPAL callback thread to real-time priority on Windows, and requests a stability-focused 4,096-frame output buffer.
- Debug builds now optimize audio dependencies, reducing codec and resampler deadline pressure during development playback.
- CPAL underruns and denied real-time scheduling are counted without incorrectly treating those non-fatal notifications as device disconnects; the player output readout exposes either condition when observed.

## [0.17.12] - 2026-08-24

### Fixed

- The Track inspector now keeps the selected or playing track's Artist credit and Publisher value instead of falling back to Album Artist and `Unknown` when the track arrived through the initial library snapshot, a restored queue, or the Genres, Publishers, Years, Ratings, or Charts routes.

## [0.17.11] - 2026-08-24

### Changed

- The bottom player now draws a continuous, directly seekable purple-to-cyan waveform from all decoded frames in the active MP3. Aurora reduces the complete song to 640 peaks, visibly separates played from upcoming audio, and automatically replaces the previous sparse 64-window cache entries.

## [0.17.10] - 2026-08-24

### Fixed

- Aurora now reads the verified legacy `Default` POPM whole-star byte scale as a MusicBee-compatible rating. Editing or clearing that rating removes both recognized owner variants, writes at most one canonical `MusicBee` frame, and preserves unrelated POPM owners.
- Background catalog synchronization now preflights Music Library's explicit legacy-rating preservation capability. Music Library older than `0.144.2` leaves the verified MP3 edit durably pending instead of risking a fallback album scan that can misclassify intact `Default` ratings as removed.

### Changed

- Aurora now requires Music Library `0.144.2` or newer for post-edit tag synchronization.

## [0.17.9] - 2026-08-24

### Added

- The **Finish what you love** workspace now has an explicit Refresh action that reloads its completion candidates and the Ratings overview on demand.

### Fixed

- Rating and Love edits, including their later catalog synchronization, no longer replace the visible completion candidates automatically. The shelf stays stable until Refresh is selected.
- The decorative cover pyramids no longer capture pointer input where the tallest 5-star artwork overflows into the header, so Track ratings and Album ratings remain clickable.

## [0.17.8] - 2026-08-24

### Fixed

- Playback now requests the active MP3's native sample rate from the shared WASAPI stream while retaining Rodio's driver-selected buffer. This bypasses Rodio's documented linear sample-rate converter for ordinary playback and leaves any endpoint-rate conversion to Windows' shared audio engine.
- Explicitly loading a track with a different native rate rebuilds the output stream at that rate. A mixed-rate source already prepared for gapless handoff keeps the existing stream and remains the compatibility case.

## [0.17.7] - 2026-08-24

### Added

- Windows System Media Transport Controls now handle physical Play, Pause, Stop, Previous, and Next keys through the Windows media-session arbitration layer and publish Aurora's playback status and track metadata.

### Fixed

- Native audio output again opens with Rodio/CPAL's driver-compatible shared-mode configuration instead of forcing the MP3 sample rate and a synthetic 100 ms buffer, reversing the 0.17.5 output-path regression while retaining bounded encoded MP3 read-ahead.
- Playback snapshots now run every two seconds on a blocking worker, while the bottom progress line advances locally every 250 ms and waveform analysis waits 1.5 seconds after a track change. Ratings refreshes preserve the visible page instead of blanking it.
- Verified tag edits return before the potentially long Music Library companion process; the durable exact-file/folder journal drives the existing focused background retry and catalog refresh.
- Listening-history and saved-position checkpoints now use 30-second buckets, and OneDrive history publication no longer runs while Aurora holds the playback runtime lock.

### Changed

- Aurora is explicit that physical media keys are independent of shared/exclusive audio mode; configurable `Ctrl+Alt` shortcuts remain separate from the Windows media session.

## [0.17.6] - 2026-08-24

### Fixed

- Inline edits, inspector saves, global rating/Love shortcuts, undo, startup recovery, and focused-window retry now share one explicit Music Library `synced` or durable `pending` receipt instead of silently dropping bridge failures or omitting shortcut synchronization.
- The newly edited album folder is attempted ahead of old backlog, folders are synchronized independently behind one process gate, and token-checked completion prevents an older bridge receipt from deleting a newer edit of the same folder.
- Successful receipts refresh every revision-backed catalog view immediately; pending status remains visible and retries one folder every five seconds while Aurora is focused.
- Playback metadata is projected before the potentially long companion import, so rapid consecutive shortcuts and edits compare against the latest verified MP3 values.
- Global rating and Love shortcuts now re-read the serialized, authoritative MP3 tag state before deriving each edit, so rapid shortcuts or a concurrent inspector save cannot discard the later action.
- The complete tag write, Music Library receipt, and native playback projection now run in one edit order, while monotonic projection tokens prevent a delayed inline, inspector, or shortcut result from overwriting a newer edit in the frontend.
- External-tag reconciliation shares that same projection order, and pending overlay totals are evaluated against each live catalog track so targeted imports cannot hide edits from untouched albums or count tracks no longer in the catalog.
- Repeated successful sync receipts restart the transient success notice correctly, and a partially successful multi-folder pass refreshes committed folders immediately even while another folder remains queued.
- Aurora durably carries the exact edited filename into Music Library so ordinary one-track rating, Love/Ban, and Release Year changes scan only that MP3; multiple pending files in one album deliberately collapse to the complete-folder safety path.
- Catalog refresh uses an opaque completion-order token and the actual last-completed import ID, so an older import that finishes after a newer one still refreshes snapshots, playback queues, and tag overlays consistently.

### Changed

- Aurora now requires Music Library `0.144.1` for the legacy half-star repair, single rollback-backup path, and prompt discarded-staging cleanup used by existing-folder synchronization.
- Added the permanent developer-workflow requirement to run `cargo clean` from `src-tauri` after Rust or Tauri verification.

## [0.17.5] - 2026-08-24

### Fixed

- Native playback now preloads ordinary encoded MP3s before Rodio's real-time callback consumes them and uses a larger fixed output buffer, removing two verified deadline risks that can cause short dropouts.
- Output creation first matches the source sample rate, avoiding Rodio's linear converter on compatible Windows endpoints; explicit alternative configurations retain the same stability buffer, with the driver-compatible default kept as a final fallback.
- Waveform cache misses are serialized and generation-cancelled during rapid track changes, with sequential MP3 buffering replacing repeated storage seeks for ordinary files.
- Playback snapshot polling no longer overlaps itself or lets an older poll overwrite a newer transport-command result.

### Changed

- Encoded playback read-ahead admits at most two files of up to 96 MiB each and revalidates file size and modification time before reuse; larger files and memory-pressure cases use a 1 MiB buffered-file fallback.

## [0.17.4] - 2026-08-24

### Fixed

- Native track explorer, album-detail, playback-queue, and stable queue-restore payloads now carry Music Library's per-track `display_artist`, so the 0.17.3 Artist presentation fix works with the live catalog instead of falling back to Album Artist.

## [0.17.3] - 2026-08-24

### Fixed

- The bottom playbar and Track inspector now show the track's Artist credit instead of substituting Album Artist.
- Opening the Artist inspector from a selected track now follows that track's Artist credit, including soundtrack and compilation tracks whose performer differs from the album artist.

## [0.17.2] - 2026-08-24

### Added

- Five-star Album Rating with half-star presentation and numeric Album Score on one compact line in every album card and expanded album detail.
- A dedicated current-playback signal in track rows, distinct from the row selected for inspection or tag editing.

### Changed

- Album covers now open their bounded track panel directly beneath the selected cover row. Clicking the same cover or the close control collapses the panel with a short slide, preserving the album-grid position.
- Successful inline track-rating edits refresh the selected album summary from the existing overlay-aware catalog calculation, while Music Library remains the sole writer of the stored album rating and Album Score.

## [0.17.1] - 2026-08-24

### Added

- A validated device-local last-view snapshot that restores Aurora's active destination, explorer mode, exact query and filters, sort direction, right-inspector section, tag target kind, and selected album context on startup.

### Changed

- Last-view state is now independent of the existing sidebar layout and display-size preferences; malformed or incompatible stored state falls back safely to the Universe view.

## [0.17.0] - 2026-08-24

### Added

- A MusicBee-style vertical **Tags** inspector for Album Artist, Artist, Album, Track Title, Genre, Publisher, Track Rating, Year, Release Year, track number/total, and disc number/total.
- Track-wide and album-wide editing with shared-value aggregation, explicit **Mixed** states, checked-field write intent, and checked blank values for deliberate tag removal while preserving Music Library's required Album Artist, Album, and Track Title identity fields.
- Full-batch stale-revision preflight, same-folder atomic MP3 replacement, per-file verification, earlier-file rollback on later failure, and full editable-tag metadata in Aurora's durable recovery journal.
- A companion `syncExistingFolders` bridge operation in Music Library `0.144.0` for guarded reimport of already-cataloged album folders after Aurora writes their MP3 tags.

### Changed

- Aurora now reads editable values directly from each selected MP3 and projects verified saves into the current views immediately; album selections are bounded at 500 tracks.
- Post-save catalog synchronization requires stable album identity and a zero add/remove delta. A helper failure never misreports the MP3 write: the editor confirms the verified file save, durably retains every affected folder, and retries the pending synchronization on startup, focus, and later saves.
- The tag writer now preserves unselected ID3 frames and audio bytes while supporting both ID3v2.3 and ID3v2.4 year/release-year conventions, MusicBee POPM ratings, `DISPLAY ARTIST` overrides that leave the underlying performer credits untouched, multi-value Album Artist credits, and coupled track/disc number pairs.
- A MusicBee POPM byte of `0` is read as unrated. Aurora continues to write only supported half-star values or removes its MusicBee POPM frame when clearing a rating.

## [0.16.0] - 2026-08-24

### Added

- A top-bar **Add music** workflow for a single already-tagged album folder or a parent containing many album folders, with General music, Movie / TV / game music, and Synthwave destinations supplied by Music Library.
- A review step showing every source-to-destination mapping, album/track totals, and the exact combined catalog delta before one explicit Move and catalog confirmation.
- A versioned, file-based native bridge to Music Library `0.143.0` or newer. Aurora discovers and launches the exact installed executable without a shell, validates capability and apply receipts, bounds response size and execution time, and removes its private request/response exchange files.

### Changed

- Completed album intake now requests Aurora's existing consistent catalog refresh immediately. Concurrent polling and focus refreshes share the same queued operation so playback rebind and views cannot race each other.
- A stale or changed intake plan can be previewed again without closing the dialog, while the same locked plan remains available for a safe retry after an uncertain helper timeout.
- Intake completion distinguishes folders whose verified source was removed from folders retained for manual cleanup; Aurora never reports a partial cleanup as a complete move.
- The Music Library catalog remains strictly read-only inside Aurora. All folder transfer, database backup, staging, normalization, and atomic catalog writes remain owned by the Music Library companion.

## [0.15.20] - 2026-08-23

### Added

- Runtime detection of completed Music Library imports every five seconds and on window focus, followed by bounded refreshes of the current catalog-backed view.

### Changed

- Live playback queues now re-resolve transient catalog IDs from stable track keys inside one SQLite read snapshot before view refresh. Unchanged stable queue order preserves the current and preloaded audio sources; removed entries are remapped conservatively and a removed current track stops safely. Transient catalog errors abort without pruning the queue.
- Catalog revisions are acknowledged only when the detector, queue rebind, and refreshed base snapshot report the same completed import, so concurrent imports and transient read failures retry automatically. Track and artist inspectors refresh without replacing stable selections or discarding a stable-key tag draft.
- Import-time tag results now patch only mutable tag state onto refreshed queue rows, preventing an in-flight rating save from restoring obsolete catalog IDs. Canonical-path and bounded case-insensitive fallbacks preserve stable identity when path spelling changes.

## [0.15.19] - 2026-08-23

### Changed

- **Explore publisher** now opens Albums with the exact `publisher:` filter instead of sending the user to Songs.

## [0.15.18] - 2026-08-23

### Added

- Distinctive deterministic Aurora monograms for every publisher, replacing the repeated generic record icon while remaining complete offline.
- Device-local publisher-logo overrides with PNG, JPEG, and WebP validation, bounded resizing, immediate row/detail updates, persistence, and a one-click return to the Aurora monogram.

## [0.15.17] - 2026-08-23

### Fixed

- Publisher Release activity and Original-year activity signals now use their actual bucket years and the full responsive timeline width, so plots remain aligned with every tick through 2026 instead of visually ending around 2013.

## [0.15.16] - 2026-08-23

### Added

- A dedicated Publishers destination under Library with the selected Publisher Signal Timeline design, three activity lenses, search, case-insensitive catalog rollups, bounded release highlights, publisher playback, and exact handoff into the existing `publisher:` search.
- Publisher metadata across track tables, album cards, Genre Atlas albums, Years editions, Ratings albums, and the right-side track and album inspectors.
- An offline-safe publisher-logo slot and documented MusicBrainz → Wikidata → Wikimedia Commons enrichment route with per-file licensing and attribution requirements.

### Changed

- Track and album explorer payloads now retain Music Library's Publisher value, while publisher grouping normalizes only case and surrounding whitespace and preserves the most common stored display spelling.

## [0.15.15] - 2026-08-23

### Fixed

- Selecting an album in Albums now makes the right inspector show that album, a track from that album, and the album artist across the Album, Track, and Artist tabs instead of leaking older selections from other pages.

## [0.15.14] - 2026-08-23

### Changed

- Selecting an artist from the Artists page now opens Albums with that artist applied as an exact filter, while Universe artist planets retain their existing Songs handoff.

## [0.15.13] - 2026-08-23

### Fixed

- The active Explorer sort is now an enabled menu choice instead of a disabled select placeholder, so it can be clicked again to reverse direction after the pointer moves across other choices.

## [0.15.12] - 2026-08-23

### Changed

- Songs, Albums, and Artists now keep only Sort and Reset in the Explorer filter row. The persistent top search remains the primary catalog-filtering surface, while Reset also clears filters applied by collection handoffs.
- Re-selecting the active Explorer sort reverses its direction across Songs, Albums, and Artists, including newest/oldest, A–Z/Z–A, year, release year, rating, and track-count ordering.

## [0.15.11] - 2026-08-23

### Added

- Catalog search now shows the exact number of matching songs, albums, or artists beside the top search field. The total covers the complete filtered result set rather than only the currently loaded 50-row page.

## [0.15.10] - 2026-08-23

### Added

- `year:` and `ryear:` searches now accept inclusive closed ranges such as `year:1985..1987`, open-ended ranges such as `year:1985..` and `ryear:..1987`, inherited range alternatives after `OR`, and the existing `NOT` or leading `-` exclusions.

## [0.15.9] - 2026-08-23

### Fixed

- Song rows labelled `Year` now return and display Music Library's `Year` value instead of incorrectly displaying `Release Year`, including publisher-filtered and other field-aware search results.
- Album cards, album ordering, Genre Atlas ranges/decades/cards, and the unrated-album shelf now use `Year` without falling back to `Release Year`.

### Changed

- Song and Album year filters now default to `Year`; `Release Year` remains available only through the explicit year-basis choice, `ryear:` search field, and release-year sort.

## [0.15.8] - 2026-08-23

### Fixed

- Rust search tokenization now accepts a string slice instead of an unnecessarily mutable `String` reference, satisfying the release workflow's warnings-as-errors Clippy gate.

## [0.15.7] - 2026-08-23

### Added

- Search groups now support uppercase `OR`, `AND`, and `NOT`, plus leading `-` exclusions. Alternatives after `OR` inherit the preceding field, so `aartist:bon jovi OR def leppard OR kiss` stays scoped to Album Artist.
- Complete quoted values use exact case-insensitive matching; unquoted values keep prefix matching, allowing `aartist:\"kiss\"` to distinguish Kiss from names such as Kissing the Pink.
- Unquoted `genre:score` and `genre:scores` expand to the Music Library Scores umbrella for film, TV, animation, anime, and video-game score genres.

### Changed

- Track-search guidance now documents boolean operators, exclusions, exact quotes, and the Scores umbrella.

## [0.15.6] - 2026-08-23

### Fixed

- Aurora Album Score charts and the score shelf now use the catalog `Year` field by default instead of substituting `Release Year`.

### Added

- Charts can optionally switch Aurora Album Score period filtering to `Release Year`; the active basis is shown beside the score shelf and in the full chart summary.

## [0.15.5] - 2026-08-23

### Added

- Field-aware catalog search for `artist:` (Display Artist), `aartist:` (Album Artist display), `album:`, `genre:`, `year:` (Year), `ryear:` (Release Year), `publisher:`, and `title:`.
- Comma-separated search clauses combine with AND, including queries such as `aartist:def leppard,genre:hard rock`.

### Changed

- Track search inputs now advertise the field syntax while ordinary unscoped prefix search remains available.

## [0.15.4] - 2026-08-23

### Changed

- Developer work now ends after a successful `git push`; agents must leave CI verification and release publication to the autonomous workflow without waiting or monitoring.
- Release documentation no longer requires post-push asset verification or a manually pushed tag.

## [0.15.3] - 2026-08-23

### Fixed

- Successful `master` CI runs now publish the matching Windows GitHub Release instead of stopping after verification while waiting for a manually pushed version tag.

### Changed

- Windows release publication now runs only after the same CI workflow's verification job succeeds, then creates the SemVer tag and uploads the NSIS installer, updater bundle and signature, and `latest.json`.
- Version validation now covers both lockfiles and Aurora's user-facing version label in addition to the manifests.
- Developer instructions now require waiting for and verifying the published GitHub Release assets after every `master` push.

## [0.15.2] - 2026-08-23

### Fixed

- Moved the six default rating bindings from the number row to `Ctrl+Alt+Numpad0` through `Ctrl+Alt+Numpad5`, preventing Windows from treating Norwegian `AltGr+2` (`@`) as Aurora's rating shortcut.
- Explicitly unregister every active Aurora shortcut when the main window closes and on application exit, including programmatic exit paths.

### Changed

- Version 1 shortcut settings migrate legacy number-row rating defaults to the numeric keypad while preserving custom bindings.
- Documented that Windows cannot transfer a released registration to an already-running app automatically; MusicBee must retry its shortcut configuration or restart if registration failed while Aurora owned the key.

## [0.15.1] - 2026-08-23

### Added

- A Display Settings tab with configurable global text and cover sizes, a live preview, and a one-click restore to Aurora's readable defaults.
- Independent text and artwork-size overrides for Universe, each Library destination, Observatory, Charts, and History. Cover controls stay unavailable on views without adjustable artwork.
- Versioned, device-local persistence for display preferences, including safe validation and fallback when stored data is missing or malformed.

### Changed

- Aurora now uses shared typography tokens across every rendered surface, with a 10 px readable global floor and a larger default for the especially dense Charts view.
- Library rows, chart entries, album grids, Years shelves, Genre Atlas, Ratings Studio, History, inspectors, settings, and player chrome now respond consistently to the selected text size.
- Cover-size choices now reflow bounded album grids and resize applicable row, chart, Years, Genre, and Ratings artwork without horizontal overflow.

### Developer workflow

- Project instructions now require every code, documentation, configuration, test, or asset change to increment Aurora's SemVer version and keep every manifest, lockfile, and user-facing version label aligned.

## [0.15.0] - 2026-08-23

### Added

- A dedicated Charts page above History with Singles and Albums modes, named presets, exact week selection, custom week ranges, and a direct full-year action.
- Historical Official UK, VG Lista, Ti i Skuddet, and Norsktoppen weekly sources; annual Billboard singles and album sources; and a first-class Aurora Album Score chart.
- A polished ranked chart table with movement, last-week, peak, appearances, real cover art, library-match state, rating, Love, source history, chart-queue playback, and library handoff.
- An Aurora Album Score year shelf using Music Library's exact existing score formula.

### Data and performance

- Period charts compare #1 finish counts first, then #2, #3, and every lower position in order, with position points and appearances as deterministic final tie-breakers.
- Chart results, detail lookups, and playback queues are bounded at 100 items and keep the shared Music Library database strictly read-only.
- Billboard is treated as annual-only because the imported catalog contains annual entries rather than weekly Billboard history.

### Verification

- Added frontend coverage for exact weeks, period calculations, full-year charts, custom ranges, and Aurora Score mode, plus Rust tests for source validation and chart ordering against synthetic and live catalog data.

## [0.14.1] - 2026-08-22

### Fixed

- Replaced the Ratings constellation's tiny rectangular cover clusters with the intended tall, tapered cover pyramids.
- Restored the visual progression from silver and amber through cyan, blue, violet, and magenta, with stronger cover-edge glow and a filled constellation stage in both Track and Album Ratings.
- Expanded the representative artwork pool used by the pyramids while keeping the rating counts and band actions exact.

### Accessibility

- The new pyramids remain decorative inside the existing semantic rating-band buttons; labels, counts, pressed states, keyboard focus, and reduced-motion-safe interaction behavior remain unchanged.

## [0.14.0] - 2026-08-22

### Added

- A dedicated Ratings Studio with separate Track Ratings and effective Album Ratings constellations, clickable whole- and half-star bands, real cover samples, and the 5 Star Collection.
- Mutually exclusive Almost Complete, Partially Rated, and Unrated Album lanes with bounded cover shelves, selected-album track details, and Play Unrated Tracks.
- Music Library's exact effective-album-rating precedence and Album Score formula, including clearly labelled provisional means for incomplete albums.
- Numeric Album Score badges in Ratings and ordinary Album detail once every track on the album is rated.
- Browser preview artwork sourced read-only from the user's real cover archive, plus persistent preview star/Love edits for interaction testing.

### Changed

- Album Explorer now accepts and visibly exposes exact rounded half-star and unrated album filters, so a Ratings constellation handoff preserves the selected band.
- Ratings overview and completion-lane requests are separated; switching lanes no longer reruns the million-track overview query.
- Ratings Studio stars and Love save instantly through the existing verified MP3 and Aurora overlay pipeline, then refresh spectra, completion, provisional means, and Album Score.
- Album detail recalculates completion, effective rating, Love count, and Album Score against pending Aurora overlays without writing the shared Music Library database.
- A normalized zero rating is treated defensively as unrated, while Music Library's two raw half-star records remain represented in their exact 3.5 and 4.5 bands.

### Performance

- The live 1,096,288-track and 72,012-album Ratings overview completes in about 1.7 seconds on the current catalog; album shelves are capped at 14 and playback requests at 100 tracks.
- Completion pages, rating collections, and selected-album queues remain bounded and stale responses cannot replace newer selections.

### Accessibility

- Rating scopes, whole- and half-star bands, completion lanes, album cards, instant track controls, and playback actions use semantic tabs, buttons, pressed states, labels, and visible keyboard focus.
- The 1280 × 720 layout retains all persistent shell controls without horizontal overflow; sidebars remain independently collapsible.

### Known limits

- Album Score remains Music Library's unbounded numeric rank measure and is intentionally not converted to stars. A future Charts page may rank it but is not part of 0.14.0.
- Aurora does not write Album Rating or Album Score into `music-library.sqlite3`; imported catalog values and verified MP3/Aurora overlays are combined read-only.

## [0.13.0] - 2026-08-22

### Added

- A cinematic paired-clock Years explorer over Music Library's separate Original Year and Release Year album fields.
- Clickable album-level histograms, aggregated flows between the two clocks, and dedicated Release, Original, and Two Clocks modes.
- Bounded edition shelves grouped by the counterpart decade, separate Missing Original Year and Missing Release Year lenses, and an Album inspector that shows both dates without substitution.
- Exact handoff from a selected year into Songs and bounded Play Year and Play Album actions.

### Changed

- Track and album Explorer requests now accept an explicit Original or Release Year basis plus a separate missing-year filter.
- The Years route loads lazily and protects overview, detail, and Album-inspector requests against stale responses.
- The live catalog remains read-only; Years overview payloads contain roughly one row per year, details return at most 100 representative albums, and playback returns at most 100 tracks.

### Accessibility

- Every year mark is keyboard-selectable and exposes its year, album count, track count, and selected state. Mode tabs, missing-year lenses, year stepping, edition cards, and actions retain semantic controls and visible focus.

### Known limits

- Edition grouping is derived from the two catalog year fields. Music Library does not currently provide edition lineage, label, catalog number, or release-format relationships for this view.
- Browser preview uses labeled synthetic covers; packaged Aurora resolves the user's real cover archive through the existing contained artwork protocol.

## [0.12.0] - 2026-08-22

### Added

- A collapsible Library tree containing Songs, Albums, Artists, Genres, Years, Ratings, and Tags, with Songs as the default destination when a closed Library is opened.
- A collapsible Playlists group with the existing pinned-preview entries kept as an explicit shell for future playlist work.
- Compact Library and pinned-playlist flyouts for the icon-only rail.
- A dedicated Years placeholder page that clearly separates future year exploration from the generic song explorer.

### Changed

- Universe, Observatory, and History remain top-level destinations while catalog-shaped views now live under Library.
- Library and Playlists disclosure choices are stored with the existing device-local layout preferences and restored on the next launch.
- Version 1 layout preferences migrate safely to the version 2 shape while preserving the user's left- and right-rail choices.

### Accessibility

- Nested destinations expose the current page, disclosure triggers expose their expanded state and controlled region, and icon-only groups retain accessible names.

### Known limits

- Playlist rows are previews only in 0.12.0; creating, pinning, editing, and opening playlists remain future work.
- Years is intentionally a placeholder. No year or decade query is run until that page receives its focused implementation.

## [0.11.0] - 2026-08-22

### Added

- A dedicated Genre Atlas with a searchable, sortable index of canonical catalog genres instead of the generic track explorer previously shown by Genres.
- Bounded genre worlds with representative covers, catalog scale and year range, release-decade distribution, top albums and artists, personal listening memory, and related genres explained by shared album artists.
- Genre Radio, Shuffle, Loved, Highest Rated, Rediscover, and Unrated Expedition queues, requested in batches of at most 100 tracks and capped at Aurora's 200-track queue limit.
- Queue append and automatic genre refill below 20 remaining tracks while retaining the current track and up to 20 recently played entries.
- Instant half-star and Love controls in Genre Atlas highlights using the existing verified MP3 transaction and optimistic Aurora-state overlay.

### Changed

- Genre Atlas loads lazily only when Genres is active, with independent index/detail loading, stale-request protection, failure states, and retries.
- Rating and Love changes now update visible genre aggregates immediately while the verified file write completes.
- The live queue can be extended without replacing playback or breaking the prepared gapless successor.

### Security

- Genre requests accept an exact bounded canonical-genre value and return bounded rows; React never supplies an audio path.
- The Music Library catalog and all MusicBrainz databases remain read-only. Genre Atlas does not rename, merge, or write genre tags.

### Known limits

- Related genres are a shared-artist navigation signal, not an authoritative genre taxonomy.
- Listening memory begins with Aurora history and does not infer MusicBee or Last.fm plays. Rediscover is therefore most useful after Aurora has recorded listening history.
- Genre queues are finite within the current catalog and selected filter; Aurora reports when an expedition has exhausted its eligible tracks.

## [0.10.0] - 2026-08-22

### Added

- Device-local Windows output selection in Audio Settings using CPAL's stable endpoint IDs, including a distinct Windows-default choice and current-default labeling.
- ReplayGain Off, Track, and Album modes for MusicBee-compatible `REPLAYGAIN_TRACK_GAIN`, `REPLAYGAIN_TRACK_PEAK`, `REPLAYGAIN_ALBUM_GAIN`, and `REPLAYGAIN_ALBUM_PEAK` ID3 text frames.
- Peak-based clipping prevention for positive ReplayGain and Track fallback when Album mode encounters an MP3 without album frames.
- Gapless-capable native queue handoff that resolves, opens, and appends the next MP3 to the same Rodio player during the final 15 seconds.
- A compact now-playing readout for the active output, effective gain, clipping protection, and fallback state.
- Atomic per-computer audio preferences in `aurora-audio.json`, intentionally outside shared SQLite and OneDrive synchronization.

### Changed

- Settings now has Audio and Shortcuts sections, loaded concurrently so either native boundary can report its own failure.
- Native playback reconciles a pre-queued source transition before transport or global-shortcut actions, preserving the actual now-playing track as the rating and Love target.
- Output stream errors automatically reopen the current track at its observed position on the Windows default without changing the preferred device setting.

### Security

- The WebView receives endpoint labels and stable IDs but never supplies an audio-file path. Rust validates the selected CPAL ID and matches it only against currently enumerated output devices.
- ReplayGain reads optional ID3 frames after catalog ID plus stable path-key resolution and does not write or rewrite the MP3.

### Known limits

- Seamless handoff requires a valid next MP3 with known duration to be prepared before the current source ends. Aurora safely falls back to the ordinary transition if preparation is impossible.
- ReplayGain affects playback only. Aurora 0.10.0 does not calculate missing gain tags, edit existing ReplayGain frames, crossfade, equalize, or apply other DSP.
- An unavailable preferred output remains selected and is retried when Aurora next creates an output stream; Aurora does not switch back mid-track after falling back.

## [0.9.0] - 2026-08-22

### Added

- Configurable Windows global shortcuts for play/pause, next track, clearing or assigning a whole-star rating from 1–5, and toggling Love.
- Default bindings matching the requested MusicBee workflow: `Ctrl+Alt+P`, `Ctrl+Alt+N`, `Ctrl+Alt+0` through `Ctrl+Alt+5`, and `Ctrl+Alt+L`.
- An Aurora-styled Settings dialog with shortcut capture, duplicate validation, enable/disable, default restoration, native registration state, and actionable conflict feedback.
- Device-local, atomic shortcut persistence in `aurora-shortcuts.json` with safe fallback to defaults when the file is missing, malformed, or unsupported.

### Changed

- Rating and Love shortcuts resolve only the now-playing track from the Rust playback engine and save immediately through Aurora's verified MP3 and optimistic state-overlay transaction. Explore selection is never a shortcut target.
- Shortcut registration is all-or-none. If MusicBee or another application owns a requested binding, Aurora retains the previously registered set instead of applying a partial configuration.

### Security

- The WebView supplies only a validated action-to-accelerator configuration; the native shortcut handler obtains track identity internally and never accepts a selected track ID or filesystem path from React.
- Custom bindings require exactly one non-modifier key, reject duplicates, and are parsed again by the native global-shortcut library before registration.

### Known limits

- Windows allows only one process to own a particular global binding. MusicBee must release a conflicting default, or that Aurora binding must be changed in Settings.
- Global shortcuts are per-device settings and intentionally do not synchronize through OneDrive.

## [0.8.3] - 2026-08-22

### Fixed

- Rapid waveform seeks no longer leave a stale draft position covering the live playback clock; the final released value stays visible only until its seek completes.
- Overlapping playback commands now ignore older responses, and polling waits for active commands instead of racing them.
- Pressing Play after a track has naturally stopped at its end restarts it from the beginning in both native and browser-preview playback.

## [0.8.2] - 2026-08-22

### Added

- A wide midnight player with a real purple-to-cyan waveform, generated from 64 evenly spaced MP3 decode windows and cached as 320 bounded peaks in a separate local SQLite database.
- Instant half-star, clear-to-unrated, and Love controls in the player using the same verified MP3 tag pipeline and optimistic Aurora overlay as Explore.
- A clickable track-end time that switches between total length and a live remaining-time countdown.

### Changed

- Playback metadata, transport controls, waveform seeking, volume, and queue access now share a denser reference-matched 120 px player surface.
- Rating and Love changes immediately refresh the active queue item and current-track player state while the native write completes.

### Security

- Waveform extraction resolves catalog ID plus stable track key on the Rust side and never accepts a filesystem path from the WebView.
- The derived `aurora-waveforms.sqlite3` cache is device-local and excluded from shared state/OneDrive synchronization.

## [0.8.1] - 2026-08-22

### Added

- A three-state left sidebar that cycles between expanded, icon-only, and collapsed layouts from an accessible top-bar control.
- An independent right-inspector collapse control that returns its width to the library workspace.
- Device-local, versioned layout preferences that restore both sidebar choices on the next launch and fall back safely if browser storage is unavailable or malformed.

### Changed

- Narrow windows now respect the chosen sidebar layout instead of forcing the left navigation into icon-only mode.
- Icon-only navigation keeps accessible names and hover labels while removing visible navigation, playlist, profile, and brand text.

## [0.8.0] - 2026-08-22

### Added

- Native per-device listening journals with stable installation identity, crash recovery, and separately named OneDrive snapshots for Desktop and Laptop histories.
- A configurable 1–3600 second registered-play threshold, defaulting to 30 seconds, with active-session updates and natural-completion handling for shorter tracks.
- A bounded Listening Memory screen with all-time summary, most-played tracks, grouped timeline, text/outcome/device/date filters, keyset pagination, replay, and catalog inspection.
- Personal registered plays, listening time, and last-listened time in the track inspector, explicitly separated from imported Last.fm popularity.
- A compact listening-memory summary on Universe plus browser-preview and native regression coverage for history behavior.

### Changed

- Playback now records only observed positive forward progress; seeking resets the timing baseline instead of adding the seek distance.
- Next/previous, queue replacement/removal/clear, natural completion, clean shutdown, and abnormal-startup recovery give sessions explicit completed, skipped, or interrupted outcomes.
- Existing device settings migrate in place to include a stable device ID and computer label without changing the persistent Desktop/Laptop choice.

### Security

- High-frequency listening events do not enter the shared `aurora-state.sqlite3` conflict lineage. Each installation is the sole writer of its local history and its own device-named OneDrive snapshot.
- History snapshots use consistent SQLite copies, ownership/schema checks, `quick_check`, and atomic replacement; peer databases are query-only and corrupt or unsupported peers are skipped without risking local history.
- History and track-insight requests are validated and bounded. Old sessions retain metadata but cannot replay unless their stable identity resolves through the read-only catalog.

### Known limits

- Listening memory starts with Aurora 0.8.0; it does not infer historical personal plays from Last.fm popularity or import MusicBee playback history.
- Peer history depends on OneDrive propagation and is eventually visible rather than a live two-computer stream.

## [0.7.1] - 2026-08-22

### Added

- A monitor icon for Desktop Mode and a laptop icon for Laptop Mode, making the active device mode visible without opening the status popover.
- Semantic snapshot comparison that can reconcile same-lineage OneDrive branches when their stable and user-authored state agrees.
- Regression coverage modeled on the observed `JornComputer`/`Keiya` generation-8 split, including transient track IDs, playback position, catalog import runs, and overlay timestamps.

### Fixed

- Aurora no longer remains permanently conflicted when OneDrive gives two equivalent snapshots different snapshot IDs at the same generation.
- Passive playback-position saves, catalog-local transient track IDs, and tag-overlay reconciliation bookkeeping no longer increment the shared content revision or trigger needless OneDrive publishes.

### Security

- Equivalent-branch repair updates only local sync metadata after a read-only, bidirectional comparison. It does not replace either database or delete OneDrive conflict copies.
- Different stable queues, playback settings, desired MP3 tags, tag-operation journals, or MusicBrainz decisions remain a hard conflict and are never auto-merged.

## [0.7.0] - 2026-08-22

### Added

- Persistent per-device Laptop Mode behind an accessible icon-only top-bar control with active-root and state-sync status.
- Exact, case-insensitive runtime path translation from the desktop `D:`, `G:`, and `H:` catalog roots to the laptop `Y:`, `V:`, and `U:` roots without changing imported catalog rows or stable track keys.
- State schema version 6 with sync lineage, snapshot generations, logical content revisions, and mutation triggers for playback, tag journal/overlays, and MusicBrainz curation.
- Verified OneDrive state snapshots, previous-remote retention, first-laptop restore, pre-OneDrive local backups, and startup-only adoption of newer clean snapshots.
- Rust and React coverage for path boundaries, per-device restart persistence, v5 migration, publishing, first-run restore, clean startup update, two-device divergence, and accessible control behavior.

### Changed

- Stored queue and tag-journal paths are translated only at filesystem boundaries; Aurora continues to query the shared Music Library database using its original desktop paths.
- Aurora publishes app-state changes no more than once per minute and forces one final consistent snapshot on normal shutdown.
- Laptop Mode is stored in device-local `aurora-device.json`, so enabling it on the laptop does not switch the desktop installation.

### Security

- Aurora uses SQLite `VACUUM INTO`, `quick_check`, schema validation, staged writes, and Windows atomic replacement instead of copying a live WAL-backed database.
- Diverged or unrelated state histories are never silently merged or overwritten. Aurora retains both files and reports the conflict for manual resolution.
- A newer OneDrive database never replaces an open local SQLite database; a clean update is adopted only before Aurora opens local state.

### Known limits

- Laptop Mode has fixed roots for Jørn's current two-machine layout; there is no editable mapping UI in 0.7.0.
- Automatic synchronization assumes Aurora is not actively edited on both computers at once. Detected divergence requires choosing which retained state to keep and restarting Aurora.
- OneDrive availability and propagation remain external dependencies; Aurora continues to browse the local catalog when the mirror folder is unavailable.

## [0.6.0] - 2026-08-22

### Added

- Observatory review queue with bounded local pages, text search, conflict/unconfirmed/decided filters, and a persistent artist inspector.
- Explicit artist confirmation, ignore, clear, release-to-local-album link, not-in-scope, release ignore, and release clear actions.
- Aurora-owned MusicBrainz decision tables and append-only curation history in state schema version 5, including one-step undo across restarts.
- Complete Music Library overlay-compatible SQLite snapshot export with an Aurora manifest and preserved pre-existing overlay rows.
- Browser previews and Rust/React coverage for candidate precedence, restart persistence, undo, local-album uniqueness, release validation, and exact export values.

### Changed

- Explicit Aurora artist decisions take display precedence over imported sources while any external MBID disagreement remains visible as a conflict.
- Release decisions display their exact provenance and only become editable after an authoritative artist identity has been established.
- Album links are restricted to an album belonging to the same normalized artist; a local album can belong to at most one Aurora release decision.
- Clearing or changing an artist identity is blocked while dependent Aurora release mappings would be orphaned.

### Performance

- Review pages are capped at 100 rows and scan at most five bounded, indexed batches from the imported candidate-bearing artist-info table.
- MusicBrainz databases remain absent from startup and ordinary Explorer queries.

### Security

- The live catalog, broad cache, and shared MusicBrainz overlay remain read-only. Export first creates a consistent new snapshot and applies Aurora decisions in one SQLite transaction.
- Candidate confirmation accepts only a valid UUID currently present in the selected artist's local sources; release decisions require a visible release group and a same-artist local album.
- Export maps Aurora's internal `linked` state to Music Library's exact `include` value and uses its timestamp and tombstone contract.

### Known limits

- Observatory 0.6.0 reviews candidate-bearing artists represented in the imported MusicBrainz artist-info table; it does not enumerate every catalog artist.
- Export produces a new overlay file in Aurora's app-data folder. Aurora does not silently replace or live-sync the shared OneDrive overlay.

## [0.5.0] - 2026-08-22

### Added

- Constellations artist inspector opened from universe planets, Artist Explorer results, and the Artist tab for a selected track.
- Lazy read-only access to the broad MusicBrainz cache and curated overlay, with independent connected/unavailable source states.
- Honest artist identity states for verified overlay links, unconfirmed catalog/cache candidates, source conflicts, ignored links, and unmatched artists.
- MBID-gated artist type, active dates, area, begin area, end area, and origin country from the existing catalog import.
- Bounded MusicBrainz release-group discographies with year, primary/secondary type, status provenance, and curated release decisions.
- Browser-preview and native coverage for populated, unmatched, conflicting, loading, error, source-fallback, and 100-row truncation behavior.

### Changed

- Curated overlay identity wins when local sources disagree, but Aurora surfaces the conflict instead of silently presenting the result as uncontested.
- Verified release groups use one source at a time: external curated overlay, embedded catalog mirror fallback, then broad cache fallback. Refreshed and stale discographies are never unioned.
- Selecting a track returns the persistent inspector to Track editing; Artist context stays reachable without entering the MusicBrainz path at startup.
- Explorer invalidates late album-detail responses and clears stale load-more state when a new bounded page request begins.
- Sparse Last.fm data is labeled popularity rather than personal plays.

### Performance

- MusicBrainz work remains off the startup and Explorer hot paths and runs only when an artist context is opened.
- Live indexed identity lookups completed below timer resolution; a worst-case 6,017-group cache artist sorted and returned 101 rows in approximately 2–5 ms warm.

### Security

- All three SQLite sources open read-only with `query_only`, short busy timeouts, bound parameters, and a 100-release response cap.
- Artist keys use the Music Library normalization contract, including Unicode dash folding, Unicode lowercase, trim, and whitespace collapse.
- Cache exact-name matches are never marked verified; audited cache ambiguity and curated/cache conflicts remain visible.
- No online MusicBrainz synchronization, catalog write, MP3 write, fuzzy match acceptance, or arbitrary filesystem path is introduced by Constellations.

## [0.4.0] - 2026-08-22

### Added

- Deep Explorer views for Tracks, Albums, and Artists with opaque keyset cursors, 50-row pages, and native hard caps of 100 records.
- Exact half-star/unrated, Love/Neutral/Ban, release-year, genre, artist, and safely quoted full-text filters with validated view-specific sorts.
- Album cover grid and bounded album details with playback, keyboard row navigation, and immediate rating/Love controls.
- Artist drill-down from both Explorer results and universe planets; the exact artist focus carries across its track and album views.
- Bounded focus-time reconciliation for pending Aurora tag overlays when MusicBee changes an MP3 externally.

### Changed

- Explore now uses feature-owned responsive layouts that keep rating and Love controls visible at Aurora's default Windows size.
- A selected MP3 refreshes on application focus unless the inspector contains unsaved work.
- Browser preview tag edits now survive Explorer reloads just as native Aurora overlays do.
- Pending files that cannot currently be read rotate behind later reconciliation work instead of starving the queue.

### Performance

- Live checks on 1,096,162 tracks measured common bounded queries at approximately 26–84 ms including SQLite process startup.
- Global title A–Z remains the borderline path at approximately 120 ms because the shared catalog has no title-only index.

### Security

- Every explorer value uses bind parameters; only validated sort enums affect SQL structure, and mismatched cursor/sort pairs are rejected.
- Explorer and detail commands preserve SQLite read-only/query-only enforcement and never return more than 100 records.
- External synchronization reads only pending-overlay MP3s, never scans the library, never writes an MP3, and preserves Aurora's operation journal and undo history.
- MP3 tags remain authoritative while Music Library's imported SQLite catalog remains read-only.

## [0.3.1] - 2026-08-22

### Added

- Direct half-star and full-star hit areas on every Explore row, plus a directly clickable Love control.
- Per-track saving, pending-import, and conflict feedback for immediate MP3 tag writes.

### Changed

- Explore rating and Love clicks now save and verify through the existing safe MP3 writer immediately, without requiring the inspector's Save to MP3 button.
- Inline edits update optimistically, prevent overlapping writes to the same track, reconcile the inspector after confirmation, and roll back the row if the native save fails.

## [0.3.0] - 2026-08-22

### Added

- MusicBee-compatible MP3 editing for half-star rating, Love/Neutral/Ban, and Release Year.
- Durable tag-operation journal, same-folder working copies, retained rollback backups, startup crash recovery, and one-step verified undo.
- Aurora-owned tag overlay that reflects file edits immediately and reconciles after a later MusicBee TSV import.
- Stable normalized-path track identity for restoring playback queues across Music Library imports that replace integer track IDs.
- Recovery coverage for the crash window immediately after atomic replacement and before its journal checkpoint.
- Recovery for Windows `ReplaceFileW` partial-failure states where the original has moved to Aurora's backup but the canonical MP3 path is temporarily absent.
- Conservative conflict recovery that only completes a known Aurora file state and retains every copy instead of overwriting an ambiguous external edit.
- Browser-preview coverage for save, undo, half-stars, and stale-edit conflicts.

### Changed

- The inspector now reads current tags from the selected MP3 and exposes a compact metadata editor.
- Rating display and editing support MusicBee's complete 0.5–5.0 scale.
- Aurora state schema advances to version 4 for stable queue references, tag overlays, save history, and crash-recoverable undo.
- Half-star catalog reads fall back to validated `rating_raw` values when Music Library leaves `normalized_rating` empty.
- Queue restoration preserves surviving entries when individual files have disappeared, and tag refreshes no longer reset filtered selection.

### Security

- Rust resolves every edit target from a bounded catalog ID; React cannot submit an arbitrary file path.
- Writes preserve the existing ID3 version, non-target frames, ID3v1/trailing bytes, and MP3 audio payload.
- Aurora verifies path identity, size, timestamps, target frames, preserved frames, and audio hash around atomic `ReplaceFileW` replacement.
- Native play and tag commands require both the transient catalog ID and stable path key; Windows blocks concurrent writers during the final check and replacement.
- Undo refuses to replace a file when any unrelated ID3 frame, ID3v1/trailing byte, or audio byte changed after Aurora's edit.
- The shared Music Library catalog remains enforced read-only; only Aurora's private SQLite state and explicitly saved MP3 tags are writable.

## [0.2.0] - 2026-08-21

### Added

- Native MP3 playback with play/pause, seek, previous/next, volume, shuffle, repeat-all, and repeat-one controls.
- Durable, bounded listening queue with play-now, reorder, remove, and clear actions.
- Aurora-owned SQLite state for queue order, current item, position, volume, shuffle, and repeat state across restarts.
- Contained `aurora-cover` protocol with exact album-ID resolution and cached 64–512 px WebP thumbnails.
- Browser-preview playback simulator and interaction coverage for transport and queue behavior.

### Changed

- Project testing guidance now prefers Browser-based UI inspection and limits Computer Use to native-only behavior that Browser cannot exercise.
- Track rows and the inspector now expose direct play actions; the persistent footer is a functional player.

### Security

- Playback accepts bounded catalog track IDs and re-resolves file paths in Rust; the WebView cannot request arbitrary files.
- Album art is canonicalized beneath the configured archive and source images above 32 MiB are rejected.
- The imported catalog and audio metadata remain read-only; file-tag editing is not part of 0.2.0.

## [0.1.0] - 2026-08-21

### Added

- Tauri 2 Windows application scaffold with a React and TypeScript interface based on the supplied Aurora design.
- Read-only SQLite overview, top-artist universe, artist drill-down, and bounded five-star track page for the existing music catalog.
- Safely quoted FTS5 prefix search across the complete catalog with stale-request protection in the interface.
- Read-only track inspector that distinguishes MusicBee rating, Love, Release Year, and Last.fm popularity.
- Aurora application icon, keyboard search shortcut, reduced-motion handling, loading, empty, and source-error states.
- Signed Tauri updater checks at startup and every minute with in-app download/install progress.
- Pinned GitHub Actions CI and Windows NSIS release workflows, aligned-version validation, and updater signing secrets.
- Verified database, 98.09%-coverage album-art, and MusicBee tag contracts for the next cover, playback, and metadata-writing sections.

### Security

- The source catalog opens with SQLite read-only flags and `query_only`; Aurora does not modify the live catalog or audio files in this release.
- Release workflow actions are pinned to immutable commit SHAs and updater private material is kept outside source control.
