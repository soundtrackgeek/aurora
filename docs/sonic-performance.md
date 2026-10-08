# Sonic performance proof

Music Library 0.180.1 and Aurora 0.31.1 optimize exact sonic retrieval without
changing the analysis profile or requiring reanalysis. Album matching begins
with saved analysis and uses the catalog's album index to count complete MP3
coverage in those albums. Unanalyzed and banned MP3s still contribute to the
coverage denominator. Unrelated albums are not counted.

Music Library streams compact track identities/features, ranks them with the
same distance and identity tie-break, verifies freshness, and loads display
metadata only for final matches. A read transaction keeps the streamed and
hydrated catalog/results consistent. Both apps clone journey metadata only when
a candidate enters a bounded waypoint shortlist. Aurora also skips individual
pending-genre checks when its sync queue is empty, checking again each batch so
new edits remain visible.

## Measured catalog

The October 8 proof uses SQLite online backups of a Windows library containing
**1,104,893 catalog tracks and 54,125 analyzed paths**. It executes the actual
native adapters, including JSON feature decoding, ranking and shortlisted file
freshness checks. The journey has five stops and three connectors per leg.
Aurora uses an empty disposable state store, so this proof does not measure a
large pending-edit queue. Regression tests cover pending edits separately.

The measurements use each app's **Cargo test profile**, not an installed
release build: Music Library is unoptimized; Aurora uses optimization level 1
with dependency optimization level 3. Compare before/after within each app.
Each case runs three times; the first pass warms caches, and the
comparison reports the median of the remaining two. Compilation is excluded.
First-pass timings depend strongly on filesystem caches and background work.
Full response JSON is compared, including IDs, metadata, distances, coverage,
radio order, and the complete journey sequence.

Music Library warm medians on that fixed snapshot:

| Request | Before | After | Ratio |
| --- | ---: | ---: | ---: |
| 50 track matches | 2.00 s | 1.91 s | 1.05× |
| 20 album matches, 50% coverage | 5.06 s | 1.73 s | 2.92× |
| Five stops, three connectors per leg | 1.88 s | 1.82 s | 1.03× |

Album matching shows the clear latency improvement. The small track/journey
timing differences should not be read as a reliable speedup from two samples;
their regression checks establish reduced metadata/clone work. Every response
was identical across all three before/after passes.

Aurora warm medians on the same snapshot:

| Request | Before | After | Ratio |
| --- | ---: | ---: | ---: |
| 50 track matches | 2.63 s | 2.06 s | 1.28× |
| 50 radio candidates | 2.42 s | 2.17 s | 1.12× |
| 20 album matches, 50% coverage | 3.29 s | 0.89 s | 3.68× |
| Five stops, three connectors per leg | 0.68 s | 0.51 s | 1.33× |

Every track, radio, album and journey response was identical across all three
before/after passes. These are measurements on this snapshot and machine,
not fixed latency promises for other coverage levels, pending edits or storage.

## Repeat the proof

Prepare **separate disposable copies** of `music-library.sqlite3` and
`music-analysis.sqlite3` using SQLite's online backup API. Each backup is a
consistent database snapshot; do not copy a live database file without its WAL.
Keep real audio paths accessible for freshness checks. Use the same snapshots
for both runs, and do not edit those audio files during the comparison.

Add `seeds.json` in the snapshot directory:

```json
{"stops":["first-track-key","second-track-key","third-track-key","fourth-track-key","fifth-track-key"],"album":"album-id"}
```

Choose five distinct, fresh, analyzed MP3s and an album with at least 50% valid
analysis coverage. The fixture must have enough eligible tracks to complete
the journey and produce track/album matches. The proof rejects empty or
incomplete responses so they cannot appear as performance improvements.

From the repository root, run before and after the change:

```powershell
$env:MUSIC_SONIC_BENCH_DIR = 'D:\sonic-proof\snapshot'
$env:MUSIC_SONIC_BENCH_OUTPUT = 'D:\sonic-proof\before.json'
cargo test --manifest-path src-tauri/Cargo.toml -j 2 sonic_performance_proof -- --ignored --nocapture --test-threads=1
# Run the changed implementation against the same snapshots.
$env:MUSIC_SONIC_BENCH_OUTPUT = 'D:\sonic-proof\after.json'
cargo test --manifest-path src-tauri/Cargo.toml -j 2 sonic_performance_proof -- --ignored --nocapture --test-threads=1
node scripts/compare-sonic-performance.mjs D:\sonic-proof\before.json D:\sonic-proof\after.json
```

The native test is ignored in ordinary CI because it requires real snapshots.
It opens the snapshots read-only. It writes only its requested JSON output;
Aurora's equivalent also creates a temporary local state store. Run heavy builds
sequentially and keep background load comparable when measuring.

## Persistent index and full-scale follow-up

The measurements above document the earlier streamed implementation at partial
analysis coverage. Music Library 0.184.0 and Aurora 0.32.0 add a persistent exact
weighted tree index, a bounded current-data delta, background rebuilds and a
streamed fallback. The [index and scale proof](sonic-index-performance.md)
compares both native implementations on a newer real snapshot and a synthetic
1.1-million-analyzed-track fixture, plus a release-profile core stress benchmark.
It distinguishes these fixtures from a fully analyzed real million-file library.
