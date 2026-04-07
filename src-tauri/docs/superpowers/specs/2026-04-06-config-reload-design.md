# Config Reload Design

## Goal

Add live runtime config reload on desktop when `config.yaml` changes, while keeping Android on an explicit reload path driven by opening settings rather than a background watcher.

## Scope

This design covers:

- desktop-only watching of the runtime `config.yaml`
- immediate application of valid desktop config edits to the running engine
- fallback to the last known-good config when a desktop edit is invalid
- surfacing desktop reload errors to the UI through an error banner
- explicit Android reload when settings is opened

This design does not cover:

- editing config files from inside the app
- background file watching on Android
- config schema changes

## Current State

- runtime config files are now seeded and loaded from real platform paths
- desktop loads config during startup in Rust
- Android reads its file-backed config when `BreakEngineConfig.loadSchedule(context)` is called
- there is no runtime config watcher
- invalid config edits after startup are not surfaced in the UI

## Requirements

- Desktop must watch its resolved runtime `config.yaml`.
- When the desktop file changes and parses successfully, the running engine must reload immediately.
- When the desktop file changes and parsing fails, the app must:
  - keep using the last persisted valid config
  - show an error banner in the UI
- A later valid desktop edit must clear the error banner automatically.
- Android must not run a background watcher.
- On Android, opening settings must trigger a fresh read from `filesDir/config/config.yaml`.

## Approaches Considered

### 1. Rust-owned desktop watcher with Android explicit reload

Pros:

- keeps backend state ownership in Rust
- works even when the frontend is not the source of truth
- fits the current engine architecture
- keeps Android simpler and battery-safe

Cons:

- requires file watch plumbing and UI event delivery
- needs careful handling for partial writes and invalid edits

Chosen.

### 2. Frontend-driven polling on desktop

Pros:

- simpler to prototype
- watcher logic can be implemented in JavaScript

Cons:

- stops being reliable when the relevant UI is closed
- makes the frontend the reload authority instead of Rust
- duplicates backend state concerns in the UI

Rejected.

### 3. Desktop watcher with deferred apply

Pros:

- safer if schedule changes are considered disruptive
- easier to explain in the UI

Cons:

- does not satisfy the requirement for immediate reload

Rejected.

## Proposed Design

### Desktop watcher ownership

Desktop config watching should live in Rust near app startup, not in the frontend. The watcher should resolve the runtime config path once, subscribe to changes for that file or its parent directory, and run for the lifetime of the desktop app.

The watcher should debounce filesystem events briefly before reading the file. This avoids reloading on partial writes from editors that save through temporary files or multiple update events.

### Reload flow on desktop

On a watched desktop config change:

1. Read `config.yaml`.
2. Parse it with the existing `BreakEngineConfig` parser.
3. If parsing succeeds:
   - update the running engine config immediately
   - store the new config as the latest known-good runtime config
   - emit a success event so the frontend can refresh schedule data and clear any error banner
4. If parsing fails:
   - keep the engine on the previous known-good config
   - emit an error event with a user-safe message
   - do not overwrite the cached known-good config

### Runtime state boundary

The config file remains the persisted source of truth on disk. The app must also keep the most recent valid in-memory config so an invalid edit does not destabilize the running engine.

This means runtime config state has two layers:

- persisted file contents on disk
- last known-good parsed config in memory

If the on-disk file is invalid, the running app continues on the last good in-memory config until the file is corrected.

### Engine update semantics

Applying a new valid config should update the live engine immediately. The engine should preserve dynamic runtime state that is unrelated to the config file where possible, such as:

- whether the timer is currently running
- current phase
- current elapsed countdown state
- current idle/fullscreen state

The reload path should change configuration-driven values such as:

- break intervals and durations
- warning timing
- postpone options
- disable options
- break templates
- idle threshold

The implementation should make this boundary explicit instead of reconstructing the entire application state opportunistically.

### UI events and banner behavior

Rust should emit desktop config reload events to the frontend, with separate success and failure cases.

The frontend should:

- show an error banner when it receives a failure event
- keep the banner visible until a later success event arrives
- refresh visible schedule/config-derived data after a success event

The failure event payload should be user-safe and concise. It should explain that the config file could not be reloaded and that the app is still using the last valid config.

### Android behavior

Android should not watch `filesDir/config/config.yaml` in the background.

Instead, opening settings should trigger a reload from the current file on disk. This gives Android an explicit refresh point without adding always-on file observation.

If Android later gains in-app config editing, saving through settings can reuse the same reload path after the write completes.

## Error Handling

- Missing desktop config files should continue to be handled by the existing bootstrap path.
- Invalid desktop YAML should not stop the engine.
- Watcher-level failures should be logged and surfaced to the UI if they affect reload behavior.
- UI event emission failures should not crash the app.
- Android reload failures when opening settings should surface through the existing settings error handling path rather than silently falling back.

## Testing

Add tests for:

- desktop watcher reloads valid config changes into the engine
- invalid desktop edits keep the previous config active
- desktop reload failure emits an error event payload
- later valid edits clear the failure state
- Android settings-triggered reload reads the latest file contents

Manual validation:

- desktop: edit `config.yaml` while the app is running and confirm the schedule updates without restart
- desktop: save invalid YAML and confirm an error banner appears while the engine keeps the previous valid config
- desktop: fix the invalid YAML and confirm the banner clears and the new config applies
- Android: modify `filesDir/config/config.yaml`, open settings, and confirm the latest values are shown

## Implementation Notes

- Keep path/bootstrap logic in `config_file.rs`.
- Keep YAML parsing in `break_engine.rs`.
- Put watcher orchestration in Rust app runtime code, preferably a small dedicated module if `lib.rs` starts to bloat further.
- Debounce editor save bursts rather than responding to every raw filesystem event.
- Prefer explicit frontend events for reload state instead of making the UI poll for watcher status.

## Success Criteria

- Desktop runtime config changes apply without restarting the app.
- Invalid desktop config edits do not replace the active valid config.
- The desktop UI shows an error banner when reload fails and clears it after the next successful reload.
- Android still avoids a background watcher.
- Opening Android settings refreshes config from the current on-disk file.
