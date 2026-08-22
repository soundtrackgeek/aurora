# Global shortcuts contract

Aurora 0.9.0 registers configurable Windows-wide controls while the native process is running. The implementation deliberately keeps shortcut registration, playback targeting, and MP3 mutation in Rust so a hidden or unfocused WebView cannot retarget an action.

## Defaults and meaning

| Action | Default | Behavior |
| --- | --- | --- |
| Play or pause | `Ctrl+Alt+P` | Toggles Aurora's playback runtime. |
| Next track | `Ctrl+Alt+N` | Advances Aurora's current queue. |
| Clear rating | `Ctrl+Alt+0` | Writes an unrated MusicBee POPM value. |
| Rate 1–5 stars | `Ctrl+Alt+1` … `Ctrl+Alt+5` | Writes the corresponding whole-star MusicBee POPM value. |
| Toggle Love | `Ctrl+Alt+L` | Changes Loved to Neutral, or Neutral/Banned to Loved. |

Only the track in `PlaybackRuntime.current_track` can receive rating or Love. The selected Explore row and inspector state are presentation-only and never cross the native shortcut command boundary.

## Persistence and registration

- Settings are device-local at `%APPDATA%\com.soundtrackgeek.aurora\aurora-shortcuts.json` and use a versioned JSON envelope.
- The file stores one binding for every supported action plus the global enabled flag. It is not part of `aurora-state.sqlite3`, Laptop Mode, or OneDrive snapshots.
- A binding needs at least one modifier and exactly one non-modifier key. Action names, count, syntax, and accelerator uniqueness are validated natively.
- Aurora unregisters and registers a requested set as one transaction. If any binding is unavailable, it releases the partial request and restores the previous registered set.
- The settings file is replaced atomically only after Windows accepts the complete set. A persistence failure also rolls registration back.
- Missing settings use enabled defaults. Unsupported or malformed settings use enabled defaults and surface a warning rather than blocking launch.

Windows permits only one process to own a global binding. A conflict commonly means MusicBee or another player already registered the same keys; Aurora reports the unavailable accelerator and leaves the previous working configuration intact.

## Tag workflow

The rating and Love handler snapshots the native now-playing track, derives the expected and desired tag values, and calls the same `TagService` transaction used by visible controls. That transaction:

1. Writes and verifies the MusicBee MP3 frames.
2. Records the operation journal and optimistic overlay in `aurora-state.sqlite3`.
3. Refreshes playback metadata and emits a result event for visible Aurora surfaces.

The shared Music Library catalog remains read-only. A later MusicBee TSV export and Music Library import updates that catalog; Aurora reconciliation then clears an overlay that has caught up. External MusicBee edits can still become visible through the existing bounded reconciliation path.

## Failure behavior

- Rating or Love without a current playback track fails without selecting or mutating another track.
- Registration conflicts do not produce a partially active set.
- Tag conflicts, missing files, unsupported files, and verification failures follow the existing tag-editing recovery contract and are surfaced through Aurora's status message.
- The browser preview can exercise the Settings workflow but explicitly reports that Windows registration is available only in the native app.

## Verification

- Rust tests cover defaults, invalid/duplicate bindings, rating clearing and mapping, Love toggling, tag-field preservation, and device-local persistence.
- React tests cover capture, complete-set save, duplicate rejection, default restoration, and the now-playing-only explanation.
- Native smoke testing confirms the full default set registers and `Ctrl+Alt+P` reaches Aurora while another Windows application has focus; playback is restored afterward and no real MP3 rating/Love value is changed during the smoke test.
