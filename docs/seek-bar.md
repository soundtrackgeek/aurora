# Coloured seek bar

Since 0.71.0 the waveform in the seek bar is coloured by the sound of each moment, marks where the sound changes, and offers **Skip intro** on tracks with a long quiet opening. Aurora NG 0.23.0 does the same.

It reads Music Library's per-second timelines (Music Library 0.208.0 or newer, `music-timelines.sqlite3`): one row of measurements per second of audio. Aurora never decodes the MP3 for this, and a track whose file changed since Music Library analyzed it keeps the plain waveform. Reading a track's map takes about 40 ms.

## Colours

The palette button at the right end of the seek bar switches between three colourings, and Aurora remembers the choice:

- **Brightness** (the default): blue where the sound is dull or dark, violet in between, gold where it is bright. It follows the spectral centroid on a log scale from 600 Hz to 3,400 Hz, about the 5th and 95th percentile of the library's seconds.
- **Stereo width**: orange where the sound is close to mono, violet in between, cyan where it is wide. It follows the Side/Mid energy from −20 dB to 0 dB, again about the 5th and 95th percentile.
- **Plain**: the purple-to-cyan gradient from before 0.71.0.

Seconds quieter than −50 dBFS, and mono seconds in Stereo width, are grey. What has played is drawn bright, the rest dim, as before.

## Sections

Faint ticks at the top and bottom edge of the waveform mark where the sound changes clearly. For each second, Aurora compares the average of the five seconds before it with the five seconds after (cosine novelty over the timeline's 29 measurements, each standardized over the track). A change counts when it stands more than one standard deviation above the track's average, and changes are at least 10 seconds apart; the stronger one wins. That gives about two or three ticks a minute.

The ticks are changes in sound, not verses or choruses, and Aurora never labels them that way. In through-composed score cues they can be frequent.

## Skip intro

A track of at least two minutes gets **Skip intro** when it opens with a stretch of at least 30 seconds that is more than 6 dB quieter than its median second, and that stretch is at most half the track. The level is averaged as power over five seconds, so a single loud second does not end the intro. The main part starts where the level rises, moved onto a section tick up to 8 seconds before or 2 seconds after it. A small cyan marker at the top of the waveform shows the spot, and the button disappears once playback is past it.

On a sample of 4,000 tracks, about 3% had such an intro. Examples from the library: Machine Head's "Imperium" skips to 0:39 and The Cars' "Moving in Stereo" to 0:33.

## Where it lives

`src-tauri/src/sound_map.rs` builds the map; its read-only test `real_library_proof` prints sections and intros for a few real tracks. `src/soundMap.ts` holds the colours and the remembered choice, and `WaveformTimeline.tsx` draws it.
