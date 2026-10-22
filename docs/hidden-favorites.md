# Hidden favorites

Available in Aurora 0.40.0 with Music Library 0.195.0 or newer.
Open **Ratings → Hidden favorites**, beside Tonight's Album, and choose **Find
hidden favorites**. Up to five tracks connect to several five-star/loved examples.
Rated favorites can return for a new listen; this is not an unrated-only queue.

**Play** starts the selected track using Aurora's regular playback queue.
The title and either named anchor open the exact track inspector. **Keep for
later** persists a shared shortlist; **Not today** hides a suggestion until the
Music Library PC's next calendar day without altering its rating. Use **Reload
shelf** to see changes from the other app. Save the visible kept tracks as a
regular Music Library playlist, preserving the reviewed order.

Music Library owns selection, its small local ratings model, shelf state,
feedback and playlist writes. Aurora calls the existing versioned bridge, gated
by `hiddenFavoritesV1`, and projects pending tag edits before showing tracks.
It blocks saving a kept track with a pending ban. Install the newer companion
build if the shelf reports an update requirement.

The model compares five-star with one-to-three-star analyzed examples in a
bounded sample; four-star tracks are not negatives. An album-artist holdout
gates its use and is shown under **About the taste evidence**. Otherwise the
shelf uses favorite sound neighborhoods. Fit labels are experimental ranking
bands, not predicted stars or probabilities. Music families are pooled in this
first version, so its usefulness for songs and scores can differ.

The fit slots cover several favorite neighborhoods; one slot samples a broader
pool. Artists, albums and repeated recordings are diversified. Recent matched
plays from the last 90 days, stale or unavailable audio, banned tracks and
low-rated tracks are excluded. Exact-audio copies of recent plays are excluded
too. History coverage is always stated: absence from available history does not
mean you have never heard a recording. This is conservative duplicate control,
not a complete acoustic twin detector.

The shelf remains stable until requested again, except newly ineligible tracks
drop out on reload. Building can take longer with a cold sonic index. There is
no new decoding pass, external model service, album recommendation mode or
Tonehavn-native shelf in this release. Repeated listening will be needed to
evaluate the recommendations themselves.

The authoritative [selection and persistence contract](https://github.com/soundtrackgeek/music_backup_v5/blob/master/docs/hidden-favorites.md)
lives with Music Library. Browser preview uses labeled sample data and separate
local storage, rather than the installed shared store.
