# Vibe search: `vibe:` and `sort:vibe`

`vibe:` finds music by a description of how it sounds, in your own words.
Use it when tags don't help, such as when you want the bleak ones among
thousands of "Sci-Fi" scores, or the piano music among albums tagged
"Soundtrack":

```text
vibe:"dark ambient drone" AND year:1990..1999 AND cr:..50 sort:vibe
```

It works in **Songs**, **Albums** and **Artists** in Aurora 0.66.0 and newer,
and on the cover wall in Aurora NG 0.22.0 and newer. `sort:vibe` puts the
best fit first.

## How it works

Music Library's audio analysis runs CLAP, a model trained on audio paired
with text descriptions. CLAP turns each track into an embedding, a list of
512 numbers. It does the same for a sentence. Audio and text that describe
the same sound end up close together, so Aurora can compare your
description with every track it has analysed.

- **Songs** compares the description with each analysed track and keeps the
  best-fitting **2%**, about 22,000 of 1.1 million tracks.
- **Albums** and **Artists** compare it with each album's average sound, the
  mean of its tracks' embeddings, and keep the best-fitting **2%** of
  albums, about 1,500 of 73,000.
- In **Artists**, an artist matches when one of their albums does.

Because it keeps a share rather than applying a score cut-off, every
description returns a usable set, even when CLAP's scores for it are low
across the board. Add other fields to narrow it down.

## Writing descriptions

Put descriptions of more than one word in quotes. Case and extra spaces don't
matter, and a description can be up to 200 characters.

CLAP learned from short captions describing what is heard, so short, plain
phrases about the sound itself work best.

| What to describe | Examples |
| --- | --- |
| Mood | `vibe:"melancholic and lonely"`, `vibe:"tense suspense"`, `vibe:"uplifting and triumphant"`, `vibe:"dark and ominous"` |
| Instruments | `vibe:"lonely solo piano"`, `vibe:"solo cello"`, `vibe:"acoustic guitar fingerpicking"`, `vibe:"pipe organ"` |
| Ensemble and texture | `vibe:"heroic orchestral fanfare"`, `vibe:"ethereal female choir"`, `vibe:"thick analog synth pads"`, `vibe:"dark ambient drone"` |
| Era and production | `vibe:"8-bit chiptune"`, `vibe:"80s synthwave"`, `vibe:"lo-fi tape hiss"`, `vibe:"warm analog synth arpeggios"` |
| Energy and rhythm | `vibe:"pounding action percussion"`, `vibe:"slow and sparse"`, `vibe:"driving techno beat"`, `vibe:"gentle lullaby"` |
| Scenes | `vibe:"space exploration"`, `vibe:"rainy night jazz bar"`, `vibe:"medieval tavern"`, `vibe:"horror music box"` |

Some tips:

- **Describe the sound, not facts about it.** CLAP never saw names, titles
  or dates, so `vibe:"John Williams"`, `vibe:"Star Wars"` or `vibe:"1985"`
  give unreliable results. Use `aartist:`, `album:` and `year:` for those.
- **Keep it short.** "Calm solo piano" usually beats a long list of
  adjectives, which pulls the description in several directions at once.
- **Name the instrument when it matters.** "Solo cello" is more precise than
  "sad strings".
- **Try a different wording** if the results miss. "Ambient drone",
  "dark drone" and "droning ambient soundscape" each land a little
  differently.

## Combining with other fields

`vibe:` behaves like any other field, so `AND`, `OR`, `NOT`, ranges,
`sort:` and `group:` all work with it:

```text
vibe:"dark ambient drone" AND year:1990..1999 AND cr:..50 sort:vibe
vibe:"heroic orchestral fanfare" AND genre:scores sort:vibe
vibe:"lonely solo piano" NOT sound:strings
vibe:"80s synthwave" AND love:1..
vibe:"tense suspense" AND genre:sci-fi group:decade
vibe:"8-bit chiptune" OR vibe:"16-bit fm synth"
NOT vibe:"pounding action percussion" AND genre:action
```

`sound:` and `vibe:` complement each other:

- `sound:` uses twelve fixed descriptors, such as `choir` and `synth`. It's
  steady and can be grouped with `group:sound`.
- `vibe:` takes any description and ranks results by fit.

See [sound descriptors](sound-descriptors.md) for `sound:`.

## Sorting by fit: `sort:vibe`

`sort:vibe` orders results by how well they fit, best first:

- In **Songs** it ranks each track.
- In **Albums** and on Aurora NG's cover wall it ranks each album's
  average sound.
- **Artists** doesn't offer it.

It needs a `vibe:` in the query. With several `vibe:` clauses, it ranks by
the first one. Results that match only through another clause, such as the
`OR` side of `vibe:"choir" OR genre:gospel`, have no score and come last.

## Speed

On the reference library (1.1 million tracks, 73,000 albums):

| Step | Time |
| --- | --- |
| A new description, read by Music Library's CLAP worker | about 0.5–1 s, once per description |
| Scoring every album | about 0.15–0.3 s |
| An Albums search | about 0.2 s |
| Scoring every track for Songs | about 1.6 s, then kept for half an hour |
| A Songs search once tracks are scored | about 0.7 s |

Aurora saves each description's embedding, so repeating it, in Aurora or in
Aurora NG, doesn't start the worker again. Album scores are kept for five
minutes and track scores for half an hour.

## What it needs

- **Music Library 0.216.0 or newer**, with audio analysis run. Its
  **Describe a sound** step makes the CLAP embeddings, and it installs
  `music-clap-worker.exe`, which reads new descriptions. Tracks without a
  CLAP result (not yet analysed, or on Intel Macs) are never matched.
- **Aurora 0.66.0 or newer.** Its background pass averages each album's
  sound into `aurora-sound-descriptors.sqlite3` in Aurora's data folder,
  in under a minute, a few minutes after a new version starts. It rebuilds
  the averages at most every 30 minutes while the library changes.
- **Aurora NG** reads the album averages and saved descriptions from that
  same file. It can't search Songs, so it scores albums only.

When something is missing, the search tells you what instead of returning
nothing silently. For example, it says "vibe: is ready once Aurora has read
Music Library's sound search results".

## Accuracy

`vibe:` is an estimate, not a tag:

- **Neighbouring sounds blur.** Orchestral, strings and choir sit close
  together, as do ambient and drone.
- **Averages hide contrast.** An album that alternates quiet piano cues
  with loud action cues averages to neither, so Songs is the better view
  for albums with mixed sounds.
- **Surprises happen.** "8-bit chiptune" also found a clawhammer banjo
  album, whose bright plucked timbre is close to a square wave.

Combining `vibe:` with a genre, year or `sound:` clause usually removes the
outliers.

## Under the hood

- **Text embeddings** come from `music-clap-worker.exe --text <model folder>`,
  which reads the description on standard input and replies with a base64
  embedding. Aurora saves them in the `sound_queries` table (description,
  CLAP model, half-float embedding).
- **Album averages** are in `sound_album_vectors`, one 1 KB half-float
  vector per album, about 100 MB in total.
- **Scores** are dot products. The embeddings are unit length, so a dot
  product is the cosine similarity. Scores are stored as integers (score ×
  1,000,000) so Aurora's explorer can page through results ordered by them.
- **In SQL**, Aurora registers `aurora_vibe_album(text, album_id)` and
  `aurora_vibe_track(text, audio_hash)`. Each returns a score inside the
  best share and NULL outside it, and scores its description once per
  query. Aurora NG registers `ng_vibe(text, album_id)` the same way.
- **The code** is in `src-tauri/src/vibe.rs`, with the album averages in
  `src-tauri/src/sound_descriptors.rs`. In Aurora NG it is `src/vibe.rs`.
