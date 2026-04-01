# Persist State Design

## Scope

Implement only Safe Eyes `persist_state` parity in this pass.

Out of scope for this design:
- `shortcut_disable_time`
- `shortcut_skip`
- `shortcut_postpone`

## Goal

When `persist_state: true`, GazeGuard should restore the break engine across app relaunches and continue from real elapsed wall-clock time rather than from a frozen in-process timer state.

When `persist_state: false`, GazeGuard should keep the current fresh-start behavior and ignore any previously saved engine snapshot.

## Product Decision

`persist_state` means true persisted resume, not session-only resume.

Elapsed real-world time while the app is closed must count toward the engine schedule. If enough time passes while the app is not running, reopening the app may immediately resume into warning, break, or a later post-break state.

This matches the product goal of protecting the user's eyes based on real elapsed time rather than only on active app uptime.

## Recommended Approach

Persist a Rust-owned engine snapshot to app-private internal storage as JSON.

This approach is preferred over a smaller heuristic restore record because the engine contains scheduling state that should not be recomputed loosely on restore:
- phase
- remaining timers
- current break metadata
- long-break cadence counters
- random-order template sequencing state
- idle/fullscreen state that affects automatic postponement
- consecutive skip state

This approach is also preferred over platform-specific persistence because Rust is already the scheduling authority and should remain the single owner of restore semantics.

## Persistence Medium

Store one snapshot file in the app's private data directory.

Storage requirements:
- desktop: use the app data directory
- Android: use the app's internal files directory
- do not use external storage
- do not use Android SharedPreferences for this engine snapshot

Format requirements:
- serialize as JSON using Rust `serde`
- keep the file Rust-owned and cross-platform
- overwrite atomically where practical

## Persisted State Model

Add a serializable persisted snapshot type in Rust that captures enough engine state to restore behavior exactly enough for future scheduling decisions.

Required fields:
- whether the timer had previously been started
- current phase
- `work_remaining`
- `warning_remaining`
- `break_remaining`
- `disabled_remaining`
- `shorts_since_long`
- `next_short_index`
- `next_long_index`
- `short_break_order`
- `long_break_order`
- `current_break`
- `idle_active`
- `idle_elapsed_seconds`
- `fullscreen`
- `consecutive_skips`
- `saved_at_unix_seconds`

The persisted snapshot should not duplicate static config values that already come from YAML. Restore should always load the current config first, then apply the saved runtime state on top of that config.

## Save Behavior

When `persist_state: true`, write the snapshot after any engine action that changes future scheduling behavior.

Required write triggers:
- `start`
- `stop`
- `disable_for`
- `skip_break`
- `complete_break`
- `postpone_break`
- `set_idle`
- `set_fullscreen`
- any clock-driven transition that changes phase or remaining timers in a way that affects future restore

Also perform one best-effort save during shutdown or app backgrounding when that lifecycle hook is available.

When `persist_state: false`, runtime code must not load from the snapshot and must not write new snapshots. Any existing snapshot file should be ignored on startup.

## Restore Behavior

Startup behavior:
1. Load the YAML config.
2. If `persist_state` is `false`, create a fresh engine exactly as today.
3. If `persist_state` is `true`, attempt to load the snapshot file.
4. If the snapshot loads successfully, restore engine runtime fields from it.
5. Advance the restored engine by elapsed wall-clock time since `saved_at_unix_seconds`.
6. Reconcile and continue with the resulting state.

Restore outcome requirements:
- a running timer may reopen already closer to warning or break
- a warning may become an active break
- an active break may already be finished
- a disabled period may already have expired
- state transitions after restore must match the same outcome the engine would have reached if it had been running continuously

If the snapshot file is missing, invalid, or incompatible:
- do not fail app startup
- log the error
- ignore or delete the bad snapshot
- fall back to a fresh engine

## Architecture Boundaries

Rust remains the only owner of:
- snapshot schema
- serialization and deserialization
- elapsed-time restore logic
- engine reconstruction

Tauri and platform-specific layers should only provide:
- the app data directory path
- lifecycle hooks for best-effort save points

Android and desktop UI code should not reimplement restore semantics.

## Persistence Boundary

Keep `BreakEngine` focused on pure engine state and transitions.

Place file I/O in a persistence layer near `src-tauri/src/lib.rs`.

The engine should expose snapshot export and import helpers so persistence code can save and restore state without pushing filesystem concerns into engine logic.

## Testing

Add Rust tests for the persistence behavior before implementation code.

Required coverage:
- snapshot round-trip preserves the engine state needed for future scheduling
- running timer restore advances by elapsed wall-clock time
- warning restore enters break when enough time elapsed
- active break restore completes when enough time elapsed
- disabled restore exits disabled when enough time elapsed
- `persist_state: false` ignores saved state and starts fresh
- corrupt snapshot falls back safely instead of crashing startup

Useful supporting tests if needed:
- random-order template sequencing survives restore
- long-break cadence survives restore
- consecutive skip count survives restore

## Expected Touchpoints

Primary files:
- `src-tauri/src/break_engine.rs`
- `src-tauri/src/lib.rs`

Potential supporting files:
- `src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/MainActivity.kt`
- `src/index.html`

## Non-Goals

This design does not include:
- implementing keyboard shortcuts
- revisiting desktop native idle/fullscreen fidelity
- redesigning the Android overlay UX
- changing the YAML config schema

## Success Criteria

The work is complete when:
- `persist_state: true` restores engine state across app relaunch
- restore applies real elapsed wall-clock time
- `persist_state: false` preserves current fresh-start startup behavior
- restore failures are safe and non-blocking
- automated Rust tests cover the critical restore cases
