# Sidebar navigation contract

Aurora 0.12.0 gives the left rail a stable information architecture without changing catalog or playback ownership.

## Hierarchy

- Universe, Observatory, and History are top-level destinations.
- Library owns Songs, Albums, Artists, Genres, Years, Ratings, and Tags.
- Playlists is a collapsible sibling of Library. Its three rows are bounded previews, not functioning playlist records.
- Opening a closed Library selects Songs. Closing it leaves the current destination unchanged so playback and exploration context are not discarded.

## Rail modes

- Expanded mode renders Library and Playlists as disclosure groups with labeled children.
- Icon-only mode renders one icon per group. Activating Library or Playlists opens a compact labeled flyout; child icons are not left as an ambiguous vertical stack.
- Collapsed mode removes the rail through the existing top-bar layout control.
- An active Library child gives both that child and the Library parent a visible state.

## Persistence and migration

Library and Playlists disclosure states live in the same device-local browser storage record as the left- and right-rail modes. They are presentation preferences only and are not synchronized through Aurora state or OneDrive.

The stored layout schema is version 2. A valid version 1 record retains its rail choices and receives expanded Library and Playlists defaults so upgrading does not hide existing navigation. Missing, malformed, or partially invalid records fall back to safe defaults.

## Years and playlists in 0.12.0

Years has a dedicated placeholder route. Activating it does not run the generic Explorer request or show song rows under a misleading title. Playlist buttons remain disabled previews until Aurora has a deliberate playlist data and identity contract.

## Accessibility

Destination buttons expose `aria-current="page"`; group triggers expose `aria-expanded` and `aria-controls`; icon-only triggers retain accessible names; Escape and an outside pointer dismiss an open flyout. Native buttons preserve keyboard focus behavior.
