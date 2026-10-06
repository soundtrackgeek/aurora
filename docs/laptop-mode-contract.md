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

Lineage, generation, snapshot identity, and logical content revisions—not file modification times—determine whether publishing is safe. SQLite files are not expected to be byte-identical: local WAL layout and catalog-local bookkeeping can legitimately differ from a compact OneDrive snapshot.

When same-lineage branches disagree only in playback position, transient catalog track IDs, overlay import-run markers, or reconciliation timestamps, Aurora 0.7.1 verifies that their stable and user-authored state agrees and then adopts the canonical snapshot identity without replacing either database. If stable queues, playback settings, desired tags, edit journals, or curation differ—or the histories are unrelated—Aurora stops automatic replacement and reports a conflict. Both databases and any OneDrive conflict copies remain intact for manual recovery.

## Interrupted publication recovery

Aurora 0.28.28 writes a flushed, atomically installed device-local `aurora-state.publish.json` receipt before replacing the OneDrive snapshot. The receipt contains the previous local snapshot identity, the intended remote destination, the sealed snapshot metadata, and its SHA-256. It is separate from the shared database and is removed only after acknowledgement completes or Aurora verifies that installation never happened.

Startup and the native sync worker check a pending receipt before normal conflict detection. Only the exact published snapshot can advance the local acknowledgement. The local content revision is preserved, and only the revision captured in that snapshot becomes mirrored, so later queue, tag, and rating changes remain dirty and publish normally. Receipt cleanup is idempotent after an acknowledged publication. A peer that has advanced independently, a changed destination, a changed snapshot hash, or invalid recovery evidence does not authorize acknowledgement or replacement. Older conflicts without a publication receipt still require a reviewed recovery with both copies preserved.

State-sync status transitions and errors are recorded on a best-effort basis in device-local `aurora-state.sync.jsonl`, with `aurora-state.sync.previous.jsonl` retaining the previous rotation. Each file is bounded to about 1 MiB. They contain timestamps, Aurora version/process ID, sync destination, status, and error messages; they remain outside OneDrive synchronization.
