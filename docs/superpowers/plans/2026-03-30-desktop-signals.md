# Desktop Signals Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Route desktop idle/fullscreen detection through a dedicated cross-platform module, using native macOS idle detection now while keeping Linux and Windows on the existing heuristic fallback.

**Architecture:** Add a new `desktop_signals` module in `src-tauri/src` that exposes a normalized `DesktopSignals` struct plus a `collect_desktop_signals` function. Keep `lib.rs` responsible only for applying those booleans to the `BreakEngine`, and introduce a small helper so that the signal-application path can be tested without constructing a real Tauri `AppHandle`.

**Tech Stack:** Rust, Tauri 2, macOS CoreGraphics FFI, cargo test

---

### Task 1: Add The Cross-Platform Desktop Signals Module And Preserve The Current Fallback Behavior

**Files:**
- Create: `src-tauri/src/desktop_signals.rs`
- Modify: `src-tauri/src/lib.rs:1-6`
- Test: `src-tauri/src/desktop_signals.rs`

- [ ] **Step 1: Write the failing fallback tests in the new module**

```rust
#[cfg(test)]
mod tests {
    use super::{fallback_from_window_state, DesktopSignals, WindowStateSnapshot};

    #[test]
    fn fallback_marks_idle_when_window_is_not_focused() {
        let signals = fallback_from_window_state(WindowStateSnapshot {
            fullscreen: false,
            focused: false,
            minimized: false,
            visible: true,
        });

        assert_eq!(
            signals,
            DesktopSignals {
                fullscreen_active: false,
                idle_active: true,
            }
        );
    }

    #[test]
    fn fallback_preserves_fullscreen_when_window_is_visible_and_focused() {
        let signals = fallback_from_window_state(WindowStateSnapshot {
            fullscreen: true,
            focused: true,
            minimized: false,
            visible: true,
        });

        assert_eq!(
            signals,
            DesktopSignals {
                fullscreen_active: true,
                idle_active: false,
            }
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test fallback_marks_idle_when_window_is_not_focused`
Expected: FAIL because `desktop_signals.rs`, `DesktopSignals`, `WindowStateSnapshot`, and `fallback_from_window_state` do not exist yet

- [ ] **Step 3: Write the minimal cross-platform module and hook it into `lib.rs`**

```rust
// src-tauri/src/desktop_signals.rs
use tauri::Manager;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesktopSignals {
    pub fullscreen_active: bool,
    pub idle_active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowStateSnapshot {
    pub fullscreen: bool,
    pub focused: bool,
    pub minimized: bool,
    pub visible: bool,
}

pub fn fallback_from_window_state(window: WindowStateSnapshot) -> DesktopSignals {
    DesktopSignals {
        fullscreen_active: window.fullscreen,
        idle_active: !window.focused || window.minimized || !window.visible,
    }
}

fn collect_fallback_desktop_signals(app: &tauri::AppHandle) -> DesktopSignals {
    app.get_webview_window("main")
        .map(|window| {
            fallback_from_window_state(WindowStateSnapshot {
                fullscreen: window.is_fullscreen().unwrap_or(false),
                focused: window.is_focused().unwrap_or(true),
                minimized: window.is_minimized().unwrap_or(false),
                visible: window.is_visible().unwrap_or(true),
            })
        })
        .unwrap_or(DesktopSignals {
            fullscreen_active: false,
            idle_active: false,
        })
}

pub fn collect_desktop_signals(app: &tauri::AppHandle, _idle_threshold_seconds: u64) -> DesktopSignals {
    collect_fallback_desktop_signals(app)
}
```

```rust
// src-tauri/src/lib.rs
mod break_engine;
mod desktop_signals;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test fallback_marks_idle_when_window_is_not_focused`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/desktop_signals.rs src-tauri/src/lib.rs
git commit -m "refactor: add desktop signals module"
```

### Task 2: Add Native macOS Idle Detection Behind The Module API

**Files:**
- Modify: `src-tauri/src/desktop_signals.rs`
- Modify: `src-tauri/Cargo.toml:17-30`
- Test: `src-tauri/src/desktop_signals.rs`

- [ ] **Step 1: Write the failing macOS-threshold test around pure decision logic**

```rust
#[test]
fn idle_threshold_uses_elapsed_idle_seconds() {
    assert!(!idle_active_from_seconds(4.0, 5));
    assert!(idle_active_from_seconds(5.0, 5));
    assert!(idle_active_from_seconds(9.5, 5));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test idle_threshold_uses_elapsed_idle_seconds`
Expected: FAIL because `idle_active_from_seconds` does not exist yet

- [ ] **Step 3: Add the macOS provider and threshold helper**

```rust
// src-tauri/src/desktop_signals.rs
#[cfg(target_os = "macos")]
mod platform {
    use super::{fallback_from_window_state, DesktopSignals, WindowStateSnapshot};
    use core_graphics::event::{CGEventSource, CGEventSourceStateID};
    use tauri::Manager;

    pub fn collect(app: &tauri::AppHandle, idle_threshold_seconds: u64) -> DesktopSignals {
        let fallback = app
            .get_webview_window("main")
            .map(|window| {
                fallback_from_window_state(WindowStateSnapshot {
                    fullscreen: window.is_fullscreen().unwrap_or(false),
                    focused: window.is_focused().unwrap_or(true),
                    minimized: window.is_minimized().unwrap_or(false),
                    visible: window.is_visible().unwrap_or(true),
                })
            })
            .unwrap_or(DesktopSignals {
                fullscreen_active: false,
                idle_active: false,
            });

        let idle_seconds = CGEventSource::seconds_since_last_event_type(
            CGEventSourceStateID::CombinedSessionState,
            core_graphics::event::CGEventType::Null,
        );

        DesktopSignals {
            fullscreen_active: fallback.fullscreen_active,
            idle_active: idle_active_from_seconds(idle_seconds, idle_threshold_seconds),
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::DesktopSignals;

    pub fn collect(app: &tauri::AppHandle, _idle_threshold_seconds: u64) -> DesktopSignals {
        super::collect_fallback_desktop_signals(app)
    }
}

fn idle_active_from_seconds(idle_seconds: f64, idle_threshold_seconds: u64) -> bool {
    idle_seconds >= idle_threshold_seconds as f64
}
```

```toml
# src-tauri/Cargo.toml
[target.'cfg(target_os = "macos")'.dependencies]
core-graphics = "0.24"
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test idle_threshold_uses_elapsed_idle_seconds`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/desktop_signals.rs src-tauri/Cargo.toml
git commit -m "feat: add macos desktop idle signals"
```

### Task 3: Route `sync_desktop_window_state` Through The Module And Make It Testable

**Files:**
- Modify: `src-tauri/src/lib.rs:477-506`
- Test: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write the failing integration test for applying normalized signals**

```rust
#[test]
fn apply_desktop_signals_updates_engine_idle_and_fullscreen_state() {
    let mut engine = BreakEngine::new(BreakEngineConfig::load());
    engine.start();

    let status = apply_desktop_signals_to_engine(
        &mut engine,
        crate::desktop_signals::DesktopSignals {
            idle_active: true,
            fullscreen_active: true,
        },
    );

    assert!(matches!(status.phase, EnginePhase::Running));
    assert_eq!(engine.snapshot(0).idle_active, true);
    assert_eq!(engine.snapshot(0).fullscreen, true);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test apply_desktop_signals_updates_engine_idle_and_fullscreen_state`
Expected: FAIL because `apply_desktop_signals_to_engine` does not exist yet

- [ ] **Step 3: Write the minimal integration refactor**

```rust
// src-tauri/src/lib.rs
fn apply_desktop_signals_to_engine(
    engine: &mut BreakEngine,
    signals: desktop_signals::DesktopSignals,
) -> EngineStatus {
    engine.set_idle(signals.idle_active);
    engine.set_fullscreen(signals.fullscreen_active);
    engine.status()
}

#[tauri::command]
fn sync_desktop_window_state(
    app: tauri::AppHandle,
    state: State<'_, SharedBreakEngine>,
) -> Result<EngineStatus, String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;

    #[cfg(desktop)]
    let signals = desktop_signals::collect_desktop_signals(&app, guard.config().idle_time);

    #[cfg(not(desktop))]
    let signals = desktop_signals::DesktopSignals {
        fullscreen_active: false,
        idle_active: false,
    };

    let status = apply_desktop_signals_to_engine(&mut guard, signals);
    drop(guard);
    let _ = save_registered_engine_snapshot(unix_now_seconds());
    Ok(status)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test apply_desktop_signals_updates_engine_idle_and_fullscreen_state`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "refactor: route desktop sync through signal provider"
```

### Task 4: Verify Fallback And Integration Coverage End To End

**Files:**
- Modify: `src-tauri/src/desktop_signals.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/desktop_signals.rs`
- Test: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write the failing regression tests for minimized and invisible fallback states**

```rust
#[test]
fn fallback_marks_idle_when_window_is_minimized() {
    let signals = fallback_from_window_state(WindowStateSnapshot {
        fullscreen: false,
        focused: true,
        minimized: true,
        visible: true,
    });

    assert!(signals.idle_active);
}

#[test]
fn fallback_marks_idle_when_window_is_hidden() {
    let signals = fallback_from_window_state(WindowStateSnapshot {
        fullscreen: false,
        focused: true,
        minimized: false,
        visible: false,
    });

    assert!(signals.idle_active);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test fallback_marks_idle_when_window_is_minimized`
Expected: FAIL if the additional regression cases have not been added yet

- [ ] **Step 3: Add the missing regression tests and any small cleanup required**

```rust
#[cfg(test)]
mod tests {
    use super::{fallback_from_window_state, idle_active_from_seconds, DesktopSignals, WindowStateSnapshot};

    // keep the earlier tests and add the minimized/hidden cases here
}
```

```rust
// src-tauri/src/lib.rs tests
#[test]
fn apply_desktop_signals_clears_idle_and_fullscreen_state() {
    let mut engine = BreakEngine::new(BreakEngineConfig::load());
    engine.start();
    engine.set_idle(true);
    engine.set_fullscreen(true);

    let status = apply_desktop_signals_to_engine(
        &mut engine,
        crate::desktop_signals::DesktopSignals {
            idle_active: false,
            fullscreen_active: false,
        },
    );

    assert!(matches!(status.phase, EnginePhase::Running));
    assert!(!engine.snapshot(0).idle_active);
    assert!(!engine.snapshot(0).fullscreen);
}
```

- [ ] **Step 4: Run the full Rust test suite**

Run: `cd src-tauri && cargo test`
Expected: PASS with all suites green

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/desktop_signals.rs src-tauri/src/lib.rs
git commit -m "test: cover desktop signal integration"
```
