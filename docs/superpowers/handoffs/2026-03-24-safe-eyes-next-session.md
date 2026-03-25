# Safe Eyes Next Session Handoff

## Current State

The app now has partial Safe Eyes parity, but not full behavior parity.

Implemented:
- Shared Safe Eyes-style JSON config at `src-tauri/gen/android/app/src/main/assets/config/safeeyes.json`
- Rust loads the same config file used by Android
- Break cadence matches current configured values:
  - `break_interval = 15`
  - `short_break_duration = 15`
  - `long_break_duration = 60`
  - `no_of_short_breaks_per_long_break = 5`
- Android service reads `break_interval` from the shared JSON asset
- Desktop `cargo tauri dev` now has a local timer path instead of failing on missing `AndroidBridge`
- Android bridge injection was moved to the correct `onWebViewCreate(webView)` hook in `MainActivity.kt`

Not implemented yet:
- `pre_break_warning_time`
- full `strict_break` behavior parity
- idle detection / postpone logic
- break selection from `short_breaks` / `long_breaks`
- fullscreen-aware postpone logic
- disable/snooze handling from `disable_options`

## Important Files

- Plan: `docs/superpowers/plans/2026-03-24-safe-eyes-parity-plan.md`
- Shared Safe Eyes config: `src-tauri/gen/android/app/src/main/assets/config/safeeyes.json`
- Rust scheduler/app entry: `src-tauri/src/lib.rs`
- Android service: `src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/BreakReminderService.kt`
- Android activity/bridge: `src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/MainActivity.kt`
- Desktop/mobile controls UI: `src/index.html`
- Break screen UI: `src/break.html`

## Recommended Next Task

Start with **Task 1** from the plan:
- Extract a shared Safe Eyes engine into `src-tauri/src/safeeyes.rs`
- Move scheduling and policy decisions out of `lib.rs`
- Keep platform code as signal/input adapters only

Reason:
- The current logic is still too spread across Rust, JS, and Kotlin
- The remaining Safe Eyes features all depend on centralized policy/state

## Expected Behavior Right Now

Desktop:
- `cargo tauri dev`
- Start button creates a desktop interval timer based on `break_interval`
- Stop button clears that timer
- Break window can still be shown

Android:
- Bridge should now be injected through `onWebViewCreate`
- Start/stop service path remains Android-only
- Break interval comes from the shared JSON asset

## Verification Commands

Rust:
```bash
cd /Users/reus/projects/per/GazeGuard/src-tauri
cargo test
```

Android unit test:
```bash
cd /Users/reus/projects/per/GazeGuard/src-tauri/gen/android
./gradlew :app:testUniversalDebugUnitTest
```

Android compile:
```bash
cd /Users/reus/projects/per/GazeGuard/src-tauri/gen/android
./gradlew :app:compileUniversalDebugKotlin
```

Desktop dev:
```bash
cd /Users/reus/projects/per/GazeGuard
cargo tauri dev
```

Android dev:
```bash
cd /Users/reus/projects/per/GazeGuard
cargo tauri android dev
```

## Known Caveats

- `gradle :app:installUniversalDebug` alone is not the correct entry point for this Tauri mobile setup; use `cargo tauri android dev` or `cargo tauri android build --debug`
- Java 25 caused Kotlin daemon issues earlier; Java 21 is the safer build choice
- There are still existing warnings in Rust (`unused variable`, `unused_mut`) that were not cleaned up because they are unrelated to parity work

## Next Session Prompt

Use this to resume:

```text
Continue Safe Eyes parity work from docs/superpowers/handoffs/2026-03-24-safe-eyes-next-session.md and docs/superpowers/plans/2026-03-24-safe-eyes-parity-plan.md. Start with Task 1: extract a shared Rust safeeyes engine into src-tauri/src/safeeyes.rs, move scheduling/policy there, and keep desktop/Android as platform adapters. Preserve the shared safeeyes.json asset as the source of truth.
```
