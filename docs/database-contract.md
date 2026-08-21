# Database contract

Verified read-only on 2026-08-21. All three databases passed `PRAGMA quick_check`.

## Sources

| Source | Approximate size | Runtime role |
| --- | ---: | --- |
| `%APPDATA%\com.local.musiclibrary\music-library.sqlite3` | 3.02 GiB | Hot catalog, FTS, albums, ratings, Love, enrichment |
| `%USERPROFILE%\OneDrive\_musicbackup\musicbrainz_cache.db` | 145 MiB | Lazy broad MusicBrainz discovery cache |
| `%USERPROFILE%\OneDrive\_musicbackup\musicbrainz-overlay-sync.sqlite3` | 5.5 MiB | Curated sync/export overlay |
| `C:\_code\music_backup_v5\AlbumCovers` | 15.3 GiB | 76,329 source images for the contained thumbnail protocol |

The originally supplied cache path included a nonexistent `musicbrainz` directory. The verified filename is `musicbrainz_cache.db` directly under `_musicbackup`.

## Verified catalog scale

- 1,095,055 tracks, all with `.mp3` filenames
- 71,952 albums
- 20,379 distinct album artists
- 682 canonical genre values
- 148,272 rated tracks, normalized as `20`, `40`, `60`, `80`, or `100`
- 5,642 loved tracks, represented as `love = 'L'`
- 673,346 tracks with a distinct `release_year`

`year` and `release_year` are not aliases: 209,447 tracks have both populated with different values. Aurora must preserve Release Year as its own field.

## Access rules

- Open the live catalog read-only with URI/read-only flags and `query_only`.
- Do not use `immutable=1`; the source is actively WAL-backed and immutable mode can miss WAL changes.
- Return 50–200 rows per native command. Use FTS5 and keyset pagination rather than WebView-side million-row state.
- Bind parameters. Convert free user search to bounded quoted FTS prefix terms before binding.
- Do not depend on the source expression index using its nonstandard `unicode_lower` function unless Aurora registers a byte-compatible implementation.
- Treat album IDs as opaque strings even when they begin with `mb:`; they are not necessarily MusicBrainz UUIDs.
- There is no local listening-history field. `lastfm_track_popularity.play_count` is sparse global Last.fm popularity and must never be labeled as the user's plays.
- File metadata writes update audio files plus Aurora optimistic state. The source catalog reconciles later through its importer/rescan.

## Runtime repository shape

```text
LibraryRepository
├── catalog: read-only music-library.sqlite3
├── musicbrainz_cache: lazy read-only connection
├── overlay: lazy curated read-only/sync adapter
└── aurora_state: app-owned writable SQLite database for durable playback state
```

The overlay's current live rows already match the copies in the main database, so it is not an additional hot-path dependency.

## Album-cover mapping

The flat cover archive is 75,680 JPG, 635 PNG, 12 GIF, and 2 BMP files. Originals range up to 27.6 MiB, and one JPEG is zero bytes. The source was live during inspection.

The exact indexed mapping is:

```text
tracks.album_id
       │
       ▼
album_covers.album_id PRIMARY KEY
       │
       └── cache_path, mime_type, file_size_bytes
```

It matches 70,628 of 72,000 current albums: 98.09% coverage, 1,372 missing, and no ambiguity. Normalizing basenames recovers no additional albums and makes 25 mappings ambiguous. Aurora must therefore use `album_id`, not reconstructed or normalized `Artist - Album (Year)` filenames.

Aurora exposes a narrow `aurora-cover://album/{album-id}?size=256`-style protocol. Rust resolves the indexed path, canonicalizes and contains it within the configured cover root, rejects zero-byte and over-32-MiB originals, and caches 64/128/256/512 px WebP thumbnails using the album ID plus source mtime/size. Missing or invalid images fall back in React; the WebView receives neither base64 IPC payloads nor broad filesystem access.
