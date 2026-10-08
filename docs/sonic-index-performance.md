# Persistent sonic index and scale proof

Music Library **0.184.0** and Aurora **0.32.0** share a persistent index for
track matches, album matches, radio candidates, journey waypoints and Music
Library's Discovery sound neighbors. Existing completed analysis is reused;
the index does not decode MP3s or change the analysis profile.

## Operation

Open Music Library once after updating to initialize the catalog and existing
analysis store's change journals. The first similarity request builds
`sonic-index.bin` beside `music-analysis.sqlite3` in the background. Requests
can continue using the streamed baseline during that build. Both apps can read
the same published file, and an OS file lock prevents simultaneous writers.

The current Bliss metric has 23 dimensions and diagonal weights. A balanced
weighted k-d tree searches that metric exactly, including the existing identity
tie-break. An unsupported metric uses the streamed baseline. Current catalog
metadata, numeric IDs, bans, pending Aurora edits, ratings, genres and file
observations are resolved after retrieval. Journeys also check that every
waypoint's eligible pool fits inside the retrieved distance frontier.

Database identities, filesystem incarnations and bounded change journals guard
each snapshot. Up to 8,192 intervening journal rows can be searched as a delta:
changed tree identities are excluded and current identities are reranked from
SQL. A background rebuild becomes due after more than 1,024 changes. Larger
deltas, missing journals, replaced stores and insufficient eligible shortlists
use the full baseline. Failed builds retry on later requests after a 30-second
cooldown. A SHA-256 checksum, format/profile validation and atomic publication
protect the derived cache; artifacts from a future format are preserved.

Exact coverage counts are saved in the index and reused after restart while
their source generations match. The catalog MP3 total does not need recounting
after an analysis-only checkpoint. Other changed coverage
counts can still require a SQL count, so the fixed-snapshot warm timings do not
measure a continuously changing analyzer workload.

The index is local and disposable. Portable analysis backups continue carrying
completed analysis only. After restoring and verifying analysis on another PC,
that PC builds its own index. No new setting or manual index export is needed.

## Measurement scope

The October 8, 2026 proof runs on Windows x64 with a Ryzen 5 7600X and 32 GB RAM.
It separates the following measurements:

- A read-only online backup of the actual catalog: **1,104,893 tracks**, with
  **56,619 analyzed paths / 56,427 unique analyzed audio payloads**.
- A disposable synthetic database with **1,100,000 analyzed tracks / 68,750
  albums**, resampled from the real feature corpus. Only benchmark shortlists
  have one-byte fixture files. This exercises SQL, JSON, freshness checks,
  ranking and response hydration, but does not measure audio extraction,
  fingerprinting or real million-file storage behavior.
- A release-profile core benchmark over **1,100,000 vectors**, using resampled
  real features with jitter and a uniform 23-dimensional stress distribution.
  It includes track targets, journey targets and 1%-eligible predicates.

Native requests use each app's Cargo test profile: Music Library is unoptimized;
Aurora uses optimization level 1 with dependencies at level 3. Compilation is
excluded. Each request runs five times; warm medians use iterations 1–4 and
the first pass is reported separately. OS filesystem caches are not flushed,
so a first pass is not a cold-storage measurement. Aurora uses an empty temporary state
store; regression tests cover pending edits separately. Complete response JSON
must agree with the exact baseline, including ordered IDs, distances, coverage,
radio candidates and the five-stop, three-connectors-per-leg journey. Empty
matches and incomplete journeys fail the proof.

## Native results

Warm medians on the actual catalog with partial analysis coverage:

| App | Request | Exact baseline | Index | Speedup |
| --- | --- | ---: | ---: | ---: |
| Music Library | 50 track matches | 1,162 ms | 83 ms | 14.01× |
| Music Library | 20 albums, 50% coverage | 1,566 ms | 278 ms | 5.63× |
| Music Library | Discovery, one album anchor | 1,654 ms | 516 ms | 3.21× |
| Music Library | Five-stop journey, three connectors per leg | 1,675 ms | 425 ms | 3.94× |
| Aurora | 50 track matches | 1,548 ms | 174 ms | 8.91× |
| Aurora | 50 radio candidates | 1,486 ms | 175 ms | 8.50× |
| Aurora | 20 albums, 50% coverage | 841 ms | 152 ms | 5.54× |
| Aurora | Five-stop journey, three connectors per leg | 555 ms | 121 ms | 4.60× |

Warm medians on the **synthetic 1,100,000-analyzed-track fixture**:

| App | Request | Exact baseline | Index | Speedup |
| --- | --- | ---: | ---: | ---: |
| Music Library | 50 track matches | 16,751 ms | 95 ms | 177.11× |
| Music Library | 20 albums, 50% coverage | 23,894 ms | 299 ms | 79.95× |
| Music Library | Discovery, one album anchor | 24,203 ms | 649 ms | 37.28× |
| Music Library | Five-stop journey, three connectors per leg | 24,770 ms | 1,102 ms | 22.48× |
| Aurora | 50 track matches | 24,073 ms | 131 ms | 184.14× |
| Aurora | 50 radio candidates | 24,315 ms | 137 ms | 178.06× |
| Aurora | 20 albums, 50% coverage | 9,543 ms | 125 ms | 76.46× |
| Aurora | Five-stop journey, three connectors per leg | 4,737 ms | 197 ms | 24.00× |

All **80 native response comparisons** agree completely with their baselines,
using the index with **zero full-scan fallbacks**. The [sanitized summary](sonic-index-benchmark.json)
records each first pass, source counts, index builds and memory measurements.

The current real-data index contains 56,619 tracks and 4,974 album
means, taking 10.7 MiB on disk. Build took 38.58
seconds; load took 0.34 seconds.

The synthetic full-scale index contains 1,100,000 tracks and 68,750 album
means, taking 227.1 MiB on disk. Build took 99.69
seconds; load took 6.42 seconds.

These build/load measurements use Music Library's unoptimized test profile.
The full-scale indexed request processes peaked at 305 MiB for Music Library and 308 MiB for Aurora,
sampled every 200 ms. These are native benchmark processes; they exclude the
desktop WebView and a concurrently running analyzer. First passes include
process-local cache initialization. Saved exact counts prevent a separate
whole-library coverage scan when source generations match.

These measurements do not establish a fixed latency for arbitrary filters,
stale files, pending queues or storage devices.

## Release-profile vector stress test

Each distribution contains 1,100,000 vectors and runs 24 exact/index queries.
The real-feature sample contains 56,427 analyzed audio vectors. All **48 ordered
top-50 results agree**, giving 100% measured recall at 50, including ties and
the eligibility predicate. Medians combine track and journey-midpoint targets:

| Distribution | Eligible vectors | Exact baseline | Index | Median vectors visited |
| --- | --- | ---: | ---: | ---: |
| Resampled real features with jitter | All | 67.67 ms | 0.94 ms | 3.27% |
| Resampled real features with jitter | 1% | 6.34 ms | 3.51 ms | 19.88% |
| Uniform 23-dimensional stress | All | 77.49 ms | 16.33 ms | 29.06% |
| Uniform 23-dimensional stress | 1% | 6.76 ms | 11.75 ms | 63.83% |

Building the two trees took 671 ms and 615 ms respectively, and reloading them
took 152 ms and 163 ms. Each core file is 120.6 MiB because it uses shorter
synthetic identities than the native database fixture. The complete core proof
process peaked at 424.3 MiB of working set, sampled every 200 ms.

The sparse uniform case is slower than a scan that filters out 99% of points
before computing distances. Tree traversal is data-dependent; high-dimensional
uniform points and very selective predicates can weaken pruning. This result
is retained rather than claiming every indexed query is faster. Core timings
exclude native SQL, feature JSON, overlays, file observations and response work.

## Reproduce

From Music Library's repository root, use separate new output directories:

```powershell
python scripts/prepare-sonic-index-proof.py --catalog 'D:\library\music-library.sqlite3' --analysis 'D:\library\music-analysis.sqlite3' --output 'D:\sonic-proof\snapshot'
$env:MUSIC_SONIC_BENCH_DIR = 'D:\sonic-proof\snapshot'
$env:MUSIC_SONIC_BENCH_DISPOSABLE = '1'
cargo test --manifest-path src-tauri/Cargo.toml sonic_index_build_proof -- --ignored --nocapture --test-threads=1
$env:MUSIC_SONIC_BENCH_MODE = 'exact'
$env:MUSIC_SONIC_BENCH_OUTPUT = 'D:\sonic-proof\ml-exact.json'
cargo test --manifest-path src-tauri/Cargo.toml sonic_performance_proof -- --ignored --nocapture --test-threads=1
$env:MUSIC_SONIC_BENCH_MODE = 'index'
$env:MUSIC_SONIC_BENCH_OUTPUT = 'D:\sonic-proof\ml-index.json'
cargo test --manifest-path src-tauri/Cargo.toml sonic_performance_proof -- --ignored --nocapture --test-threads=1
node scripts/compare-sonic-performance.mjs D:\sonic-proof\ml-exact.json D:\sonic-proof\ml-index.json
```

Run the same native comparison from Aurora's root, with different output names.
The generator uses SQLite's online backup API instead of copying live WAL
databases. Build helpers mutate only explicitly disposable copies; ordinary
request benchmarks read those copies and inspect their referenced files.

To generate the full analyzed scale fixture:

```powershell
python scripts/prepare-sonic-scale-fixture.py --corpus 'D:\sonic-proof\snapshot\corpus.json' --output 'D:\sonic-proof\scale' --count 1100000
$env:MUSIC_SONIC_BENCH_DIR = 'D:\sonic-proof\scale'
cargo test --manifest-path src-tauri/Cargo.toml sonic_index_build_proof -- --ignored --nocapture --test-threads=1
cargo test --manifest-path src-tauri/Cargo.toml sonic_index_materialize_proof -- --ignored --nocapture --test-threads=1
# Repeat both apps' exact/index comparisons against this fixture.
cargo run --release --manifest-path crates/sonic-core/Cargo.toml --example index_benchmark -- D:\sonic-proof\snapshot\corpus.json D:\sonic-proof\vectors.json 1100000
```

The materialization helper requires the generator's synthetic marker, verifies
its fixture root, and refuses to overwrite existing files. Run it once per new
fixture. Raw request outputs contain local music metadata and paths; keep them
local. The committed summary contains timings, counts and equality checks only.
Run heavy builds sequentially and measure when compilation is idle.
