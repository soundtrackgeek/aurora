# Guest DJ — design proposal

*Proposal · 14 November 2026 · not implemented*

Guest DJ adds occasional "guest picks" between the tracks you chose yourself.
You stay in charge of the queue. The DJ adds a track now and then, says why it
chose it, and learns from what you skip. Each DJ uses one kind of knowledge
Aurora already has: how songs sound, the charts, years, ratings, listening
history, countries or labels.

The idea comes from the [Plexamp comparison](#background). Plexamp's Guest DJs
use only sonic similarity and simple metadata. Aurora can do more because it
has data no other player combines: weekly US/UK/Norwegian chart peaks, official
singles, original and release years, a library that is 86 % unrated, history
merged across devices, and a sonic index with stable neighbourhoods.

---

## Contents

1. [What Guest DJ is, and what it is not](#1-what-guest-dj-is-and-what-it-is-not)
2. [How it behaves while you listen](#2-how-it-behaves-while-you-listen)
3. [The DJ roster](#3-the-dj-roster)
4. [Rules every DJ follows](#4-rules-every-dj-follows)
5. [Explaining picks honestly](#5-explaining-picks-honestly)
6. [Feedback and learning](#6-feedback-and-learning)
7. [Interface](#7-interface)
8. [Architecture](#8-architecture)
9. [Phased delivery](#9-phased-delivery)
10. [Non-goals](#10-non-goals)
11. [Open questions](#11-open-questions)
12. [Background](#background)

---

## 1. What Guest DJ is, and what it is not

| | Sonic radio (exists) | Guest DJ (proposed) |
| --- | --- | --- |
| Who owns the queue | The station. Starting it replaces your queue. | You. The DJ only inserts tracks. |
| What it adds | A stream of tracks close to one seed | One guest track at a time, between yours |
| How often | Refills whenever fewer than 20 tracks remain | Your choice: rarely, sometimes or often |
| Selection basis | Sound only | Depends on the DJ: sound, charts, years, ratings, history, country, label… |
| Stops when | You start another queue or press **Stop radio** | You turn the DJ off. It stays on when the queue changes. |

Guest DJ is a **layer on top of whatever you are playing**: an album, a
search result, a playlist, a chart, or even sonic radio (with limits, see
[§8.5](#85-interaction-with-existing-playback-features)).

It is **not** an AI chat assistant, a mood engine, or a beatmatching mixer.
See [Non-goals](#10-non-goals).

## 2. How it behaves while you listen

### 2.1 Frequency

| Setting | Behaviour |
| --- | --- |
| **Rarely** | About one guest pick per 6 of your tracks |
| **Sometimes** (default) | About one per 4 |
| **Often** | About one per 2 |
| **Back to back** | Guest picks follow each other until you play something yourself. This turns the DJ into a radio built from the current track. |

The interval varies by ±1 track so the pattern does not feel mechanical. It
never goes below one of your own tracks between two guest picks, except in
**Back to back**.

### 2.2 When the DJ chooses

The DJ chooses **when a track of yours starts playing**, not in the last
15 seconds before the handoff. Choosing early means:

- the guest track is visible in the queue straight away, so you can remove it
  or move it before it plays;
- the existing gapless preparation ("prepare the successor 15 s before the
  end") works unchanged, because the guest is already an ordinary queue entry;
- a slow candidate query never delays the transition. If the DJ has not
  decided in time, it does nothing this round.

When you reorder, remove or replace the queue, the next pick is planned again
from the new current track.

### 2.3 Respecting albums

Most of your listening is full albums. Breaking up an album is the quickest way
to make a DJ annoying, so **Album respect** is on by default:

- When the queue is one album played in order (same album ID, ascending
  disc/track), the DJ waits for the **end of the album** and adds at most one
  guest pick before the next album starts.
- It never inserts between two tracks that hand off gaplessly when both belong
  to the same album. That would break segues, live albums and continuous mixes.
- Album respect can be switched off for people who listen to single tracks.

### 2.4 One DJ at a time

Only one DJ is active at a time, which keeps each pick's reason simple to
explain. **Surprise me** (see the [roster](#3-the-dj-roster)) rotates through
the DJs you have enabled, one pick each.

## 3. The DJ roster

Each DJ has a short name, a personality you can describe in one line, a
defined candidate source, and a clear **availability check**. A DJ whose data
is missing is shown greyed out with the reason, never offered with an empty
pool.

### Launch set

#### The Neighbour — "sounds like what you're hearing"

- **Picks:** a track close in sound to the current track, by a **different
  album artist**.
- **Data:** the sonic index (the same retrieval as **More like this**).
- **Guardrails:** respects the radio caps (one guest per artist per 10 picks);
  excludes identical analysed audio.
- **Available when:** the current track has current analysis. If it doesn't,
  the Neighbour skips this round and doesn't guess.
- **Why it matters:** the safest DJ. It shows the concept with the least
  surprise.

#### The Crate Digger — "the deep cuts you never rated"

- **Picks:** an **unrated** track from an album where you have rated other
  tracks highly (album mean ≥ 4★, or at least one loved track), preferably
  from the same artist or a sonic neighbour of the current track.
- **Data:** ratings, Love, sonic index (optional ranking signal).
- **Why it matters:** about 947,000 tracks (86 %) are unrated. This DJ turns
  background listening into rating progress: the guest row has the normal
  star control, and the Now Playing line says "Unrated · from an album you rate
  4.3". It shares the idea behind Rating missions without taking over the
  queue.

#### The Chart Historian — "this was on the charts that week"

- **Picks:** a library track that charted **in the same period** as the
  current track: the same ISO week ± 4 weeks for weekly sources (Official UK,
  VG-lista, Ti i Skuddet, Norsktoppen), the same year for Billboard (annual
  only, see `charts-contract.md`). If the current track never charted, it uses
  its original year.
- **Data:** materialised chart ranks and the chart song matcher, with the
  same title/version rules as the tracklist badges.
- **Explains like:** "#3 on VG-lista, week 14 1985, when *Take On Me* was #1."
- **Why it matters:** no other player can do this. It turns the chart archive
  into listening rather than browsing.

### Second wave

#### The Time Traveller — "same year, somewhere else in your library"

- **Picks:** a track whose **original year** is within ±1 year of the current
  track's, from a different genre or artist.
- **Variant — Then and now:** the same day/year 10, 20, 30 … years ago, using
  original year and release date when present.
- **Data:** catalog `year` (`originalYear`). It never uses the release year
  instead, the same rule as Years and Same sound, different world.

#### The Superfan — "more from this artist, from another era"

- **Picks:** a track by the **same artist** from a *different* album, ideally
  from a different decade of their career, preferring loved or highly rated
  tracks you haven't played recently.
- **Data:** artist splits (reviewed album assignments; never merges same-name
  artists), years, ratings, history.
- **Guardrails:** only applies when the artist has at least three albums.
  Never picks from the album that is playing.

#### The Rediscoverer — "you loved this once"

- **Picks:** a 5★ or loved track you **haven't played for a long time** (no
  registered play on any device for 12 months or more), near the current track
  in sound or genre.
- **Data:** ratings, Love, merged cross-device history.
- **Relation to Hidden favorites:** Hidden favorites finds tracks you *would*
  like. The Rediscoverer brings back tracks you *already* like but forgot.

#### The Hometown DJ — "from the same country"

- **Picks:** a track by an artist from the **same origin country** as the
  current artist, from a different artist and preferably a different decade.
- **Data:** MusicBrainz origin country (the `country:` search field).
- **Note:** this suits Norwegian music well, and VG-lista and Norsktoppen chart
  data can strengthen the pick.

#### The Label Boss — "same label, different artist"

- **Picks:** a track released on the **same publisher/label** as the current
  album, within ±5 years.
- **Data:** the publisher field (the Publishers page).
- **Why:** labels had sounds (Factory, 4AD, Motown, ECM). Following the label
  is an old way of finding records that a sound index can't reproduce.

### Third wave

#### The Drifter — "a little further each time"

- **Picks:** a sonic walk. Each guest is a little further away in sound from
  your **original** starting track than the last one, then it drifts back
  toward your queue.
- **Data:** sonic index. It reuses the journey planner's distance steps but
  plans **one step at a time** rather than a whole route.
- **Guardrails:** the walk resets when you skip two guests in a row. A skip
  means it drifted too far.

#### The Border Crosser — "same sound, different world"

- **Picks:** a track close in sound but with a **different decade, album
  artist or genre**, using the boundary rules of *Same sound, different world*
  (Cross borders).
- **Data:** the Music Library bridge used by Same sound, different world.
- **Explains like:** "Sound only · 1972 instead of 2019."

#### The Single Spotter — "the hit from that album"

- **Picks:** an **official single** (💿) from albums near the current track by
  year or sound. It favours singles that charted.
- **Data:** album singles from Music Library 0.210.0+.
- **Guardrail:** only MusicBrainz-confirmed singles. Missing singles data is
  never treated as "not a single".

#### The Anniversary DJ — "on this day"

- **Picks:** a track from an album released **on this date** in a past year, or
  by an artist with a birthday or memorial today, using the same data as Music
  Library's *Your Daily Edition*.
- **Explains like:** "*Hounds of Love* was released 41 years ago today."

#### The Score Supervisor — "for the soundtrack collection"

- **Picks:** only when the current track belongs to the **Scores** genre
  group. It picks another cue from the same composer or the same film decade.
  For non-score tracks it offers nothing.
- **Why:** scores live in their own root (`G:\_BACKUP\SCORES`) and behave
  differently from songs. Other DJs give scores less weight, as Hidden
  favorites already does for "music families".

#### Surprise me

Rotates through all enabled, available DJs, one pick each, and avoids
choosing the same DJ twice in a row. The pick's reason always names the DJ
that made it.

### Roster summary

| DJ | Main data | Wave | Needs |
| --- | --- | --- | --- |
| The Neighbour | Sonic index | Launch | Analysis of the current track |
| The Crate Digger | Ratings + Love | Launch | Rated albums |
| The Chart Historian | Chart ranks | Launch | Imported charts |
| The Time Traveller | Original year | 2 | Year data |
| The Superfan | Artist splits + years | 2 | ≥ 3 albums by the artist |
| The Rediscoverer | Ratings + history | 2 | Play history |
| The Hometown DJ | Origin country | 2 | Country data |
| The Label Boss | Publisher | 2 | Publisher data |
| The Drifter | Sonic index | 3 | Analysis |
| The Border Crosser | Sonic + boundaries | 3 | Music Library bridge |
| The Single Spotter | Album singles | 3 | Music Library 0.210.0+ |
| The Anniversary DJ | Release dates, life spans | 3 | Release dates |
| The Score Supervisor | Scores group | 3 | Score-genre tracks |
| Surprise me | All of the above | 3 | At least two DJs available |

## 4. Rules every DJ follows

These apply to every candidate before any DJ-specific ranking. They follow the
rules of sonic radio and Hidden favorites.

1. **Never Ban.** Excludes Ban tracks and tracks with a pending Ban or pending
   rating/Love edit.
2. **Never a repeat.** No track already in the queue or offered earlier in this
   DJ session (up to the last 300 guest picks, like *Another round*). Excludes
   identical analysed audio, so the same recording on a compilation counts as
   a repeat.
3. **Not recently heard.** No registered play of the candidate on **any
   device** in the last 30 days (adjustable). Uses the merged history.
4. **Spread.** At most one guest per album artist in any 10 guest picks, and
   never the album artist of the track that just played (except the
   Superfan).
5. **Valid files only.** Only catalog paths with current, matching files, the
   same identity checks as playback (`playback-contract.md`). Stale or missing
   files are skipped, never queued.
6. **Bounded work.** Each pick reads at most 120 candidates, works off the
   playback thread, and gives up after 2 seconds. It never blocks a transport
   command. If it gives up, this round has no guest pick.
7. **Queue limit.** Guest picks count toward the 200-track queue limit. If the
   queue is full, the DJ pauses until there is room.
8. **No file changes.** The DJ never writes tags, ratings or catalog rows. Only
   the user changes ratings, through the normal controls.
9. **Leveling unchanged.** Guests use the existing ReplayGain/estimate
   settings. In **Album** ReplayGain mode a guest track uses **Track** gain,
   because it is not part of the surrounding album.

## 5. Explaining picks honestly

Every guest pick shows **one line of reason**, built only from data Aurora
actually has, in the same honest style as the existing sonic docs.

| DJ | Example reason | What it must *not* claim |
| --- | --- | --- |
| Neighbour | "Sounds close to *Enjoy the Silence* · sound only" | "same mood", "same energy" |
| Crate Digger | "Unrated · album average 4.3★" | "you'll love this" |
| Chart Historian | "#3 VG-lista, week 14 1985" | a debut date the archive can't prove |
| Time Traveller | "Also 1984 · Synth-pop → Hard rock" | "the sound of 1984" |
| Rediscoverer | "Loved · last played March 2024" | personal-preference predictions |
| Hometown | "Also from Norway 🇳🇴" | inferred nationality from names |
| Border Crosser | "Sound only · 1972 instead of 2019" | style or influence |

Rules:

- The reason names the DJ: *"The Chart Historian: #3 VG-lista …"*.
- Clicking the reason opens the evidence: the chart row, the seed track in the
  inspector, the album's ratings.
- When evidence is partial (for example a Billboard annual rank only), the
  reason says so: "Billboard 1985 (annual)".

## 6. Feedback and learning

Guest DJ uses signals Aurora already records. Version one needs no new
feedback buttons.

### 6.1 Signals

| Signal | Source | Meaning |
| --- | --- | --- |
| Guest `completed` | listening history outcome | good pick |
| Guest `skipped` before the played threshold | history outcome + 30 s threshold | bad pick |
| Guest removed from queue before playing | queue command | bad pick (weaker) |
| Guest rated / loved while playing | tag edit | strong positive (or negative if ≤ 2★) |
| Guest added to a playlist | playlist bridge | strong positive |

### 6.2 What the DJ learns (version one)

Keep it simple and visible:

- **Per-DJ scorecard** in History: picks, completed, skipped, rated, average
  rating given. This shows which DJs suit you.
- **Short-term correction:** two skipped guests in a row from the same DJ halve
  its frequency for the rest of the session, and the Drifter resets its walk.
- **Long-term filter:** a candidate skipped as a guest twice is never offered as
  a guest again (it can still be played normally). The list is stored in
  Aurora's state database, and **Reset DJ memory** in Settings clears it.

A model trained on these signals (like the Hidden favorites ranking model)
can come later, once there is enough data. See [§9](#9-phased-delivery).

### 6.3 Optional explicit buttons (later)

- **More like this pick:** the next guest from this DJ uses the guest as its
  seed.
- **Not this:** removes it immediately and counts as a strong negative.
- **Keep:** adds it to a "DJ finds" playlist through the existing playlist
  bridge.

## 7. Interface

### 7.1 Player bar

- A **DJ button** next to shuffle/repeat. It shows the active DJ's monogram
  (for example **CH** for the Chart Historian) when on, and an outline when off.
- Clicking it opens a popover with:
  - the roster, each DJ with its one-line personality and an availability note
    ("Needs sonic analysis · 62 % of your library analysed");
  - the frequency control (Rarely / Sometimes / Often / Back to back);
  - **Album respect** on/off;
  - the active DJ's session stats ("4 picks · 3 completed").
- A global shortcut (default unbound) toggles the DJ, and another cycles through
  DJs, using the existing shortcut system.

### 7.2 Queue

- Guest rows are tinted slightly and carry a small DJ badge and the reason on a
  second line.
- They can be dragged, removed or **Kept** like any queue entry. A guest you
  move keeps its badge.

### 7.3 Now Playing

- While a guest plays, the reason appears under the title:
  *"Guest pick · The Chart Historian · #3 VG-lista, week 14 1985"*.
- The operating-system media controls show the normal title and artist only.

### 7.4 Context menus

- **Start DJ from here** on any track: plays that track and turns on the most
  recently used DJ.
- **Guest DJ → [DJ name]** on the queue's header to switch DJs.

### 7.5 History

- Sessions record their **origin** (`user` / `radio` / `guest:<dj-id>`), so
  Listening Report can filter or label guest picks and the per-DJ scorecard can
  be built.

## 8. Architecture

### 8.1 Ownership

| Concern | Owner | Notes |
| --- | --- | --- |
| DJ on/off, frequency, active DJ | Aurora state DB (`aurora-state.sqlite3`) | Per device. Not synced unless settings sync is added later. |
| Insertion into the queue | **Rust playback** | Like sonic radio refills. Not React polling. |
| Candidate selection | Rust `guest_dj` module | Reads the catalog read-only, `music-analysis.sqlite3` read-only, and history (local + peer snapshots). |
| Border Crosser, Single Spotter, Anniversary | Music Library bridge | Reuses existing capabilities. Gated by capability flags like other bridge features. |
| Skip-twice blocklist, scorecard data | Aurora state DB + history DB | Never in the shared catalog. |

### 8.2 Proposed module shape

```text
src-tauri/src/guest_dj/
  mod.rs          // session state, scheduling (when to pick), queue insertion
  rules.rs        // the shared guardrails in §4
  candidates/
    neighbour.rs  // sonic retrieval (reuses sonic radio queries)
    crate_digger.rs
    chart_historian.rs
    ...           // one file per DJ, each returning ≤120 scored candidates
  reasons.rs      // builds the honest reason line from evidence
src/guestDj.ts    // React: popover, queue badge, Now Playing line
```

Each DJ implements one interface:

```rust
trait GuestDj {
    fn id(&self) -> DjId;
    fn availability(&self, ctx: &DjContext) -> Availability; // Ready | Unavailable(reason)
    fn candidates(&self, seed: &SeedTrack, ctx: &DjContext) -> Result<Vec<Candidate>>; // ≤120
}
```

`rules.rs` filters and spreads the candidates, then a seeded random choice
among the top few (default top 5) picks one, so the DJ varies without becoming
erratic.

### 8.3 Queue entries

Queue entries get an optional origin field:

```text
origin: user | radio | guest { dj_id, reason, evidence_ref }
```

The field is persisted with the queue (like radio's saved station state), so a
restored session still shows its guest badges. Re-resolving the queue after an
import keeps the origin with the stable track key.

### 8.4 Performance

- Picks run on a background worker, at most one at a time per session.
- Chart and year candidates are indexed lookups with bounded results. Sonic
  candidates reuse radio's bounded neighbour query.
- History checks use stable track keys over the same bounded peer-snapshot
  reads as Listening Report (at most 16 peers).
- Target: a pick in under 300 ms on the full catalog. The 2-second limit in §4
  is a safety cap, not the target.

### 8.5 Interaction with existing playback features

| Feature | Behaviour with Guest DJ on |
| --- | --- |
| Shuffle | DJ inserts after the current track. The shuffle order skips guests already played. |
| Repeat one | DJ pauses (nothing to insert between). |
| Repeat all | Guests are not repeated on the next pass. New picks are made instead. |
| Sonic radio | Allowed, but the Neighbour and Drifter are disabled (radio already does sound). |
| Sonic journey / Fit my time | DJ is paused, because journeys are planned to a time budget. |
| Stop after (roadmap #16) | Respected. No guest is inserted after the stop point. |
| Gapless | Unchanged. A guest is an ordinary prepared successor. |
| Network Mode (Mac) | DJs whose data is in the local catalog work. Sonic DJs need the analysis DB on the Mac, otherwise they show as unavailable. |
| Linked tracks (proposed elsewhere) | No guest is inserted between linked tracks. |

## 9. Phased delivery

| Phase | Scope | Depends on |
| --- | --- | --- |
| **1 · Engine + 3 DJs** | Scheduling, Album respect, guardrails, queue origin, reasons, player-bar popover, queue badges. Neighbour, Crate Digger, Chart Historian. | Existing sonic, chart and rating data |
| **2 · More DJs + scorecard** | Time Traveller, Superfan, Rediscoverer, Hometown, Label Boss. History origin field, per-DJ scorecard, skip-twice blocklist. | History origin field |
| **3 · Bridge DJs + learning** | Drifter, Border Crosser, Single Spotter, Anniversary, Score Supervisor, Surprise me. Explicit buttons. Optional ranking model trained on guest outcomes. | Music Library bridge capabilities |

Phase 1 alone is useful: it gives three very different DJs (sound, ratings,
charts), so you can judge the concept quickly.

## 10. Non-goals

- **No language model in selection.** Plexamp removed Sonic Sage in 2026
  because its results were not "consistent or useful enough". DJ picks must be
  reproducible from catalog data and explainable. An LLM may later *phrase* a
  reason, never invent one.
- **No mood or energy claims.** Aurora's sonic data does not establish mood,
  genre or key (see `sonic-atlas.md`). BPM/key DJs wait for real BPM/key
  analysis.
- **No beatmatching or DJ-style mixing.** Transitions stay gapless or ordinary.
  Crossfade, if it is added, is a separate setting.
- **No streaming catalog.** Only tracks in your own library.
- **No change to your files.** The DJ never edits tags, ratings or the catalog.

## 11. Open questions

1. **Default frequency:** is *Sometimes* (1 in 4) right for album-heavy
   listening, or should Album respect make *one guest per album boundary* the
   default instead?
2. **Shared settings:** should the active DJ and frequency follow you between
   the desktop, laptop and Mac (via the shared state snapshot), or stay per
   device?
3. **Crate Digger and rating missions:** should ratings given to Crate Digger
   picks count toward rating missions' progress?
4. **Chart Historian sources:** which charts first? Suggested order:
   VG-lista → Official UK → Billboard annual → Norsktoppen / Ti i Skuddet.
5. **Names:** are character names (*The Chart Historian*) the right tone, or
   plain labels (*Charts DJ*)?
6. **Scores:** should scores be excluded from all DJs except the Score
   Supervisor by default?

## Background

Guest DJ came out of the November 2026 comparison of Aurora with Roon,
MediaMonkey, Helium, JRiver, MusicBee and Plexamp. Five of those six apps keep
playback going with some form of radio, Auto-DJ or Auto-Queue, but only
Plexamp *inserts* picks into a queue the user owns. Aurora's version keeps
that idea but builds its DJs on data no other player has: charts, singles,
original years, cross-device history and the size of the unrated backlog.

Related documents:

- [Sonic analysis and radio](sonic-analysis.md)
- [Same sound, different world](same-sound-different-world.md)
- [Hidden favorites](hidden-favorites.md)
- [Rating missions](rating-missions.md)
- [Charts contract](charts-contract.md)
- [Listening history contract](listening-history-contract.md)
- [Playback contract](playback-contract.md)
