# Aurora

Aurora is a fast, local-first Windows 11 explorer and player for a personal music universe. Version 0.3.0 adds transactional MusicBee-compatible MP3 rating, Love/Ban, and Release Year edits while keeping the imported Music Library catalog read-only.

![Aurora design reference](Aurora.png)

## Current 0.3.0 slice

- Tauri 2, Rust, React, TypeScript, and Vite Windows application.
- Strictly read-only access to `%APPDATA%\com.local.musiclibrary\music-library.sqlite3`.
- Bounded startup payload: summary, eight high-volume artists, and 50 five-star tracks.
- Clickable artist planets that query 50 tracks for the selected artist.
- Debounced, safely quoted FTS5 prefix search across the entire catalog.
- Inspector editor for half-star ratings, Love/Neutral/Ban, and Release Year, plus read-only genre, duration, and optional Last.fm popularity.
- Native MP3 playback with play/pause, seek, previous/next, volume, shuffle, and repeat-one/repeat-all controls.
- A bounded 200-track queue with play-now, reorder, remove, and clear actions.
- Durable queue, current track, position, volume, shuffle, and repeat state in Aurora's own SQLite database.
- Stable queue identity based on the normalized MP3 path, verified alongside every transient track ID so queue items survive Music Library TSV imports without being retargeted.
- Transactional same-folder MP3 writes using MusicBee's exact POPM byte map, `LOVE RATING`, and Release Time conventions.
- Conflict detection, post-write tag/audio verification, Windows atomic replacement, retained rollback copies, crash recovery, and one-step undo. Ambiguous or externally changed files are never auto-overwritten; Aurora retains both versions for manual recovery.
- Aurora-owned tag overlays that update the UI immediately and reconcile automatically after a later MusicBee TSV import updates Music Library.
- Half-star track reconciliation reads Music Library's raw rating when its older normalized field is null; removed-track overlays are excluded from library totals.
- Album covers served through a narrow Rust protocol that resolves exact album IDs, contains canonical paths to the configured archive, rejects oversized sources, and caches 64–512 px WebP thumbnails.
- Packaged-app update checks at startup and every 60 seconds, with an Aurora-styled install prompt.
- Windows NSIS release workflow with mandatory Tauri updater signatures.

The MP3 is authoritative for Aurora tag edits. Aurora never writes the shared Music Library SQLite database; it records a small optimistic overlay in its own state database until the normal MusicBee TSV export and Music Library import catch up. See [docs/tag-editing-contract.md](docs/tag-editing-contract.md), [docs/musicbee-tags.md](docs/musicbee-tags.md), and [docs/playback-contract.md](docs/playback-contract.md).

## Data model

The primary catalog currently contains 1,096,162 MP3 tracks, 72,000 albums, and 20,392 album artists. Aurora opens the active WAL-backed database with SQLite read-only flags and `query_only`; it does not use immutable mode and does not write ratings back into this imported catalog.

The broad MusicBrainz cache and curated overlay are deferred from startup. See [docs/database-contract.md](docs/database-contract.md) for verified sizes, responsibilities, query limits, and authority rules.

The album-cover archive at `C:\_code\music_backup_v5\AlbumCovers` contains 76,329 images and maps to current albums through `album_covers.album_id` with 98.09% coverage. Rust now resolves and decodes those images outside the WebView; missing or invalid art falls back to Aurora's generated artwork.

## Requirements

- Windows 11 with WebView2
- Node.js 22+
- Rust stable with the MSVC target and Windows C++ build tools
- The music catalog at the default `%APPDATA%` path above
- The referenced MP3 files and album-cover archive mounted at their cataloged paths for playback and real artwork

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

Push a SemVer tag matching all three manifests, for example `v0.3.0`. The release workflow builds a Windows NSIS setup executable, signs the updater artifact, publishes the GitHub release, and uploads `latest.json`.

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
