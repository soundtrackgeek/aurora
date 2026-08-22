# Genre Atlas contract

Aurora 0.11.0 replaces the generic Genres route with a dedicated, bounded explorer for the catalog's canonical genres.

## Authority and identity

- `music-library.sqlite3` remains read-only and is authoritative for canonical genre membership, albums, artists, years, ratings, Love, durations, and track paths.
- `aurora-history*.sqlite3` remains authoritative for registered plays, listening time, and last-listened timestamps. Peer-device snapshots are merged with the same device de-duplication rules as Listening History.
- A genre is selected by the exact `canonical_genre` display value returned by the catalog. Requests reject blank values and values longer than 256 Unicode scalar values.
- Raw or compound genre tags are not renamed, merged, or written by Genre Atlas.

## Bounded data flow

- The genre index returns at most 1,000 rollups. Aurora currently has 687 canonical genres.
- A detail response returns at most 12 albums, 10 artists, 8 connected genres, 12 highlighted tracks, and one aggregate row per release decade.
- Connected genres mean shared album artists. They are a navigational relationship, not a claim that Aurora owns an authoritative genre taxonomy.
- Genre artwork is resolved only for returned representative albums; Aurora never eagerly opens every cover in the collection.

## Playback queues

- Genre actions request at most 100 tracks per batch and exclude keys already present in the current queue.
- Genre Radio favors rated and Loved material while retaining variety. Shuffle is unbiased within a bounded random album sample. Loved, Highest Rated, Rediscover, and Unrated Expedition apply their named filters.
- Rediscover prefers rated tracks that do not have a registered play in Aurora history. When history is sparse, this honestly behaves like a rated discovery queue.
- A live genre session asks for another batch when fewer than 20 tracks remain. Queue append keeps at most the previous 20 entries plus the current and future queue, and never exceeds 200 tracks.
- Starting an unrelated queue or clearing playback ends genre auto-refill. The source intent is device-local view state; the already-built playback queue remains durable through Aurora's existing playback persistence.
- An empty smart queue is a recoverable result. Aurora leaves the existing queue untouched and explains that no matching tracks were found.

## Loading and failure behavior

- Genre index and selected-detail requests have independent loading, empty, error, and retry states.
- Late detail or queue responses are ignored when the selected genre or requested queue action has changed.
- History-source errors from peer snapshots do not make catalog genre data unavailable. The affected personal metrics remain empty while catalog exploration continues.
- No Genre Atlas query runs on Aurora's startup critical path; the feature loads only while Genres is active.

## Acceptance checks

- Opening Genres no longer renders Deep Explorer and produces a genre rollup from the live read-only catalog.
- Search and all sort modes remain responsive with 687 genres and a 1.09-million-track source.
- Selecting a genre shows correct bounded albums, artists, decades, related genres, highlights, and personal history.
- Every playback action starts only tracks from the selected canonical genre, never exceeds the queue limits, and auto-refills without changing the current track.
- Library, Albums, Artists, Songs, Ratings, Tags, History, tag editing, playback, and updater behavior remain unchanged.
