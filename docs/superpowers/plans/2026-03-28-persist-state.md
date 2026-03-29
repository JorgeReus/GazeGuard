# Persist State Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `persist_state` so the Rust break engine can be saved to app-private storage and restored across relaunches using real elapsed wall-clock time.

**Architecture:** Keep `BreakEngine` as the owner of runtime scheduling state and add explicit snapshot export/import helpers in `src-tauri/src/break_engine.rs`. Put filesystem persistence and startup restore wiring in `src-tauri/src/lib.rs`, so disk I/O stays outside the engine while the engine remains the single authority for restore semantics.

**Tech Stack:** Rust, Tauri, serde, serde_json, std filesystem APIs, cargo test

---

### Task 1: Add Config Parsing And Engine Snapshot Types

**Files:**
- Modify: `src-tauri/src/break_engine.rs`
- Test: `src-tauri/src/break_engine.rs`

- [ ] **Step 1: Write the failing config test for `persist_state` parsing**

```rust
#[test]
fn loads_persist_state_from_yaml() {
    let yaml = r#"
random_order: false
allow_postpone: false
short_break_interval: 15
long_break_interval: 75
long_break_duration: 60
pre_break_warning_time: 10
short_break_duration: 15
persist_state: true
strict_break: false
consecutive_skip_limit: 2
idle_time: 5
"#;

    let config = BreakEngineConfig::from_yaml(yaml).unwrap();

    assert!(config.persist_state);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test loads_persist_state_from_yaml -- --exact`
Expected: FAIL because `BreakEngineConfig` and `RawBreakEngineConfig` do not expose `persist_state`.

- [ ] **Step 3: Write the minimal config implementation**

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreakEngineConfig {
    pub break_interval: u64,
    pub long_break_duration: u64,
    pub no_of_short_breaks_per_long_break: u8,
    pub pre_break_warning_time: u64,
    pub short_break_duration: u64,
    pub random_order: bool,
    pub allow_postpone: bool,
    pub persist_state: bool,
    pub postpone_duration_seconds: u64,
    pub postpone_options: Vec<PostponeOption>,
    pub strict_break: bool,
    pub consecutive_skip_limit: u8,
    pub idle_time: u64,
    pub short_breaks: Vec<BreakTemplate>,
    pub long_breaks: Vec<BreakTemplate>,
    pub disable_options: Vec<DisableOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RawBreakEngineConfig {
    #[serde(default)]
    random_order: bool,
    #[serde(default)]
    allow_postpone: bool,
    #[serde(default)]
    persist_state: bool,
    // existing fields stay in place
}

impl BreakEngineConfig {
    fn from_raw(raw: RawBreakEngineConfig) -> Self {
        let breaks_per_long = if raw.short_break_interval == 0 {
            0
        } else {
            raw.long_break_interval
                .saturating_div(raw.short_break_interval)
                .saturating_sub(1) as u8
        };

        Self {
            break_interval: raw.short_break_interval,
            long_break_duration: raw.long_break_duration,
            no_of_short_breaks_per_long_break: breaks_per_long,
            pre_break_warning_time: raw.pre_break_warning_time,
            short_break_duration: raw.short_break_duration,
            random_order: raw.random_order,
            allow_postpone: raw.allow_postpone,
            persist_state: raw.persist_state,
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
            short_breaks: raw.short_breaks,
            long_breaks: raw.long_breaks,
            disable_options: raw.disable_options,
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test loads_persist_state_from_yaml -- --exact`
Expected: PASS

- [ ] **Step 5: Write the failing snapshot round-trip test**

```rust
#[test]
fn snapshot_round_trip_preserves_runtime_state() {
    let mut config = BreakEngineConfig::load();
    config.random_order = false;
    let mut engine = BreakEngine::new(config);
    engine.start();
    engine.set_idle(true);
    engine.advance_by(17);
    engine.begin_break_now();

    let snapshot = engine.snapshot(1_700_000_000);
    let restored = BreakEngine::from_snapshot(engine.config().clone(), snapshot);

    assert_eq!(restored.snapshot(1_700_000_000), engine.snapshot(1_700_000_000));
}
```

- [ ] **Step 6: Run test to verify it fails**

Run: `cd src-tauri && cargo test snapshot_round_trip_preserves_runtime_state -- --exact`
Expected: FAIL because `snapshot` and `from_snapshot` do not exist yet.

- [ ] **Step 7: Write the minimal snapshot model**

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreakEngineSnapshot {
    pub was_started: bool,
    pub phase: EnginePhase,
    pub work_remaining: u64,
    pub warning_remaining: u64,
    pub break_remaining: u64,
    pub disabled_remaining: u64,
    pub shorts_since_long: u8,
    pub next_short_index: usize,
    pub next_long_index: usize,
    pub short_break_order: Vec<usize>,
    pub long_break_order: Vec<usize>,
    pub current_break: Option<BreakInfo>,
    pub idle_active: bool,
    pub idle_elapsed_seconds: u64,
    pub fullscreen: bool,
    pub consecutive_skips: u8,
    pub saved_at_unix_seconds: u64,
}

impl BreakEngine {
    pub fn snapshot(&self, saved_at_unix_seconds: u64) -> BreakEngineSnapshot {
        BreakEngineSnapshot {
            was_started: self.last_synced_at.is_some(),
            phase: self.phase.clone(),
            work_remaining: self.work_remaining,
            warning_remaining: self.warning_remaining,
            break_remaining: self.break_remaining,
            disabled_remaining: self.disabled_remaining,
            shorts_since_long: self.shorts_since_long,
            next_short_index: self.next_short_index,
            next_long_index: self.next_long_index,
            short_break_order: self.short_break_order.clone(),
            long_break_order: self.long_break_order.clone(),
            current_break: self.current_break.clone(),
            idle_active: self.idle_active,
            idle_elapsed_seconds: self.idle_elapsed_seconds,
            fullscreen: self.fullscreen,
            consecutive_skips: self.consecutive_skips,
            saved_at_unix_seconds,
        }
    }

    pub fn from_snapshot(config: BreakEngineConfig, snapshot: BreakEngineSnapshot) -> Self {
        Self {
            config,
            phase: snapshot.phase,
            work_remaining: snapshot.work_remaining,
            warning_remaining: snapshot.warning_remaining,
            break_remaining: snapshot.break_remaining,
            disabled_remaining: snapshot.disabled_remaining,
            shorts_since_long: snapshot.shorts_since_long,
            next_short_index: snapshot.next_short_index,
            next_long_index: snapshot.next_long_index,
            short_break_order: snapshot.short_break_order,
            long_break_order: snapshot.long_break_order,
            current_break: snapshot.current_break,
            idle_active: snapshot.idle_active,
            idle_elapsed_seconds: snapshot.idle_elapsed_seconds,
            fullscreen: snapshot.fullscreen,
            last_synced_at: if snapshot.was_started { Some(Instant::now()) } else { None },
            consecutive_skips: snapshot.consecutive_skips,
            shuffle_rng: fastrand::Rng::new(),
        }
    }
}
```

- [ ] **Step 8: Run test to verify it passes**

Run: `cd src-tauri && cargo test snapshot_round_trip_preserves_runtime_state -- --exact`
Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/break_engine.rs
git commit -m "feat: add persist state snapshot model"
```

### Task 2: Add Restore-By-Elapsed-Time Engine Tests And Helpers

**Files:**
- Modify: `src-tauri/src/break_engine.rs`
- Test: `src-tauri/src/break_engine.rs`

- [ ] **Step 1: Write the failing restore test for running timers**

```rust
#[test]
fn restore_running_snapshot_applies_elapsed_wall_clock_time() {
    let mut engine = BreakEngine::new(BreakEngineConfig::load());
    engine.start();
    let before = engine.status().seconds_remaining.unwrap();
    let snapshot = engine.snapshot(1_000);

    let mut restored = BreakEngine::from_snapshot(engine.config().clone(), snapshot);
    restored.restore_elapsed(12);

    let after = restored.status().seconds_remaining.unwrap();
    assert_eq!(after, before - 12);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test restore_running_snapshot_applies_elapsed_wall_clock_time -- --exact`
Expected: FAIL because `restore_elapsed` does not exist.

- [ ] **Step 3: Write the failing restore transition tests**

```rust
#[test]
fn restore_warning_snapshot_enters_break_when_elapsed_crosses_zero() {
    let mut engine = BreakEngine::new(BreakEngineConfig::load());
    engine.start();
    let work_seconds = engine.config().break_interval * 60;
    let warning_seconds = engine.config().pre_break_warning_time;
    let status = engine.advance_by(work_seconds - warning_seconds);
    assert!(matches!(status.phase, EnginePhase::Warning));

    let snapshot = engine.snapshot(1_000);
    let mut restored = BreakEngine::from_snapshot(engine.config().clone(), snapshot);
    let status = restored.restore_elapsed(warning_seconds);

    assert!(matches!(status.phase, EnginePhase::OnBreak));
}

#[test]
fn restore_disabled_snapshot_expires_when_elapsed_is_long_enough() {
    let mut engine = BreakEngine::new(BreakEngineConfig::load());
    engine.start();
    engine.disable_for(30).unwrap();
    let snapshot = engine.snapshot(1_000);

    let mut restored = BreakEngine::from_snapshot(engine.config().clone(), snapshot);
    let status = restored.restore_elapsed(30);

    assert!(matches!(status.phase, EnginePhase::Running));
}
```

- [ ] **Step 4: Run test to verify they fail**

Run: `cd src-tauri && cargo test restore_warning_snapshot_enters_break_when_elapsed_crosses_zero restore_disabled_snapshot_expires_when_elapsed_is_long_enough -- --exact`
Expected: FAIL because restore helpers are missing.

- [ ] **Step 5: Write the minimal restore helper**

```rust
impl BreakEngine {
    pub fn restore_elapsed(&mut self, elapsed_seconds: u64) -> EngineStatus {
        if self.last_synced_at.is_some() {
            self.last_synced_at = Some(Instant::now());
        }
        self.advance_by_seconds(elapsed_seconds);
        self.reconcile();
        self.status()
    }
}
```

- [ ] **Step 6: Run targeted tests to verify they pass**

Run: `cd src-tauri && cargo test restore_running_snapshot_applies_elapsed_wall_clock_time -- --exact`
Expected: PASS

Run: `cd src-tauri && cargo test restore_warning_snapshot_enters_break_when_elapsed_crosses_zero -- --exact`
Expected: PASS

Run: `cd src-tauri && cargo test restore_disabled_snapshot_expires_when_elapsed_is_long_enough -- --exact`
Expected: PASS

- [ ] **Step 7: Add the remaining failing tests for break completion and `persist_state: false` behavior**

```rust
#[test]
fn restore_active_break_snapshot_completes_break_when_elapsed_is_long_enough() {
    let mut engine = BreakEngine::new(BreakEngineConfig::load());
    engine.start();
    engine.begin_break_now();
    let break_seconds = engine.current_break().unwrap().duration_seconds;
    let snapshot = engine.snapshot(1_000);

    let mut restored = BreakEngine::from_snapshot(engine.config().clone(), snapshot);
    let status = restored.restore_elapsed(break_seconds);

    assert!(matches!(status.phase, EnginePhase::Running));
}
```

- [ ] **Step 8: Run the new test to verify it fails for the right reason**

Run: `cd src-tauri && cargo test restore_active_break_snapshot_completes_break_when_elapsed_is_long_enough -- --exact`
Expected: FAIL only if restore logic still mishandles break completion; otherwise PASS with no further code changes needed.

- [ ] **Step 9: Make the smallest follow-up adjustment if the break-completion test fails**

```rust
fn finish_break_cycle(&mut self) {
    self.phase = EnginePhase::Running;
    self.current_break = None;
    self.break_remaining = 0;
    self.warning_remaining = 0;
    self.work_remaining = self.config.break_interval.saturating_mul(60);
}
```

- [ ] **Step 10: Run the focused engine persistence test group**

Run: `cd src-tauri && cargo test restore_ -- --nocapture`
Expected: PASS for all restore-focused tests

- [ ] **Step 11: Commit**

```bash
git add src-tauri/src/break_engine.rs
git commit -m "feat: add elapsed restore support"
```

### Task 3: Add Snapshot File Persistence In Tauri Bootstrap

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/break_engine.rs`
- Test: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write the failing lib test for loading a saved snapshot**

```rust
#[test]
fn create_break_engine_restores_saved_snapshot_when_persist_state_is_enabled() {
    let temp = std::env::temp_dir().join("gazeguard-persist-state-restore");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();
    let mut config = crate::break_engine::BreakEngineConfig::load();
    config.persist_state = true;

    let mut engine = crate::break_engine::BreakEngine::new(config.clone());
    engine.start();
    engine.advance_by(25);

    let snapshot_path = temp.join("break-engine.json");
    std::fs::write(
        &snapshot_path,
        serde_json::to_vec(&engine.snapshot(1_000)).unwrap(),
    )
    .unwrap();

    let restored = create_break_engine_for_tests(config, &snapshot_path, 1_025);
    let mut guard = restored.lock().unwrap();

    assert!(guard.status().seconds_remaining.unwrap() < config.break_interval * 60);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test create_break_engine_restores_saved_snapshot_when_persist_state_is_enabled -- --exact`
Expected: FAIL because `create_break_engine_for_tests` and snapshot-path injection do not exist.

- [ ] **Step 3: Add a persistence wrapper and injectable startup helper**

```rust
fn snapshot_path(app_data_dir: &std::path::Path) -> std::path::PathBuf {
    app_data_dir.join("break-engine-state.json")
}

fn load_engine_from_disk(
    config: BreakEngineConfig,
    path: &std::path::Path,
    now_unix_seconds: u64,
) -> BreakEngine {
    if !config.persist_state {
        let mut engine = BreakEngine::new(config);
        engine.start();
        return engine;
    }

    let snapshot = std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<break_engine::BreakEngineSnapshot>(&bytes).ok());

    match snapshot {
        Some(snapshot) => {
            let elapsed = now_unix_seconds.saturating_sub(snapshot.saved_at_unix_seconds);
            let mut engine = BreakEngine::from_snapshot(config, snapshot);
            engine.restore_elapsed(elapsed);
            engine
        }
        None => {
            let mut engine = BreakEngine::new(config);
            engine.start();
            engine
        }
    }
}

#[cfg(test)]
fn create_break_engine_for_tests(
    config: BreakEngineConfig,
    path: &std::path::Path,
    now_unix_seconds: u64,
) -> SharedBreakEngine {
    Arc::new(Mutex::new(load_engine_from_disk(config, path, now_unix_seconds)))
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test create_break_engine_restores_saved_snapshot_when_persist_state_is_enabled -- --exact`
Expected: PASS

- [ ] **Step 5: Write the failing lib tests for disabled persistence and corrupt snapshots**

```rust
#[test]
fn create_break_engine_ignores_saved_snapshot_when_persist_state_is_disabled() {
    let temp = std::env::temp_dir().join("gazeguard-persist-state-disabled");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();
    let mut config = crate::break_engine::BreakEngineConfig::load();
    config.persist_state = false;

    std::fs::write(temp.join("break-engine.json"), br#"{"phase":"warning"}"#).unwrap();

    let engine = create_break_engine_for_tests(
        config,
        &temp.join("break-engine.json"),
        2_000,
    );
    let mut guard = engine.lock().unwrap();

    assert!(matches!(guard.status().phase, EnginePhase::Running));
}

#[test]
fn create_break_engine_falls_back_to_fresh_state_when_snapshot_is_corrupt() {
    let temp = std::env::temp_dir().join("gazeguard-persist-state-corrupt");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();
    let mut config = crate::break_engine::BreakEngineConfig::load();
    config.persist_state = true;

    std::fs::write(temp.join("break-engine.json"), b"not json").unwrap();

    let engine = create_break_engine_for_tests(
        config.clone(),
        &temp.join("break-engine.json"),
        2_000,
    );
    let mut guard = engine.lock().unwrap();

    assert!(matches!(guard.status().phase, EnginePhase::Running));
    assert_eq!(guard.status().seconds_remaining, Some(config.break_interval * 60));
}
```

- [ ] **Step 6: Run tests to verify they fail**

Run: `cd src-tauri && cargo test create_break_engine_ignores_saved_snapshot_when_persist_state_is_disabled -- --exact`
Expected: FAIL until the disabled path explicitly ignores snapshots.

Run: `cd src-tauri && cargo test create_break_engine_falls_back_to_fresh_state_when_snapshot_is_corrupt -- --exact`
Expected: FAIL until corrupt input falls back cleanly.

- [ ] **Step 7: Add explicit disabled-mode and corrupt-file handling**

```rust
fn load_engine_from_disk(
    config: BreakEngineConfig,
    path: &std::path::Path,
    now_unix_seconds: u64,
) -> BreakEngine {
    if !config.persist_state {
        let mut engine = BreakEngine::new(config);
        engine.start();
        return engine;
    }

    match std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<break_engine::BreakEngineSnapshot>(&bytes).ok())
    {
        Some(snapshot) => {
            let elapsed = now_unix_seconds.saturating_sub(snapshot.saved_at_unix_seconds);
            let mut engine = BreakEngine::from_snapshot(config, snapshot);
            engine.restore_elapsed(elapsed);
            engine
        }
        None => {
            let mut engine = BreakEngine::new(config);
            engine.start();
            engine
        }
    }
}
```

- [ ] **Step 8: Add the snapshot save helper and mutation hook**

```rust
fn save_engine_snapshot(
    engine: &BreakEngine,
    path: &std::path::Path,
    now_unix_seconds: u64,
) -> Result<(), String> {
    let snapshot = engine.snapshot(now_unix_seconds);
    let bytes = serde_json::to_vec(&snapshot).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}
```

- [ ] **Step 9: Wire the runtime helper into startup and command mutation points**

```rust
fn create_break_engine(app_data_dir: &std::path::Path) -> SharedBreakEngine {
    let config = BreakEngineConfig::load();
    let path = snapshot_path(app_data_dir);
    let now = unix_now_seconds();
    let engine = Arc::new(Mutex::new(load_engine_from_disk(config, &path, now)));
    register_shared_break_engine(engine.clone());
    engine
}

fn with_engine_mutation<T>(
    state: &State<'_, SharedBreakEngine>,
    app_data_dir: &std::path::Path,
    f: impl FnOnce(&mut BreakEngine) -> Result<T, String>,
) -> Result<T, String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    let result = f(&mut guard)?;
    if guard.config().persist_state {
        let _ = save_engine_snapshot(&guard, &snapshot_path(app_data_dir), unix_now_seconds());
    }
    Ok(result)
}
```

- [ ] **Step 10: Run the focused lib persistence tests**

Run: `cd src-tauri && cargo test create_break_engine_ -- --nocapture`
Expected: PASS for restore, disabled-mode, and corrupt-file tests

- [ ] **Step 11: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/break_engine.rs
git commit -m "feat: persist break engine state across relaunch"
```

### Task 4: Run Full Verification And Clean Up

**Files:**
- Modify: `src-tauri/src/break_engine.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/break_engine.rs`
- Test: `src-tauri/src/lib.rs`

- [ ] **Step 1: Run the full Rust test suite**

Run: `cd src-tauri && cargo test`
Expected: PASS

- [ ] **Step 2: Replace any brittle full-snapshot equality assertions with field-level assertions if needed**

```rust
assert_eq!(snapshot.phase, restored_snapshot.phase);
assert_eq!(snapshot.work_remaining, restored_snapshot.work_remaining);
assert_eq!(snapshot.short_break_order, restored_snapshot.short_break_order);
assert_eq!(snapshot.long_break_order, restored_snapshot.long_break_order);
assert_eq!(snapshot.current_break, restored_snapshot.current_break);
```

- [ ] **Step 3: Re-run the full Rust test suite**

Run: `cd src-tauri && cargo test`
Expected: PASS

- [ ] **Step 4: Inspect the final diff for accidental scope creep**

Run: `git diff -- src-tauri/src/break_engine.rs src-tauri/src/lib.rs`
Expected: only `persist_state` config parsing, snapshot types/helpers, persistence wiring, and tests

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/break_engine.rs src-tauri/src/lib.rs
git commit -m "test: verify persist state implementation"
```
