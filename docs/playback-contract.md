# Playback contract

Aurora owns playback and queue state without claiming write ownership of the imported catalog. Version 0.8.0 also feeds playback transitions into the separate [listening-history contract](listening-history-contract.md); version 0.8.2 adds bounded, catalog-resolved waveform extraction; version 0.8.3 makes seek command ordering explicit; version 0.10.0 adds the [audio-output contract](audio-output-contract.md); version 0.11.0 adds bounded append/refill behavior for [Genre Atlas](genre-atlas-contract.md); version 0.15.20 adds live queue rebinding after a completed Music Library import; version 0.17.5 adds encoded read-ahead; version 0.17.7 moves native snapshot work off the Tauri command thread and adds Windows media-session controls; version 0.17.13 stabilizes the endpoint-format output stream; and version 0.17.14 moves decoding, gain, and resampling into bounded lock-free PCM producers.

## Trust boundary

- React sends catalog track IDs paired with their stable normalized path keys. A queue command accepts at most 200 pairs.
- Rust re-queries each ID, requires the current path key to match, and constructs the audio path from its indexed directory and single-component MP3 filename.
- Absolute filenames, nested filenames, non-MP3 extensions, missing records, and missing files are rejected.
- No native command accepts an arbitrary path from the WebView.

## Runtime behavior

- The selected audio device is opened lazily on the first play action, so browsing remains available when an output device is absent. A missing, failed, or disconnected preference falls back to the Windows default without changing the saved preference.
- Play/pause, seek, previous/next, volume, shuffle, repeat-all, and repeat-one are native operations.
- Starting a visible track replaces the current bounded queue with the current result set and begins at that track.
- Appending a genre batch de-duplicates stable track identities, retains at most 20 entries before the current track, preserves the current and prepared successor, and keeps the complete queue at or below 200 tracks.
- During the final 15 seconds of a known-duration track, Aurora prepares the authoritative repeat/shuffle successor and appends it to the same native player. Natural audio handoff therefore does not wait for frontend polling; polling reconciles metadata and history after the source boundary.
- Decoder construction uses a bounded encoded-MP3 cache. Ordinary current and prepared-next tracks are read sequentially before the native callback consumes them; oversized or memory-constrained tracks retain a large buffered-file fallback. See the audio-output contract for exact limits and stream-buffer policy.
- Every transport and global-shortcut action reconciles a prepared source boundary before resolving the current track.
- Optional ReplayGain is applied per source before the independent player-volume multiplier. See the audio-output contract for tag precedence and peak limiting.
- A stopped track positioned at its natural end restarts from zero when Play is pressed; an explicitly paused track resumes from its paused position.
- Range input displays a local draft only while its exact seek command is pending. The most recently issued command owns the resulting snapshot, older overlapping responses are ignored, and polling resumes after all active commands finish.
- The two-second playback poll admits only one native snapshot request at a time and executes native snapshot work on a blocking worker. A poll that overlaps a newer transport command cannot replace that command's snapshot when it returns. The bottom progress line interpolates locally every 250 ms, so a delayed snapshot does not visually freeze the playhead.
- Native playback—not React polling—is responsible for beginning, observing, seeking, and finalizing listening-history sessions.
- The player waveform waits 1.5 seconds after a track change, then samples 64 evenly spaced windows from the decoded MP3 stream and reduces them to 320 normalized peaks. It does not fully decode a song just to draw the timeline. Cache misses use one decode slot, cancel superseded generations at preload/seek/decode checkpoints, and sequentially buffer MP3s up to 96 MiB before window seeking.
- Waveform requests contain only catalog ID plus stable track key. Rust performs the same identity and path validation as playback before opening the MP3.
- Player rating and Love controls use the tag-editing boundary; they do not mutate the imported catalog directly.
- When a new completed import revision appears, Rust re-resolves every live queue entry inside one SQLite read transaction, preserving queue order by stable track key and replacing catalog metadata and transient row IDs. An exact indexed lookup is preferred, followed by canonical filesystem spelling and normalized slash/case matching. Missing identities may be dropped, but busy, I/O, schema, and decode failures abort the rebind without pruning or persisting the queue.
- If stable key order is unchanged, the current source, play/pause state, position, listening session, and any appended gapless successor remain untouched. If entries disappeared, Aurora remaps the current key, discards an unsafe prepared successor, and reloads only when required. A removed current entry stops immediately and leaves the next surviving entry selected but paused.
- A tag read, write, or undo that overlaps an import may return a summary carrying the prior transient row ID. Queue refresh applies only rating, Love/Ban, release-year, sync, and undo fields from that summary; it never replaces the rebound identity or catalog projection.

## Persistence

Aurora writes its own `aurora-state.sqlite3` under the Tauri application-data directory. The current schema persists:

- queue order and current index;
- approximate playback position;
- volume;
- shuffle state;
- repeat mode.
- a normalized path key plus the exact indexed directory and filename for every queue entry.

Position is checkpointed in 30-second buckets and once more during window shutdown. Queue and control changes are transactional. Restored sessions remain paused until the user explicitly resumes them. Stable path re-resolution keeps queues valid when a full catalog import replaces source track IDs; unavailable entries are skipped without discarding surviving tracks.

Decoded peaks are derived data in device-local `aurora-waveforms.sqlite3`. A cache row is reused only while MP3 size and modification time still match, is capped to the 2,000 most recently accessed tracks, and is not copied to OneDrive or included in Aurora's shared-state lineage.

Output endpoint and ReplayGain preferences are versioned separately in device-local `aurora-audio.json`. They do not enter shared-state conflict lineage or OneDrive synchronization.

## Deliberate limits

Version 0.11.0 does not edit the source catalog, calculate or write ReplayGain, crossfade, equalize, or apply other DSP. A seamless queue handoff depends on preparing a valid known-duration successor; the ordinary transition remains the safe fallback. Waveforms are overview peaks rather than forensic sample-accurate audio analysis. Tag writes are isolated behind the separate [tag-editing contract](tag-editing-contract.md).
