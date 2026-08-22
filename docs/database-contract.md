# Database contract

Verified read-only on 2026-08-22. All three databases passed `PRAGMA quick_check` during the initial audit; the live catalog counts below were refreshed for 0.3.0.

## Sources

| Source | Approximate size | Runtime role |
| --- | ---: | --- |
| `%APPDATA%\com.local.musiclibrary\music-library.sqlite3` | 3.02 GiB | Hot catalog, FTS, albums, ratings, Love, enrichment |
| `%USERPROFILE%\OneDrive\_musicbackup\musicbrainz_cache.db` | 145 MiB | Lazy broad MusicBrainz discovery cache |
| `%USERPROFILE%\OneDrive\_musicbackup\musicbrainz-overlay-sync.sqlite3` | 5.5 MiB | Curated sync/export overlay |
| `C:\_code\music_backup_v5\AlbumCovers` | 15.3 GiB | 76,329 source images for the contained thumbnail protocol |

The originally supplied cache path included a nonexistent `musicbrainz` directory. The verified filename is `musicbrainz_cache.db` directly under `_musicbackup`.

## Verified catalog scale

- 1,096,162 tracks, all with `.mp3` filenames
- 72,000 albums
- 20,392 distinct album artists
- 687 canonical genre values
- 148,400 normalized rated tracks, plus two current raw half-star rows
- 5,647 loved tracks, represented as `love = 'L'`
- 674,222 tracks with a distinct `release_year`

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
- Treat `rating_raw` values on MusicBee's exact half-star scale as the catalog fallback when the current importer leaves `normalized_rating` null.
- Pending overlays from an older import remain durable but are excluded from summary totals when their exact track path is absent from the current import.

## Runtime repository shape

```text
LibraryRepository
├── catalog: read-only music-library.sqlite3
├── musicbrainz_cache: lazy read-only connection
├── overlay: lazy curated read-only/sync adapter
└── aurora_state: app-owned writable SQLite for playback, tag overlays, recovery, and curation
```

The overlay's current live rows already match the copies in the main database, so it is not an additional hot-path dependency.

## MusicBrainz identity and release-group contract

The 0.5.0 audit verified 20,208 `artist_cache` rows and 483,675 cache `release_groups`. The external overlay contains 493 verified artist links, 9,658 artist release groups, and one explicit release decision. Across the current catalog, verified links cover 483 artists, 2,052 albums, and 25,542 tracks. Exact-MBID catalog profile data exists for 464 of the 493 curated identities and origin-country data for 437.

Artist identity uses the indexed normalized `local_artist_key`: fold Unicode dash variants to ASCII `-`, Unicode-lowercase, trim, and collapse whitespace. Cache lookup binds that normalized value directly to the `artist_cache.name` primary key; it must not wrap the column in SQL `lower()`. Release groups join by indexed `artist_mbid` and use `(artist_mbid, release_mbid)` as identity.

Authority is intentionally asymmetric:

1. An explicit Aurora confirmation in `aurora-state.sqlite3` is the presentation authority and is labeled `auroraState`.
2. A verified, non-ignored curated overlay link is the imported authority when Aurora has no local decision.
3. The imported catalog identity and exact cache-name result are candidates, not authorities.
4. An authoritative choice wins when candidate MBIDs disagree, while the UI still reports the external conflict.
5. Without an authority, agreeing catalog/cache candidates remain `unconfirmed`; disagreeing candidates remain unresolved conflicts.
6. Release groups come from one source at a time: external overlay for a verified identity, embedded catalog mirror fallback, then broad cache fallback. Aurora never unions refreshed and stale snapshots.

This is necessary because 44 of 292 cache identities overlapping verified links disagree with the curated MBID. In addition, 1,656 cache MBIDs map to multiple names, one to 407 names. Aurora reports the cached-name count for the selected MBID and never presents an exact-name cache hit as verified.

Every enrichment command opens sources lazily with SQLite read-only flags, `query_only`, a two-second busy timeout, bound parameters, and a 100-release response cap. Missing, invalid, or busy optional sources become source-state data; they never fail normal catalog browsing. Live warm measurements were below timer resolution for indexed identity lookups and approximately 2–5 ms for sorting and limiting the worst observed 6,017-group cache artist.

The audited sources contain MusicBrainz release groups, not individual release editions. They do not contain label, format, catalog-number, recording, work, alias, or relationship-edge data. Local albums also lack a release-group MBID. Exact-title comparison is incomplete and ambiguous, so Aurora does not assign album identities automatically.

## Aurora curation and export contract

State schema version 5 adds `musicbrainz_artist_decisions`, `musicbrainz_release_decisions`, and `musicbrainz_curation_events`. Artist decisions are `confirmed` or `ignored`; clearing removes the current row while preserving an undo event. Release decisions are `linked`, `not-in-scope`, or `ignored`; `linked` requires a non-empty local album ID and a unique index prevents one album from being mapped to multiple release groups. Every mutation is transactional and records enough before/after state for one-step durable undo.

Confirmation accepts only a valid MusicBrainz UUID exposed by the artist's current local candidates. A release mutation requires an authoritative identity, a release group in the current bounded discography, and—when linked—an album whose normalized album artist equals the selected artist. Aurora blocks identity removal or replacement when doing so would orphan its own dependent release mappings.

The live OneDrive overlay is a read-only input. **Export overlay snapshot** creates a new database under Aurora's app-data `exports` folder, uses SQLite `VACUUM INTO` to preserve a consistent copy of the existing overlay when present, and applies all current Aurora decisions in one immediate transaction. Internal `linked` exports as Music Library's `include`; other decision names remain exact. Timestamps use SQLite UTC `YYYY-MM-DD HH:MM:SS`, compatible tombstones are cleared for exported live rows, and `aurora_export_manifest` records version and counts. The result is a complete overlay candidate, not a patch and not an automatic replacement of the shared file.

Observatory pages are capped at 100 rows. Version 0.6.0 discovers review items from candidate-bearing rows in the imported `musicbrainz_artist_infos` table, scanning at most five 100-row batches per request. This gives a fast useful queue but does not enumerate all catalog artists or perform online lookup.

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
