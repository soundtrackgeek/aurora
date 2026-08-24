# Audio output contract

Aurora 0.10.0 added a device-local output and loudness layer without expanding the catalog or MP3 write boundaries. Version 0.17.7 retained bounded encoded read-ahead and restored Rodio/CPAL's driver-selected buffer after the forced 0.17.5 buffer caused a playback regression. Version 0.17.8 requests the active MP3's native sample rate so ordinary playback bypasses Rodio's linear converter. Rust owns endpoint discovery, stream creation, ReplayGain parsing, clipping prevention, encoded read-ahead, and queue preparation. React displays native status and submits only a selected endpoint ID plus ReplayGain mode.

## Windows output selection

- Aurora enumerates output endpoints through CPAL and exposes their stable Windows IDs and human-readable labels. The WebView cannot submit a device object or filesystem path.
- `system-default` is a deliberate virtual choice. An explicit endpoint selection is accepted only when it parses as a CPAL device ID; opening still matches it against the native output list.
- The output stream opens lazily on first playback. Browsing and Settings remain usable without an audio device.
- If the requested endpoint is absent or fails while opening, Aurora uses the current Windows default and keeps the requested preference unchanged.
- Rodio's stream-error callback records a disconnect or output failure. The next playback-state reconciliation reopens the current MP3 at its observed position on the Windows default.
- Aurora does not jump back to a recovered preferred endpoint during a track. It retries the preference the next time the output stream is created, including after a saved audio-setting change or process restart.

## Playback deadline resilience

- Before constructing a decoder, Aurora sequentially reads an ordinary MP3 into a signature-checked, two-entry encoded-media cache. The current track and prepared successor can then seek and read without routine filesystem I/O on the device callback.
- Cache admission is capped at 96 MiB per file. A larger file or failed memory reservation uses a 1 MiB `BufReader`, preserving playback instead of failing under memory pressure. Size and modification time are checked again after preload; files without a reliable modification timestamp are not reused.
- Stream creation starts with the selected endpoint's Rodio/CPAL configuration, replaces only its sample rate with the active MP3's decoded rate, and retains the driver-selected buffer and sample format. On Windows, CPAL opens the stream in shared mode with its default-quality automatic conversion flags, so Windows mixes a source-rate stream into the endpoint format.
- Matching the stream and source rates bypasses Rodio's linear sample-rate converter during ordinary playback. This matters because Rodio documents that converter as capable of audible distortion, and its open higher-quality-resampler issue specifically describes high-frequency noise.
- Explicitly loading a track at a different rate rebuilds the output stream. A differently rated successor already appended for gapless handoff remains on the existing stream and uses Rodio conversion; preserving an uninterrupted handoff is the bounded compatibility exception.

The device callback still performs MP3 decoding. Encoded read-ahead removes routine storage latency, but this is not a background PCM ring buffer and Windows/CPAL does not expose a reliable per-underrun counter here.

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
- That source is appended to the same Rodio player. The audio thread consumes it immediately after the current source; React's two-second polling updates metadata but does not start the next audio stream.
- When the native source count crosses the prepared boundary, Aurora finalizes the previous listening-history session, advances the current queue index, starts the next history session at zero, and reconciles its observed position.
- Transport and global-shortcut actions perform the same boundary reconciliation before resolving the current track.
- A shuffle/repeat/queue change invalidates an already prepared source and rebuilds the current source at its observed position so the new queue contract remains authoritative.

The handoff is capability-based, not an unconditional guarantee. Missing files, invalid MP3s, unknown durations, late preparation, or output-device recovery use the safe ordinary load path and may contain a gap. Crossfade, equalization, preamp controls, and other DSP are outside 0.10.0.

## Verification

- Browser interaction covers Audio/Shortcuts navigation, device and ReplayGain draft/save behavior, and the compact player readout.
- Native Windows-only tests enumerate non-empty output endpoints, round-trip their stable IDs, identify the current default, and verify that its shared stream accepts a 44.1 kHz source-rate configuration without replacing the driver-selected buffer.
- Unit tests cover the observed numeric formats, negative gain, positive-gain peak limiting, safe defaults, and invalid endpoint IDs.
