# Config Reload Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add desktop runtime config watching with immediate live reload, keep the last valid config on invalid edits, show a desktop error banner, and use an explicit settings-triggered reload path on Android.

**Architecture:** Keep config parsing in the Rust engine layer, move reload orchestration into a dedicated Rust runtime helper, and expose a small reload-status/event surface to the frontend. Desktop uses a Rust-owned polling watcher thread on the resolved `config.yaml` path, while Android stays on an explicit command-driven reload path when the settings screen refreshes schedule data.

**Tech Stack:** Rust, Tauri 2, HTML/JS frontend, Android Kotlin bridge surface, standard library polling watcher

---

## File Map

- Create: `src-tauri/src/config_reload.rs`
- Modify: `src-tauri/src/break_engine.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/index.html`
- Modify: `src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/BreakEngineConfig.kt`
- Modify: `src-tauri/docs/superpowers/plans/2026-04-06-config-reload.md`

### Task 1: Add engine-level config apply/reload primitives

**Files:**
- Modify: `src-tauri/src/break_engine.rs`
- Test: `src-tauri/src/break_engine.rs`

- [ ] **Step 1: Write the failing Rust tests for applying a new config while preserving runtime state**

```rust
#[test]
fn apply_config_updates_schedule_values_and_preserves_runtime_state() {
    let mut engine = BreakEngine::new(BreakEngineConfig::load());
    engine.start();
    engine.tick(3);
    engine.set_idle(true);
    engine.set_fullscreen(true);

    let mut updated = BreakEngineConfig::load();
    updated.break_interval = 9;
    updated.pre_break_warning_time = 12;
    updated.short_break_duration = 22;
    updated.long_break_duration = 70;
    updated.idle_time = 11;

    engine.apply_config(updated.clone());

    let status = engine.status();
    let snapshot = engine.snapshot(0);
    assert!(matches!(status.phase, EnginePhase::Running));
    assert!(snapshot.idle_active);
    assert!(snapshot.fullscreen);
    assert_eq!(engine.config().break_interval, 9);
    assert_eq!(engine.config().pre_break_warning_time, 12);
    assert_eq!(engine.config().short_break_duration, 22);
    assert_eq!(engine.config().long_break_duration, 70);
    assert_eq!(engine.config().idle_time, 11);
}

#[test]
fn load_from_path_reports_yaml_errors_without_mutating_existing_engine() {
    let mut engine = BreakEngine::new(BreakEngineConfig::load());
    engine.start();
    let before = engine.config().clone();
    let temp = unique_test_dir("invalid-config");
    let config_path = temp.path().join("config.yaml");
    std::fs::write(&config_path, "not: [valid").unwrap();

    let error = BreakEngineConfig::load_or_create_from_path(&config_path, "short_break_interval: 1\n")
        .unwrap_err();

    assert!(error.contains("did not find expected"));
    assert_eq!(engine.config(), &before);
}
```

- [ ] **Step 2: Run the targeted Rust tests to verify the new engine apply path does not exist yet**

Run: `cargo test apply_config_updates_schedule_values_and_preserves_runtime_state --lib`
Expected: FAIL with a missing method error for `apply_config`.

Run: `cargo test load_from_path_reports_yaml_errors_without_mutating_existing_engine --lib`
Expected: FAIL only if the helper name does not exist yet or the assertion no longer matches the current API.

- [ ] **Step 3: Add the minimal engine apply method**

```rust
impl BreakEngine {
    pub fn apply_config(&mut self, config: BreakEngineConfig) {
        self.config = config;
        self.reconcile_phase_after_config_change();
    }

    fn reconcile_phase_after_config_change(&mut self) {
        if matches!(self.phase, EnginePhase::Stopped) {
            return;
        }

        self.work_remaining = self.work_remaining.min(self.config.break_interval.saturating_mul(60));
        self.warning_remaining = self.warning_remaining.min(self.config.pre_break_warning_time);

        if let Some(current_break) = self.current_break.as_mut() {
            let replacement_duration = match current_break.kind {
                BreakKind::Short => self.config.short_break_duration,
                BreakKind::Long => self.config.long_break_duration,
            };
            current_break.duration_seconds = replacement_duration;
            self.break_remaining = self.break_remaining.min(replacement_duration);
        }
    }
}
```

- [ ] **Step 4: Run the targeted Rust tests to verify the apply path passes**

Run: `cargo test apply_config_updates_schedule_values_and_preserves_runtime_state --lib`
Expected: PASS

Run: `cargo test load_from_path_reports_yaml_errors_without_mutating_existing_engine --lib`
Expected: PASS

- [ ] **Step 5: Commit the engine apply primitive**

```bash
git add src-tauri/src/break_engine.rs
git commit -m "feat: add live config apply support"
```

### Task 2: Add desktop reload state and a polling watcher module

**Files:**
- Create: `src-tauri/src/config_reload.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/config_reload.rs`

- [ ] **Step 1: Write the failing Rust tests for watcher reload decisions**

```rust
#[test]
fn reload_result_updates_last_good_config_on_valid_yaml() {
    let initial = BreakEngineConfig::load();
    let temp = unique_test_dir("valid-reload");
    let config_path = temp.path().join("config.yaml");
    std::fs::write(
        &config_path,
        "short_break_interval: 7\nlong_break_interval: 75\nlong_break_duration: 60\npre_break_warning_time: 10\nshort_break_duration: 15\nstrict_break: false\n",
    )
    .unwrap();
    let updated = BreakEngineConfig::load_or_create_from_path(&config_path, "short_break_interval: 1\n").unwrap();

    let state = ConfigReloadState::new(initial.clone());
    let result = state.finish_reload(Ok(updated.clone()));

    assert!(matches!(result.outcome, ConfigReloadOutcome::Reloaded));
    assert_eq!(state.last_good_config(), updated);
}

#[test]
fn reload_result_keeps_last_good_config_on_invalid_yaml() {
    let initial = BreakEngineConfig::load();
    let state = ConfigReloadState::new(initial.clone());

    let result = state.finish_reload(Err("bad yaml".to_string()));

    assert!(matches!(result.outcome, ConfigReloadOutcome::Failed));
    assert_eq!(state.last_good_config(), initial);
    assert_eq!(result.message.as_deref(), Some("Could not reload config.yaml. Using the last valid config."));
}
```

- [ ] **Step 2: Run the watcher-state tests to verify the new module does not exist yet**

Run: `cargo test reload_result_updates_last_good_config_on_valid_yaml --lib`
Expected: FAIL with unresolved import or missing type errors for `ConfigReloadState`.

- [ ] **Step 3: Create the config reload module with state and polling helpers**

```rust
use crate::break_engine::BreakEngineConfig;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigReloadOutcome {
    Reloaded,
    Failed,
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigReloadResult {
    pub outcome: ConfigReloadOutcome,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConfigReloadState {
    last_good_config: Arc<Mutex<BreakEngineConfig>>,
    last_error: Arc<Mutex<Option<String>>>,
}

impl ConfigReloadState {
    pub fn new(initial: BreakEngineConfig) -> Self {
        Self {
            last_good_config: Arc::new(Mutex::new(initial)),
            last_error: Arc::new(Mutex::new(None)),
        }
    }

    pub fn last_good_config(&self) -> BreakEngineConfig {
        self.last_good_config.lock().unwrap().clone()
    }

    pub fn finish_reload(&self, result: Result<BreakEngineConfig, String>) -> ConfigReloadResult {
        match result {
            Ok(config) => {
                *self.last_good_config.lock().unwrap() = config;
                *self.last_error.lock().unwrap() = None;
                ConfigReloadResult { outcome: ConfigReloadOutcome::Reloaded, message: None }
            }
            Err(_) => {
                let message = "Could not reload config.yaml. Using the last valid config.".to_string();
                *self.last_error.lock().unwrap() = Some(message.clone());
                ConfigReloadResult { outcome: ConfigReloadOutcome::Failed, message: Some(message) }
            }
        }
    }
}

pub fn file_mtime(path: &Path) -> Result<Option<SystemTime>, String> {
    match std::fs::metadata(path) {
        Ok(metadata) => metadata.modified().map(Some).map_err(|error| error.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

pub fn should_reload(previous: Option<SystemTime>, current: Option<SystemTime>) -> bool {
    current.is_some() && current != previous
}
```

- [ ] **Step 4: Register the reload module and add a desktop watcher spawner in `lib.rs`**

```rust
mod break_engine;
mod config_file;
mod config_reload;
mod desktop_signals;

#[cfg(desktop)]
fn spawn_config_watcher(
    app: tauri::AppHandle,
    engine: SharedBreakEngine,
    config_path: PathBuf,
    state: crate::config_reload::ConfigReloadState,
) {
    thread::spawn(move || {
        let mut last_seen = crate::config_reload::file_mtime(&config_path).ok().flatten();
        loop {
            thread::sleep(Duration::from_millis(750));
            let current = match crate::config_reload::file_mtime(&config_path) {
                Ok(value) => value,
                Err(error) => {
                    let _ = app.emit("config-reload-error", json!({ "message": error }));
                    continue;
                }
            };
            if !crate::config_reload::should_reload(last_seen, current) {
                continue;
            }
            last_seen = current;
            let result = BreakEngineConfig::load_or_create_from_path(
                &config_path,
                include_str!("../config/defaults.yaml"),
            );
            let reload = state.finish_reload(result);
            match reload.outcome {
                crate::config_reload::ConfigReloadOutcome::Reloaded => {
                    if let Ok(mut guard) = engine.lock() {
                        guard.apply_config(state.last_good_config());
                    }
                    let _ = app.emit("config-reload-success", json!({}));
                }
                crate::config_reload::ConfigReloadOutcome::Failed => {
                    let _ = app.emit("config-reload-error", json!({
                        "message": reload.message.unwrap_or_else(|| "Could not reload config.yaml. Using the last valid config.".to_string())
                    }));
                }
                crate::config_reload::ConfigReloadOutcome::Unchanged => {}
            }
        }
    });
}
```

- [ ] **Step 5: Run the watcher-state tests to verify the module passes**

Run: `cargo test reload_result_updates_last_good_config_on_valid_yaml --lib`
Expected: PASS

Run: `cargo test reload_result_keeps_last_good_config_on_invalid_yaml --lib`
Expected: PASS

- [ ] **Step 6: Commit the watcher state layer**

```bash
git add src-tauri/src/config_reload.rs src-tauri/src/lib.rs
git commit -m "feat: add desktop config reload watcher"
```

### Task 3: Expose reload events and an explicit Android reload command

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/BreakEngineConfig.kt`
- Test: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write the failing Rust test for explicit runtime config reload**

```rust
#[test]
fn reload_runtime_config_command_updates_registered_engine() {
    let temp = unique_test_dir("reload-runtime-config");
    let config_path = temp.join("config.yaml");
    fs::write(
        &config_path,
        "short_break_interval: 15\nlong_break_interval: 75\nlong_break_duration: 60\npre_break_warning_time: 10\nshort_break_duration: 15\nstrict_break: false\n",
    )
    .unwrap();

    let engine = Arc::new(Mutex::new(BreakEngine::new(BreakEngineConfig::load())));
    set_shared_break_engine_for_tests(Some(engine.clone()));

    let result = reload_runtime_config_for_tests(engine.clone(), &config_path).unwrap();

    assert_eq!(result.break_interval_minutes, 15);
}
```

- [ ] **Step 2: Run the targeted Rust test to verify the reload command helper is missing**

Run: `cargo test reload_runtime_config_command_updates_registered_engine --lib`
Expected: FAIL with a missing helper error for `reload_runtime_config_for_tests`.

- [ ] **Step 3: Add a shared reload helper and Tauri command in `lib.rs`**

```rust
fn reload_runtime_config_into_engine(
    engine: &SharedBreakEngine,
    config_path: &Path,
) -> Result<BreakSchedule, String> {
    let config = BreakEngineConfig::load_or_create_from_path(
        config_path,
        include_str!("../config/defaults.yaml"),
    )?;

    let schedule = BreakSchedule {
        break_interval_minutes: config.break_interval,
        pre_break_warning_seconds: config.pre_break_warning_time,
        disable_options: config.disable_options.clone(),
        postpone_options: config.postpone_options.clone(),
    };

    let mut guard = engine.lock().map_err(|_| "State lock poisoned".to_string())?;
    guard.apply_config(config);
    Ok(schedule)
}

#[tauri::command]
fn reload_runtime_config(state: State<'_, SharedBreakEngine>) -> Result<BreakSchedule, String> {
    let app_data_dir = get_snapshot_app_data_dir().ok_or_else(|| "App data dir unavailable".to_string())?;
    let path = runtime_config_path(app_data_dir.as_path())?;
    let engine = state.inner().clone();
    reload_runtime_config_into_engine(&engine, &path)
}
```

- [ ] **Step 4: Keep Android on explicit reload by leaving `BreakEngineConfig.loadSchedule(context)` file-backed and only documenting that settings opens must call the new command**

```kotlin
fun loadSchedule(context: Context): Schedule {
    val defaultYaml = context.assets.open(CONFIG_ASSET_PATH).bufferedReader().use { it.readText() }
    val configFile = ensureConfigFile(resolveConfigFile(context), defaultYaml)
    return parseSchedule(configFile.readText())
}
```

- [ ] **Step 5: Run the targeted Rust test to verify the explicit reload path passes**

Run: `cargo test reload_runtime_config_command_updates_registered_engine --lib`
Expected: PASS

- [ ] **Step 6: Commit the explicit reload command**

```bash
git add src-tauri/src/lib.rs src-tauri/gen/android/app/src/main/java/com/reus/gazeguard/BreakEngineConfig.kt
git commit -m "feat: add explicit runtime config reload command"
```

### Task 4: Show desktop reload errors in the frontend and clear them on success

**Files:**
- Modify: `src/index.html`
- Test: manual desktop verification

- [ ] **Step 1: Add the failing UI behavior checkpoint by confirming no config reload banner exists**

Run: `rg -n "config-reload|configReload|error banner" src/index.html`
Expected: no matches

- [ ] **Step 2: Add a banner container and event listeners in `src/index.html`**

```html
  <div id="configReloadBanner" style="display:none; margin-top: 16px; padding: 14px; border-radius: 10px; background: #ffe5e5; border: 1px solid #d99;">
    <strong>Config error:</strong> <span id="configReloadBannerText"></span>
  </div>
```

```javascript
    const { invoke } = window.__TAURI__.core;
    const { listen } = window.__TAURI__.event;
    const configReloadBannerEl = document.getElementById('configReloadBanner');
    const configReloadBannerTextEl = document.getElementById('configReloadBannerText');

    function showConfigReloadBanner(message) {
      configReloadBannerEl.style.display = 'block';
      configReloadBannerTextEl.textContent = message;
    }

    function hideConfigReloadBanner() {
      configReloadBannerEl.style.display = 'none';
      configReloadBannerTextEl.textContent = '';
    }

    listen('config-reload-error', (event) => {
      showConfigReloadBanner(event.payload?.message ?? 'Could not reload config.yaml. Using the last valid config.');
    });

    listen('config-reload-success', async () => {
      hideConfigReloadBanner();
      await syncEngineStatus();
    });
```

- [ ] **Step 3: Use the explicit reload command when the settings screen refreshes schedule data on Android**

```javascript
    async function loadScheduleForUi() {
      if (window.AndroidBridge) {
        return invoke('reload_runtime_config');
      }
      return invoke('get_break_schedule');
    }
```

- [ ] **Step 4: Replace direct `get_break_schedule` calls with the shared helper**

```javascript
      const schedule = await loadScheduleForUi();
```

- [ ] **Step 5: Run the targeted grep check to verify the new banner/event wiring exists**

Run: `rg -n "config-reload-error|config-reload-success|loadScheduleForUi|configReloadBanner" src/index.html`
Expected: matches for the new event listeners, helper, and banner nodes.

- [ ] **Step 6: Commit the frontend reload feedback**

```bash
git add src/index.html
git commit -m "feat: show config reload status in desktop ui"
```

### Task 5: Verify the full reload flow and clean up stale assumptions

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/index.html`
- Test: `src-tauri/src/break_engine.rs`
- Test: `src-tauri/src/config_reload.rs`
- Test: manual desktop and Android checks

- [ ] **Step 1: Run the focused Rust verification suite**

Run: `cargo test apply_config_updates_schedule_values_and_preserves_runtime_state --lib`
Expected: PASS

Run: `cargo test reload_result_updates_last_good_config_on_valid_yaml --lib`
Expected: PASS

Run: `cargo test reload_result_keeps_last_good_config_on_invalid_yaml --lib`
Expected: PASS

Run: `cargo test reload_runtime_config_command_updates_registered_engine --lib`
Expected: PASS

- [ ] **Step 2: Run the existing config bootstrap verification to catch regressions**

Run: `cargo test load_or_create_from_path_reads_seeded_yaml_file --lib`
Expected: PASS

Run: `cargo test ensure_config_file_ --lib`
Expected: PASS

- [ ] **Step 3: Run the Android config unit tests**

Run: `cd src-tauri/gen/android && ./gradlew app:testUniversalDebugUnitTest --tests com.reus.gazeguard.BreakEngineConfigTest`
Expected: PASS

- [ ] **Step 4: Perform desktop manual validation**

1. Start the desktop app.
2. Edit `~/.config/GazeGuard/config.yaml` or the platform-equivalent resolved path.
3. Save a valid `short_break_interval` change.
4. Confirm the countdown/schedule updates without restarting.
5. Save invalid YAML.
6. Confirm an error banner appears and the timer keeps using the previous valid config.
7. Fix the YAML.
8. Confirm the banner clears and the new config applies.

- [ ] **Step 5: Perform Android manual validation**

1. Launch the Android app.
2. Modify `filesDir/config/config.yaml`.
3. Open the settings screen that loads schedule data.
4. Confirm the updated values are shown without adding a background watcher.

- [ ] **Step 6: Commit the verification pass**

```bash
git add src-tauri/src/config_reload.rs src-tauri/src/break_engine.rs src-tauri/src/lib.rs src/index.html
git commit -m "test: verify desktop live config reload flow"
```
