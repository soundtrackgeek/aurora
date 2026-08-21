# Changelog

All notable Aurora changes are recorded here.

## [Unreleased]

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
