# Safe Eyes Next Session Handoff

Refreshed on `2026-03-30` to align the handoff with the current branch head and current repo state.

## Current Branch State

- Branch: `feat/safe-eyes-compat`
- Current HEAD: `aa2fe38`
- Working tree status at handoff time: dirty with uncommitted desktop-signal work and matching spec/plan docs

## What Is In Place Now

- Rust remains the scheduling and policy authority in:
  - `src-tauri/src/break_engine.rs`
  - `src-tauri/src/lib.rs`
- Android background delivery is now Rust-driven rather than Kotlin-timer-driven:
  - `src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/BreakReminderService.kt`
  - `src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/RustProbe.kt`
  - `src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/AndroidBreakDeliverySnapshot.kt`
  - `src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/BreakDeliveryCoordinator.kt`
- `random_order` is implemented in the Rust engine.
- Postpone semantics are implemented in the Rust engine and exposed through Tauri.
- Postpone is now config-driven from YAML and available on:
  - desktop/web break page in `src/break.html`
  - Android native overlay in `src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/BreakReminderService.kt`
- `persist_state` is now implemented end to end:
  - YAML-backed config parsing in `src-tauri/src/break_engine.rs`
  - snapshot import/export and elapsed-time restore in `src-tauri/src/break_engine.rs`
  - startup load/save persistence wiring in `src-tauri/src/lib.rs`
  - desktop and Android manual relaunch behavior were validated after implementation fixes
- desktop signal collection now goes through a dedicated cross-platform module:
  - `src-tauri/src/desktop_signals.rs`
  - `src-tauri/src/lib.rs`
  - `src-tauri/Cargo.toml`
- on `macOS`, desktop idle detection now uses native OS inactivity rather than the old focus/minimize heuristic
- Linux and Windows still use the existing window-state fallback path through the same module boundary
- the stale shortcut config fields were removed from:
  - `src-tauri/gen/android/app/src/main/assets/config/defaults.yaml`
  - `src-tauri/src/break_engine.rs`

## Current Config Baseline

The shared config is in:
- `src-tauri/gen/android/app/src/main/assets/config/defaults.yaml`

Important currently committed values:
- `short_break_interval: 15`
- `long_break_interval: 75`
- `pre_break_warning_time: 10`
- `short_break_duration: 15`
- `random_order: true`
- `allow_postpone: true`
- `postpone_duration: 5`
- `postpone_unit: minutes`
- `persist_state: true`
- `postpone_options`:
  - `5 minutes`
  - `10 minutes`
  - `15 minutes`

The quick-test timing change is not active anymore. The branch is back on the normal schedule values above.

## Verified State

Rust verification:
- `cargo test` passes from `src-tauri`

Android unit-test verification:
- `./gradlew app:testUniversalDebugUnitTest --tests com.reus.gazeguard.AndroidBreakDeliverySnapshotTest --tests com.reus.gazeguard.BreakDeliveryCoordinatorTest`

Manual verification completed in this session:
- Android break overlay works
- Android postpone flow works
- desktop/web postpone flow works
- break-screen postpone control was refined to open from the button instead of always showing the picker
- overlap between the break content and postpone controls was fixed
- desktop/macOS relaunch restores persisted countdown state
- Android relaunch restores persisted countdown state
- Android relaunch crash introduced by exit-time persistence was fixed
- desktop stale-snapshot regression after forced close was fixed

Additional verification completed after the last handoff:
- `cargo test` now passes with the desktop-signals work in place
- desktop signal fallback tests cover unfocused, minimized, hidden, and no-window cases
- desktop signal integration tests cover both setting and clearing idle/fullscreen state

## What Changed Since The Previous Handoff

The older handoff is now stale in a few important ways:

- Android is no longer using the old independent `Timer`-driven schedule loop.
- `random_order` is no longer a missing config feature.
- postpone is no longer missing:
  - engine semantics exist
  - YAML-backed postpone options exist
  - desktop/web break page supports config-driven postpone
  - Android overlay supports config-driven postpone
- `persist_state` is no longer missing:
  - config parsing exists
  - engine snapshot import/export exists
  - elapsed restore exists
  - Tauri startup/load/save wiring exists
  - manual relaunch validation was completed on desktop and Android
- the old shortcut-config gap is no longer relevant because those dead keys were removed from the shipped config and parser
- desktop signal quality is no longer purely heuristic on `macOS`

## Remaining Gaps That Still Matter

### 1. Linux and Windows still use the fallback desktop signal path

The `desktop_signals` abstraction is in place, but only `macOS` has a native idle implementation today.

Current concrete state:
- `src-tauri/src/desktop_signals.rs` has a native `macOS` idle path
- Linux and Windows still route through the fallback window heuristic

Implication for next session:
- Linux is the next platform to improve
- Windows should follow after Linux if cross-platform desktop fidelity remains the priority

### 2. Fullscreen/session fidelity is still heuristic on all desktop targets

The current work improved idle detection on `macOS`, but fullscreen still uses the existing window fullscreen heuristic inside `desktop_signals`.

This is acceptable for now, but it remains a fidelity gap if stricter Safe Eyes parity is required later.

### 3. Android overlay UX could still be refined

The Android overlay postpone/skip controls now work, but the remaining work here is polish rather than missing core functionality:
- spacing/styling polish
- touch affordance polish
- consistency with the desktop break page

## Recommended Next Session Order

### Task 1: Extend native desktop idle support beyond `macOS`

Start with Linux:
- keep the existing `desktop_signals` boundary
- add a Linux-native idle provider
- preserve the current fallback path until the Linux implementation is stable

Primary files:
- `src-tauri/src/desktop_signals.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/Cargo.toml`

### Task 2: Revisit fullscreen/session fidelity later

After Linux idle support is in place:
- better fullscreen/session detection on `macOS`
- decide whether Linux or Windows also need native fullscreen/session signals

## Suggested Resume Prompt

```text
Continue Safe Eyes parity work on feat/safe-eyes-compat from the current desktop-signals state. Android Rust-driven delivery, random_order, config-driven postpone, and persist_state are implemented and validated on desktop and Android. The stale shortcut config fields were removed, and `macOS` now uses native idle detection through `src-tauri/src/desktop_signals.rs`. The next highest-value work is extending native desktop idle support to Linux while keeping the current abstraction boundary and fallback path intact.
```

## Quick Verification Commands For The Next Session

Rust tests:
- `cd src-tauri && cargo test`

Focused Android unit tests:
- `cd src-tauri/gen/android && ./gradlew app:testUniversalDebugUnitTest --tests com.reus.gazeguard.AndroidBreakDeliverySnapshotTest --tests com.reus.gazeguard.BreakDeliveryCoordinatorTest`

Useful searches before coding:
- `rg -n "desktop_signals|collect_desktop_signals|idle_active_from_seconds" src-tauri/src`
- `rg -n "shortcut_disable_time|shortcut_skip|shortcut_postpone" src-tauri src`
