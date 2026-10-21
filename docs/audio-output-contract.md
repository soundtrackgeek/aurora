# Audio output contract

Aurora 0.10.0 added a device-local output and loudness layer without expanding the catalog or MP3 write boundaries. Version 0.17.14 keeps one endpoint-format output stream and moves decoding, gain, and resampling into bounded background PCM producers. Rust owns endpoint discovery, stream creation, ReplayGain parsing, clipping prevention, encoded and PCM read-ahead, and queue preparation. React displays native status and submits only a selected endpoint ID plus ReplayGain mode.

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
- Stream creation uses the selected endpoint's default shared-mode sample rate and format, requests a 4,096-frame stability buffer, and remains open across source-rate changes. A fallback configuration is still allowed when an endpoint rejects the preferred buffer.
- CPAL's `realtime` feature promotes the Windows output callback thread; if promotion is denied, playback continues and the player reports the condition instead of reopening the device.
- Aurora explicitly selects Rodio's balanced sinc configuration to convert MP3s into the stable stream format through Rubato, including mixed-rate gapless successors. The FFT path is enabled for efficient fixed-ratio conversions; the mixer therefore receives sources already matching its output rate and does not fall back to its default linear policy.
- Each current or prepared-next track receives a dedicated producer thread and bounded lock-free single-producer/single-consumer PCM ring. The producer decodes, applies ReplayGain, and resamples up to three seconds ahead; track loading waits for up to 500 ms of endpoint-format PCM before appending the source to Rodio.
- The real-time consumer never reads MP3 bytes, decodes frames, resamples, allocates, or takes the producer's lock. It performs a non-blocking ring read. A temporarily empty live ring emits silence and increments the underrun count once per starvation interval rather than ending the track.
- Seeking assigns a new buffer generation, discards queued samples, asks the producer to seek the decoder, and refills before the audio callback resumes. Samples produced by the old generation are rejected, preventing stale pre-seek audio.
- CPAL xrun notifications increment an atomic underrun counter. Xruns, real-time denial, and automatic route-change notices are non-fatal; actual invalidation, device loss, or backend failures retain Aurora's Windows-default recovery path.

The count combines detectable CPAL xruns with Aurora PCM-ring starvation. A zero count is still not proof that no hardware, driver, or downstream Windows audio glitch occurred.

## Device-local persistence

Audio preferences are stored atomically in `%APPDATA%\com.soundtrackgeek.aurora\aurora-audio.json` with a versioned schema:

- selected endpoint ID or `system-default`;
- ReplayGain mode: `off`, `track`, or `album`;
- `estimateMissingGain` (0.38.0, default `true`). Files saved by earlier versions omit it and load with the default.

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
- **Track:** Track tag → Track estimate → unity.
- **Album:** Album tag → Album estimate → Track tag → Track estimate → unity.

Estimates are used only when **Estimate from audio analysis when tags are missing** is on. Tags always win.

The requested linear multiplier is `10^(gain_db / 20)`. When a positive multiplier and tagged peak would exceed full scale, Aurora caps it at `1 / peak`. The player reports the effective gain, whether that cap prevented clipping, and whether the gain is estimated (`−8.2 dB est.`). Volume remains an independent user multiplier.

ReplayGain is read-only playback metadata. Aurora does not calculate, add, remove, or rewrite ReplayGain frames, and it does not alter audio bytes.

### Estimated gain from audio analysis (0.38.0)

Music Library's `music-analysis.sqlite3` stores a level for every analyzed MP3: Bliss feature 8, the mean frame power of the whole file, normalized from −90…0 dBFS. Aurora reads it through the same read-only, `query_only` catalog connection it uses for sonic features. Estimates never open, decode, measure, or rewrite an MP3.

- **Calibration** `bliss-v2-r128-2026-10-a`: `LUFS ≈ 0.88 × level_dBFS − 1.0`. It was fitted on 2026-10-21 against FFmpeg EBU R128 integrated loudness for 360 random library MP3s (one-time, read-only measurement).
  - Cross-validated error: mean 0.89 dB, median 0.67 dB, 95th percentile 2.3 dB.
  - General music averages 0.55 dB of error and scores 0.92 dB, because R128 gating ignores quiet passages.
- **Target −18 LUFS**, the reference implied by the library's existing ReplayGain frames, so tagged and estimated tracks share one scale.
- **Attenuation only:** `gain = clamp(−18 − estimate, −24 dB, 0 dB)`. The analysis has no peak value, so an estimate never boosts and never needs clipping protection.
- **Album estimate:** a duration-weighted energy mean of the album's track estimates. It is used only when at least 80% of the album's cataloged MP3s have a current estimate.
- **No estimate**, so the next rule applies, when:
  - the analysis store is absent;
  - the profile differs;
  - the file's size or modification time no longer matches the analysis row;
  - the features are invalid;
  - the track is near-silent (below −60 dBFS).

On the calibration sample in random order, attenuation-only estimates reduced the median loudness jump between consecutive tracks from 3.9 dB to 1.1 dB. Jumps over 6 dB fell from 32% of transitions to 7%.

## Gapless-capable queue handoff

- While a known-duration track is playing, Aurora resolves the authoritative next index from shuffle and repeat state during the final 15 seconds.
- The next track is resolved again through catalog ID plus stable path key, its file is opened, its MP3 decoder is initialized with gapless trimming, and its ReplayGain multiplier is applied.
- That source is appended to the same Rodio player. The audio thread consumes it immediately after the current source; React's two-second polling updates metadata but does not start the next audio stream.
- When the native source count crosses the prepared boundary, Aurora finalizes the previous listening-history session, advances the current queue index, starts the next history session at zero, and reconciles its observed position.
- Transport and global-shortcut actions perform the same boundary reconciliation before resolving the current track.
- A shuffle/repeat/queue change invalidates an already prepared source and rebuilds the current source at its observed position so the new queue contract remains authoritative.

The handoff is capability-based, not an unconditional guarantee. Missing files, invalid MP3s, unknown durations, late preparation, or output-device recovery use the safe ordinary load path and may contain a gap. Crossfade, equalization, preamp controls, and other DSP are outside 0.10.0.

## Verification

- Browser interaction covers Audio/Shortcuts navigation, device, ReplayGain and estimate draft/save behavior, and the compact player readout including `est.`.
- Rust tests cover the calibration formula, the attenuation clamp, silence and invalid vectors, album energy means and the 80% coverage gate, stale-file and wrong-profile rejection, tag-versus-estimate precedence, and loading pre-0.38.0 settings files.
- Native Windows-only tests enumerate non-empty output endpoints, round-trip their stable IDs, identify the current default, and verify that Aurora can open its stable shared output configuration.
- Unit tests cover the observed numeric formats, negative gain, positive-gain peak limiting, safe defaults, and invalid endpoint IDs.
