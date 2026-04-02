# macOS Desktop Signals Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add native macOS "other app is fullscreen" detection alongside the existing native idle path, while preserving heuristic fallback behavior and documenting a real-device validation checklist.

**Architecture:** Extend `src-tauri/src/desktop_signals.rs` with macOS-specific helper functions that compose fallback and native fullscreen signals under the existing provider trait. Keep idle on the existing CoreGraphics path, add macOS-native fullscreen detection for a foreign frontmost window or app, exclude GazeGuard from the fullscreen-positive result, and update the handoff with a concrete manual validation checklist.

**Tech Stack:** Rust, Tauri 2, CoreGraphics/ApplicationServices, macOS window/app APIs, cargo test

---

### Task 1: Add macOS Signal Composition And Fullscreen Decision Helpers

**Files:**
- Modify: `src-tauri/src/desktop_signals.rs`
- Test: `src-tauri/src/desktop_signals.rs`

- [ ] **Step 1: Write the failing macOS helper tests**

```rust
#[test]
fn macos_signals_from_sources_uses_native_values_when_present() {
    let fallback = DesktopSignals {
        fullscreen_active: false,
        idle_active: true,
    };

    let signals = macos_signals_from_sources(fallback, Some(false), Some(true));

    assert_eq!(
        signals,
        DesktopSignals {
            fullscreen_active: true,
            idle_active: false,
        }
    );
}

#[test]
fn macos_fullscreen_helper_rejects_own_app_window() {
    let screen = ScreenRect {
        left: 0,
        top: 0,
        right: 1728,
        bottom: 1117,
    };

    assert_eq!(
        macos_other_app_fullscreen_from_bounds(Some(42), Some(42), Some(screen), Some(screen)),
        Some(false)
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test macos_signals_from_sources_uses_native_values_when_present`
Expected: FAIL because `macos_signals_from_sources` does not exist yet

- [ ] **Step 3: Write the minimal helper implementation**

```rust
fn macos_signals_from_sources(
    fallback: DesktopSignals,
    native_idle_active: Option<bool>,
    native_fullscreen_active: Option<bool>,
) -> DesktopSignals {
    DesktopSignals {
        fullscreen_active: native_fullscreen_active.unwrap_or(fallback.fullscreen_active),
        idle_active: native_idle_active.unwrap_or(fallback.idle_active),
    }
}

fn macos_other_app_fullscreen_from_bounds(
    frontmost_window: Option<usize>,
    app_window: Option<usize>,
    frontmost_bounds: Option<ScreenRect>,
    screen_bounds: Option<ScreenRect>,
) -> Option<bool> {
    let frontmost_window = frontmost_window?;

    if app_window.is_some_and(|app_window| app_window == frontmost_window) {
        return Some(false);
    }

    let frontmost_bounds = frontmost_bounds?;
    let screen_bounds = screen_bounds?;
    Some(screen_rect_covers_monitor(frontmost_bounds, screen_bounds))
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test macos_signals_from_sources_uses_native_values_when_present`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/desktop_signals.rs
git commit -m "test: add macos desktop signal helpers"
```

### Task 2: Implement Native macOS Fullscreen Collection

**Files:**
- Modify: `src-tauri/src/desktop_signals.rs`
- Test: `src-tauri/src/desktop_signals.rs`

- [ ] **Step 1: Write the failing fullscreen geometry tests**

```rust
#[test]
fn macos_fullscreen_helper_detects_foreign_window_covering_screen() {
    let screen = ScreenRect {
        left: 0,
        top: 0,
        right: 1728,
        bottom: 1117,
    };
    let window = ScreenRect {
        left: 0,
        top: 0,
        right: 1728,
        bottom: 1117,
    };

    assert_eq!(
        macos_other_app_fullscreen_from_bounds(Some(99), Some(42), Some(window), Some(screen)),
        Some(true)
    );
}

#[test]
fn macos_fullscreen_helper_rejects_foreign_window_smaller_than_screen() {
    let screen = ScreenRect {
        left: 0,
        top: 0,
        right: 1728,
        bottom: 1117,
    };
    let window = ScreenRect {
        left: 20,
        top: 20,
        right: 1700,
        bottom: 1080,
    };

    assert_eq!(
        macos_other_app_fullscreen_from_bounds(Some(99), Some(42), Some(window), Some(screen)),
        Some(false)
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test macos_fullscreen_helper_detects_foreign_window_covering_screen`
Expected: FAIL because the new test has not been added yet

- [ ] **Step 3: Implement the native macOS fullscreen provider path**

```rust
#[cfg(all(desktop, target_os = "macos"))]
mod platform {
    use super::{
        desktop_signals_from_desktop_window, idle_active_from_seconds,
        macos_other_app_fullscreen_from_bounds, macos_signals_from_sources,
        DesktopSignalProvider, DesktopSignals, ScreenRect,
    };
    use core_graphics::event_source::CGEventSourceStateID;

    pub(super) struct PlatformDesktopSignalProvider;

    // keep the existing idle API and add thin macOS-native fullscreen wrappers here
    // - frontmost window/app lookup
    // - screen/frame extraction
    // - app-window exclusion for GazeGuard
    // - fallback to the Tauri-window fullscreen heuristic when native lookup fails
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test macos_fullscreen_helper_detects_foreign_window_covering_screen`
Expected: PASS

- [ ] **Step 5: Run the full Rust suite**

Run: `cd src-tauri && cargo test`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/desktop_signals.rs
git commit -m "feat: add macos native fullscreen signals"
```

### Task 3: Update The Handoff With A Concrete macOS Validation Checklist

**Files:**
- Modify: `docs/superpowers/handoffs/2026-03-26-safe-eyes-next-session.md`

- [ ] **Step 1: Add a concrete manual validation checklist to the handoff**

```markdown
Manual `macOS` validation checklist:
- confirm active typing/mouse movement keeps `idle_active` false
- confirm real inactivity flips `idle_active` true after the expected delay
- confirm a foreign fullscreen app triggers `fullscreen_active`
- confirm GazeGuard itself does not count as the fullscreen app
- confirm fallback behavior remains sane if native fullscreen lookup fails
```

- [ ] **Step 2: Review the handoff for stale Windows/Linux priority language**

Run: `sed -n '1,260p' docs/superpowers/handoffs/2026-03-26-safe-eyes-next-session.md`
Expected: the task order still shows `macOS` stabilization first

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/handoffs/2026-03-26-safe-eyes-next-session.md
git commit -m "docs: add macos desktop signal validation checklist"
```
