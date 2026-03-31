# Linux Desktop Signals Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Linux desktop signal provider that attempts Wayland native idle detection first and safely falls back to the current heuristic behavior when Wayland idle is unavailable.

**Architecture:** Extend `src-tauri/src/desktop_signals.rs` with a Linux-specific provider behind `#[cfg(all(desktop, target_os = "linux"))]` and keep `src-tauri/src/lib.rs` unchanged at the call site. Preserve fullscreen via the existing window heuristic, keep the engine as the owner of idle-threshold semantics, and make Linux-native idle collection degrade cleanly to the existing fallback path.

**Tech Stack:** Rust, Tauri 2, Linux Wayland idle client crate, cargo test

---

### Task 1: Add Linux Provider Decision Helpers And Preserve Fallback Behavior

**Files:**
- Modify: `src-tauri/src/desktop_signals.rs`
- Test: `src-tauri/src/desktop_signals.rs`

- [ ] **Step 1: Write the failing Linux fallback decision tests**

```rust
#[test]
fn linux_provider_uses_fallback_when_native_idle_is_unavailable() {
    let fallback = DesktopSignals {
        fullscreen_active: true,
        idle_active: false,
    };

    let signals = linux_idle_from_sources(fallback, None);

    assert_eq!(signals, fallback);
}

#[test]
fn linux_provider_overrides_fallback_idle_when_native_idle_reports_inactive_input() {
    let fallback = DesktopSignals {
        fullscreen_active: false,
        idle_active: true,
    };

    let signals = linux_idle_from_sources(fallback, Some(false));

    assert_eq!(
        signals,
        DesktopSignals {
            fullscreen_active: false,
            idle_active: false,
        }
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test linux_provider_uses_fallback_when_native_idle_is_unavailable`
Expected: FAIL because `linux_idle_from_sources` does not exist yet

- [ ] **Step 3: Add the Linux decision helper with no runtime Wayland wiring yet**

```rust
fn linux_idle_from_sources(
    fallback: DesktopSignals,
    native_idle_active: Option<bool>,
) -> DesktopSignals {
    DesktopSignals {
        fullscreen_active: fallback.fullscreen_active,
        idle_active: native_idle_active.unwrap_or(fallback.idle_active),
    }
}

#[cfg(all(desktop, target_os = "linux"))]
mod platform {
    use super::{desktop_signals_from_desktop_window, linux_idle_from_sources, DesktopSignals};

    fn native_idle_active() -> Option<bool> {
        None
    }

    pub fn collect(app: &tauri::AppHandle, _idle_threshold_seconds: u64) -> DesktopSignals {
        let fallback = desktop_signals_from_desktop_window(app);
        linux_idle_from_sources(fallback, native_idle_active())
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test linux_provider_uses_fallback_when_native_idle_is_unavailable`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/desktop_signals.rs
git commit -m "refactor: add linux desktop signal provider shape"
```

### Task 2: Add Wayland Session Gating And Runtime Fallback

**Files:**
- Modify: `src-tauri/src/desktop_signals.rs`
- Test: `src-tauri/src/desktop_signals.rs`

- [ ] **Step 1: Write the failing Wayland-session tests**

```rust
#[test]
fn linux_wayland_detection_accepts_wayland_session_type() {
    assert!(linux_prefers_wayland_session(Some("wayland"), None));
}

#[test]
fn linux_wayland_detection_rejects_non_wayland_sessions() {
    assert!(!linux_prefers_wayland_session(Some("x11"), None));
    assert!(!linux_prefers_wayland_session(None, None));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test linux_wayland_detection_accepts_wayland_session_type`
Expected: FAIL because `linux_prefers_wayland_session` does not exist yet

- [ ] **Step 3: Add the minimal Wayland session gate**

```rust
fn linux_prefers_wayland_session(
    xdg_session_type: Option<&str>,
    wayland_display: Option<&str>,
) -> bool {
    matches!(xdg_session_type, Some(value) if value.eq_ignore_ascii_case("wayland"))
        || wayland_display.is_some()
}

#[cfg(all(desktop, target_os = "linux"))]
mod platform {
    use super::{
        desktop_signals_from_desktop_window, linux_idle_from_sources, linux_prefers_wayland_session,
        DesktopSignals,
    };

    fn native_idle_active() -> Option<bool> {
        if !linux_prefers_wayland_session(
            std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
            std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
        ) {
            return None;
        }

        None
    }

    pub fn collect(app: &tauri::AppHandle, _idle_threshold_seconds: u64) -> DesktopSignals {
        let fallback = desktop_signals_from_desktop_window(app);
        linux_idle_from_sources(fallback, native_idle_active())
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test linux_wayland_detection_accepts_wayland_session_type`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/desktop_signals.rs
git commit -m "test: add linux wayland session gating"
```

### Task 3: Add Wayland Idle Client Dependency And Native Linux Idle Probe

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/desktop_signals.rs`
- Test: `src-tauri/src/desktop_signals.rs`

- [ ] **Step 1: Write the failing Linux-native idle normalization test**

```rust
#[test]
fn linux_native_idle_uses_same_signal_floor_as_macos() {
    assert!(!idle_active_from_seconds(0.5));
    assert!(idle_active_from_seconds(1.0));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test linux_native_idle_uses_same_signal_floor_as_macos`
Expected: FAIL because the new Linux-native test has not been added yet

- [ ] **Step 3: Add the Linux-specific dependency and native probe**

```toml
[target.'cfg(target_os = "linux")'.dependencies]
wayland-client = "0.31"
wayland-protocols = { version = "0.32", features = ["client"] }
wayland-protocols-wlr = { version = "0.3", features = ["client"] }
```

```rust
#[cfg(all(desktop, target_os = "linux"))]
mod platform {
    use super::{
        desktop_signals_from_desktop_window, idle_active_from_seconds, linux_idle_from_sources,
        linux_prefers_wayland_session, DesktopSignals,
    };

    fn native_idle_active() -> Option<bool> {
        if !linux_prefers_wayland_session(
            std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
            std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
        ) {
            return None;
        }

        native_wayland_idle_seconds().map(idle_active_from_seconds)
    }

    fn native_wayland_idle_seconds() -> Option<f64> {
        None
    }

    pub fn collect(app: &tauri::AppHandle, _idle_threshold_seconds: u64) -> DesktopSignals {
        let fallback = desktop_signals_from_desktop_window(app);
        linux_idle_from_sources(fallback, native_idle_active())
    }
}
```

Add the smallest compilable Wayland client wrapper needed so `native_wayland_idle_seconds()` can attempt a real idle query and return `None` on any failure.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test linux_native_idle_uses_same_signal_floor_as_macos`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/desktop_signals.rs
git commit -m "feat: add linux wayland idle probe"
```

### Task 4: Add Linux Regression Coverage And Run Full Verification

**Files:**
- Modify: `src-tauri/src/desktop_signals.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/desktop_signals.rs`
- Test: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write the failing Linux regression test for state clearing**

```rust
#[test]
fn apply_desktop_signals_clears_linux_idle_state() {
    let mut engine = BreakEngine::new(BreakEngineConfig::load());
    engine.start();
    engine.set_idle(true);

    let status = apply_desktop_signals_to_engine(
        &mut engine,
        crate::desktop_signals::DesktopSignals {
            idle_active: false,
            fullscreen_active: false,
        },
    );

    assert!(matches!(status.phase, EnginePhase::Running));
    assert!(!engine.snapshot(0).idle_active);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test apply_desktop_signals_clears_linux_idle_state`
Expected: FAIL because the Linux-specific regression test does not exist yet

- [ ] **Step 3: Add the missing regression coverage and any minimal cleanup**

```rust
// src-tauri/src/desktop_signals.rs tests
#[test]
fn linux_provider_keeps_fullscreen_from_fallback_path() {
    let fallback = DesktopSignals {
        fullscreen_active: true,
        idle_active: false,
    };

    let signals = linux_idle_from_sources(fallback, Some(false));

    assert!(signals.fullscreen_active);
    assert!(!signals.idle_active);
}
```

```rust
// src-tauri/src/lib.rs tests
#[test]
fn apply_desktop_signals_clears_linux_idle_state() {
    let mut engine = BreakEngine::new(BreakEngineConfig::load());
    engine.start();
    engine.set_idle(true);

    let status = apply_desktop_signals_to_engine(
        &mut engine,
        crate::desktop_signals::DesktopSignals {
            idle_active: false,
            fullscreen_active: false,
        },
    );

    assert!(matches!(status.phase, EnginePhase::Running));
    assert!(!engine.snapshot(0).idle_active);
}
```

- [ ] **Step 4: Run the full Rust test suite**

Run: `cd src-tauri && cargo test`
Expected: PASS with all suites green

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/desktop_signals.rs src-tauri/src/lib.rs
git commit -m "test: cover linux desktop signal fallback"
```
