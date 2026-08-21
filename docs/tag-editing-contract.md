# Tag editing contract

Aurora 0.3.0 edits MusicBee-compatible metadata in MP3 files while keeping `%APPDATA%\com.local.musiclibrary\music-library.sqlite3` strictly read-only.

## Authority and identity

- React submits a bounded catalog track ID, its stable normalized path key, and tag values, never a filesystem path. Rust requires both identities to match the same current row.
- Rust resolves the ID from the read-only catalog and accepts only an existing `.mp3` with a single-component filename.
- The MP3 becomes authoritative for rating, Love/Ban, and Release Year after save.
- Aurora's private SQLite overlay supplies immediate UI state until the next MusicBee TSV export and Music Library import produces matching catalog values.
- A normalized directory-plus-filename key persists across full imports that replace integer track IDs.

## Save transaction

1. Validate a rating on MusicBee's 0.5–5.0 scale, Love state, and optional year from 1000–2999.
2. Re-read the MP3 and reject the save if its current target values differ from the editor's expected values.
3. Record a durable `prepared` operation with the source path identity, size, timestamps, and intended before/after values.
4. Copy the MP3 to a uniquely owned same-folder working path and mutate only MusicBee POPM, `LOVE RATING`, and Release Time.
5. Flush and re-read the working copy. Verify target values, every non-target parsed frame, and the SHA-256 of all bytes after ID3v2.
6. Open a Windows handle that permits reads and replacement but denies concurrent writers, then recheck canonical path, volume/file ID, size, modified time, and creation time.
7. Atomically replace it with `ReplaceFileW`, retaining the original as the operation backup.
8. Checkpoint `replaced`, verify the installed MP3 again, then atomically commit `verified` together with Aurora's overlay.

Any failure before replacement leaves the source untouched. After replacement, Aurora never auto-overwrites a target it cannot prove is the exact Aurora-written state. A verification or journal failure retains the target and recovery copies; startup completes only a known save or undo state and marks ambiguous/external changes as conflicts without writing to the MP3. A `prepared` operation with a retained backup is recognized as the post-replace crash window, while other prepared work files are removed.

Windows can report a failed `ReplaceFileW` after moving the original into the requested backup while leaving the replacement at its working path. Aurora keeps that operation recoverable. If the canonical path is absent and both remaining files exactly match the journaled source/replacement states, startup moves the verified replacement into the missing path without replace-existing semantics. If another file appears or either copy is ambiguous, Aurora leaves every file untouched.

## Undo and retention

Aurora retains the newest 20 verified or rolled-back operation backups globally. Undo is available only for the latest operation on a track, while the installed MP3 still contains Aurora's written values and every non-target ID3 frame, ID3v1/trailing byte, and audio byte still matches the retained original. Undo denies concurrent writers, replaces from a copy so the original backup is retained, verifies the result, and commits the rolled-back journal state together with the overlay. An `undoing` journal state makes its replacement window recoverable after a crash.

## Catalog workflow

Aurora never updates Music Library's database directly. The established workflow remains:

```text
Aurora edits MP3 → MusicBee rescan → MusicBee TSV export → Music Library import
        │                                                   │
        └── Aurora private overlay (pending) ───────────────┘ auto-reconciles
```
