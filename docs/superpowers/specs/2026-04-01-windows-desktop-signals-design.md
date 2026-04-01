# Windows Desktop Signals Design

## Scope

Add a Windows-native desktop signal provider that reports real OS idle state and detects when another application is occupying the screen in fullscreen.

Out of scope for this design:
- Linux native idle/fullscreen implementation
- Android signal behavior changes
- break engine policy changes
- UI performance tuning beyond documenting it in the handoff

## Goal

Extend `src-tauri/src/desktop_signals.rs` so `Windows` no longer depends only on the Tauri window heuristic. The module should gather native Windows idle and fullscreen signals behind the same cross-platform provider boundary already used by desktop signal collection.

In this pass:
- `idle_active` should come from Windows last-input timing
- `fullscreen_active` should indicate when some foreground app other than GazeGuard occupies the active monitor bounds
- the existing fallback heuristic should remain available if native calls fail

## Product Decision

Windows should follow the same provider architecture as the other desktop targets instead of adding more platform logic to `lib.rs`. The provider should own native collection and internal fallback decisions, while the engine continues to consume only normalized booleans.

## Recommended Approach

Refine `desktop_signals` around a small provider trait that returns `DesktopSignals`. Keep platform selection behind `#[cfg(...)]`, with each target implementing the same collection contract.

For Windows:
- use Win32 last-input APIs to derive idle seconds
- use the current foreground top-level window and monitor bounds to detect fullscreen occupancy
- exclude the GazeGuard main window from positive fullscreen results
- fall back to the existing window heuristic if any native step fails

## API Shape

The public surface should remain:
- `DesktopSignals { idle_active: bool, fullscreen_active: bool }`
- `collect_desktop_signals(app: &tauri::AppHandle, idle_threshold_seconds: u64) -> DesktopSignals`

Internally, introduce a provider trait so platform implementations conform to one collection interface without leaking Win32 details to the rest of the app.

## Windows Provider

Idle behavior:
- query Windows last-input information from the current session
- convert elapsed time to the existing normalized idle boolean using the same small signal floor already used by native providers
- if the query fails, preserve the fallback idle signal

Fullscreen behavior:
- inspect the current foreground window
- if the foreground window is GazeGuard’s own main window, report `false`
- otherwise compare the foreground window bounds against the nearest monitor bounds
- only report fullscreen when the foreground window effectively covers the monitor
- if any native query fails, preserve the fallback fullscreen signal

## Architecture Boundaries

`desktop_signals` owns:
- the provider trait
- platform-specific collection modules
- Win32 wrappers and normalization helpers
- fallback composition when native queries are unavailable

`lib.rs` owns:
- calling `collect_desktop_signals`
- applying normalized booleans to the engine

`break_engine.rs` remains unchanged.

## Testing

Required automated coverage:
- Windows signal composition prefers native values when present and falls back otherwise
- fullscreen helper rejects the app’s own window
- fullscreen helper accepts a foreground window that covers the monitor bounds
- fullscreen helper rejects foreground windows smaller than the monitor

Testing should focus on deterministic helper functions, not deep Win32 mocking.

## Expected Touchpoints

Primary files:
- `src-tauri/src/desktop_signals.rs`
- `src-tauri/Cargo.toml`

Potential supporting files:
- `docs/superpowers/handoffs/2026-03-26-safe-eyes-next-session.md`

## Success Criteria

The work is complete when:
- Windows has a native desktop signal provider behind the shared provider boundary
- idle detection uses Windows OS inactivity rather than the Tauri-window heuristic
- fullscreen detection identifies other foreground fullscreen apps rather than only the Tauri window state
- fallback behavior remains intact when native data is unavailable
- `cargo test` passes
