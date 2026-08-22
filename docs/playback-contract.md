# Playback contract

Aurora owns playback and queue state without claiming write ownership of the imported catalog. Version 0.8.0 also feeds playback transitions into the separate [listening-history contract](listening-history-contract.md); version 0.8.2 adds bounded, catalog-resolved waveform extraction; version 0.8.3 makes seek command ordering explicit; and version 0.10.0 adds the [audio-output contract](audio-output-contract.md).

## Trust boundary

- React sends catalog track IDs paired with their stable normalized path keys. A queue command accepts at most 200 pairs.
- Rust re-queries each ID, requires the current path key to match, and constructs the audio path from its indexed directory and single-component MP3 filename.
- Absolute filenames, nested filenames, non-MP3 extensions, missing records, and missing files are rejected.
- No native command accepts an arbitrary path from the WebView.

## Runtime behavior

- The selected audio device is opened lazily on the first play action, so browsing remains available when an output device is absent. A missing, failed, or disconnected preference falls back to the Windows default without changing the saved preference.
- Play/pause, seek, previous/next, volume, shuffle, repeat-all, and repeat-one are native operations.
- Starting a visible track replaces the current bounded queue with the current result set and begins at that track.
- During the final 15 seconds of a known-duration track, Aurora prepares the authoritative repeat/shuffle successor and appends it to the same native player. Natural audio handoff therefore does not wait for frontend polling; polling reconciles metadata and history after the source boundary.
- Every transport and global-shortcut action reconciles a prepared source boundary before resolving the current track.
- Optional ReplayGain is applied per source before the independent player-volume multiplier. See the audio-output contract for tag precedence and peak limiting.
- A stopped track positioned at its natural end restarts from zero when Play is pressed; an explicitly paused track resumes from its paused position.
- Range input displays a local draft only while its exact seek command is pending. The most recently issued command owns the resulting snapshot, older overlapping responses are ignored, and polling resumes after all active commands finish.
- Native playback—not React polling—is responsible for beginning, observing, seeking, and finalizing listening-history sessions.
- The player waveform samples 64 evenly spaced windows from the decoded MP3 stream and reduces them to 320 normalized peaks. It does not fully decode a song just to draw the timeline.
- Waveform requests contain only catalog ID plus stable track key. Rust performs the same identity and path validation as playback before opening the MP3.
- Player rating and Love controls use the tag-editing boundary; they do not mutate the imported catalog directly.

## Persistence

Aurora writes its own `aurora-state.sqlite3` under the Tauri application-data directory. The current schema persists:

- queue order and current index;
- approximate playback position;
- volume;
- shuffle state;
- repeat mode.
- a normalized path key plus the exact indexed directory and filename for every queue entry.

Position is checkpointed in roughly ten-second buckets and once more during window shutdown. Queue and control changes are transactional. Restored sessions remain paused until the user explicitly resumes them. Exact directory and filename re-resolution keeps queues valid when a full TSV import replaces source track IDs; unavailable entries are skipped without discarding surviving tracks.

Decoded peaks are derived data in device-local `aurora-waveforms.sqlite3`. A cache row is reused only while MP3 size and modification time still match, is capped to the 2,000 most recently accessed tracks, and is not copied to OneDrive or included in Aurora's shared-state lineage.

Output endpoint and ReplayGain preferences are versioned separately in device-local `aurora-audio.json`. They do not enter shared-state conflict lineage or OneDrive synchronization.

## Deliberate limits

Version 0.10.0 does not edit the source catalog, calculate or write ReplayGain, crossfade, equalize, or apply other DSP. A seamless queue handoff depends on preparing a valid known-duration successor; the ordinary transition remains the safe fallback. Waveforms are overview peaks rather than forensic sample-accurate audio analysis. Tag writes are isolated behind the separate [tag-editing contract](tag-editing-contract.md).
