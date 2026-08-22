# Laptop Mode contract

Laptop Mode is a per-computer runtime view over one unchanged Music Library catalog. It does not rewrite a database path, mount a drive, or copy audio.

## Device setting and paths

Aurora stores the mode in `%APPDATA%\com.soundtrackgeek.aurora\aurora-device.json`. That file is deliberately excluded from the shared state snapshot, so the desktop can remain in Desktop Mode while the laptop restarts in Laptop Mode.

| Catalog root | Laptop filesystem root |
| --- | --- |
| `D:\MUSIC` | `Y:\MUSIC` |
| `G:\_BACKUP\SCORES` | `V:\_BACKUP\SCORES` |
| `H:\Synthwave` | `U:\Synthwave` |

Matching is case-insensitive and requires a complete root boundary. Aurora translates paths for playback, MP3 tag reads/writes, pending-tag reconciliation, and tag-journal recovery. SQLite queries and stable track keys continue to use the original catalog path.

## State lifecycle

The open source of truth is always local:

`%APPDATA%\com.soundtrackgeek.aurora\aurora-state.sqlite3`

Aurora mirrors verified snapshots to:

`%USERPROFILE%\OneDrive\_musicbackup\aurora-state.sqlite3`

On startup, a valid remote snapshot supplies a missing local file. If the remote has a newer generation and local has made no independent changes, Aurora retains `aurora-state.pre-onedrive.sqlite3` and applies the remote before opening SQLite. While running, local changes publish at most once per minute; normal shutdown forces a final snapshot. The previous remote is retained as `aurora-state.previous.sqlite3`.

## Conflict rule

Lineage, generation, snapshot identity, and logical content revisions—not file modification times—determine whether publishing is safe. If both machines changed from the same snapshot, or their histories are unrelated, Aurora stops automatic replacement and reports a conflict. Both databases remain intact. Version 0.7.0 intentionally requires manual selection of the state to keep, followed by an Aurora restart; it does not guess or merge records.
