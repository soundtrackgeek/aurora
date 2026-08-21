# Playback contract

Aurora 0.2.0 owns playback and queue state without claiming ownership of the imported catalog or audio metadata.

## Trust boundary

- React sends only catalog track IDs. A queue command accepts at most 200 decimal IDs.
- Rust re-queries each ID from the read-only catalog and constructs the audio path from its indexed directory and single-component MP3 filename.
- Absolute filenames, nested filenames, non-MP3 extensions, missing records, and missing files are rejected.
- No native command accepts an arbitrary path from the WebView.

## Runtime behavior

- The audio device is opened lazily on the first play action, so browsing remains available when an output device is absent.
- Play/pause, seek, previous/next, volume, shuffle, repeat-all, and repeat-one are native operations.
- Starting a visible track replaces the current bounded queue with the current result set and begins at that track.
- Natural completion advances according to repeat and shuffle state while the frontend polls playback state.

## Persistence

Aurora writes its own `aurora-state.sqlite3` under the Tauri application-data directory. Schema version 1 persists:

- queue order and current index;
- approximate playback position;
- volume;
- shuffle state;
- repeat mode.

Position is checkpointed in roughly ten-second buckets and once more during window shutdown. Queue and control changes are transactional. Restored sessions remain paused until the user explicitly resumes them.

## Deliberate limits

Version 0.2.0 does not write MP3 tags, edit the source catalog, select an output device, apply ReplayGain/DSP, crossfade, or promise gapless transitions. Those are separate slices because they require different integrity and audio-quality tests.
