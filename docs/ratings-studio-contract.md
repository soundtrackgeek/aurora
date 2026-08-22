# Ratings Studio contract (0.14.0)

Aurora 0.14.0 gives Ratings its own bounded, read-only catalog surface while preserving Aurora's verified instant MP3 tag pipeline.

## Authority and identity

- `music-library.sqlite3` remains authoritative for imported track ratings, Love, album membership, explicit MusicBee Album Rating, album metadata, and the last imported Album Score.
- `aurora-state.sqlite3` overlays newer verified MP3 rating/Love edits until the next Music Library import catches up. Ratings queries apply those overlays without writing to the shared catalog.
- Track identity remains Aurora's normalized full file path. Album identity remains `albums.id`.

## Rating definitions

- Track stars use the effective MP3/catalog rating on the 0–5 scale, including half stars.
- Effective album rating follows Music Library exactly: use explicit MusicBee Album Rating when present; otherwise, only when every track is rated, use the rounded mean of normalized 0–100 track ratings. Divide by 20 for display stars.
- A partially rated album may expose a clearly labelled provisional mean, but it is not counted in the album-rating constellation.
- Album Score follows Music Library exactly:

  `((effective album rating × 0.5) + (5-star-time ratio × 100) + (5-star minutes × 0.3)) ÷ 10 + (loved tracks × 100)`

- Album Score is unbounded and is shown as a numeric rank measure, never converted into stars.

## Completion states

- Almost complete: at least one track is rated and only 1–3 tracks remain unrated.
- Partially rated: at least one track is rated and more than 3 tracks remain unrated.
- Unrated album: no tracks are rated.
- Fully rated albums are omitted from the completion workbench and may show their current Album Score in album detail.

## Bounded behavior

- Aggregate counts come from grouped catalog queries plus the small Aurora overlay delta.
- Each constellation band carries only a small representative cover sample.
- Completion tabs return at most 14 albums.
- Selected album detail uses the existing bounded album-detail command.
- Rating and album queues return at most 100 tracks; the player still enforces its existing 200-item ceiling.

## Interaction

- Track and album-rating constellation bands are clickable.
- Five-star track selection exposes the 5 Star Collection.
- Completion tabs switch between Almost Complete, Partially Rated, and Unrated Albums.
- Selecting an album opens its bounded track list. Stars and Love save instantly through the existing verified MP3 transaction and Aurora overlay.
- `Play unrated tracks` queues only unrated tracks from the selected album.
- A successful tag edit refreshes Ratings so completion, spectra, provisional averages, and Album Score reconcile with the newest overlay.

