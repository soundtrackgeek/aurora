# Developer workflow

- Every project change must increment Aurora's SemVer version. Use a patch bump unless the scope warrants a minor or major release; keep `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, the Aurora entry in `src-tauri/Cargo.lock`, `src-tauri/tauri.conf.json`, and any user-facing version label aligned, then run `npm run check:version`.
- Always update `README.md` and `CHANGELOG.md` whenever changes are made.
- After running Rust or Tauri builds or tests, run `cargo clean` from `src-tauri` before finishing so build artifacts do not consume excessive disk space.
- Keep CI toolchains pinned to explicit versions. Keep the Rust version in `rust-toolchain.toml` and `.github/workflows/ci.yml` aligned, upgrade it deliberately in a focused change, and run the same lint and test commands used by CI before pushing. Do not use a floating `stable` toolchain for release workflows.
- After changing or adding code, documentation, configuration, or any other project content, run `git add`, commit the changes, and push the commit to the configured remote. Once `git push` succeeds, the work is complete; do not wait for or monitor CI or release publication.
- Prefer the Browser plugin when testing or visually inspecting the app whenever the behavior is reachable there. Use the Computer Use plugin only for native-only behavior that Browser cannot exercise.
