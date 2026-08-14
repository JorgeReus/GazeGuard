# macOS Validation Logger Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add config-driven runtime logging for desktop signal validation so `macOS` idle/fullscreen behavior can be debugged from terminal output using only `config.yaml`.

**Architecture:** Parse a global `log_level` field into typed runtime config, add a tiny internal logger in `src-tauri/src/logger.rs`, and emit logs only from desktop signal collection plus signal-application path. Keep default behavior silent and avoid changing scheduler or signal semantics.

**Tech Stack:** Rust, Tauri 2, existing YAML config parser, existing desktop signal provider code

---

### Task 1: Add Typed Logger Module

**Files:**
- Create: `src-tauri/src/logger.rs`
- Test: `src-tauri/src/logger.rs`

- [ ] **Step 1: Write failing tests for log level parsing and ordering**

```rust
#[cfg(test)]
mod tests {
    use super::LogLevel;

    #[test]
    fn parses_known_log_levels() {
        assert_eq!(LogLevel::parse(Some("off")), LogLevel::Off);
        assert_eq!(LogLevel::parse(Some("error")), LogLevel::Error);
        assert_eq!(LogLevel::parse(Some("warn")), LogLevel::Warn);
        assert_eq!(LogLevel::parse(Some("info")), LogLevel::Info);
        assert_eq!(LogLevel::parse(Some("debug")), LogLevel::Debug);
        assert_eq!(LogLevel::parse(Some("trace")), LogLevel::Trace);
    }

    #[test]
    fn missing_or_invalid_log_level_defaults_to_off() {
        assert_eq!(LogLevel::parse(None), LogLevel::Off);
        assert_eq!(LogLevel::parse(Some("")), LogLevel::Off);
        assert_eq!(LogLevel::parse(Some("verbose")), LogLevel::Off);
    }

    #[test]
    fn higher_levels_enable_lower_levels() {
        assert!(LogLevel::Debug.allows(LogLevel::Info));
        assert!(LogLevel::Trace.allows(LogLevel::Debug));
        assert!(!LogLevel::Warn.allows(LogLevel::Info));
        assert!(!LogLevel::Off.allows(LogLevel::Error));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test parses_known_log_levels
```

Expected: FAIL with unresolved import or missing `LogLevel`

- [ ] **Step 3: Write minimal logger module**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    pub fn parse(raw: Option<&str>) -> Self {
        match raw.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
            Some("error") => Self::Error,
            Some("warn") => Self::Warn,
            Some("info") => Self::Info,
            Some("debug") => Self::Debug,
            Some("trace") => Self::Trace,
            Some("off") => Self::Off,
            _ => Self::Off,
        }
    }

    pub fn allows(self, message_level: LogLevel) -> bool {
        self != Self::Off && self >= message_level
    }
}
```

- [ ] **Step 4: Extend logger module with small emit helper**

```rust
use std::fmt::Arguments;

pub fn log(level: LogLevel, configured: LogLevel, target: &str, args: Arguments<'_>) {
    if !configured.allows(level) {
        return;
    }

    eprintln!("[gazeguard][{target}][{level}] {args}");
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            LogLevel::Off => "off",
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        };
        f.write_str(label)
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run:

```bash
cargo test parses_known_log_levels
cargo test missing_or_invalid_log_level_defaults_to_off
cargo test higher_levels_enable_lower_levels
```

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/logger.rs
git commit -m "feat: add internal log level helper"
```

### Task 2: Parse `log_level` from Shared Config

**Files:**
- Modify: `src-tauri/src/break_engine.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/main.rs` if module list requires explicit export
- Test: `src-tauri/src/break_engine.rs`

- [ ] **Step 1: Write failing config parsing tests**

Add tests near existing config parsing tests in `src-tauri/src/break_engine.rs`:

```rust
#[test]
fn config_defaults_log_level_to_off() {
    let yaml = r#"
short_break_interval: 15
long_break_interval: 75
long_break_duration: 60
pre_break_warning_time: 10
short_break_duration: 15
strict_break: false
"#;

    let config = BreakEngineConfig::from_yaml(yaml).unwrap();

    assert_eq!(config.log_level, crate::logger::LogLevel::Off);
}

#[test]
fn config_parses_log_level_when_present() {
    let yaml = r#"
log_level: debug
short_break_interval: 15
long_break_interval: 75
long_break_duration: 60
pre_break_warning_time: 10
short_break_duration: 15
strict_break: false
"#;

    let config = BreakEngineConfig::from_yaml(yaml).unwrap();

    assert_eq!(config.log_level, crate::logger::LogLevel::Debug);
}

#[test]
fn config_invalid_log_level_falls_back_to_off() {
    let yaml = r#"
log_level: noisy
short_break_interval: 15
long_break_interval: 75
long_break_duration: 60
pre_break_warning_time: 10
short_break_duration: 15
strict_break: false
"#;

    let config = BreakEngineConfig::from_yaml(yaml).unwrap();

    assert_eq!(config.log_level, crate::logger::LogLevel::Off);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test config_defaults_log_level_to_off
```

Expected: FAIL because `BreakEngineConfig` has no `log_level`

- [ ] **Step 3: Add raw and parsed config fields**

Update config types in `src-tauri/src/break_engine.rs`:

```rust
use crate::logger::LogLevel;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreakEngineConfig {
    pub break_interval: u64,
    pub long_break_duration: u64,
    pub no_of_short_breaks_per_long_break: u8,
    pub pre_break_warning_time: u64,
    pub short_break_duration: u64,
    pub persist_state: bool,
    pub random_order: bool,
    pub allow_postpone: bool,
    pub postpone_duration_seconds: u64,
    pub postpone_options: Vec<PostponeOption>,
    pub strict_break: bool,
    pub consecutive_skip_limit: u8,
    pub idle_time: u64,
    pub log_level: LogLevel,
    pub short_breaks: Vec<BreakTemplate>,
    pub long_breaks: Vec<BreakTemplate>,
    pub disable_options: Vec<DisableOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBreakEngineConfig {
    #[serde(default)]
    meta: Option<RawConfigMeta>,
    #[serde(default)]
    log_level: Option<String>,
    #[serde(default)]
    random_order: bool,
    #[serde(default)]
    allow_postpone: bool,
    #[serde(default)]
    persist_state: bool,
    // existing fields continue here
}
```

- [ ] **Step 4: Normalize parsed log level into runtime config**

Update `BreakEngineConfig::from_raw`:

```rust
        Self {
            break_interval: raw.short_break_interval,
            long_break_duration: raw.long_break_duration,
            no_of_short_breaks_per_long_break: breaks_per_long,
            pre_break_warning_time: raw.pre_break_warning_time,
            short_break_duration: raw.short_break_duration,
            persist_state: raw.persist_state,
            random_order: raw.random_order,
            allow_postpone: raw.allow_postpone,
            postpone_duration_seconds: postpone_seconds(raw.postpone_duration, &raw.postpone_unit),
            postpone_options: raw
                .postpone_options
                .into_iter()
                .map(|option| PostponeOption {
                    seconds: postpone_seconds(option.duration, &option.unit),
                    ..option
                })
                .collect(),
            strict_break: raw.strict_break,
            consecutive_skip_limit: raw.consecutive_skip_limit,
            idle_time: raw.idle_time,
            log_level: LogLevel::parse(raw.log_level.as_deref()),
            short_breaks: raw.short_breaks,
            long_breaks: raw.long_breaks,
            disable_options: raw.disable_options,
        }
```

- [ ] **Step 5: Export logger module from crate root**

Update module declarations in `src-tauri/src/lib.rs`:

```rust
mod logger;
```

If `src-tauri/src/main.rs` mirrors module wiring, keep it aligned with existing pattern.

- [ ] **Step 6: Run config parsing tests**

Run:

```bash
cargo test config_defaults_log_level_to_off
cargo test config_parses_log_level_when_present
cargo test config_invalid_log_level_falls_back_to_off
```

Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/break_engine.rs src-tauri/src/lib.rs src-tauri/src/main.rs
git commit -m "feat: parse global log level from config"
```

### Task 3: Add Logging to Desktop Signal Collection

**Files:**
- Modify: `src-tauri/src/desktop_signals.rs`
- Test: `src-tauri/src/desktop_signals.rs`

- [ ] **Step 1: Write failing helper-level test for merged macOS signals**

Add a focused test near existing desktop signal tests:

```rust
#[test]
fn macos_signals_from_sources_prefers_native_values_when_present() {
    let fallback = DesktopSignals {
        fullscreen_active: false,
        idle_active: false,
    };

    let merged = macos_signals_from_sources(fallback, Some(true), Some(true));

    assert_eq!(
        merged,
        DesktopSignals {
            fullscreen_active: true,
            idle_active: true,
        }
    );
}
```

- [ ] **Step 2: Run test to verify current behavior still passes**

Run:

```bash
cargo test macos_signals_from_sources_prefers_native_values_when_present
```

Expected: PASS

- [ ] **Step 3: Add small logging wrappers in desktop signal path**

Add imports and helper calls in `src-tauri/src/desktop_signals.rs`:

```rust
use crate::logger::{log, LogLevel};
```

In `desktop_window_snapshot` or immediate caller:

```rust
log(
    LogLevel::Debug,
    configured_level,
    "desktop_signals",
    format_args!("fallback window snapshot: {:?}", snapshot),
);
```

In `macOS` provider after native lookups:

```rust
log(
    LogLevel::Debug,
    configured_level,
    "desktop_signals",
    format_args!(
        "macos native signals idle={:?} fullscreen={:?}",
        native_idle,
        native_fullscreen
    ),
);
```

After merge:

```rust
log(
    LogLevel::Debug,
    configured_level,
    "desktop_signals",
    format_args!("merged desktop signals: {:?}", merged),
);
```

If helper functions cannot take config directly, thread `configured_level: LogLevel` through provider boundary with minimal signature changes.

- [ ] **Step 4: Add failure-path debug logging**

Emit on native lookup miss/failure paths:

```rust
log(
    LogLevel::Trace,
    configured_level,
    "desktop_signals",
    format_args!("macos native fullscreen unavailable, using fallback"),
);
```

Keep this in best-effort paths only. Do not change return values.

- [ ] **Step 5: Run focused desktop signal tests**

Run:

```bash
cargo test macos_signals_from_sources_prefers_native_values_when_present
cargo test idle_active_from_seconds
```

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/desktop_signals.rs
git commit -m "feat: log desktop signal collection details"
```

### Task 4: Add Logging to Signal Application Path

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing test proving apply path still updates engine**

Use existing engine-apply coverage pattern and add focused test if needed:

```rust
#[test]
fn apply_desktop_signals_updates_engine_idle_and_fullscreen_state() {
    let mut engine = BreakEngine::new(BreakEngineConfig::load());

    let status = apply_desktop_signals_to_engine(
        &mut engine,
        crate::desktop_signals::DesktopSignals {
            idle_active: true,
            fullscreen_active: true,
        },
    );

    assert!(status.postpone_reason.is_some() || matches!(status.phase, EnginePhase::Stopped | EnginePhase::Running | EnginePhase::Warning | EnginePhase::OnBreak | EnginePhase::Disabled));
    let snapshot = engine.snapshot(0);
    assert!(snapshot.idle_active);
    assert!(snapshot.fullscreen);
}
```

If this exact test already exists, reuse it as regression coverage and do not duplicate.

- [ ] **Step 2: Run existing apply-path tests before editing**

Run:

```bash
cargo test apply_desktop_signals_updates_engine_idle_and_fullscreen_state
cargo test apply_desktop_signals_clears_idle_and_fullscreen_state
```

Expected: PASS

- [ ] **Step 3: Add logger calls in `sync_desktop_window_state`**

In `src-tauri/src/lib.rs`, around collection/apply path:

```rust
let configured_level = guard.config().log_level;
let idle_threshold_seconds = guard.config().idle_time.saturating_mul(60);

crate::logger::log(
    crate::logger::LogLevel::Debug,
    configured_level,
    "desktop_signals",
    format_args!("collecting signals with idle_threshold_seconds={idle_threshold_seconds}"),
);
```

After collecting signals:

```rust
crate::logger::log(
    crate::logger::LogLevel::Debug,
    configured_level,
    "desktop_signals",
    format_args!("collected desktop signals: {:?}", signals),
);
```

After applying:

```rust
crate::logger::log(
    crate::logger::LogLevel::Debug,
    configured_level,
    "desktop_signals",
    format_args!(
        "applied desktop signals idle_active={} fullscreen_active={}",
        status.postpone_reason.as_deref() == Some("idle"),
        status.postpone_reason.as_deref() == Some("fullscreen")
    ),
);
```

Prefer logging from engine snapshot or raw signal booleans if that reads clearer than inferring from `postpone_reason`.

- [ ] **Step 4: Keep non-desktop path behavior untouched**

For `#[cfg(not(desktop))]` fallback block, preserve:

```rust
    let signals = crate::desktop_signals::DesktopSignals {
        fullscreen_active: false,
        idle_active: false,
    };
```

Do not add behavior changes there beyond optional silent logger calls.

- [ ] **Step 5: Run focused lib tests**

Run:

```bash
cargo test apply_desktop_signals_updates_engine_idle_and_fullscreen_state
cargo test apply_desktop_signals_clears_idle_and_fullscreen_state
```

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: log desktop signal application path"
```

### Task 5: End-to-End Rust Verification and Manual macOS Validation

**Files:**
- Modify: `src-tauri/config/defaults.yaml` only if you deliberately want non-`off` default during local testing, then revert before commit
- Manual validation target: runtime `config.yaml`

- [ ] **Step 1: Run targeted Rust verification**

Run:

```bash
cd src-tauri
cargo test parses_known_log_levels
cargo test missing_or_invalid_log_level_defaults_to_off
cargo test higher_levels_enable_lower_levels
cargo test config_defaults_log_level_to_off
cargo test config_parses_log_level_when_present
cargo test config_invalid_log_level_falls_back_to_off
cargo test apply_desktop_signals_updates_engine_idle_and_fullscreen_state
cargo test apply_desktop_signals_clears_idle_and_fullscreen_state
```

Expected: PASS

- [ ] **Step 2: Run full Rust test suite**

Run:

```bash
cd src-tauri
cargo test
```

Expected: PASS

- [ ] **Step 3: Set runtime `config.yaml` log level for macOS validation**

Edit runtime config file for local run to include:

```yaml
log_level: debug
```

Expected location on `macOS`:

```text
~/.config/GazeGuard/config.yaml
```

If file does not yet exist, let app create it first, then edit generated file.

- [ ] **Step 4: Launch app from terminal and capture logs**

Run from repo:

```bash
cd src-tauri
cargo tauri dev
```

Expected: app launches and terminal shows `[gazeguard][desktop_signals]...` lines once polling path runs

- [ ] **Step 5: Execute manual `macOS` validation checklist**

Manual steps:

1. Keep GazeGuard visible and actively type/move mouse.
Expected:
`idle_active` stays false in logs.

2. Stop touching keyboard/mouse long enough to exceed configured idle threshold.
Expected:
native idle sample grows, merged signal shows `idle_active=true`.

3. Resume activity.
Expected:
merged signal returns `idle_active=false`.

4. Put another app into fullscreen.
Expected:
native fullscreen signal becomes true and merged signal reflects it.

5. Put GazeGuard itself fullscreen.
Expected:
foreign fullscreen detection stays false.

6. Force any observable failure path if practical.
Expected:
fallback/no-native message appears without crashing or changing core behavior.

- [ ] **Step 6: Reset runtime logging if you do not want noisy local runs**

Edit runtime config back to:

```yaml
log_level: off
```

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/logger.rs src-tauri/src/break_engine.rs src-tauri/src/desktop_signals.rs src-tauri/src/lib.rs docs/superpowers/specs/2026-05-10-macos-validation-logger-design.md
git commit -m "feat: add config-driven desktop signal logging"
```

## Self-Review

- Spec coverage: config field, parser, logger helper, desktop signal logs, apply-path logs, and manual `macOS` validation all covered.
- Placeholder scan: no `TBD`, `TODO`, or “implement later” steps remain.
- Type consistency: `log_level`, `LogLevel`, `desktop_signals`, and apply-path names match current code and proposed files.
