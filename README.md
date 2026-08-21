# Aurora

Aurora is a fast, local-first Windows 11 explorer for a personal music universe. Version 0.1.0 establishes the Tauri 2 desktop shell, reads the existing catalog without modifying it, turns top artists into clickable planets, searches the million-track FTS index, and ships through signed in-app updates.

![Aurora design reference](Aurora.png)

## Current 0.1.0 slice

- Tauri 2, Rust, React, TypeScript, and Vite Windows application.
- Strictly read-only access to `%APPDATA%\com.local.musiclibrary\music-library.sqlite3`.
- Bounded startup payload: summary, eight high-volume artists, and 50 five-star tracks.
- Clickable artist planets that query 50 tracks for the selected artist.
- Debounced, safely quoted FTS5 prefix search across the entire catalog.
- Read-only inspector for rating, Love, Release Year, genre, duration, and optional Last.fm popularity.
- Packaged-app update checks at startup and every 60 seconds, with an Aurora-styled install prompt.
- Windows NSIS release workflow with mandatory Tauri updater signatures.

Playback and tag editing are intentionally not enabled yet. The exact MusicBee MP3 conventions are documented in [docs/musicbee-tags.md](docs/musicbee-tags.md), but file mutation needs its own backup, verification, rollback, and concurrency-tested slice.

## Data model

The primary catalog currently contains roughly 1.095 million MP3 tracks, 71,952 albums, and 20,379 album artists. Aurora opens the active WAL-backed database with SQLite read-only flags and `query_only`; it does not use immutable mode and does not write ratings back into this imported catalog.

The broad MusicBrainz cache and curated overlay are deferred from startup. See [docs/database-contract.md](docs/database-contract.md) for verified sizes, responsibilities, query limits, and authority rules.

The album-cover archive at `C:\_code\music_backup_v5\AlbumCovers` contains 76,329 images and already maps to current albums through `album_covers.album_id` with 98.09% coverage. 0.1.0 keeps lightweight generated placeholders; the cover section will add a contained, thumbnail-caching protocol instead of exposing or decoding large originals in the WebView.

## Requirements

- Windows 11 with WebView2
- Node.js 22+
- Rust stable with the MSVC target and Windows C++ build tools
- The music catalog at the default `%APPDATA%` path above

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

Push a SemVer tag matching all three manifests, for example `v0.1.0`. The release workflow builds a Windows NSIS setup executable, signs the updater artifact, publishes the GitHub release, and uploads `latest.json`.

Before tagging a new version:

1. Update `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` to the same version.
2. Move the relevant changelog notes from `Unreleased` into a dated version section.
3. Run `npm run check:version` and the full verification commands above.
4. Commit and push, then push the matching `vX.Y.Z` tag.

The repository already has `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` Actions secrets. The local encrypted key material is under `%USERPROFILE%\.tauri`:

- `aurora-updater.key` — encrypted private updater key
- `aurora-updater.key.pub` — public updater key
- `aurora-updater-password.dpapi.xml` — passphrase protected for the current Windows account

Back up the private key and passphrase separately and securely. Losing either prevents installed Aurora copies from accepting future updates. This updater signature proves artifact integrity; it is separate from optional Windows Authenticode signing, so unsigned 0.1.0 installers may still show an Unknown publisher/SmartScreen warning.

## Architecture brief

The behavioral scope, performance target, source-of-truth decisions, and next sections are captured in [docs/app-brief.md](docs/app-brief.md).
