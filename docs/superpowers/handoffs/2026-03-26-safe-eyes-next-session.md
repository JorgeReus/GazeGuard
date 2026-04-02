# Safe Eyes Next Session Handoff

Refreshed on `2026-04-01` to align the handoff with the current branch head and current repo state.

## Current Branch State

- Branch: `feat/safe-eyes-compat`
- Working tree status at handoff time: includes updated handoff/spec/plan docs plus in-progress desktop-signal follow-up

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
- Linux now has Wayland-session detection and a dedicated provider path inside `src-tauri/src/desktop_signals.rs`, but it still falls back to the existing heuristic behavior
- Windows now has a native provider path for OS idle and "other app is fullscreen" detection inside `src-tauri/src/desktop_signals.rs`
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
- Linux desktop-signal tests now cover:
  - Linux fallback decision helpers
  - Wayland session detection
  - Wayland-ready versus fallback-only helper paths
- Windows desktop-signal tests now cover:
  - native/fallback signal composition helpers
  - own-window fullscreen exclusion
  - monitor-coverage fullscreen helpers
- Windows compile verification now passes with:
  - `PATH=/opt/homebrew/opt/llvm/bin:$PATH cargo check --target x86_64-pc-windows-msvc`

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
- Windows no longer relies purely on the Tauri-window heuristic for idle/fullscreen signals

## Remaining Gaps That Still Matter

### 1. macOS stabilization and validation should come first

The current priority is to stabilize the `macOS` path before more platform-native signal work lands on Linux or Windows.

Current concrete state:
- `src-tauri/src/desktop_signals.rs` has a native `macOS` idle path
- Windows now has a native provider for:
  - OS idle detection
  - "another app is fullscreen" detection
- Linux has:
  - `linux_prefers_wayland_session(...)`
  - Linux-specific fallback decision helpers
  - a dedicated Linux provider branch
  - no working native Wayland idle query in the current safe state

Recommended focus before more platform work:
- manual `macOS` validation around real inactivity, fullscreen behavior, and regression checks
- fix any `macOS` desktop-signal regressions before expanding Linux or Windows further

### 2. Linux native idle is not implemented yet, only scaffolded

Important note for next session:
- a naive Wayland probe using `ext_idle_notify_v1` was attempted and then backed out
- the problem was semantic, not just compile-related:
  - that protocol is event-driven, not a simple synchronous "current idle time" query
  - a one-shot polling model cannot rely on it without a longer-lived client/event-loop design
- do not reintroduce the reverted probe shape without redesigning the Linux idle approach first

Implication for next session:
- Linux work is deferred until after `macOS` is stable
- when resumed, the next step is design/architecture work for a long-lived Wayland idle client, not just wiring another helper

### 3. Fullscreen/session fidelity is still heuristic on some desktop targets

The current work improved idle detection on `macOS` and added a native fullscreen path on Windows, but fullscreen/session fidelity is still incomplete across the desktop targets overall.

This is acceptable for now, but it remains a fidelity gap if stricter Safe Eyes parity is required later.

### 4. Android overlay UX could still be refined

The Android overlay postpone/skip controls now work, but the remaining work here is polish rather than missing core functionality:
- spacing/styling polish
- touch affordance polish
- consistency with the desktop break page

### 5. Desktop signal delivery is still frontend-driven

Desktop signal collection now exists in Rust, but delivery into the engine still depends on the frontend calling `sync_desktop_window_state`.

Current concrete state:
- `src-tauri/src/desktop_signals.rs` owns platform signal collection
- `src-tauri/src/break_engine.rs` already uses `fullscreen` / `idle_active` to postpone warning and break transitions
- `src-tauri/src/lib.rs` only applies desktop signals when `sync_desktop_window_state` is invoked
- `src/index.html` currently drives that invocation on a one-second interval

Implication:
- fullscreen or idle suppression can fail when the desktop UI page is not actively driving the polling loop
- the architecture still has frontend-driven delivery even though the signal logic itself has moved into Rust

Direction chosen for the next session:
- move desktop signal delivery into Rust
- prefer a hybrid model rather than pure polling everywhere
- use event-driven or native watcher paths where the platform makes that practical
- keep a Rust-owned polling fallback where necessary
- do not keep frontend polling as the source of truth for desktop signal delivery

## Recommended Next Session Order

### Task 1: Stabilize and validate `macOS`

Before more Linux or Windows work:
- manually validate native `macOS` idle behavior during real inactivity and active use
- verify fullscreen/session behavior did not regress on `macOS`
- fix any `macOS` desktop-signal regressions first

Manual `macOS` validation checklist:
- confirm active typing/mouse movement keeps `idle_active` false
- confirm real inactivity flips `idle_active` true after the expected delay
- confirm a foreign fullscreen app triggers `fullscreen_active`
- confirm GazeGuard itself does not count as the fullscreen app
- confirm fallback behavior remains sane if native fullscreen lookup fails

Primary files:
- `src-tauri/src/desktop_signals.rs`
- `src-tauri/src/lib.rs`

### Task 2: Revisit Linux native idle later

After `macOS` is stable:
- keep the existing `desktop_signals` boundary
- preserve the current fallback path until the Linux implementation is stable
- redesign the Linux native idle approach around a long-lived Wayland client or other viable Wayland-compatible mechanism
- do not reuse the reverted one-shot `ext_idle_notify_v1` probe pattern

### Task 3: Revisit Windows follow-up later

After `macOS` is stable:
- manually validate the Windows native idle/fullscreen provider on a real Windows machine
- only refine Windows signals further if manual validation exposes gaps

### Task 4: Performance tuning pass

After desktop signal fidelity work is stable:
- profile baseline memory usage on desktop and Android
- minimize always-alive webviews and avoid hidden helper UI surfaces
- confirm scheduler/background behavior stays in Rust or native services rather than frontend timers
- reduce unnecessary polling, animation, and offscreen UI work on break and settings flows
- document concrete hotspots and follow-up optimizations before broader UI polish

### Task 5: Move desktop signal delivery into Rust with a hybrid model

After `macOS` stabilization work:
- remove the frontend as the source of truth for desktop signal delivery
- add a Rust-owned desktop signal delivery path that keeps the engine updated even when the UI is closed
- prefer event-driven/native delivery where a platform supports it cleanly
- keep a Rust polling fallback where event-driven delivery is not yet practical
- preserve the existing `desktop_signals` abstraction boundary while changing who drives the updates

## Suggested Resume Prompt

```text
Continue Safe Eyes parity work from the current desktop-signals state on `feat/safe-eyes-compat`. Android Rust-driven delivery, random_order, config-driven postpone, and persist_state are implemented and validated on desktop and Android. `macOS` uses native idle detection through `src-tauri/src/desktop_signals.rs`, Windows has a native provider for OS idle plus "other app is fullscreen" detection, and Linux still has only safe provider scaffolding plus Wayland session detection. Desktop signal collection is in Rust, but desktop signal delivery into the engine is still frontend-driven through `sync_desktop_window_state`. The next highest-value work after current macOS validation is moving desktop signal delivery into Rust using a hybrid event-driven plus Rust-polling-fallback model.
```

## Quick Verification Commands For The Next Session

Rust tests:
- `cd src-tauri && cargo test`

Focused Android unit tests:
- `cd src-tauri/gen/android && ./gradlew app:testUniversalDebugUnitTest --tests com.reus.gazeguard.AndroidBreakDeliverySnapshotTest --tests com.reus.gazeguard.BreakDeliveryCoordinatorTest`

Useful searches before coding:
- `rg -n "desktop_signals|collect_desktop_signals|idle_active_from_seconds" src-tauri/src`
- `rg -n "windows_signals_from_sources|windows_other_app_fullscreen_from_bounds|PlatformDesktopSignalProvider" src-tauri/src/desktop_signals.rs`
- `rg -n "linux_prefers_wayland_session|linux_native_idle_active_from_env|linux_should_query_native_idle" src-tauri/src/desktop_signals.rs`
- `rg -n "sync_desktop_window_state|set_idle_active|set_fullscreen_active" src-tauri/src src`
- `rg -n "shortcut_disable_time|shortcut_skip|shortcut_postpone" src-tauri src`
