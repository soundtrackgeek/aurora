# Shared playlists and saved views

Aurora 0.33.0 and Music Library 0.185.0 use one playlist catalog. Aurora reads
`saved_playlists` and `playlist_automations` through its read-only catalog connection.
Creation, rule edits and explicit refresh go through protocol-1 bridge operations
`playlistSaveSelection`, `playlistSaveSmart` and `playlistRefresh`, advertised by
the `smartPlaylistAuthoring` capability. Older companions produce an update message
and continue to provide playlist reading.

## Create from Songs or Albums

Select songs or albums with the existing Ctrl/Shift selection, then choose
**Create playlist**. Song order follows the selected displayed rows. Each selected
album contributes its complete catalog song list in disc/track order, with duplicates
removed. The bridge accepts at most 100 albums and 1,000 songs, and rejects larger
selections rather than truncating them. It validates current song ID plus file path
and filename inside the save transaction. Aurora validates its durable track key first.
No audio files or tags are changed, and saving keeps the source view and selection.

**Create Smart playlist** saves reusable Music Library rules. Song rules match songs;
album rules match albums and include all their songs. The editor exposes general text,
artist/title/publisher comparisons, genre, rating bounds, original year, Love,
sort/direction, a 1–10,000 song limit and refresh policy.
Existing advanced Music Library filters are preserved. Supported explorer filters and
exact quoted text/year query clauses prefill the recipe. Aurora prefix, OR/NOT and
personal search syntax is not flattened into different companion semantics: the editor
explains the mismatch and starts a new shared recipe. **Save view** retains any Aurora
search expression exactly. Personal listening-history rules remain dependent on the
roadmap's shared personal-search work.

The album rating picker uses Aurora's half-star buckets; the shared recipe records
their equivalent 100-point bounds. Exact score-group genre names and unrated filters
also remain saved-view queries when Music Library would give them a different meaning.

Definitions remain ordinary Music Library `playlist_json` recipes. The optional
`smartSettings` object adds `trackLimit` and `refreshPolicy` (`library` or `manual`).
Legacy recipes without it retain their original refresh behavior. Empty Smart results
are valid and can later acquire songs. Automatic recipes refresh after imports/syncs
and when opening playlists in Music Library; manual recipes refresh only explicitly.
Aurora's **Refresh rules** updates the shared snapshot. Mixtapes and fixed track-ID
selections retain their exact order and cannot be replaced with Smart rules.
Playlist lists reload on focus and catalog imports. Editors retain the revision they
opened and reject saving after another app changes it, so a draft cannot overwrite
newer rules.

## Read and play

Aurora resolves saved file path and filename against current catalog IDs, retaining
duplicates and order and reporting missing entries. Each payload has at most 100 songs.
The cursor is the saved JSON position (or a deterministic seeded shuffle position),
and every continuation checks `updated_at`. A concurrent recipe edit/refresh stops
continuation with a refresh message. Playback refills with the next page when fewer
than 20 songs remain; the native queue still caps itself at 200. Shuffle covers the
whole saved playlist across pages.

Shared authoring/refresh requires a Windows Music Library bridge installation with
the new capability. A Mac/catalog snapshot can read and play the published snapshots;
it does not create an independent playlist copy. Both apps must be updated before
editing the new settings. Older Music Library releases may drop unknown JSON fields
when saving a recipe, so upgrade the companion first.

## Saved views

Saved views are Aurora navigation preferences, not playlists. **Save view** stores
the name, Songs/Albums/Artists destination, complete query/filter object and sort in
the schema-16 `saved_views` state table. Up to 100 views can be pinned. Updating an
existing view replaces it with the current explorer context and can rename it; deletion
removes the pin. All mutations increment the normal state-sync content revision.
Snapshot comparison includes the table, and normal state conflict handling applies.
Opening a saved view starts a fresh bounded explorer request within the navigation
transition, so an older retained destination cannot restore over its filters.

Browser preview keeps saved views in preview localStorage. Shared playlist writes
show the desktop requirement; they do not pretend to write the catalog.

## Regular playlist authoring (Aurora 0.34.0 / Music Library 0.186.1)

The protocol-1 `playlistAuthor` operation is advertised separately as
`playlistAuthoring`. Its actions are `create`, `rename`, `delete`, `append`, `move`
and `remove`. Existing-playlist writes require `expectedUpdatedAt`; Music Library
checks it and all selected song identities inside one transaction. Music Library's
own Update saved sends the revision captured when the draft opened, including after
focus refreshes. Reopen after a conflict. Rename and delete work for all playlist
types; manual song edits target regular playlists. Mixtape sides and Smart rules are
preserved. Raw JSON edits retain unknown fields and entries absent from the catalog.

Aurora's Playlists page creates empty regular playlists, renames/deletes playlists,
and moves/removes songs by their saved JSON positions. Unavailable songs retain their
positions; moving past one never drops it. List ordering remains most recently edited
first. Row Add to playlist and selection actions append complete albums in disc/track
order or songs in selected display order. Queues capture the currently loaded queue
(at most 200 songs), not future pages of a playing playlist, and preserve repeats.
Selections/appends accept up to 1,000 songs or 100 albums per request; regular playlists
support up to 10,000 stored entries. No audio files or tags change.

Import M3U8 uses a native file picker and accepts UTF-8 text up to 2 MiB / 1,000 entries.
Comments, BOMs, LF/CRLF, relative paths and local `file:` URLs are supported. Relative
paths resolve beside the M3U8. Only exact local catalog paths are matched (separator,
trailing-slash and ASCII-case normalization); missing/ambiguous paths or stream URLs
reject the entire import. Preview shows the count and first ten filenames; saving
rechecks the file digest and resolves current identities before a bridge transaction.
Import creates a regular snapshot rather than reconstructing Smart rules.

Export M3U8 takes a consistent saved JSON snapshot, retains all entries and repeated
paths (including catalog-unavailable songs), and writes UTF-8 with EXTINF metadata.
The native Save dialog selects a `.m3u8` file; publication uses a synced temporary file
and atomic replacement. The revision is checked again after the dialog. Paths use
the catalog's original filesystem location, so another device may need a path mapping.
Export supports up to 10,000 songs. Browser preview displays the desktop requirement
for writes/import/export; it never substitutes a private Aurora playlist store.
