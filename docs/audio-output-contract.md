# Audio output contract

Aurora 0.10.0 adds a device-local output and loudness layer without expanding the catalog or MP3 write boundaries. Rust owns endpoint discovery, stream creation, ReplayGain parsing, clipping prevention, and queue preparation. React displays native status and submits only a selected endpoint ID plus ReplayGain mode.

## Windows output selection

- Aurora enumerates output endpoints through CPAL and exposes their stable Windows IDs and human-readable labels. The WebView cannot submit a device object or filesystem path.
- `system-default` is a deliberate virtual choice. An explicit endpoint selection is accepted only when it parses as a CPAL device ID; opening still matches it against the native output list.
- The output stream opens lazily on first playback. Browsing and Settings remain usable without an audio device.
- If the requested endpoint is absent or fails while opening, Aurora uses the current Windows default and keeps the requested preference unchanged.
- Rodio's stream-error callback records a disconnect or output failure. The next playback-state reconciliation reopens the current MP3 at its observed position on the Windows default.
- Aurora does not jump back to a recovered preferred endpoint during a track. It retries the preference the next time the output stream is created, including after a saved audio-setting change or process restart.

## Device-local persistence

Audio preferences are stored atomically in `%APPDATA%\com.soundtrackgeek.aurora\aurora-audio.json` with a versioned schema:

- selected endpoint ID or `system-default`;
- ReplayGain mode: `off`, `track`, or `album`.

The file is separate from `aurora-state.sqlite3`, Laptop Mode snapshots, and OneDrive. Desktop and Laptop therefore keep independent device choices while sharing the existing catalog and Aurora state workflows.

## ReplayGain

The live-library audit sampled 250 existing MP3s from the read-only catalog. Seven contained ReplayGain, consistently as `REPLAYGAIN_TRACK_GAIN` plus `REPLAYGAIN_TRACK_PEAK` ID3 user-text frames. Aurora supports those observed frames and the corresponding Album pair:

- `REPLAYGAIN_TRACK_GAIN`
- `REPLAYGAIN_TRACK_PEAK`
- `REPLAYGAIN_ALBUM_GAIN`
- `REPLAYGAIN_ALBUM_PEAK`

Keys are matched case-insensitively with spaces or hyphens normalized to underscores. Gain accepts a signed decimal followed by an optional `dB` suffix; peak accepts a positive decimal. Missing, malformed, or unreadable optional frames mean unity gain rather than a playback failure.

Mode behavior:

- **Off:** unity gain.
- **Track:** use Track gain and its peak when present.
- **Album:** use Album gain and peak; fall back to Track gain and peak when Album frames are absent.

The requested linear multiplier is `10^(gain_db / 20)`. When a positive multiplier and tagged peak would exceed full scale, Aurora caps it at `1 / peak`. The player reports the effective gain and whether that cap prevented clipping. Volume remains an independent user multiplier.

ReplayGain is read-only playback metadata. Aurora does not calculate, add, remove, or rewrite ReplayGain frames, and it does not alter audio bytes.

## Gapless-capable queue handoff

- While a known-duration track is playing, Aurora resolves the authoritative next index from shuffle and repeat state during the final 15 seconds.
- The next track is resolved again through catalog ID plus stable path key, its file is opened, its MP3 decoder is initialized with gapless trimming, and its ReplayGain multiplier is applied.
- That source is appended to the same Rodio player. The audio thread consumes it immediately after the current source; React's 500 ms polling updates metadata but does not start the next audio stream.
- When the native source count crosses the prepared boundary, Aurora finalizes the previous listening-history session, advances the current queue index, starts the next history session at zero, and reconciles its observed position.
- Transport and global-shortcut actions perform the same boundary reconciliation before resolving the current track.
- A shuffle/repeat/queue change invalidates an already prepared source and rebuilds the current source at its observed position so the new queue contract remains authoritative.

The handoff is capability-based, not an unconditional guarantee. Missing files, invalid MP3s, unknown durations, late preparation, or output-device recovery use the safe ordinary load path and may contain a gap. Crossfade, equalization, preamp controls, and other DSP are outside 0.10.0.

## Verification

- Browser interaction covers Audio/Shortcuts navigation, device and ReplayGain draft/save behavior, and the compact player readout.
- Native Windows-only tests enumerate non-empty output endpoints, round-trip their stable IDs, identify the current default, and open its stream configuration.
- Unit tests cover the observed numeric formats, negative gain, positive-gain peak limiting, safe defaults, and invalid endpoint IDs.
