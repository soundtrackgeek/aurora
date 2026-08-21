# Aurora application brief

## Job and product shape

Aurora is a personal Windows 11 desktop application for navigating a very large, unusually rich music library at interactive speed. The core journey is: launch, understand the library at a glance, choose an artist planet or search, select a track, and keep listening context visible.

The app is local-first and offline-capable. Rust owns SQLite, filesystem, audio, tagging, updater, and Windows boundaries. React owns presentation, selection, transient filters, and request reconciliation.

## Authorities and identity

- The existing music-library database is authoritative for catalog display, but Aurora opens it read-only.
- Audio files become authoritative for rating, Love/Ban, and Release Year after the future verified tag writer lands.
- Aurora-owned preferences, optimistic writes, and operation journals belong in a separate writable Aurora database.
- Existing integer track IDs and opaque string album IDs are preserved. Artist identity needs an explicit normalized key before cross-database joins; display text is never used as a durable provider identity.
- The MusicBrainz overlay is curated sync/export state. The broad cache is a lazy discovery source, not a startup dependency.
- Album covers resolve only through the catalog's exact `album_id` mapping. Filename normalization is not an identity strategy.

## 0.1.0 acceptance checks

- Launch without modifying the 3+ GiB catalog or its WAL.
- Return a bounded startup payload rather than serializing the million-track library.
- Click any visible artist planet and receive up to 50 indexed tracks.
- Search the whole catalog with safely quoted FTS prefix terms; late results cannot replace a newer query.
- Preserve distinct loading, empty, unavailable, preview, and populated states.
- Check for a signed update on packaged startup and every 60 seconds without overlapping checks.
- Produce a Windows GUI executable and NSIS updater artifact with aligned `0.1.0` versions.

## Performance budget

Warm source measurements are approximately 26–89 ms for the current bounded queries before WebView transport. The 0.1.0 UI requests no more than 50 tracks per command. Expensive MusicBrainz enrichment never sits on the startup path.

## Explicit non-goals for 0.1.0

- Audio playback, queueing, gapless output, ReplayGain, and device selection.
- Writing ratings, Love/Ban, Release Year, or any other file tags.
- Editing the imported catalog directly.
- Album art extraction, biographies, lyrics, playlists, or MusicBrainz discovery UI.
- Authenticode publisher signing; updater cryptographic signing is configured separately.

## Planned sections

1. Playback engine and durable queue.
2. Transactional MP3 rating/Love/Release Year writer with backup, verification, rollback, and MusicBee conflict detection.
3. Album/artist routes, contained cover thumbnail protocol, and keyset-paginated library browsing.
4. Aurora-owned state database and listening history.
5. Lazy MusicBrainz discovery and curated overlay workflows.
