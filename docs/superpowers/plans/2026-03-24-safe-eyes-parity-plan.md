# Safe Eyes Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make GazeGuard match the requested Safe Eyes behaviors on both desktop and Android for warnings, strict breaks, idle postponement, break selection, fullscreen-aware postponement, and disable/snooze flows.

**Architecture:** Move scheduling and break-policy decisions into a shared Rust Safe Eyes engine driven by the existing `safeeyes.json` asset, then expose explicit state/commands to the HTML UI and Android service. Keep platform-specific work limited to sensors and lifecycle integration: desktop fullscreen/idle detection and Android service/activity hooks feed signals into the shared scheduler instead of duplicating policy in JavaScript or Kotlin.

**Tech Stack:** Tauri 2, Rust, vanilla HTML/JS, Android Kotlin service/activity code, shared Safe Eyes JSON config, Cargo tests, Gradle Android unit tests.

---

### Task 1: Extract a Shared Safe Eyes Engine

**Files:**
- Create: `src-tauri/src/safeeyes.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/safeeyes.rs`

- [ ] **Step 1: Write failing Rust tests for config parsing and scheduler state**
  Add tests covering `break_interval`, `pre_break_warning_time`, `strict_break`, `short_breaks`, `long_breaks`, and `disable_options`.

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test safeeyes -- --nocapture`
  Expected: FAIL with missing module/types/functions.

- [ ] **Step 3: Implement the `safeeyes` module**
  Add focused types for config, runtime state, current break selection, warning state, and snooze/disable state.

- [ ] **Step 4: Move existing break cadence logic behind the new module**
  Replace ad-hoc fields in `src-tauri/src/lib.rs` with a `SafeEyesEngine` wrapper around the shared scheduler.

- [ ] **Step 5: Run Rust tests**
  Run: `cargo test`
  Expected: PASS for existing cadence tests plus new config tests.

- [ ] **Step 6: Commit**
  ```bash
  git add src-tauri/src/lib.rs src-tauri/src/safeeyes.rs docs/superpowers/plans/2026-03-24-safe-eyes-parity-plan.md
  git commit -m "refactor: extract safe eyes scheduling engine"
  ```

### Task 2: Add Pre-Break Warning Timing

**Files:**
- Modify: `src-tauri/src/safeeyes.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/index.html`
- Modify: `src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/BreakReminderService.kt`
- Test: `src-tauri/src/safeeyes.rs`

- [ ] **Step 1: Write failing Rust tests for warning transitions**
  Cover "warning starts `pre_break_warning_time` seconds before the break" and "break opens after warning finishes".

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test warning -- --nocapture`
  Expected: FAIL because warning states/events are not implemented.

- [ ] **Step 3: Add warning event support in Rust**
  Expose commands/events for "next warning at", "warning active", and "warning complete".

- [ ] **Step 4: Update desktop UI to show a pre-break warning state**
  Add a visible countdown/message in `src/index.html` before opening the break screen.

- [ ] **Step 5: Update Android service to fire warning notifications before breaks**
  Use the same config value instead of immediate break triggering.

- [ ] **Step 6: Run verification**
  Run: `cargo test`
  Run: `./gradlew :app:compileUniversalDebugKotlin`
  Expected: PASS.

- [ ] **Step 7: Commit**
  ```bash
  git add src-tauri/src/safeeyes.rs src-tauri/src/lib.rs src/index.html src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/BreakReminderService.kt
  git commit -m "feat: add pre-break warning flow"
  ```

### Task 3: Implement Strict Break Semantics

**Files:**
- Modify: `src-tauri/src/safeeyes.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/break.html`
- Test: `src-tauri/src/safeeyes.rs`

- [ ] **Step 1: Write failing tests for strict-break behavior**
  Cover "strict break disables skip", "non-strict break allows skip", and "strict mode persists across both short and long breaks".

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test strict_break -- --nocapture`
  Expected: FAIL on skip semantics.

- [ ] **Step 3: Implement strict-break policy in the shared engine**
  Make `strict_break` authoritative instead of the current `skip_limit` approximation.

- [ ] **Step 4: Update break UI**
  Make `src/break.html` hide or disable skip/postpone affordances based on engine state.

- [ ] **Step 5: Run verification**
  Run: `cargo test`
  Expected: PASS.

- [ ] **Step 6: Commit**
  ```bash
  git add src-tauri/src/safeeyes.rs src-tauri/src/lib.rs src/break.html
  git commit -m "feat: implement strict break semantics"
  ```

### Task 4: Implement Break Selection from `short_breaks` and `long_breaks`

**Files:**
- Modify: `src-tauri/src/safeeyes.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/break.html`
- Test: `src-tauri/src/safeeyes.rs`

- [ ] **Step 1: Write failing tests for break message selection**
  Cover deterministic selection order or the exact Safe Eyes selection strategy chosen for parity.

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test break_selection -- --nocapture`
  Expected: FAIL because selected break content is not returned yet.

- [ ] **Step 3: Implement break template selection**
  Parse the configured `short_breaks` and `long_breaks` arrays and expose selected break identifiers/text through Rust commands.

- [ ] **Step 4: Render selected break content in the UI**
  Update `src/break.html` to show the selected exercise/message rather than a fixed "Take a Break" string.

- [ ] **Step 5: Run verification**
  Run: `cargo test`
  Expected: PASS.

- [ ] **Step 6: Commit**
  ```bash
  git add src-tauri/src/safeeyes.rs src-tauri/src/lib.rs src/break.html
  git commit -m "feat: use configured safe eyes break selections"
  ```

### Task 5: Add Idle Detection and Postpone Logic

**Files:**
- Create: `src-tauri/src/desktop_idle.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/safeeyes.rs`
- Modify: `src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/BreakReminderService.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/MainActivity.kt`
- Test: `src-tauri/src/safeeyes.rs`

- [ ] **Step 1: Write failing tests for idle postponement**
  Cover "active idle window postpones break" and "break resumes after activity returns".

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test idle -- --nocapture`
  Expected: FAIL because idle signals are not part of scheduling yet.

- [ ] **Step 3: Implement idle-aware scheduling hooks in Rust**
  Add explicit inputs for idle started/ended and decision logic based on `idle_time`.

- [ ] **Step 4: Add desktop idle signal provider**
  Implement a small platform adapter in `src-tauri/src/desktop_idle.rs` and wire it into the app lifecycle.

- [ ] **Step 5: Add Android idle/app-foreground signal handling**
  Use activity/service lifecycle signals as the minimum parity layer; document any Android OS limitations if true user-idle parity is not available without extra APIs.

- [ ] **Step 6: Run verification**
  Run: `cargo test`
  Run: `./gradlew :app:compileUniversalDebugKotlin`
  Expected: PASS.

- [ ] **Step 7: Commit**
  ```bash
  git add src-tauri/src/lib.rs src-tauri/src/safeeyes.rs src-tauri/src/desktop_idle.rs src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/BreakReminderService.kt src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/MainActivity.kt
  git commit -m "feat: add idle postpone behavior"
  ```

### Task 6: Add Fullscreen-Aware Postponement Rules

**Files:**
- Create: `src-tauri/src/desktop_window_state.rs`
- Modify: `src-tauri/src/safeeyes.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/MainActivity.kt`
- Test: `src-tauri/src/safeeyes.rs`

- [ ] **Step 1: Write failing tests for fullscreen defer rules**
  Cover "scheduled break is postponed while fullscreen is active" and "warning/break resumes when fullscreen exits".

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test fullscreen -- --nocapture`
  Expected: FAIL because fullscreen signals are not connected.

- [ ] **Step 3: Implement fullscreen-aware policy in Rust**
  Add explicit fullscreen-active inputs to the engine and use them in scheduling decisions.

- [ ] **Step 4: Add desktop fullscreen detection**
  Use Tauri/window/platform state in `src-tauri/src/desktop_window_state.rs` to feed the engine.

- [ ] **Step 5: Add Android fullscreen/activity-compatible behavior**
  Reuse immersive/activity state as the Android signal source where possible.

- [ ] **Step 6: Run verification**
  Run: `cargo test`
  Run: `./gradlew :app:compileUniversalDebugKotlin`
  Expected: PASS.

- [ ] **Step 7: Commit**
  ```bash
  git add src-tauri/src/lib.rs src-tauri/src/safeeyes.rs src-tauri/src/desktop_window_state.rs src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/MainActivity.kt
  git commit -m "feat: defer breaks during fullscreen sessions"
  ```

### Task 7: Implement Disable/Snooze Options from Config

**Files:**
- Modify: `src-tauri/src/safeeyes.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/index.html`
- Modify: `src/break.html`
- Modify: `src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/BreakReminderService.kt`
- Test: `src-tauri/src/safeeyes.rs`

- [ ] **Step 1: Write failing tests for disable window behavior**
  Cover the configured `disable_options` entries and re-enable timing.

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test disable_options -- --nocapture`
  Expected: FAIL because disable windows are not modeled.

- [ ] **Step 3: Add disable/snooze state to the shared engine**
  Parse `disable_options` and expose commands to disable reminders for the selected duration.

- [ ] **Step 4: Add desktop controls**
  Add disable/snooze actions in `src/index.html` using the configured values instead of hardcoded durations.

- [ ] **Step 5: Add Android service integration**
  Ensure Android scheduling honors disabled windows and resumes cleanly.

- [ ] **Step 6: Run verification**
  Run: `cargo test`
  Run: `./gradlew :app:testUniversalDebugUnitTest`
  Expected: PASS.

- [ ] **Step 7: Commit**
  ```bash
  git add src-tauri/src/safeeyes.rs src-tauri/src/lib.rs src/index.html src/break.html src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/BreakReminderService.kt
  git commit -m "feat: add safe eyes disable and snooze options"
  ```

### Task 8: Final Cross-Platform Integration Pass

**Files:**
- Modify: `src/index.html`
- Modify: `src/break.html`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/BreakReminderService.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/MainActivity.kt`

- [ ] **Step 1: Remove leftover duplicated scheduling paths**
  Ensure there is one authoritative scheduler and no desktop/Android drift.

- [ ] **Step 2: Run full verification**
  Run: `cargo test`
  Run: `./gradlew :app:testUniversalDebugUnitTest`
  Run: `./gradlew :app:compileUniversalDebugKotlin`
  Expected: PASS.

- [ ] **Step 3: Manual validation checklist**
  Validate desktop start/stop, warning, postpone, fullscreen defer, strict break, disable window, and break selection.
  Validate Android start/stop, warning notification, break open, strict break, and config-based disable interval.

- [ ] **Step 4: Commit**
  ```bash
  git add src/index.html src/break.html src-tauri/src/lib.rs src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/BreakReminderService.kt src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/MainActivity.kt
  git commit -m "feat: complete safe eyes parity pass"
  ```
