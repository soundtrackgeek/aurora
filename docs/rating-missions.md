# What should I rate next?

Music Library **0.196.0** and Aurora **0.41.0** implement opportunity **02**
from the October sonic reports. Open **Discovery → What should I rate next?**
in Music Library or the same panel under **Ratings** in Aurora.

Choose your available time (10–240 minutes) and **Albums to finish** or
**A mix of unrated tracks**, then **Find my next mission**. The panel offers
up to three choices from different album artists:

- **A likely fit**, ordered by the existing held-out ratings prior and favorite
  sound neighborhoods, with a distance-only fallback when the model is unreliable.
- **Close to completion**, reserved for partially rated albums, fewest remaining
  tracks first. This slot is reserved before selecting the likely fit.
- **A deliberate stretch**, selected for acoustic distance from the favorite
  anchors and the other selected choices within the bounded candidate pool.

Album missions contain all currently unrated album tracks in disc/track order.
The budget is the duration of those remaining tracks, rather than the full album.
Mixed missions contain at most twelve complete unrated tracks, with different
album artists and albums, no repeated identical analyzed audio, and coverage of
different favorite anchors where available. They may draw individual tracks from
albums whose remaining duration exceeds the budget. A short pool can leave some
choices empty; the system never fills them with rated or banned music.

Review the exact track list and sound anchors before starting. **Start mission**
saves it in Music Library; Aurora's **Start & play** also starts its normal playback
queue. **Play remaining** reloads the shared mission before queueing unrated
tracks. Track and anchor links open the existing inspector and rating controls.
**Save mission as playlist** sends the exact reviewed remaining tracks, in order,
through Music Library's existing playlist writer. Existing playlist export and
Tonehavn publication can carry that fixed mission. No Tonehavn-native mission or
new rating-sync path is included.

## Progress and persistence

Only one mission is active across the two desktop apps. Navigation, restarts and
new suggestions retain it. **Reload progress** reads current ratings and changes
from the other app. Rate at your own pace using the existing rating workflow;
any saved nonzero rating counts, including a low rating. There is no target score,
automatic rating, or manual completion checkbox. **Leave mission** keeps all
ratings and records the mission as left; it does not count as completed.

Aurora projects its pending ratings and bans before display or playback. Shared
completion is recorded once those edits reach Music Library's catalog. Starting
or saving a reduced list can require waiting for catalog sync and reloading.
Mission completion records when the app observed all reviewed tracks rated,
not a reconstructed listening or rating timestamp. A completed mission remains
a historical event if a rating is later removed.

The panel reports completed versus started missions, distinct reviewed album
artist identities, and favorite anchor identities reached by completed missions.
Anchors are an exploration proxy, not measured sound-region coverage; they rotate
on explicit refresh. These counters measure workflow use, not listening quality.

`rating-missions.sqlite3`, beside Music Library's catalog, owns the saved choices,
active mission, exact audio bindings and completion history. Its versioned store
serializes mutations with an immediate SQLite transaction. Bridge protocol 1's
`ratingMissions` operation is gated by `supports.ratingMissionsV1`. Older Music
Library versions show an update instruction in Aurora. Mutations reject stale
revisions, changed remaining-track lists and reordered playlist requests.

Reload validates saved track IDs, paths, current ratings, bans, album membership
and analyzed audio hashes. Retagged MP3s with an old file signature are verified
by reading the exact audio payload with stable signatures before/after the read;
no new decoding or cache write is required. Replaced or missing recordings make
the mission unavailable instead of falsely completing it. Newly unrated album
members require a new album mission. New choices exclude the active mission's
albums. A completed mission can be cleared before starting another.

## Ranking boundaries

Reuse the current 23-value whole-track profile. The same `hidden-logistic-v1`
ranking implementation as Hidden favorites trains on a deterministic sample of
up to 20,000 rated/loved identities. Five-star tracks are positives, one-to-three
stars are negatives, and four-star tracks are excluded from training. Require
artist-disjoint validation with sufficient examples/artists and AUC at least
0.60. Show model version, held-out AUC and sample sizes. Scores pool all music
families and do not predict stars or probabilities. Family evaluation and the
full scheduled model in opportunity 24 remain future work.

Retrieval considers up to 512 rotating backlog albums, 128 close-to-completion
albums and 256 albums from indexed favorite neighbors. The existing exact index
retrieves up to 512 neighbors per anchor when ready; a cold index uses the bounded
catalog sample without waiting for a full-library scan. Album candidates have
at most 200 tracks, known positive durations for unrated MP3s, no bans, and all
remaining tracks analyzed. Mixed missions can use partial analysis from those
albums. Remaining-track means rank album choices; they do not describe an entire
varied album. Only final candidates receive file freshness checks. The mix fills
different favorite-anchor neighborhoods first, then different artists.

No audio, vectors or ratings leave the PC. Mission feedback never changes tags,
MP3 bytes, ratings or completed analysis. Recording-twin rating carry-over from
opportunity 28 is separate future work; identical recordings are only suppressed
within mixed missions here. Selection runs only on explicit request.

## Verification

Regression coverage includes budget enforcement, partial completion, disc order,
different artists and audio payloads, album versus mixed missions, persistence,
stale revisions, reviewed playlist order, actual-rating completion, newly unrated
album members, changed and retagged recordings, absent analysis, pending Aurora
ratings/bans and UI request/unmount behavior. Browser previews use labeled local
sample data; they neither read the installed library nor share state across origins.

The opt-in Rust test `live_readonly_missions` accepts `RATING_MISSIONS_LIVE_DIR`.
It reads the live catalog and analysis in a read-only snapshot, uses a temporary
mission store, and disables derived-index writes. Automated checks do not prove
recommendation enjoyment or mission completion over repeated listening sessions.

On 2026-10-22 the read-only live check returned **Total 13** (likely fit,
13 tracks, 39:19), **Merchants Of Doubt** (close to completion, one track,
2:49) and **Crawlspace** (stretch, two tracks, 2:19). All three fit the
40-minute request. The bounded fallback ran in 42.8 seconds in a debug build
and measured held-out AUC 0.748. It used a temporary mission store and left
the live catalog, analysis, index and music files unchanged. This is observed
execution evidence, not a release-build latency or listening-quality guarantee.
