# Inbox contract

Aurora 0.23.9 provides Inbox as a device-local preparation area. Its files are ordinary MP3 folders outside Aurora's catalog until an explicit Music Library intake succeeds.

## Monitoring and identity

- Inbox stores at most ten canonical folder paths in `%APPDATA%\com.soundtrackgeek.aurora\aurora-inbox.json`. The setting is device-local and is not copied into Aurora's shared state or OneDrive snapshots.
- Aurora performs a bounded recursive scan on entry, every 15 seconds while visible, and whenever the window regains focus. Directory symlinks and reparse-style links are not followed; one directory containing MP3 files is one staged album.
- Scanning never opens the Music Library database for writes and never moves, renames, or catalogs a file. A SHA-256 of the canonical album path provides React's stable selection identity without exposing it as catalog identity.
- Aurora scans every track's embedded ID3 pictures and uses the first usable front cover in disc/track/filename order as the album display and repair source, falling back to another valid embedded picture only as a repair source. Bounded cached WebP thumbnails still pass through the contained local cover protocol.
- Readiness requires exactly one valid embedded front cover in every MP3 and identical image bytes across the album. It also reports missing or inconsistent Album Artist/Artist, Album, Track Title, track number/total, Genre, and Publisher. Disc numbering is optional, but a partially numbered multidisc album is reported as incomplete. Genre and Publisher are preparation requirements rather than Music Library identity fields, and can be overridden before promotion.

## Metadata providers

- MusicBrainz release search and lookup use the public JSON API, a meaningful Aurora/version User-Agent, a 20-second request timeout, and one process-wide request per second.
- Discogs search and release lookup accept either a personal access token or a consumer key plus matching secret. Production saves those credentials through the operating-system credential vault under Aurora's credential namespace. React receives only configuration status and the active authentication mode; it never receives a saved credential.
- Ignored `.env.local` values named `DISCOGS` and `DISCOGS_SECRET` (or `DISCOGS_TOKEN` for personal-token authentication) are read only by debug builds for local provider testing. They are never copied into an artifact, frontend environment, settings JSON, SQLite database, log, or error message.
- Search results identify concrete releases rather than release groups. Provider values are proposals: the user chooses a release, chooses fields, and may correct Genre or any track title before applying.

## Tag transaction

The Auto-Tagger submits one canonical album path, one bounded list of its visible MP3 paths, selected editable fields, and desired per-track values. Rust re-canonicalizes every path and rejects any track whose direct parent is not the selected album.

Before replacing originals, Aurora:

1. Reads every MP3 and merges only selected fields through the same MusicBee-compatible frame functions as the catalog tag editor.
2. Copies each file to a uniquely named same-folder working MP3 and writes the proposed ID3 values there.
3. Reopens the working copy, compares the complete parsed editable value set, and verifies the SHA-256 of all bytes after ID3v2 against the original.
4. Creates same-folder safety backups for every changed track.
5. Atomically replaces each original. If a later replacement fails, earlier installed files are restored in reverse order from their backups.
6. Removes working files and backups only after the complete album succeeds.

Inbox files are not cataloged, so this transaction does not queue an existing-folder Music Library sync. The files remain staged for further edits or promotion.

## Artwork transaction

- When some tracks already contain artwork, **Embed cover in all tracks** uses the displayed first valid embedded image. When the album has no usable embedded source, **Choose album cover** accepts a user-selected JPG, PNG, GIF, BMP, or WebP within the 32 MiB and 100-megapixel safety bounds.
- Aurora removes only front-cover frames, writes one normalized front-cover frame to every MP3, and preserves other pictures and all non-cover frames.
- Each changed MP3 is copied to a uniquely named same-folder working file. Aurora reopens it, verifies one exact front-cover image digest, compares every preserved non-front-cover frame, and verifies the SHA-256 of all bytes after ID3v2.
- The complete changed set receives same-folder safety backups and atomic replacement. A later install failure restores every earlier track in reverse order. Intake remains blocked until a fresh album scan confirms matching artwork on every track.

## Rename transaction

- Auto-Tagger offers a default-on **Rename after tagging** option. `Ctrl+R` runs the same operation from the selected Inbox album's existing tags without contacting a metadata provider.
- Inbox track checkboxes scope `Ctrl+Shift+T` to a subset. Release matching and track numbering use only that ordered selection, while Disc # and Disc total overrides allow separate CD1/CD2 releases to become one correctly numbered multidisc album. Partial selections do not auto-rename; after every disc is tagged, `Ctrl+R` renames the complete album together.
- The album directory is renamed in place, under its current parent, to `Album Artist - Album (Year)`. MP3 files become `Disc-01 - Artist - Title.mp3` when a disc number exists, or `01 - Artist - Title.mp3` when it does not. Track numbers use at least two digits.
- Discogs vinyl positions such as A1, A2, B1, and B2 are treated as one continuous sequence rather than separate discs. Explicit numeric multidisc positions such as `2-03` retain their disc number.
- Aurora replaces Windows-invalid filename characters, rejects missing required tags and existing destinations before mutation, stages every changed filename through unique temporary names, and reverses the batch if any file or folder rename fails.

## Promotion

Album-level **Move to library** and the monitored-folder/**All folders** **Add to Library** actions use the existing Music Library bridge without a second mover implementation. Folder intake assigns a General, Scores, or Synthwave destination independently to each non-empty monitored root, previews every root for review, and blocks the complete scope when any contained Inbox album is not ready. Because every successful Music Library apply changes the active catalog and invalidates later plans, Aurora obtains a fresh preview immediately before each root's apply and verifies that its albums, track counts, and destinations still match the reviewed preview.

```text
staged album folder
        ↓
General / Scores / Synthwave selection
        ↓
Music Library preview with exact destination and catalog delta
        ↓
explicit move-and-catalog apply
        ↓
Music Library archives embedded front art in AlbumCovers
        ↓
catalog revision refresh + Inbox rescan
```

The readiness gate prevents promotion while required identity, numbering, organization, or embedded-artwork issues remain. Music Library remains the only component that moves folders and writes the shared catalog.
