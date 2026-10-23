# Same sound, different world

Opportunity 09, delivered with Music Library 0.200.0 and Aurora 0.45.0
(2026-10-23). Music Library owns the shared boundary policy, catalog metadata,
analysis and saved playlists. Aurora hosts the listening challenge.

In Aurora, open a track inspector, then **More like this → Same sound, different
world**. Choose **Decade (album year)**, **Album artist**, or **Catalog genre**,
then one of three directions:

- **Stay close** requires the same value of the chosen metadata field.
- **Either** ranks by sound without a metadata boundary.
- **Cross borders** requires a different, known value.

This incorporates the companion report's correction: sonic similarity already
crosses artists and genres frequently. Coherence must come from explicit
metadata, and staying in the same decade does not guarantee a shared style.
This is the metadata control relevant to 09, not the full Last.fm station
blending proposed in idea 31. Results are labeled **sound only**.

Choose **Find three sounds**, play the seed or individual candidates, and
**Reveal connections** when ready. The dialog retains its seed while playback
changes. Each play replaces the queue with that one complete track and stops
sonic radio. Pause and playback errors remain available inside the dialog.
Candidate names are concealed in the dialog until reveal; the
player and operating-system media controls can still show identity. This is
not a fully blind playback mode. It does not select excerpts or grade answers.

After revealing, save one candidate or **Save discoveries** through the existing
Music Library playlist dialog. The reviewed order is preserved. **Another round**
excludes up to the last 300 offered track keys; changing the boundary or direction
resets the round. No rating, love/ban state or music file is changed by discovery.

## Metadata and selection rules

- Decade uses catalog `year`, exposed as Aurora's `originalYear`. Release and
  reissue years are never substituted. Missing/invalid years cannot prove a
  boundary; changing to Either still permits acoustic retrieval.
- Album artist means the normalized full stored album-artist credit, not the
  track performer, country, influence or an inferred artist identity. Combined
  credits are compared as whole credits. Empty, unknown and Various Artists
  values cannot establish an artist boundary.
- Genre uses the stored genre, including pending Aurora genre changes. It is
  not inferred from sound and does not interpret genre families or film moods.
- The seed and results must have current-profile analysis, available unchanged
  MP3 files and no effective ban. Pending Aurora bans apply. Invalid candidate
  vectors are skipped. Equal audio hashes exclude the seed recording and
  duplicate recordings within a round, including copies on other albums.
- The seed album is excluded and each round contains at most one track per
  album. Matching normalized title-and-performer credits are skipped as likely
  repeats, even when payload hashes differ. This conservative metadata rule can
  also skip a rerecording; it does not identify every re-encoding or cover.
- A round contains at most three nearest eligible recordings with weighted
  profile distance at most **0.25**. This is a conservative product heuristic,
  not a validated perceptual cutoff, confidence, percentage or rating prediction.
  Subjective listening quality remains to be evaluated.
- Retrieval uses the existing exact index's nearest 4,096 keys plus dirty
  identities. Boundary, audio identity, bans, freshness and distance checks
  happen before filling the three slots. This is a bounded shortlist; a sparse
  or empty round does not mean the entire collection has no qualifying tracks.
  Rules are never weakened to fill the round. If the index is unavailable, the
  dialog asks you to open Music Library and retry after indexing; it does not
  start a full-library scan on the interactive path.

The policy is `music-sonic-core::worlds`, version `sonic-worlds-v1`, vendored into
Aurora at a recorded immutable Music Library revision. Aurora reads existing
catalog and analysis stores; no new schema, decoding pass or publication contract
is needed. Saving uses the supported catalog-playlist bridge.

Country and barely-played-artist challenges, Last.fm evidence blending, and
Tonehavn's native challenge remain future work. Tonehavn can play saved discoveries
through its existing linked-playlist publication. Native sonic selection and
reveal evidence there need a versioned publication contract.

## Verification

Shared policy tests cover both directions, unknown values, normalized credits
and invalid years. Native regression fixtures exercise filtering before the
three-result limit, original versus release year, pending bans, stale/missing
files, incompatible/malformed analysis, identical recording exclusion, round
exclusions, distance ceilings, read-only catalog access and unavailable indexes.
UI tests cover concealed identities, actual single-track playback requests,
pause, reveal/save order, round rotation, stale asynchronous results, empty/partial
rounds and error/retry states. Browser preview uses explicitly illustrative
distances; it is not evidence of native audio quality.

Browser inspection at 1280×720 verified empty and partial rounds,
seed retention while playing a candidate, pause, reveal and the playlist-dialog
handoff, with no console errors. The preview correctly requires desktop for
the final shared-playlist write; its form retains the draft on that error.
Installed-app audio output, subjective listening quality and mobile behavior
have not been verified by that browser inspection.

A read-only native probe on 2026-10-23 tested three real seeds in both decade
directions: West Street Mob's *Break Dance Electric Boogie*, Video Kids'
*Woodpeckers from Space*, and Climie Fisher's *Love Changes (Everything)*.
All six rounds returned three current recordings satisfying their rule and
distance ceiling. The Climie Fisher cross-decade round excluded its repeat on
the 2023 compilation *80's Hits* and returned album years from the 1960s, 1970s
and 1990s. This establishes catalog constraints, not recording-era truth or
listening quality. On this run the cold index query took 13.6 seconds, first
queries for the other seeds about 4.5 seconds, and repeated-seed queries
0.24–0.44 seconds. These are observations during local checks, not latency guarantees.
