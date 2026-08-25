# Inbox contract

Aurora 0.18.0 introduces Inbox as a device-local preparation area. Its files are ordinary MP3 folders outside Aurora's catalog until an explicit Music Library intake succeeds.

## Monitoring and identity

- Inbox stores at most ten canonical folder paths in `%APPDATA%\com.soundtrackgeek.aurora\aurora-inbox.json`. The setting is device-local and is not copied into Aurora's shared state or OneDrive snapshots.
- Aurora performs a bounded recursive scan on entry, every 15 seconds while visible, and whenever the window regains focus. Directory symlinks and reparse-style links are not followed; one directory containing MP3 files is one staged album.
- Scanning never opens the Music Library database for writes and never moves, renames, or catalogs a file. A SHA-256 of the canonical album path provides React's stable selection identity without exposing it as catalog identity.
- Readiness reports missing or inconsistent Album Artist/Artist, Album, Track Title, track number/total, disc number, Genre, and Publisher. Genre and Publisher are preparation requirements rather than Music Library identity fields, and can be overridden before promotion.

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

## Promotion

Move to library uses the existing Music Library bridge without a second mover implementation:

```text
staged album folder
        ↓
General / Scores / Synthwave selection
        ↓
Music Library preview with exact destination and catalog delta
        ↓
explicit move-and-catalog apply
        ↓
catalog revision refresh + Inbox rescan
```

The readiness gate prevents promotion while required identity or numbering issues remain. Music Library remains the only component that moves folders and writes the shared catalog.
