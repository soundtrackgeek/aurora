# Sonic analysis and radio

Install Music Library 0.178.0 or newer alongside Aurora. In Music Library,
open **Tools → Audio analysis** or an album's details, save the idle/time-window
schedule, and analyze an album or your favorites before the full library.
Results become available after every completed MP3. The computer must remain
awake and Music Library running for a scheduled batch to progress.

Aurora's **Track** sidebar has **More like this**: find similar tracks, optionally
filter by minimum rating or same genre, and start sonic radio. An unanalyzed or
changed seed offers **Analyze this track**, which runs one explicit, immediate
request through Music Library's headless bridge. Pause the background analysis
first if it already owns the analyzer. This does not edit music files or tags.

Radio queues nearest matches to the seed, limits each batch to three tracks per
artist and two per album, excludes Ban tracks and pending Aurora rating/Love
edits, and refills below 20 remaining tracks. It saves the seed, filters, and
queued track identities alongside Aurora's normal saved playback queue. Start
a new station after the bounded 1,900-track session. Starting another queue
ends the station; **Stop radio** stops refills and keeps the current queue.

Use `sonic:yes` in Songs search for current catalog paths with saved analysis
for the compatible profile, or `sonic:no` for those without it. These work with
existing fields and boolean operators, for example `genre:synth-pop sonic:yes`.
Albums/Artists qualify through matching tracks. An album with some analyzed and
some unanalyzed tracks can qualify for both searches. Saved coverage does not
probe every MP3; similarity checks seed and shortlisted candidate file size/time
observations and excludes changed/missing files until reanalysis.

Analysis is stored in `music-analysis.sqlite3` beside Music Library's catalog;
Aurora attaches it read-only. Music Library owns checkpoint/schedule storage and
the isolated GPL-3.0 analyzer. Profile identifiers pin Bliss 0.13.0, Symphonia
0.6.1 and feature version 2; results from other profiles are excluded. Queries
resolve current catalog IDs by path and preserve Track Artist versus Album
Artist. Existing tag-only edits can reuse audio features after reanalysis.

## Album similarity

Aurora 0.30.0 adds **Album sidebar → More like this album → Find similar albums**.
Open a result in Albums or play its album queue. Music Library 0.179.0 offers the
same comparisons under **Albums → Sounds like this album**. Existing compatible
track analysis is reused without decoding again.

Albums are compared by the equally weighted mean of their usable track features
with the same profile-specific distance matrix. Choose **50%**, **80%**, or
**Complete albums only**. Both seed and matches need three usable tracks, or all
tracks for shorter releases, plus that percentage of their cataloged MP3 tracks.
Results show usable counts and label partial analysis. Banned tracks, including
pending Aurora Ban edits, do not contribute. Album IDs preserve separate editions
and namesakes; tracks across discs sharing one album ID are aggregated together.

Rollups are computed from a consistent catalog/results read snapshot on each
request. Ranking streams albums and retains a bounded shortlist; the seed and
all contributing files in shortlisted albums are checked for freshness and their
means recomputed. Missing/changed/incompatible results cannot contribute. The
pool count describes saved analysis before shortlist freshness checks.

After finding albums, **Start album sonic radio** ranks individual tracks against
the selected album's mean and excludes all tracks from that seed album. Rating,
same-genre, Ban filtering, diversity caps and bounded refills work as for track
radio. The album ID and coverage threshold persist across restarts; old saved
track stations remain compatible. Refills include any further completed analysis
of the seed, so its mean can evolve while analysis progresses.

## Sonic journey playlists

Music Library **Playlist Builder → Sonic journey** and Aurora **Track sidebar →
Sonic journey** connect **2–10 ordered track stops**. Search analyzed tracks by
artist/title; Aurora can also add the currently selected track while you browse.
Add, remove, or reorder stops, then choose **1–10 connecting tracks between each
pair**. Five stops and three connectors per leg produce 17 tracks. Every chosen
stop remains in its original position relative to the other stops; tracks never
repeat across the journey. The maximum is 100 tracks.

Minimum rating and **first stop's genre** apply to connecting tracks. Explicitly
chosen stops can have other ratings/genres, but every stop and connector must
have compatible current MP3 analysis and must not be banned. Partial-library
analysis is enough; missing stop analysis and insufficient eligible connectors
are reported separately. The builder returns a complete journey or no playlist,
so it never silently drops a stop or reduces the requested length.

The shared math interpolates sound-feature waypoints between each stop, streams
analyzed candidates into bounded shortlists, and uses a bounded beam search to
balance adjacent sound changes and waypoint proximity. It is an approximation,
not a global shortest path or a promise of matched BPM/key or seamless mixing.
Only stops and shortlisted files are checked for freshness, keeping memory and
filesystem work bounded as the library grows. No decoding or full reanalysis is
required. Browser previews use sample analysis rather than real music files.

Review the complete numbered playlist, name it, and choose **Save journey
playlist**. The exact reviewed sequence is revalidated and saved as an ordinary
Music Library playlist, not regenerated at save time. Changed/missing/banned
tracks require a new preview. Aurora honors pending Ban/rating edits when
building and revalidating; the companion also checks its catalog on save, so
pending changes may need to finish syncing first. Aurora can **Play journey**
with shuffle disabled and radio stopped. Its draft survives track selection
changes in the running app. Music Library 0.180.0 is required for Aurora's save
bridge; Aurora 0.31.0 adds the journey controls.

This release ranks tracks/albums with bounded streaming candidate lists.
Aurora 0.31.1 counts complete album coverage only in albums with analyzed
paths, moves track/radio metadata through overlay batches, skips per-track
genre-sync checks when no edits are pending, and clones journey metadata only
for shortlisted candidates. See [the native performance proof](sonic-performance.md).
Saved journeys preserve the reviewed sequence; Smart refresh is unavailable
because rebuilding from ordinary filters would lose the chosen stops and order.
Approximate indexing and discovery blending remain future work. Radio follows
the original seed's sound throughout the station.

## Backup and cross-PC reuse

Music Library 0.181.0 owns **Tools → Audio analysis → Backup and cross-PC reuse**.
Aurora 0.31.2 includes these directions under **Track → More like this**.
Backups default to **OneDrive\\_musicbackup\\sonic-analysis**, beside the shared
state/history backups. Choose a synced folder manually if OneDrive is not detected.

On the source computer, use **Back up analysis** while the analyzer can continue
running. Wait for OneDrive to finish uploading. On the receiving computer, make
the catalog and MP3s available at Music Library's local cataloged paths, download
the archive, and use **Choose backup to restore**. Review the date/count, pause
any running analysis, then **Merge this backup**. A local safety archive is saved
before merging existing features; conflicting or corrupt data aborts the merge.

Run **Verify reused analysis**, or resume normal analysis. The verification job
obeys the receiving computer's idle/hours schedule, supports pause/resume/cancel,
and reads audio fingerprints without decoding files that have no cached result.
Different local paths, timestamps, numeric IDs and tag-only differences do not
require extraction again when audio payloads match. Imported file observations
are never trusted: each binding is checked against local audio and current
catalog membership. Files with no match remain unanalyzed.

Aurora continues reading the local analysis store beside the catalog. Each
verified checkpoint becomes usable for `sonic:yes`, track/album similarity,
radio and journeys; refresh the search or request new matches as coverage grows.
Archives include only compatible hashes, features and weights, with versions,
checksum and integrity checks. They exclude music, catalog, machine settings,
jobs and queues. Large verification scans still incur disk/network I/O. These
manual backups replace unsafe live SQLite/WAL copying; a successful local save
does not itself confirm OneDrive upload or a physical second-PC transfer.

Aurora vendors the small MIT contract/math crate from Music Library at the
immutable revision recorded in `vendor/music-sonic-core/SOURCE.json`. Aurora
0.32.0 and Music Library 0.184.0 also share a local persistent index of completed
vectors and album means, with background rebuilding, current-data validation and
a streamed fallback. Receiving PCs rebuild this disposable cache after analysis
reuse; it is excluded from portable backups. See [index and scale proof](sonic-index-performance.md).

Shared source checksums are verified by `npm run check:sonic` and CI. This keeps builds
independent of a companion checkout or Music Library's large Git history.
Update that snapshot from the canonical `crates/sonic-core` source when changing
the shared profile; do not fork its implementation.
