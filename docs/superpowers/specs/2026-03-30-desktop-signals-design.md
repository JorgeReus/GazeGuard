# Desktop Signals Design

## Scope

Improve desktop activity detection by introducing a cross-platform `desktop_signals` boundary and implementing a native `macOS` provider in this pass.

Out of scope for this design:
- Linux native desktop signal implementation
- Windows native desktop signal implementation
- break UI polish
- Android signal changes
- break engine policy changes beyond replacing the source of desktop idle/fullscreen inputs

## Goal

Replace the current desktop heuristic logic in `src-tauri/src/lib.rs` with a dedicated `desktop_signals` module that exposes one stable API for desktop activity signals.

In this pass:
- `macOS` should use native OS signals for user idle detection
- Linux and Windows should continue using the current heuristic fallback
- the break engine should keep consuming only normalized `idle_active` and `fullscreen_active` values

## Product Decision

Desktop signal fidelity should improve incrementally behind a stable abstraction rather than by adding platform-specific logic directly in `lib.rs`.

This keeps the current branch moving on macOS without blocking on Linux and Windows research, and it gives later sessions a clean seam for filling in native support on the remaining desktop targets.

## Recommended Approach

Create a new `desktop_signals` Rust module under `src-tauri/src` that owns desktop signal collection.

That module should expose:
- a normalized signal snapshot type containing `idle_active` and `fullscreen_active`
- one function that gathers the current desktop signals for the running platform

Implementation split:
- `macOS`: native provider for idle detection, with fullscreen detection routed through the same abstraction but still using the existing window fullscreen heuristic in this pass
- Linux and Windows: fallback provider that preserves the current Tauri window heuristic behavior

`src-tauri/src/lib.rs` should stop inspecting window state directly and should instead ask `desktop_signals` for the latest snapshot before updating the break engine.

## API Shape

The abstraction should stay small and match the engine's current needs.

Recommended shape:
- `DesktopSignals { idle_active: bool, fullscreen_active: bool }`
- `collect_desktop_signals(app: &tauri::AppHandle, idle_threshold_seconds: u64) -> DesktopSignals`

The API should not expose platform-specific details, native framework types, or partial state. If a provider cannot determine a signal reliably, it should fall back internally and still return a complete `DesktopSignals` value.

## macOS Provider

The `macOS` implementation should use native OS facilities to determine user idle state based on actual last input time rather than Tauri window focus, visibility, or minimize status.

Behavior requirements:
- idle should reflect real user inactivity at the OS level
- the configured idle threshold should remain engine-owned rather than being reimplemented as a second full threshold inside the provider
- a small provider-side debounce floor is acceptable if needed to avoid poll-loop flapping during active use
- provider code should remain thin and isolated from engine logic

For fullscreen detection in this pass:
- keep the current window fullscreen heuristic inside the provider
- do not add a separate native fullscreen implementation yet

The important requirement is that `lib.rs` no longer owns the detection strategy.

## Fallback Provider

Linux and Windows should keep the current heuristic behavior for now:
- fullscreen from window fullscreen state
- idle inferred from focus/minimize/visibility heuristics

This fallback logic should move into the new module so the behavior is preserved while the call site becomes platform-agnostic.

## Architecture Boundaries

`desktop_signals` owns:
- desktop signal collection
- platform selection via `#[cfg(...)]`
- fallback heuristics for unsupported desktop targets
- any native `macOS` wrappers required to gather idle/fullscreen state

`lib.rs` owns:
- calling the abstraction during desktop sync
- applying the returned booleans to the break engine
- snapshot persistence and command handling as it does today

`break_engine.rs` remains unchanged in scope:
- it receives normalized idle/fullscreen state
- it should not learn about OS-specific APIs

## Testing

Automated coverage should focus on stable Rust logic rather than trying to deeply unit test native `macOS` frameworks.

Required coverage:
- fallback provider preserves the existing heuristic output for representative window states
- `sync_desktop_window_state` integration still applies the returned `idle_active` and `fullscreen_active` values to the engine

Manual verification is required on `macOS` for:
- active usage keeps the engine out of idle mode
- real OS inactivity enters idle mode even if the app window remains visible
- fullscreen behavior does not regress compared with current behavior

## Expected Touchpoints

Primary files:
- `src-tauri/src/lib.rs`
- `src-tauri/src/desktop_signals.rs`

Potential supporting files:
- `src-tauri/Cargo.toml`

## Non-Goals

This design does not include:
- changing break cadence or engine semantics
- redesigning the sync polling cadence
- implementing Linux native idle/fullscreen support
- implementing Windows native idle/fullscreen support

## Success Criteria

The work is complete when:
- desktop signal collection is routed through a dedicated cross-platform module
- `macOS` uses native idle detection instead of the old focus/minimize heuristic
- Linux and Windows continue to work through the fallback provider
- `lib.rs` no longer embeds platform-specific desktop signal logic
- automated tests cover the fallback path and integration behavior
