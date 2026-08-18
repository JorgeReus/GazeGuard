# Task 2 report: Rust OS autostart synchronization

## Delivered

- Added official `tauri-plugin-autostart` v2 for macOS, Windows, and Linux targets, plus its default capability permission.
- Initialized autostart with `MacosLauncher::LaunchAgent` and used `ManagerExt` to enable or disable OS autostart.
- Added `sync_autostart(app, enabled)` and a small `AutostartAction` selector.
- `update_settings` validates YAML, selects `start_at_login`, synchronizes OS autostart, then atomically replaces runtime YAML. A synchronization error returns before any YAML write.

## Test-first evidence

1. Added `autostart_action_matches_start_at_login_setting` with required enable and disable assertions.
2. Ran `cargo test --manifest-path src-tauri/Cargo.toml --lib autostart_action` before implementation.
3. It failed as expected with `E0432`: `autostart_action` and `AutostartAction` did not exist.
4. Added minimal selector, plugin initialization, synchronization, and setting-write ordering.

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml --lib autostart_action`: passed, 1 passed, 108 filtered out.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib`: passed, 109 passed.
- `git diff --check`: passed.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: not clean because of existing formatting drift in `src-tauri/src/break_engine.rs` and pre-existing portions of `src-tauri/src/lib.rs`; formatter was not run, preserving Task 1 work and unrelated formatting.

## Scope

Task commit includes `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/capabilities/default.json`, `src-tauri/src/lib.rs`, and this report. Existing untracked `docs/` content remains excluded.
