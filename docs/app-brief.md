# Aurora application brief

## Job and product shape

Aurora is a personal Windows 11 desktop application for navigating a very large, unusually rich music library at interactive speed. The core journey is: launch, understand the library at a glance, choose an artist planet or search, select a track, and keep listening context visible.

The app is local-first and offline-capable. Rust owns SQLite, filesystem, audio, tagging, updater, and Windows boundaries. React owns presentation, selection, transient filters, and request reconciliation.

## Authorities and identity

- The existing music-library database is authoritative for catalog display, but Aurora opens it read-only.
- Audio files are authoritative for rating, Love/Ban, and Release Year after an Aurora edit.
- Aurora-owned playback state, optimistic tag overlays, and the tag-operation journal live in a separate writable Aurora database.
- Integer track IDs are transient because Music Library's full TSV import replaces rows. Aurora persists queue and overlay identity with the normalized directory plus filename; album IDs remain opaque strings.
- The MusicBrainz overlay is curated sync/export state. The broad cache is a lazy discovery source, not a startup dependency.
- Aurora's explicit MusicBrainz decisions live in its own writable state database; the imported overlay remains read-only until the user deliberately publishes an exported snapshot.
- Album covers resolve only through the catalog's exact `album_id` mapping. Filename normalization is not an identity strategy.

## 0.6.0 acceptance checks

- Open Observatory from the persistent navigation and page candidate-bearing artists without adding MusicBrainz work to startup or ordinary Explorer queries.
- Filter review items by needs-review, conflict, unconfirmed, Aurora decision, or all-candidate state and search by artist without exceeding 100 returned rows.
- Confirm only a currently visible valid candidate, ignore or clear an artist, preserve external conflict evidence, and survive a process restart.
- Require an authoritative artist before release curation; link only a visible release group to a local album owned by the same artist, or mark it not in scope/ignored.
- Prevent one local album from being linked to multiple MusicBrainz groups and prevent artist changes that would orphan release mappings.
- Undo the latest artist or release action from durable history.
- Export a new complete overlay-compatible SQLite snapshot while preserving the live catalog, cache, and OneDrive overlay byte-for-byte.
- Produce a Windows GUI executable and signed NSIS updater artifact with aligned `0.6.0` versions.

## 0.5.0 foundation checks

- Open Constellations from a universe planet, Artist result, or the selected track's Artist tab without adding MusicBrainz work to startup or Explorer queries.
- Resolve artists through the normalized local artist key and label only verified, non-ignored curated overlay links as verified.
- Surface unconfirmed exact catalog/cache candidates, curated conflicts, ignored links, unmatched artists, and per-source unavailability without blocking the catalog shell.
- Show catalog profile and origin fields only when their MBID exactly equals the selected identity.
- Return at most 100 release groups from one precedence-selected source: curated overlay, embedded catalog mirror fallback, or broad cache fallback.
- Never union refreshed overlay release groups with stale cache rows and never automatically infer a local album mapping from title similarity.
- Preserve request ordering when users switch artists rapidly and return to Track editing as soon as they select a track.
- Produce a Windows GUI executable and signed NSIS updater artifact with aligned `0.5.0` versions.

## 0.4.0 foundation checks

- Browse Tracks, Albums, and Artists beyond the initial 50-row snapshot through bounded keyset pages; no offset walk or million-row WebView state.
- Filter tracks by bounded search text, exact half-star or unrated state, Love/Neutral/Ban, release-year range, genre, and artist with stale-request protection.
- Sort only through explicit indexed or otherwise bounded choices and retain active filters and track selection while drilling down.
- Open a dedicated artist focus from a universe planet or explorer result, switch that focus between tracks and albums, then open an album's bounded track listing of up to 100 tracks.
- Play or queue a track collection from artist and album views while keeping the persistent player and inspector available.
- Preserve inline verified rating and Love writes in every track list and keep keyboard row navigation usable without triggering playback accidentally.
- Re-read only pending-overlay MP3 files when Aurora refreshes external MusicBee changes; never scan the full audio library for synchronization.
- Treat the MP3 as authoritative, update or remove stale overlays when the catalog catches up, preserve the operation journal, and surface skipped or invalid pending files without writing them.
- Keep every native explorer request validated and bounded to at most 100 records and preserve read-only SQLite enforcement.
- Produce a Windows GUI executable and signed NSIS updater artifact with aligned `0.4.0` versions.

## Existing foundation checks

- Launch without modifying the 3+ GiB catalog or its WAL.
- Return a bounded startup payload rather than serializing the million-track library.
- Click any visible artist planet and receive up to 50 indexed tracks.
- Search the whole catalog with safely quoted FTS prefix terms; late results cannot replace a newer query.
- Preserve distinct loading, empty, unavailable, preview, and populated states.
- Check for a signed update on packaged startup and every 60 seconds without overlapping checks.
- Start a selected catalog track through a bounded track-ID command without exposing its file path to React.
- Control play/pause, seek, previous/next, volume, shuffle, repeat-one, and repeat-all from the persistent footer.
- Reorder, remove, play, and clear a queue capped at 200 tracks.
- Restore queue order, current track, position, volume, shuffle, and repeat state after restart and after a full Music Library import changes track IDs.
- Reject a live play or tag command when its transient ID no longer matches its stable path key, and preserve available queue entries when individual files disappear.
- Resolve album covers by exact album ID through a contained Rust protocol and cached bounded thumbnails.
- Read a selected MP3's current MusicBee rating, Love/Ban, and Release Time without exposing its path to React.
- Save all three fields as one verified transaction using a same-folder working copy and Windows atomic replacement.
- Preserve non-target ID3 frames, tag version, ID3v1/trailing data, and audio bytes; retain both files without overwriting either when verification or journal recovery finds an ambiguous external change.
- Detect external edits before replacement, retain the latest 20 rollback copies, recover interrupted writes on startup, and support safe one-step undo.
- Show the file edit immediately from Aurora's own overlay without writing Music Library; clear it when a later catalog import matches.
- Reconcile 3.5/4.5 ratings from validated raw catalog values even when the current importer leaves its normalized field empty.
- Produce a Windows GUI executable and NSIS updater artifact with aligned release versions.

## Performance budget

Live source measurements including SQLite process startup are approximately 26–84 ms for common bounded queries. Global title A–Z is the known borderline path at approximately 120 ms because the shared catalog has no title-only index. The UI requests 50 rows at a time, native commands cap pages and details at 100 records, and playback accepts no more than 200 IDs. Expensive MusicBrainz enrichment and cover decoding never sit on the startup query path.

## Explicit non-goals for 0.6.0

- Gapless output, ReplayGain, crossfade, DSP, and output-device selection.
- Editing the imported catalog directly.
- Editing non-MP3 files or ID3 fields other than MusicBee rating, Love/Ban, and Release Time.
- Embedded album-art extraction, biographies, lyrics, playlists, or online MusicBrainz synchronization.
- Release editions, labels, formats, catalog numbers, recordings, works, aliases, and relationship edges; the audited local sources do not contain them.
- Automatic local-album-to-release-group assignment; title comparison is incomplete and can be ambiguous.
- Accepting cache-only identity candidates as verified or silently resolving a source conflict.
- Enumerating every catalog artist in Observatory; 0.6.0 covers candidate-bearing imported artist-info rows.
- Silently replacing or continuously syncing the shared OneDrive MusicBrainz overlay; export is deliberate and produces a new complete snapshot.
- A recursive filesystem watcher or full-library MP3 tag scan; synchronization is intentionally bounded to pending overlays and selected tracks.
- Authenticode publisher signing; updater cryptographic signing is configured separately.

## Planned sections after 0.6.0

1. Listening history, ReplayGain, device selection, and gapless playback research.
2. Smart playlists and saved explorer views after the browsing/filter contract is proven.
3. Broader MusicBrainz queue coverage or a deliberate two-way overlay sync after the export workflow is proven.
