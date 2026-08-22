# Changelog

All notable Aurora changes are recorded here.

## [Unreleased]

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
