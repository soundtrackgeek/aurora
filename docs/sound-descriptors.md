# Sound descriptors: `sound:` and `group:sound`

Aurora 0.65.0 describes what a track sounds like (orchestral, choir, synth,
chiptune and so on) and lets you search and group by it. It is made for film,
TV and game scores, where the genre tag is only "Sci-Fi" or "Video Game", but it
works on the whole library.

```text
genre:scores AND sound:synth AND year:1980..1989
genre:"video game" NOT sound:chiptune
genre:sci-fi group:sound
```

See [the search guide](search.md#search-fields) for the syntax.

## The descriptors

| Descriptor | Meant for | Share of score tracks |
| --- | --- | --- |
| `orchestral` | A full orchestra | 24.6% |
| `choir` | Choral voices | 12.6% |
| `strings` | Small string ensembles, quartets | 11.2% |
| `piano` | Solo or leading piano | 5.0% |
| `guitar` | Acoustic or electric guitar | 5.6% |
| `synth` | Analog and modern synthesizer scores | 8.8% |
| `hybrid` | Orchestra combined with electronics and trailer percussion | 19.9% |
| `ambient` | Drones, textures, soundscapes | 7.0% |
| `percussion` | Driving action drums, taiko | 8.8% |
| `chiptune` | 8- and 16-bit sound-chip game music | 1.0% |
| `jazz` | Jazz bands, big band, crime jazz | 10.3% |
| `vocals` | A lead singer or a solo voice | 4.8% |

A track can carry several descriptors, or none. The share column is measured on
the 533,467 score-genre tracks the model was trained on.

## Where it comes from

Music Library 0.216.0 and later store one LAION CLAP embedding per analyzed
MP3 in `music-clap.sqlite3`, for its **Describe a sound** search. CLAP places
audio and text in one space, but scoring a track directly against a text label
does not work well: each label's text has its own bias (almost everything scores
high for "orchestral"), and text and audio sit apart in the space. Labelled that
way, Vangelis's *Blade Runner* comes out "orchestral".

The descriptors therefore come from a small model trained once, offline, by
[`scripts/train-sound-descriptors.py`](../scripts/train-sound-descriptors.py):

1. Each descriptor has three or four text prompts ("a large choir singing",
   "vintage 1980s analog synthesizer film score", …), embedded with Music
   Library's own CLAP text encoder.
2. Every score-genre track is scored against every descriptor. The tracks where
   one descriptor beats all the others by the widest margin, 2,000 per
   descriptor (800 for the rarer chiptune), become examples of it.
3. One logistic regression per descriptor learns, from those examples' audio,
   to tell it apart from the other descriptors' examples.

Aurora compiles in only the result, [`sound_descriptors_model.json`](../src-tauri/src/sound_descriptors_model.json):
12 × 512 weights and 12 biases. Describing a track is 12 dot products on an
embedding Music Library already computed, so Aurora runs no audio model and
needs no GPU. A descriptor applies when its probability is at least 0.4. The
most likely one is the track's `group:sound` heading.

An album carries a descriptor when at least a third of its described tracks do;
its `group:sound` heading is the descriptor most of its tracks carry (the
earlier one in the table on a tie).

## How well it works

Checked by hand against albums whose sound is well known, as the share of each
album's tracks:

| Album | Descriptors |
| --- | --- |
| Vangelis – *Blade Runner* | synth 47% |
| John Carpenter – *Halloween* | synth 43%, orchestral 36% |
| Kyle Dixon & Michael Stein – *Stranger Things* | synth 86%, ambient 36% |
| Danny Elfman – *Batman* | orchestral 95% |
| John Williams – *Jurassic Park* | orchestral 86% |
| Hans Zimmer – *Dune: Part Two* | hybrid 64%, ambient 28%, percussion 20% |
| Hans Zimmer – *Interstellar* | choir 50% |
| Ramin Djawadi – *Game of Thrones* season 2 | hybrid 67%, orchestral 43% |
| Akira Kosemura – *One Day* | piano 58% |
| Darren Korb – *Pyre* | guitar 75% |
| *Famicom 20th Anniversary* | chiptune 100% |
| Koji Kondo – *Super Mario World* | chiptune 56%, jazz 25% |
| Lavagnino & Umiliani | jazz 67% |

Outside scores the model behaves sensibly too: on a random sample of the library,
Pop tracks are 70% vocals, Rock and Country mostly guitar, Synthwave 71% synth.

Known weaknesses:

- Orchestral, strings and choir sound alike, and some choir is reported on
  orchestral albums with only a little choir.
- A third of Halloween is still orchestral; old, thin recordings of synths and
  pianos are harder to tell apart.
- The labels are estimates from sound, not facts. A missing descriptor does not
  prove an instrument is absent.

## How Aurora keeps it up to date

A background task (`aurora-sound-descriptors`) checks every minute, starting a
minute after Aurora opens. It follows Music Library's results by row, so it only
reads embeddings saved since the last pass. The first pass over 1.1 million
tracks reads every embedding and took about three minutes on the reference PC;
later passes take a fraction of a second. Album descriptors are rebuilt when
the catalog, the analysis or the descriptors change (a few seconds).

The same pass keeps an average CLAP embedding per album for
[`vibe:`](search.md), in the `sound_album_vectors` table: about 100 MB for 73,000
albums, built in under a minute, and rebuilt at most every 30 minutes while
the library changes. Descriptions searched with `vibe:` are saved with their
embedding in `sound_queries`, so Music Library's CLAP worker reads each one only
once; Aurora NG reads both tables from the same file.

Results are kept in Aurora's data folder as `aurora-sound-descriptors.sqlite3`,
about 200 MB for 1.1 million tracks with the album vectors, keyed by audio hash, so identical files share
one row and renamed folders keep their descriptors. Delete the file to rebuild it.
Music Library's files are only read. Searches see descriptors as soon as they
are saved, including from `aurora.exe --mcp`.

Tracks without a CLAP result (not yet analyzed, or Intel Macs, where Music
Library cannot run CLAP) have no descriptors: `sound:` does not find them, and
they are grouped under "No sound descriptor".

## Retraining

Change the prompts or seed counts in `scripts/train-sound-descriptors.py`, bump
its `VERSION`, and run it with Python, numpy and scikit-learn:

```text
python -I scripts/train-sound-descriptors.py "%APPDATA%\com.local.musiclibrary" <path to music-clap-worker.exe>
```

A new `version` (or a new CLAP model identity) makes Aurora rebuild its cache
on the next pass. Keep the label order, or update `ALIASES` and the label lists
in `src-tauri/src/sound_descriptors.rs` and `src/library.ts` with it.
