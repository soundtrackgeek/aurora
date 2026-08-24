# Changelog

All notable Aurora changes are recorded here.

## [Unreleased]

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
