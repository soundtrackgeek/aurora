# Tag editing contract

Aurora 0.24.0 edits MusicBee-compatible metadata and album artwork in MP3 files while keeping `%APPDATA%\com.local.musiclibrary\music-library.sqlite3` strictly read-only.

## Selection and write intent

- The right-side **Tags** tab accepts either one selected track or every cataloged track in one selected album. Album selection is complete and capped at 500 MP3s; it is never a visible-page subset.
- Aurora reads all editable values from the MP3 files. A field is **Mixed** when at least two selected files differ.
- Only fields whose checkbox is selected are part of the save. Typing into a field selects it automatically. An untouched Mixed field is omitted, and a selected blank field explicitly removes that tag. Album Artist, Album, and Track Title cannot be cleared because Music Library requires them for a safe existing-folder sync.
- Editable fields are Album Artist, Artist, Album, Track Title, Genre, Publisher, Track Rating, Year, Release Year, track number/total, and disc number/total.
- One complete album selection also exposes its cover above Album Artist. Clicking it opens the native image picker in the album directory. The selected JPG, PNG, GIF, BMP, or WebP is validated within a 32 MiB and 100-megapixel bound, represented in React by an opaque token, and remains a draft until Save. Track and mixed-album selections do not expose album artwork.
- Artist is MusicBee's display override (`TXXX/DISPLAY ARTIST`). If it is absent, Aurora displays the underlying `TPE1` performer credits joined with `; `; saving Artist never rewrites those credits. Album Artist maps to the actual multi-value `TPE2` credits.
- Track and disc numbers share one ID3 frame with their totals. Changing only a total preserves each file's existing number; removing a number also removes a total that cannot exist independently.

## Authority and identity

- React submits a catalog track/album identity, a complete per-file revision snapshot, selected field names, and desired values. It never submits filesystem paths.
- Rust resolves current rows from the read-only catalog and accepts only existing `.mp3` files with safe single-component filenames.
- The revision combines the canonical file identity, size, and timestamps. Every selected file is opened, decoded, re-read, and revision-checked before the first write.
- The MP3 is authoritative after save. Aurora rejects the whole batch before mutation when selection membership, file revision, or any expected editable value changed after the editor opened.

## Save transaction

For each file, Aurora:

1. Merges only the selected fields with that file's current values and validates text, half-star rating, year, track/disc constraints, and any selected album artwork.
2. Records a durable `prepared` journal operation containing legacy overlay values plus complete before/after editable values and the exact selected-field list.
3. Copies the MP3 to a uniquely owned same-folder working path and changes only the selected ID3 frame families. A cover change removes front-cover pictures and writes one normalized front-cover picture to every album MP3; non-front pictures remain untouched.
4. Flushes and re-reads the working copy, verifying the complete editable value set, the exact front-cover fingerprint when changed, every unselected parsed frame, and the SHA-256 of all bytes after ID3v2.
5. Opens a Windows handle that permits reads and replacement but denies concurrent writers, then rechecks canonical path, volume/file ID, size, modified time, and creation time.
6. Atomically replaces the MP3 with `ReplaceFileW`, retaining the original as the operation backup.
7. Checkpoints `replaced`, verifies the installed MP3 again, and atomically commits `verified` with Aurora's rating/Love/Release Year overlay.
8. Deletes the original backup and clears its journal path after the complete single-file or album operation succeeds.

The batch preflights all files before step 1 begins. If a later file fails, Aurora attempts to undo every earlier completed file in reverse order. Atomicity is therefore per MP3 rather than filesystem-wide: a failure reported after Windows replacement can require the retained backup and startup recovery, and Aurora states that uncertainty explicitly.

## Recovery and preservation

- Recovery metadata is operation-specific. Startup knows which frames Aurora intended to change and verifies the full before/after editable value set plus optional artwork fingerprints before completing or rejecting a recovery path.
- Except for an explicitly selected front-cover replacement, artwork, lyrics, ReplayGain, MusicBrainz identifiers, unknown frames, other POPM owners, ID3v1/trailing bytes, and audio bytes are outside the operation and must remain unchanged. Non-front pictures are always preserved.
- Successful operations do not retain an undo copy. Album writes keep their per-file originals only until every track has verified so a later failure can restore earlier tracks.
- Ambiguous, interrupted, or externally changed files are never auto-overwritten or prematurely cleaned up. Aurora retains the available files and records a candid recovery error.

## Catalog workflow

Aurora never updates Music Library's database directly. After verified MP3 writes it asks Music Library `0.144.1` or newer to synchronize the exact edited MP3 when that narrow path is safe, with complete-folder synchronization as the conservative fallback:

```text
Track/album selection
        ↓
verified MP3 write + durable exact-file/folder queue
        ↓
focused background Music Library guarded existing-folder sync
        ↓
explicit receipt + completed import revision → Aurora refreshes catalog-backed views
```

Music Library requires every target to remain inside a configured library root, rejects linked/reparse paths, preserves album identity, and refuses any preview that would add or remove tracks or albums. Aurora durably queues each affected folder and, when unambiguous, its exact filename in the same state transaction that verifies the MP3 write or undo. The verified edit returns before the companion process starts. Multiple different pending files in one folder deliberately collapse to a complete-folder request. Background calls are serialized, and receipts are token-checked so an older response cannot erase newer queued work.

If the companion is missing, outdated, or rejects the sync, the already-verified MP3 save remains successful and Aurora reports Music Library synchronization as pending. Startup and focus retry one folder, and a focused Aurora retries remaining work every five seconds. Successful receipts trigger an immediate revision-backed view refresh; the periodic revision check remains a fallback. Tag edits and external-tag reconciliation share one projection order so a delayed response cannot overwrite newer visible tag state.
