# A playable sonic atlas

Music Library **0.201.0** and Aurora **0.47.0** implement the local-neighborhood
first version of October opportunity 11.

## Use it

- Music Library: **Tools → Audio analysis → Open sonic atlas**, or the Audio
  analysis panel on an album. Search an analyzed track by artist/title.
- Aurora: open a track inspector and choose **Explore sonic atlas**. Music
  Library 0.201.0 or later must be available through the existing local bridge.
- Choose a numbered region. Its name is an example track, not an inferred genre
  or mood. Inspect all its tracks, rated/loved progress and available-history
  counts. Open a track's album, or center a new neighborhood on it. **Back**
  reopens the previous saved neighborhood.
- In Aurora, **Play entry point**, individual **Play**, or **Play region** uses
  complete tracks and the existing player/normalization. **Pause playback** stays
  available inside the atlas. No automatic excerpt
  selection or gain adjustment is added.
- **Save region playlist** writes precisely the reviewed available members, in
  the displayed order, to a regular Music Library playlist. Give it your own
  name or use the region's suggested name. It does not refresh automatically.

## Stable maps and honest evidence

**Reload counts** reconciles current ratings, Love, available play history and
file eligibility without moving points or repartitioning regions. **Rebuild
neighborhood** deliberately replaces the snapshot and revision. A save or play
based on an older revision fails with reload guidance. Rating and Love edits do
not change geometry. Aurora projects its pending rating/Love edits and blocks
locally banned tracks before save/play.

Each atlas contains at most **180 distinct MP3 recordings**, including its seed,
drawn from **4,096 indexed candidates** using the current profile's original
weighted distance. Candidates are re-ranked by exact distance and stable track
identity, checked against local file size/mtime, and deduplicated by MP3 payload
hash. Counts cover this selected neighborhood, not a library-wide census. Sparse
coverage and duplicate-heavy neighborhoods can yield fewer tracks. Cold indexes
request a retry instead of scanning the whole collection synchronously.

The shared `sonic-atlas-v1` algorithm uses deterministic farthest-first centers
and assigns tracks with the full metric, then chooses a real medoid in each
region. It produces one to six regions, with one region for uniform data.
Grouping is a navigational approximation, not a claim about natural genres or
listener preference.

The horizontal position is the profile's mean spectral centroid (brightness);
the vertical position is mean frame level. Each axis is scaled to the saved
neighborhood's extent; a constant dimension stays centered. These are readable
feature axes, not a distance-preserving embedding. Two tracks can overlap on the
map and differ in the other 21 dimensions. Numbered region buttons and the track
list provide the same navigation without relying on color, hover, or tiny dots.
Level is not LUFS; brightness is not mood, instrumentation or quality. No BPM or
musical-key label is inferred.

Owned/rated/loved/history totals count currently eligible snapshot members;
unavailable points are counted separately and retain faded positions. Rated
means any saved nonzero rating. History uses Music Library's reviewed identity
links, reports the available history size/start, and says **absent from available
history**, never **never heard**. A track played on another edition may lack a
matching identity link; this is not recording-wide listening-history inference.

## Ownership and storage

Music Library's `sonic-atlas.sqlite3` stores snapshots by seed identity. Snapshot
evidence binds each point to its MP3 payload, vector and profile weights. Local
audio, catalog and saved analysis are never changed by atlas computation. The
normal listening identity refresh and existing playlist writer retain ownership
of their respective catalog changes. Missing/banned/changed recordings and
incompatible weights become unavailable until rebuilt.

Aurora uses operation `sonicAtlas`, capability `sonicAtlasV1`, with load, rebuild,
audition and save actions. It owns presentation and playback, without a second
atlas database. Playback validates the requested tracks and then resolves them
through Aurora's player. Playlist creation uses Music Library's existing author
and preserves the displayed order. No audio or vectors leave the PC.

## Scope and validation

This release implements a playable local atlas, not a collection-wide hierarchy,
an optimized learned mapping metric, a two-favorite corridor, or Tonehavn shelves
and offline packs. Ideas 21/23 informed readable axes and explicit metric
provenance; the full descriptor/metric-lab proposals remain separate work.
Bookmarks here are the persistent per-seed snapshots and named regular playlists;
editable region taxonomy and a saved-atlas browser are future work.

Regression coverage exercises full-metric grouping, deterministic membership,
degenerate data, persisted geometry across rating changes, freshness/profile
invalidation, reviewed playlist membership, stale revisions, pending Aurora tags,
navigation, search and late playback responses after closing. Browser fixtures
are explicitly illustrative. Native speaker output and subjective discovery
quality require listening evaluation and are not established by browser tests.

### Local-library probe, 2026-10-23

An opt-in Windows debug-build probe opened the live catalog and analysis
read-only and rebuilt only the disposable sound index. Starting at Gary U.S.
Bonds’ **Quarter to Three**, it returned 180 eligible recordings in six regions
against 7,552 available plays. After index preparation, the first atlas build
and reconciliation took **16.67 s**; reconciliation of the saved geometry took
**0.063 s**. Cache preparation plus the probe took **217 s**. These are observed
local debug timings, not release-build or cold-computer guarantees. The probe
did not save a live playlist, rate/tag files, or perform a listening-quality test.

Reproduce with the ignored Rust test `live_readonly_neighborhood` and
`SONIC_ATLAS_LIVE_DIR` pointing at an existing catalog directory. Optionally set
`SONIC_ATLAS_REBUILD_INDEX=1` to replace its disposable index first; omit this
when the index is current. `SONIC_ATLAS_PROBE_OUTPUT` optionally writes the
returned map to a separate JSON file for inspection.
