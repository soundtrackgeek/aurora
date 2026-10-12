# Global shortcuts contract

Aurora 0.9.0 registers configurable Windows-wide controls while the native process is running. The implementation deliberately keeps shortcut registration, playback targeting, and MP3 mutation in Rust so a hidden or unfocused WebView cannot retarget an action.

Aurora 0.17.7 separately registers a Windows System Media Transport Controls session for physical Play/Pause, Stop, Previous, and Next keyboard buttons. Windows arbitrates those media-session commands between players and Aurora publishes its playback status plus current title, artist, and album. These buttons are not configurable global hotkeys and have no relationship to WASAPI shared or exclusive output mode.

Aurora 0.26.17 joins macOS Now Playing when there is a current track. It publishes title, artist, album, duration, playback state, and progress; Play/Pause, Next, Previous, Stop, and position commands run against the native playback runtime. It detaches when no track is current and on exit. macOS chooses the active Now Playing app, so another player can take precedence if it is also active.

## Defaults and meaning

| Action | Windows default | macOS default | Behavior |
| --- | --- | --- | --- |
| Play or pause | `Ctrl+Alt+P` | `Command+Option+P` | Toggles Aurora's playback runtime. |
| Next track | `Ctrl+Alt+N` | `Command+Option+N` | Advances Aurora's current queue. |
| Clear rating | `Ctrl+Alt+Numpad0` | `Command+Option+Numpad0` | Writes an unrated MusicBee POPM value. |
| Rate 1–5 stars | `Ctrl+Alt+Numpad1` … `Ctrl+Alt+Numpad5` | `Command+Option+Numpad1` … `Command+Option+Numpad5` | Writes the corresponding whole-star MusicBee POPM value. |
| Toggle Love | `Ctrl+Alt+L` | `Command+Option+L` | Changes Loved to Neutral, or Neutral/Banned to Loved. |

Only the track in `PlaybackRuntime.current_track` can receive rating or Love. The selected Explore row and inspector state are presentation-only and never cross the native shortcut command boundary.

## Persistence and registration

- Settings are device-local at `%APPDATA%\com.soundtrackgeek.aurora\aurora-shortcuts.json` and use a versioned JSON envelope.
- On macOS the same file lives in Aurora's app data directory under `~/Library/Application Support`.
- The file stores one binding for every supported action plus the global enabled flag. It is not part of `aurora-state.sqlite3`, Laptop Mode, or OneDrive snapshots.
- A binding needs at least one modifier and exactly one non-modifier key. Action names, count, syntax, and accelerator uniqueness are validated natively.
- Aurora unregisters and registers a requested set as one transaction. If any binding is unavailable, it releases the partial request and restores the previous registered set.
- The settings file is replaced atomically only after the OS accepts the complete set. A persistence failure also rolls registration back.
- Missing settings use enabled defaults. Unsupported or malformed settings use enabled defaults and surface a warning rather than blocking launch.
- Version 1 settings migrate only legacy `Ctrl+Alt+0` through `Ctrl+Alt+5` rating defaults to their numeric-keypad equivalents. Custom bindings are preserved.
- Version 2 settings on macOS migrate only unchanged Windows defaults to Command–Option defaults. Custom bindings are preserved; Windows keeps its existing defaults. Version 3 stores the result.
- Aurora explicitly unregisters its complete set when the main window closes and again on application exit, making shutdown cleanup idempotent across normal and programmatic exit paths.

Windows permits only one process to own a global binding. A conflict commonly means MusicBee or another player already registered the same keys; Aurora reports the unavailable accelerator and leaves the previous working configuration intact.

Windows exposes AltGr combinations as `Ctrl+Alt`. Numeric-keypad rating defaults avoid consuming number-row AltGr characters such as Norwegian `AltGr+2` (`@`). Releasing a shortcut does not make another running application retry an earlier failed registration; if MusicBee started while Aurora owned a shared shortcut, MusicBee must refresh its shortcut configuration or restart once Aurora exits.

## Tag workflow

On Windows, the rating and Love handler captures the stable identity of the native now-playing track at keypress and queues the action on one ordered worker. It emits **Saving tags** without changing the stars or playback tags. The worker reads the current MP3 values before each action, so rapid Love presses toggle from the preceding confirmed save. It calls the same `TagService` transaction used by visible controls. That transaction:

1. Writes and verifies the MusicBee MP3 frames.
2. Records the operation journal and verified file-tag overlay in `aurora-state.sqlite3`.
3. Refreshes native playback metadata and emits the complete verified track plus its shared catalog projection token for visible Aurora surfaces. A late older receipt cannot replace a newer rating; moving to another track or album does not retarget the save or reopen details.
4. Leaves the durable exact-file/folder synchronization journal for the focused background Music Library retry, so a slow companion process cannot delay the shortcut result.

The shared Music Library catalog remains read-only. A later tagged-album folder sync, or a legacy MusicBee TSV import, updates that catalog; Aurora reconciliation then clears an overlay that has caught up. External file-tag edits can still become visible through the existing bounded reconciliation path.

On macOS in Network Mode, rating and Love shortcuts capture the now-playing track identity at keypress and enter the same ordered background queue. Each task resolves the latest local catalog and overlay values, then uses the same `TagService::update` route as inline edits. Tonehavn checks the expected values and verifies the PC MP3 and catalog edit. Aurora refreshes playback and visible track metadata only after the confirmed result; a rejected edit leaves the displayed values alone. No direct MP3 write or local Music Library import runs on the Mac. Rapid Love presses derive each toggle from the preceding confirmed result.

## Failure behavior

- Rating or Love without a current playback track fails without selecting or mutating another track.
- Registration conflicts do not produce a partially active set.
- Temporary read/lookup failures (SQLite locks or Windows sharing violations) and specifically known pre-replacement sharing failures retry up to three attempts, with 250 ms and 500 ms waits. Each retry reads the same captured track again. Remote writes, external-tag conflicts, verification failures, and post-replacement journal or snapshot errors are never replayed automatically.
- Failed rating/Love saves display a persistent, dismissible alert independent of routine reconciliation status. A later successful save clears only that track's alert; another track's save or catalog refresh cannot hide it. No failed shortcut projects unverified tag values.
- Worker failures also append the stable track key, action, full error, UTC timestamp, and version to device-local `aurora-shortcut-errors.jsonl`. This separate log and its `aurora-shortcut-errors.previous.jsonl` rotation are bounded to approximately 1 MiB each and stay outside OneDrive state snapshots.
- The browser preview can exercise the Settings workflow but explicitly reports that global registration is available only in the native app.

## Verification

- Rust tests cover defaults, invalid/duplicate bindings, rating clearing and mapping, Love toggling, remote shortcut request sequencing and tag-field preservation, and device-local persistence.
- React tests cover capture, complete-set save, duplicate rejection, default restoration, the now-playing-only explanation, confirmed-only track projection, stale receipt rejection, persistent failure alerts across focus reconciliation, and dismissal.
- Native smoke testing confirms the full default set registers and `Ctrl+Alt+P` reaches Aurora while another Windows application has focus; playback is restored afterward and no real MP3 rating/Love value is changed during the smoke test.

For an optional live source check, set `AURORA_TAG_SMOKE_TRACK_ID` and
`AURORA_TAG_SMOKE_TRACK_KEY`, then run
`cargo test --manifest-path src-tauri/Cargo.toml live_shortcut_tag_copy_smoke -- --ignored --nocapture --test-threads=1`.
It reads the selected catalog/MP3 with a temporary Aurora state database, writes
and verifies four stars on a temporary copy, and verifies that the original
fingerprint and tag values remain unchanged. It does not simulate global key
registration or repair an earlier unsuccessful rating. Run `cargo clean` in
`src-tauri` after Rust checks.
