# Developer workflow

- Every project change must increment Aurora's SemVer version. Use a patch bump unless the scope warrants a minor or major release; keep `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, the Aurora entry in `src-tauri/Cargo.lock`, `src-tauri/tauri.conf.json`, and any user-facing version label aligned, then run `npm run check:version`.
- Always update `README.md` and `CHANGELOG.md` whenever changes are made.
- After changing or adding code, documentation, configuration, or any other project content, run `git add`, commit the changes, and push the commit to the configured remote. On `master`, wait for that CI run's `release-windows` job and verify the matching GitHub Release contains the NSIS `.exe`, updater bundle and signature, and `latest.json` before reporting completion.
- Prefer the Browser plugin when testing or visually inspecting the app whenever the behavior is reachable there. Use the Computer Use plugin only for native-only behavior that Browser cannot exercise.
