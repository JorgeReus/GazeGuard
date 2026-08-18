# Task 1 report: behavior settings and signal gating

## Delivered

- Added persisted `start_at_login`, `pause_during_fullscreen`, and `pause_when_idle` fields to `RawBreakEngineConfig` and public `BreakEngineConfig`.
- Defaults are `false`, `true`, and `false`, respectively. Serde defaults retain compatibility for existing configuration files.
- Gated desktop idle/fullscreen signals using current engine configuration.
- Added real regression coverage for defaults and disabled pause settings. Updated pre-existing positive signal test to enable idle pausing explicitly, since it is now opt-in.

## Test-first evidence

1. Added assertions for three new default fields and a signal-gating test.
2. Ran `cargo test --manifest-path src-tauri/Cargo.toml --lib behavior` before implementation.
3. It failed at compilation with five `E0609` errors: all three fields were missing from `BreakEngineConfig`.
4. Implemented minimal raw/public field mapping, defaults, and signal gates.

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml --lib behavior`: passed, 1 passed, 107 filtered out.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib`: passed, 108 passed.
- `git diff --check`: passed.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: fails on existing unrelated formatting across `break_engine.rs` and `lib.rs`; formatter was not run to avoid unrelated edits.

## Scope

Only `src-tauri/src/break_engine.rs`, `src-tauri/src/lib.rs`, and this report are included. Existing untracked `docs/` remains untouched.
