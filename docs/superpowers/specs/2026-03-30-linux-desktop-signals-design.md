# Linux Desktop Signals Design

## Scope

Extend the existing `desktop_signals` abstraction with a Linux-native idle provider that targets Wayland first in this pass.

Out of scope for this design:
- Windows native desktop signal implementation
- Linux native fullscreen/session detection
- break UI changes
- Android signal changes
- break engine policy changes

## Goal

Improve Linux desktop fidelity by adding a Wayland-first native idle path behind `src-tauri/src/desktop_signals.rs`, while preserving the existing heuristic fallback when Wayland idle detection is unavailable or unusable.

## Product Decision

Linux support should be incremental and resilient.

In this pass:
- prefer native Wayland idle detection when the runtime environment exposes it
- keep fullscreen on the existing window heuristic
- fall back to the current heuristic desktop behavior instead of failing hard when native Wayland idle is unavailable

This keeps Linux support practical across mixed compositor environments without weakening the cross-platform abstraction already in place.

## Recommended Approach

Keep `desktop_signals` as the only desktop signal boundary and add a Linux-specific provider behind `#[cfg(all(desktop, target_os = "linux"))]`.

Implementation split:
- `macOS`: keep the current native idle path
- Linux: add a Wayland-first idle path with heuristic fallback
- Windows: keep the current fallback path for now

`src-tauri/src/lib.rs` should continue calling `collect_desktop_signals(...)` without learning anything about Wayland or Linux-specific behavior.

## Linux Provider

The Linux provider should:
- preserve fullscreen from the existing window fullscreen heuristic
- attempt to obtain native idle state through a Wayland-capable path first
- fall back to the current heuristic window-state behavior when:
  - Wayland is not the active session
  - the compositor does not expose the needed idle protocol
  - native idle detection errors or times out

The provider should not crash startup or desktop polling if Wayland idle support is unavailable.

## Idle Semantics

The Linux provider should follow the same contract as the current `macOS` path:
- return a normalized `idle_active` boolean
- avoid reimplementing the full engine-owned idle threshold in the provider
- allow a small provider-side debounce floor if needed to avoid poll-loop flapping during active input

`BreakEngine` remains the only owner of the configured idle threshold semantics.

## Fallback Behavior

When native Linux idle detection is unavailable, preserve the current heuristic behavior:
- fullscreen from window fullscreen state
- idle inferred from focus/minimize/visibility heuristics

This fallback should remain inside `desktop_signals` so callers do not need platform-specific branching.

## Architecture Boundaries

`desktop_signals` owns:
- Linux runtime selection between Wayland-native idle and heuristic fallback
- any Linux-specific wrappers or helper functions needed to query idle state
- normalization into `DesktopSignals`

`lib.rs` owns:
- calling the abstraction
- applying returned booleans to the break engine
- snapshot persistence and command wiring

`break_engine.rs` remains unchanged in scope.

## Testing

Automated coverage should focus on Linux provider decision logic and fallback behavior rather than live Wayland session integration.

Required coverage:
- Linux provider falls back safely when Wayland idle is unavailable
- Linux provider preserves fullscreen from the heuristic path
- Linux provider keeps the existing fallback behavior for non-Wayland or unsupported sessions

Manual verification is required on Linux for:
- Wayland active use does not incorrectly report idle
- real Wayland inactivity enters idle mode
- unsupported compositor/session cases cleanly fall back to the heuristic behavior

## Expected Touchpoints

Primary files:
- `src-tauri/src/desktop_signals.rs`
- `src-tauri/Cargo.toml`

Potential supporting files:
- `src-tauri/src/lib.rs`

## Non-Goals

This design does not include:
- Linux native fullscreen/session detection
- Windows native idle support
- removing the current heuristic fallback
- changing polling cadence

## Success Criteria

The work is complete when:
- Linux has a dedicated provider path inside `desktop_signals`
- Wayland idle is attempted first on Linux
- unsupported or failing Wayland idle detection falls back safely to the current heuristic behavior
- fullscreen remains stable through the existing heuristic path
- automated tests cover the Linux provider decision logic and fallback path
