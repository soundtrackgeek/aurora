# Context menus

Aurora 0.35.0 uses one accessible WebView menu on library song rows and album
cards, artists and artist links, artist-page songs/albums, charts, listening
history, playlist songs, queue rows, genre highlights/releases, Ratings and
Tonight's Album, plus sonic matches. Existing click/double-click behavior remains
available. Right-click does not change selection or start playback. On a selected
Songs/Albums row, menu actions capture all selected displayed rows in their order;
on an unselected row, they act on that row.

Open with right-click, Shift+F10 or the Menu key. Arrow Up/Down moves through
enabled items; Home/End moves to either end, and typing selects matching labels.
Arrow Right opens Playlist or Rate; Arrow Left returns. Escape/Tab closes and
restores focus. Clicking elsewhere, scrolling the underlying page, resizing or
losing window focus closes the menu. Menus stay within the viewport and scroll
when necessary.

## Playback and queue

Play starts the captured songs in order. Play next inserts them immediately
after the current song; Add to queue appends. Both keep the current song,
position and playing/paused state. An empty queue becomes a stopped queue ready
to play. Deliberate repeats remain distinct queue occurrences.

Native enqueue resolves all durable song ID/key pairs before mutating the queue
under the playback lock. Additions and total queue size are bounded to 200 songs;
old played history beyond the 20 retained songs can be reclaimed, but upcoming
songs are never truncated. Too-large additions fail without a partial queue edit.
Album queue actions resolve the complete bounded album detail; truncated albums
produce an error instead of quietly queueing part of an album. Artist actions use
the existing first-50-song catalog query, ordered by rating and title, and the
menu shows this limit.

Play next temporarily prioritizes the inserted songs over shuffle and Repeat One
without changing those settings. Priority and queue order persist in Aurora's
device-local playback state (schema 17). Another Play next action inserts before
the pending priority songs. Manual move/removal or a catalog rebind that changes
the queue clears priority; the visible queue order then governs normal playback.
Prepared audio is invalidated at the retained current position before insertion.
Automatic long-playlist/radio refills keep their existing deduplicating command.

## Playlists

Add to playlist shows up to three available editable regular playlists. Successful
create, import, rename, append, remove and reorder work records the playlist ID
and timestamp in device-local browser storage. The latest shared `updatedAt`
also contributes to ordering, so Music Library edits appear without Aurora-side
history. Deleted playlists are filtered out; Smart playlists and mixtapes are
excluded. Merely playing or opening a playlist does not count as editing work.

Choose playlist opens the full picker with an existing playlist selected when available; Create playlist opens it with
New playlist selected. Album selections pass album IDs to Music Library so it can
expand complete albums in disc/track order. Song selections retain repeats. Each
shortcut sends the displayed playlist revision to `playlistAuthor`; stale writes
fail visibly and require refresh, with no automatic retry or partial append.
Music Library remains the sole catalog writer. Aurora refreshes the playlist
list after success without navigating away from the source view.

## Files, navigation, copy and tags

Show in Windows Explorer is available for one song or album. The native command
accepts catalog identity, never an arbitrary frontend path. It resolves and checks
the existing local file/folder and normalizes Windows canonical path prefixes
for Explorer. Song files use the Windows Shell selection API;
albums open their song directory in Explorer. Finder selects a file on macOS;
Linux opens the containing folder. Unavailable paths produce an error.

Go to album/artist reuses normal navigation. Copy uses Display Artist for songs
and Album Artist for albums; multiple selected songs become one line per song.
Unmatched charts/history still support copy and artist navigation. Play, queue,
playlist, file and tag actions require a resolved local item.

Open Tags opens the existing verified tag editor for the captured songs/albums,
independent of the current inspector selection. Rate offers clear and 0.5–5 stars
for a single song through the existing verified/Network Mode affinity route.
Artist-only selections expose navigation and bounded playback/playlist actions.

Browser preview exercises menu and queue behavior; Explorer and shared playlist
writes require desktop. Windows native launch and an installed Music Library
round trip need separate desktop validation; browser preview is not that proof.
