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

This first release ranks tracks with a bounded streaming candidate list.
Album vectors, A-to-B sonic paths, approximate indexing, discovery blending,
and coordinated cross-PC analysis snapshots remain future work. Radio follows
the original seed's sound throughout the station.

Aurora vendors the small MIT contract/math crate from Music Library at the
immutable revision recorded in `vendor/music-sonic-core/SOURCE.json`. File
checksums are verified by `npm run check:sonic` and CI. This keeps builds
independent of a companion checkout or Music Library's large Git history.
Update that snapshot from the canonical `crates/sonic-core` source when changing
the shared profile; do not fork its implementation.
