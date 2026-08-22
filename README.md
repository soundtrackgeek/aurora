# Aurora

Aurora is a fast, local-first Windows 11 explorer and player for a personal music universe. Version 0.5.0 adds lazy, provenance-aware MusicBrainz artist constellations while keeping the imported Music Library catalog read-only and MP3 tags authoritative.

![Aurora design reference](Aurora.png)

## Current 0.5.0 slice

- Tauri 2, Rust, React, TypeScript, and Vite Windows application.
- Strictly read-only access to `%APPDATA%\com.local.musiclibrary\music-library.sqlite3`.
- Bounded startup payload: summary, eight high-volume artists, and 50 five-star tracks.
- Keyset-paged Tracks, Albums, and Artists views that request 50 rows at a time and never hold a million-row result in the WebView.
- Exact half-star/unrated, Love/Neutral/Ban, release-year, genre, and artist filters, plus safely quoted FTS5 prefix search across the entire catalog.
- Validated sorts for newest, title, artist, album, release year, rating, and artist track count; opaque cursors cannot be reused with a different sort.
- Clickable artist planets and artist results that open an exact artist focus which can be switched between tracks and albums.
- A functional Constellations artist inspector opened from universe planets, Artist results, or the selected track.
- Lazy local MusicBrainz identity resolution with verified, unconfirmed, conflict, ignored, and unmatched states; only curated overlay links are labeled verified.
- MBID-gated artist type, active dates, area, birthplace, and origin-country context from the existing catalog import.
- Source-precedence release-group discographies capped at 100 rows: curated overlay first for verified identities, catalog mirror fallback, then the broad cache without mixing stale and refreshed sources.
- Visible local provenance and source availability for the catalog, curated overlay, and broad cache; missing optional databases never block normal library browsing.
- Album cover grids with bounded album track details, playback activation, keyboard row navigation, and inline tag controls.
- Inspector editor for half-star ratings, Love/Neutral/Ban, and Release Year, plus read-only genre, duration, and optional Last.fm popularity.
- Direct Explore-row rating and Love controls: click either half of a star for an exact 0.5 step or click the heart to toggle Love, and Aurora saves to the MP3 immediately with per-row verification feedback.
- Native MP3 playback with play/pause, seek, previous/next, volume, shuffle, and repeat-one/repeat-all controls.
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

The MP3 is authoritative for Aurora tag edits. Aurora never writes the shared Music Library SQLite database; it records a small optimistic overlay in its own state database until the normal MusicBee TSV export and Music Library import catch up. See [docs/tag-editing-contract.md](docs/tag-editing-contract.md), [docs/musicbee-tags.md](docs/musicbee-tags.md), and [docs/playback-contract.md](docs/playback-contract.md).

## Data model

The primary catalog currently contains 1,096,162 MP3 tracks, 72,000 albums, and 20,392 album artists. Aurora opens the active WAL-backed database with SQLite read-only flags and `query_only`; it does not use immutable mode and does not write ratings back into this imported catalog. Live checks measured common bounded explorer paths at approximately 26–84 ms including SQLite process startup. Global title A–Z is the known slower path at approximately 120 ms because the shared catalog has no title-only index.

The broad MusicBrainz cache and curated overlay are deferred from startup and opened independently only when the Artist inspector requests context. The audited cache contains 20,208 artist-name rows and 483,675 release groups; the curated overlay contains 493 verified artist links and 9,658 release groups. Cache-only identities remain unconfirmed because 44 audited exact-name candidates conflict with verified links and many MBIDs are shared by multiple names. See [docs/database-contract.md](docs/database-contract.md) for verified responsibilities, limits, and authority rules.

The album-cover archive at `C:\_code\music_backup_v5\AlbumCovers` contains 76,329 images and maps to current albums through `album_covers.album_id` with 98.09% coverage. Rust now resolves and decodes those images outside the WebView; missing or invalid art falls back to Aurora's generated artwork.

## Requirements

- Windows 11 with WebView2
- Node.js 22+
- Rust stable with the MSVC target and Windows C++ build tools
- The music catalog at the default `%APPDATA%` path above
- The referenced MP3 files and album-cover archive mounted at their cataloged paths for playback and real artwork
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

Browser development uses clearly labeled preview records. Prefer the Browser plugin for UI testing and visual inspection; reserve Computer Use for native-only behavior that Browser cannot exercise. Only the Tauri runtime exercises the real SQLite boundary.

Build a local NSIS installer with:

```powershell
npm run tauri -- build
```

## Releases and in-app updates

Push a SemVer tag matching all three manifests, for example `v0.5.0`. The release workflow builds a Windows NSIS setup executable, signs the updater artifact, publishes the GitHub release, and uploads `latest.json`.

Before tagging a new version:

1. Update `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` to the same version.
2. Move the relevant changelog notes from `Unreleased` into a dated version section.
3. Run `npm run check:version` and the full verification commands above.
4. Commit and push, then push the matching `vX.Y.Z` tag.

The repository already has `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` Actions secrets. The local encrypted key material is under `%USERPROFILE%\.tauri`:

- `aurora-updater.key` — encrypted private updater key
- `aurora-updater.key.pub` — public updater key
- `aurora-updater-password.dpapi.xml` — passphrase protected for the current Windows account

Back up the private key and passphrase separately and securely. Losing either prevents installed Aurora copies from accepting future updates. This updater signature proves artifact integrity; it is separate from optional Windows Authenticode signing, so installers may still show an Unknown publisher/SmartScreen warning.

## Architecture brief

The behavioral scope, performance target, source-of-truth decisions, and next sections are captured in [docs/app-brief.md](docs/app-brief.md).
