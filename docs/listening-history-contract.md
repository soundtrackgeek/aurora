# Listening history contract

Aurora 0.8.0 records playback sessions locally and combines them across Aurora installations without putting high-frequency listening events into the conflict-protected shared state database.

## Files and ownership

- This installation owns `%APPDATA%\com.soundtrackgeek.aurora\aurora-history.sqlite3`.
- Its stable device ID and display name live in the device-local `aurora-device.json` file.
- Aurora publishes a validated snapshot named `aurora-history-{device-id}.sqlite3` under `%USERPROFILE%\OneDrive\_musicbackup`.
- Each installation writes only its own local history and its own named snapshot. Peer snapshots are opened read-only and are never merged back into a writable database.
- `aurora-state.sqlite3` remains the source of truth for queues, playback settings, tag overlays/journals, and MusicBrainz curation. Listening sessions do not change its sync generation.

If local history is missing at startup and this device's valid named OneDrive snapshot exists, Aurora restores that snapshot before opening the local database. It never restores another device's snapshot into the local writer. If the matching remote snapshot exists but cannot be validated or read, Aurora starts a safe local journal, reports the problem, and blocks publication to that remote name so the questionable copy is never overwritten.

## When a session is recorded

Starting a catalog track, or resuming a track restored from a previous launch, begins a session. Pausing and resuming within the same launch continues that session without counting the paused interval. Aurora stores the normalized stable track key plus a metadata snapshot, so old history remains understandable after a Music Library import changes transient integer IDs or removes a track.

The default played threshold is 30 seconds and can be configured per installation from 1 through 3600 seconds in History. Each session keeps the threshold used by the device that recorded it. The rule is:

1. Only positive forward position changes observed while Aurora is playing accumulate.
2. A seek checkpoints the prior position and resets the timing baseline at the destination; the skipped distance does not count.
3. A track shorter than the configured threshold registers when its natural duration is reached.
4. Registration is monotonic for a session. Raising the threshold cannot retract a play that already registered; lowering it can register the active session immediately when its accumulated time already qualifies.

Aurora checkpoints accumulated time in 30-second buckets and when a play first registers. Normal completion, next/previous, queue replacement, removal, clearing, and clean shutdown finalize the current session. A session left active after an abnormal exit is marked interrupted on the next startup.

Outcomes describe how a session ended, independently of whether it crossed the played threshold:

- `completed`: the track reached its natural end.
- `skipped`: the user or queue moved away before natural completion.
- `interrupted`: Aurora stopped without a normal completion/skip transition.
- `active`: the current unfinished session.

## Cross-device snapshots

Aurora uses SQLite's consistent-copy mechanism, validates schema ownership and `quick_check`, then atomically replaces only this device's OneDrive snapshot. It publishes at most once per minute during playback on a background thread, outside the playback runtime lock, and forces a final snapshot on clean shutdown.

History queries open the local database and up to 16 named peer snapshots read-only, skip corrupt or unsupported peers, de-duplicate this device's own remote snapshot, and combine bounded results in memory. The timeline returns at most 100 sessions per request and uses a keyset cursor for earlier pages. Catalog resolution is bounded to the returned sessions and top tracks; a missing catalog file remains visible as unavailable history rather than being discarded.

This design avoids the weak assumption that two computers can safely write the same SQLite history file through OneDrive. OneDrive propagation can still be delayed, so a peer's newest sessions appear only after its snapshot reaches the current machine. Local recording continues when OneDrive is unavailable.

## Scope and privacy

History stays on the user's computer and configured OneDrive folder. Aurora does not submit plays to Last.fm, MusicBrainz, or another service. Imported Last.fm popularity remains separate from Aurora's personal registered-play count.
