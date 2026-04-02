# macOS Desktop Signals Design

## Scope

Stabilize the `macOS` desktop-signals path by implementing both native OS idle detection and native "another app is fullscreen" detection behind the existing provider boundary in `src-tauri/src/desktop_signals.rs`.

Out of scope for this design:
- Linux native idle/fullscreen work
- further Windows signal refinements
- break engine policy changes
- UI performance tuning beyond documenting the follow-up priority in the handoff

## Goal

Make the `macOS` provider match the same product intent now implemented on Windows:
- `idle_active` should reflect real OS inactivity
- `fullscreen_active` should indicate whether some other app is effectively fullscreen
- GazeGuard itself must not count as the fullscreen app

The break engine should continue consuming only normalized booleans.

## Product Decision

`macOS` should stop relying on the Tauri-window fullscreen heuristic as the primary signal when native desktop state is available. Native idle and native fullscreen should be owned by the provider and should degrade safely to fallback heuristics if the native query cannot determine a reliable answer.

## Recommended Approach

Keep the provider architecture already established in `src-tauri/src/desktop_signals.rs`.

Extend the `macOS` provider so it gathers:
- idle seconds from the existing CoreGraphics last-input API
- fullscreen state from native macOS window/app state for the currently active app/window, excluding GazeGuard

The provider should:
- normalize both signals into `DesktopSignals`
- preserve fallback behavior when native fullscreen cannot be resolved
- keep `lib.rs` unchanged at the call site

## API Shape

The public API remains:
- `DesktopSignals { idle_active: bool, fullscreen_active: bool }`
- `collect_desktop_signals(app: &tauri::AppHandle, idle_threshold_seconds: u64) -> DesktopSignals`

No macOS-specific types should escape the module boundary.

## macOS Idle Provider

The existing native idle path should remain the source of truth for `idle_active`.

Behavior requirements:
- active input should keep `idle_active` false
- real OS inactivity should flip `idle_active` true
- the existing small provider-side idle floor should remain in place to avoid noisy flapping
- the engine remains the owner of broader break timing semantics

## macOS Fullscreen Provider

The `macOS` provider should detect whether another app is effectively fullscreen.

Behavior requirements:
- identify the frontmost active app/window
- compare that window’s bounds to the target screen bounds
- only report fullscreen when the active foreign window effectively occupies the screen
- if the frontmost app/window belongs to GazeGuard, report `false`
- if the native fullscreen query fails or returns incomplete state, fall back to the existing Tauri-window fullscreen heuristic instead of returning an indeterminate result

The intent is to suppress breaks based on what the user is actually doing on the desktop, not based on GazeGuard’s own window state.

## Architecture Boundaries

`desktop_signals` owns:
- the provider implementation
- macOS-native wrappers and helper functions
- exclusion logic for the app’s own window/process
- fallback composition when native data is unavailable

`lib.rs` owns:
- polling the provider
- applying `idle_active` and `fullscreen_active` to the break engine

`break_engine.rs` remains unchanged.

## Testing

Automated coverage should focus on deterministic Rust helper logic rather than deep native-framework mocking.

Required automated coverage:
- macOS signal composition uses native values when present and falls back otherwise
- fullscreen helper rejects GazeGuard’s own window/app
- fullscreen helper accepts a foreign window that covers the screen bounds
- fullscreen helper rejects foreign windows that do not cover the screen

Manual validation is required on a real `macOS` machine for:
- active use keeps the engine out of idle mode
- real inactivity enters idle mode
- another fullscreen app suppresses correctly
- GazeGuard itself does not count as “other app fullscreen”
- fallback behavior does not regress if native fullscreen lookup fails

## Expected Touchpoints

Primary files:
- `src-tauri/src/desktop_signals.rs`
- `docs/superpowers/handoffs/2026-03-26-safe-eyes-next-session.md`

Potential supporting files:
- `docs/superpowers/plans/2026-04-01-macos-desktop-signals.md`

## Non-Goals

This design does not include:
- Linux Wayland idle redesign
- broader macOS session-state redesign beyond the needed fullscreen detection
- changes to break cadence, persistence, or postpone semantics

## Success Criteria

The work is complete when:
- `macOS` has native idle and native “other app is fullscreen” detection behind the shared provider boundary
- GazeGuard is excluded from the positive fullscreen result
- fallback behavior remains intact if native fullscreen cannot be resolved
- automated tests cover the deterministic helper logic
- the handoff includes a concrete manual macOS validation checklist
