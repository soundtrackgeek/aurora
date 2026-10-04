# Single-instance startup

Aurora registers `tauri-plugin-single-instance` as the first builder plugin. Duplicate launches exit before updater/dialog/window-state/shortcut initialization and before application setup opens Aurora databases, creates playback, or starts native background schedules.

The guard uses the application identifier `com.soundtrackgeek.aurora`. The plugin's optional `semver` feature stays disabled, so different Aurora versions with that identifier share the guard. An older release without the plugin cannot participate; close it before starting the updated release.

On a duplicate launch, the primary process schedules activation on the UI thread, shows `main`, unminimizes it, and requests focus. Each operation is attempted independently. Aurora does not unmaximize, resize, navigate, or restart playback. The callback emits `app://second-instance` with `{ args: string[], cwd: string }`, including the executable argument provided by the plugin. Native or frontend listeners can use the launching directory to resolve relative paths. There are currently no file/deep-link handlers, and this live event is not a durable launch queue.

No new frontend plugin, capability, command, or file association is required. Browser preview cannot exercise an OS instance guard.

## Native verification

Use a built Aurora executable on Windows or macOS:

1. Launch once and wait for the main window. Start playback or note the current destination and selection.
2. Launch the same executable again. Confirm the second process exits and the original process remains; its window receives focus and its playback/navigation state stays intact.
3. Minimize the original window, launch again, and confirm it is restored. Repeat with a maximized window and verify it stays maximized.
4. With a native listener for `app://second-instance`, launch with a path containing spaces and Unicode characters from a different working directory. Confirm the listener receives the arguments and that directory.
5. Close the original process normally and launch again. Confirm a new primary instance can start. Repeat after forcibly terminating a test primary to check lock recovery.

Use an isolated application identifier and a minimal Tauri harness when automating these checks, so tests do not open the real catalog, alter Aurora state, register global shortcuts, or publish sync snapshots.

The reusable harness is `src-tauri/examples/single_instance_probe.rs`. Build it with `cargo build --manifest-path src-tauri/Cargo.toml --example single_instance_probe`. Set `AURORA_INSTANCE_PROBE_ID` to a fresh identifier suffix and `AURORA_INSTANCE_PROBE_OUTPUT` to a temporary JSON-lines file before launching the executable under `src-tauri/target/debug/examples/`. Set `AURORA_INSTANCE_PROBE_MODE` to `hidden`, `minimized`, or `maximized` (the latter starts maximized and then minimized). The harness records application setup, readiness, forwarded payloads, and window state after activation. A duplicate must add a launch record without a second setup record; `--probe-exit` requests normal shutdown of the primary. Use the same ID for restart/recovery checks, then run `cargo clean` from `src-tauri`.

On Windows, run `pwsh -File scripts/test-single-instance.ps1` after building the harness. It automates hidden/minimized/maximized activation, spaces/Unicode argument and working-directory forwarding, exclusion from application setup (including another launch during startup), normal restart, and recovery after terminating an isolated primary. Evidence remains in the printed temporary directory. macOS activation and lock recovery require a native macOS run.
