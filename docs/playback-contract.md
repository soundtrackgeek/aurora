# Playback contract

Aurora owns playback and queue state without claiming write ownership of the imported catalog. Version 0.8.0 also feeds playback transitions into the separate [listening-history contract](listening-history-contract.md).

## Trust boundary

- React sends catalog track IDs paired with their stable normalized path keys. A queue command accepts at most 200 pairs.
- Rust re-queries each ID, requires the current path key to match, and constructs the audio path from its indexed directory and single-component MP3 filename.
- Absolute filenames, nested filenames, non-MP3 extensions, missing records, and missing files are rejected.
- No native command accepts an arbitrary path from the WebView.

## Runtime behavior

- The audio device is opened lazily on the first play action, so browsing remains available when an output device is absent.
- Play/pause, seek, previous/next, volume, shuffle, repeat-all, and repeat-one are native operations.
- Starting a visible track replaces the current bounded queue with the current result set and begins at that track.
- Natural completion advances according to repeat and shuffle state while the frontend polls playback state.
- Native playback—not React polling—is responsible for beginning, observing, seeking, and finalizing listening-history sessions.

## Persistence

Aurora writes its own `aurora-state.sqlite3` under the Tauri application-data directory. Schema version 4 persists:

- queue order and current index;
- approximate playback position;
- volume;
- shuffle state;
- repeat mode.
- a normalized path key plus the exact indexed directory and filename for every queue entry.

Position is checkpointed in roughly ten-second buckets and once more during window shutdown. Queue and control changes are transactional. Restored sessions remain paused until the user explicitly resumes them. Exact directory and filename re-resolution keeps queues valid when a full TSV import replaces source track IDs; unavailable entries are skipped without discarding surviving tracks.

## Deliberate limits

Version 0.8.0 does not edit the source catalog, select an output device, apply ReplayGain/DSP, crossfade, or promise gapless transitions. Tag writes are isolated behind the separate [tag-editing contract](tag-editing-contract.md).
