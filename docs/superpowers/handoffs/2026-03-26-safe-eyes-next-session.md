# Safe Eyes Next Session Handoff

## Current Branch State

- Branch: `feat/safe-eyes-compat`
- Current HEAD: `a57bd0e` `refactor: centralized rust behab`
- Scope of this handoff: describe the current committed state after the Android overlay merge and the remaining Safe Eyes parity work

## What Is In Place Now

- Rust remains the primary break-state engine in:
  - `src-tauri/src/break_engine.rs`
  - `src-tauri/src/lib.rs`
- The Android app can now surface breaks while backgrounded by using an overlay path instead of relying only on an activity launch:
  - `src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/BreakReminderService.kt`
  - `src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/MainActivity.kt`
  - `src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/RustProbe.kt`
- The main UI exposes Android setup guidance for notification, full-screen alert, and overlay permissions:
  - `src/index.html`
- The break page still handles the in-app break experience and skip behavior:
  - `src/break.html`

## Current Config Baseline

The shared Safe Eyes-compatible config is in:
- `src-tauri/gen/android/app/src/main/assets/config/defaults.yaml`

Important values currently committed there:
- `short_break_interval: 15`
- `long_break_interval: 75`
- `pre_break_warning_time: 10`
- `short_break_duration: 15`
- `random_order: true`
- `allow_postpone: false`
- `postpone_duration: 5`
- `postpone_unit: minutes`

The spike timing is not active anymore. Any next-session work should assume the normal interval values above.

## Real Remaining Gaps

### 1. Android background timing still lives in Kotlin

`BreakReminderService.kt` still owns its own `Timer`, computes warning delays, and recursively schedules the next cycle. That means Android background behavior can drift from Rust engine state even though Rust now owns the canonical break model.

The next cleanup should make Android a delivery adapter:
- Rust decides when warning/break phases happen
- Android reads or receives that state and only renders notifications/overlay
- Kotlin stops owning independent interval math

### 2. Several Safe Eyes config fields are still ignored

These keys exist in `defaults.yaml` but do not appear to be consumed outside the YAML file:
- `random_order`
- `allow_postpone`
- `postpone_duration`
- `postpone_unit`
- `persist_state`
- `shortcut_disable_time`
- `shortcut_skip`
- `shortcut_postpone`

The current Rust config loader only maps:
- break intervals and durations
- `strict_break`
- `consecutive_skip_limit`
- `idle_time`
- break templates
- disable options

So parity is still incomplete at the configuration level.

### 3. Postpone is not a user-facing feature yet

The engine currently exposes `postpone_reason`, but that is only used to explain automatic deferral caused by idle/fullscreen heuristics. There is no explicit postpone command or UX path in the desktop UI, break page, or Android overlay flow.

If Safe Eyes compatibility requires postpone behavior, the next session needs both:
- engine semantics for postpone
- a real user action that invokes them

### 4. Desktop fidelity is still heuristic

Desktop delay logic still depends on app/window visibility and fullscreen heuristics. That is acceptable for now, but it is lower priority than:
- removing Android scheduling drift
- implementing the missing config semantics

## Recommended Next Session Order

### Task 1: Make Android a Rust-driven delivery layer

Start with:
- `src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/BreakReminderService.kt`
- `src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/RustProbe.kt`
- `src-tauri/src/lib.rs`
- `src-tauri/src/break_engine.rs`

Target outcome:
- no independent Android timer loop for warning/break cadence
- overlay and notifications follow Rust-owned phase/state

### Task 2: Implement the missing Safe Eyes config semantics

First pass:
- `allow_postpone`
- `postpone_duration`
- `postpone_unit`
- `random_order`

Second pass:
- `persist_state`
- shortcut-related fields

### Task 3: Add postpone UX only after the engine semantics exist

Likely touch points:
- `src-tauri/src/break_engine.rs`
- `src-tauri/src/lib.rs`
- `src/index.html`
- `src/break.html`
- Android overlay actions if postpone is supported on mobile

## Suggested Resume Prompt

```text
Continue Safe Eyes parity work on feat/safe-eyes-compat from HEAD a57bd0e. The Android overlay path is merged, but BreakReminderService.kt still owns an independent Timer-based schedule, so start by making Android a delivery adapter driven by Rust engine state. After that, implement the missing Safe Eyes config semantics that are already present in defaults.yaml but not wired into the engine yet, especially random_order and the postpone-related fields. Do not assume the old spike timing is still active; defaults.yaml is back to the normal 15/75-minute schedule.
```
