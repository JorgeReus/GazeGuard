# Config File Location Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the generated `gen/.../defaults.yaml` runtime dependency with a real per-platform config file that is created on first run and loaded from disk afterward.

**Architecture:** Move the canonical YAML template into the repo-level `config/` directory, add a small Rust config bootstrap module to resolve and seed desktop config files, and make Android seed and read an app-private `filesDir/config/config.yaml` from the same canonical YAML bundled as an asset. Keep `BreakEngineConfig` parsing unchanged and isolate file bootstrap logic from engine logic.

**Tech Stack:** Rust, Tauri 2, Kotlin, Android Gradle, `serde_yaml`, JUnit 4

---

## File Map

- Create: `config/defaults.yaml`
- Create: `src/config_file.rs`
- Create: `docs/superpowers/plans/2026-04-04-config-file-location.md`
- Modify: `src/lib.rs`
- Modify: `src/break_engine.rs`
- Modify: `gen/android/app/build.gradle.kts`
- Modify: `gen/android/app/src/main/java/com/reus/gazeguard/BreakEngineConfig.kt`
- Modify: `gen/android/app/src/test/java/com/reus/gazeguard/BreakEngineConfigTest.kt`
- Delete: `gen/android/app/src/main/assets/config/defaults.yaml`

### Task 1: Move the canonical default YAML into `config/`

**Files:**
- Create: `config/defaults.yaml`
- Delete: `gen/android/app/src/main/assets/config/defaults.yaml`
- Test: `src/break_engine.rs`

- [ ] **Step 1: Write the failing Rust test that reads the canonical YAML from `config/defaults.yaml`**

```rust
#[test]
fn loads_yaml_defaults_shape() {
    let config = BreakEngineConfig::load_from_embedded_defaults().unwrap();

    assert_eq!(config.break_interval, 1);
    assert_eq!(config.pre_break_warning_time, 10);
    assert_eq!(config.short_break_duration, 15);
    assert_eq!(config.long_break_duration, 60);
    assert_eq!(config.no_of_short_breaks_per_long_break, 74);
    assert_eq!(config.idle_time, 5);
    assert!(!config.strict_break);
    assert!(config.allow_postpone);
    assert_eq!(config.postpone_duration_seconds, 5 * 60);
    assert_eq!(config.postpone_options.len(), 3);
    assert_eq!(config.short_breaks[0].name, "Gently close your eyes");
    assert_eq!(config.long_breaks[0].name, "Walk for a while");
}
```

- [ ] **Step 2: Run the Rust test to verify it fails because the helper does not exist yet**

Run: `cargo test loads_yaml_defaults_shape --lib`
Expected: FAIL with an error about `load_from_embedded_defaults` not existing.

- [ ] **Step 3: Add `config/defaults.yaml` by copying the current YAML byte-for-byte**

```yaml
meta:
  config_version: "6.0.4"
random_order: true
allow_postpone: true
short_break_interval: 1
long_break_interval: 75
long_break_duration: 60
pre_break_warning_time: 10
short_break_duration: 15
persist_state: true
postpone_duration: 5
postpone_unit: minutes
postpone_options:
  - duration: 5
    unit: minutes
  - duration: 10
    unit: minutes
  - duration: 15
    unit: minutes
strict_break: false
consecutive_skip_limit: 2
idle_time: 5
disable_options:
  - label: for_x_minutes
    time: 30
    unit: minute
  - label: for_x_hour
    time: 1
    unit: hour
  - label: for_x_hours
    time: 2
    unit: hour
  - label: for_x_hours
    time: 3
    unit: hour
short_breaks:
  - name: Gently close your eyes
  - name: Roll your eyes a few times to each side
  - name: Rotate your eyes in clockwise direction
  - name: Rotate your eyes in counterclockwise direction
  - name: Blink your eyes
  - name: Focus on a point in the far distance
  - name: Have some water
long_breaks:
  - name: Walk for a while
  - name: Lean back at your seat and relax
```

- [ ] **Step 4: Point the embedded Rust default loader at the new canonical path**

```rust
impl BreakEngineConfig {
    pub fn load_from_embedded_defaults() -> Result<Self, serde_yaml::Error> {
        Self::from_yaml(include_str!("../config/defaults.yaml"))
    }
}
```

- [ ] **Step 5: Run the Rust test to verify it passes from the new path**

Run: `cargo test loads_yaml_defaults_shape --lib`
Expected: PASS

- [ ] **Step 6: Remove the old generated asset file**

```bash
rm gen/android/app/src/main/assets/config/defaults.yaml
```

- [ ] **Step 7: Commit the canonical default move**

```bash
git add config/defaults.yaml src/break_engine.rs gen/android/app/src/main/assets/config/defaults.yaml
git commit -m "refactor: move defaults yaml out of generated assets"
```

### Task 2: Add desktop config bootstrap and load from the seeded file

**Files:**
- Create: `src/config_file.rs`
- Modify: `src/lib.rs`
- Modify: `src/break_engine.rs`
- Test: `src/config_file.rs`
- Test: `src/break_engine.rs`

- [ ] **Step 1: Write the failing Rust tests for file bootstrap behavior**

```rust
#[test]
fn ensure_config_file_writes_defaults_when_missing() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join(".config/GazeGuard/config.yaml");

    let resolved = ensure_config_file(&config_path, "short_break_interval: 1\n").unwrap();

    assert_eq!(resolved, config_path);
    assert_eq!(std::fs::read_to_string(&config_path).unwrap(), "short_break_interval: 1\n");
}

#[test]
fn ensure_config_file_keeps_existing_contents() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join(".config/GazeGuard/config.yaml");
    std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    std::fs::write(&config_path, "short_break_interval: 99\n").unwrap();

    ensure_config_file(&config_path, "short_break_interval: 1\n").unwrap();

    assert_eq!(std::fs::read_to_string(&config_path).unwrap(), "short_break_interval: 99\n");
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn desktop_config_path_uses_dot_config_directory() {
    let path = desktop_config_path_from_home(Path::new("/tmp/test-home"));

    assert_eq!(path, PathBuf::from("/tmp/test-home/.config/GazeGuard/config.yaml"));
}
```

- [ ] **Step 2: Run the bootstrap tests to verify they fail because the module does not exist**

Run: `cargo test ensure_config_file_ --lib`
Expected: FAIL with unresolved import or missing function errors.

- [ ] **Step 3: Add `tempfile` as a dev-dependency for filesystem tests**

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 4: Create `src/config_file.rs` with path resolution and seeding helpers**

```rust
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const CONFIG_FILE_NAME: &str = "config.yaml";

pub fn ensure_config_file(path: &Path, default_yaml: &str) -> Result<PathBuf, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    if !path.exists() {
        fs::write(path, default_yaml).map_err(|error| error.to_string())?;
    }

    Ok(path.to_path_buf())
}

pub fn desktop_config_path() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        let app_data = std::env::var_os("APPDATA").ok_or("APPDATA is not set")?;
        return Ok(PathBuf::from(app_data).join("GazeGuard").join(CONFIG_FILE_NAME));
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
        return Ok(desktop_config_path_from_home(Path::new(&home)));
    }

    #[allow(unreachable_code)]
    Err("unsupported desktop platform".to_string())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn desktop_config_path_from_home(home: &Path) -> PathBuf {
    home.join(".config").join("GazeGuard").join(CONFIG_FILE_NAME)
}
```

- [ ] **Step 5: Add a failing Rust integration point test for loading from a seeded file**

```rust
#[test]
fn load_or_create_from_path_reads_seeded_yaml_file() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("config.yaml");

    let config = BreakEngineConfig::load_or_create_from_path(
        &config_path,
        "short_break_interval: 7\nlong_break_interval: 75\nlong_break_duration: 60\npre_break_warning_time: 10\nshort_break_duration: 15\nstrict_break: false\n",
    )
    .unwrap();

    assert_eq!(config.break_interval, 7);
    assert_eq!(
        std::fs::read_to_string(&config_path).unwrap(),
        "short_break_interval: 7\nlong_break_interval: 75\nlong_break_duration: 60\npre_break_warning_time: 10\nshort_break_duration: 15\nstrict_break: false\n",
    );
}
```

- [ ] **Step 6: Run the targeted Rust test to verify it fails because the loader does not exist yet**

Run: `cargo test load_or_create_from_path_reads_seeded_yaml_file --lib`
Expected: FAIL with an error about `load_or_create_from_path` not existing.

- [ ] **Step 7: Wire `BreakEngineConfig` through the new file bootstrap helper**

```rust
impl BreakEngineConfig {
    pub fn load() -> Self {
        let config_path = crate::config_file::desktop_config_path()
            .and_then(|path| crate::config_file::ensure_config_file(&path, include_str!("../config/defaults.yaml")));
        let yaml = match config_path.and_then(|path| std::fs::read_to_string(&path).map_err(|error| error.to_string())) {
            Ok(yaml) => yaml,
            Err(error) => panic!("failed to load config.yaml: {error}"),
        };

        Self::from_yaml(&yaml).expect("config.yaml should be valid YAML")
    }

    pub fn load_or_create_from_path(path: &std::path::Path, default_yaml: &str) -> Result<Self, String> {
        let path = crate::config_file::ensure_config_file(path, default_yaml)?;
        let yaml = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        Self::from_yaml(&yaml).map_err(|error| error.to_string())
    }
}
```

- [ ] **Step 8: Register the new module in `src/lib.rs`**

```rust
mod break_engine;
mod config_file;
mod desktop_signals;
```

- [ ] **Step 9: Run the Rust config bootstrap tests and config parser tests**

Run: `cargo test ensure_config_file_ --lib`
Expected: PASS

Run: `cargo test desktop_config_path_uses_dot_config_directory --lib`
Expected: PASS

Run: `cargo test load_or_create_from_path_reads_seeded_yaml_file --lib`
Expected: PASS

Run: `cargo test loads_yaml_defaults_shape --lib`
Expected: PASS

- [ ] **Step 10: Commit the desktop file-backed config loader**

```bash
git add Cargo.toml src/config_file.rs src/break_engine.rs src/lib.rs
git commit -m "feat: load desktop config from user config file"
```

### Task 3: Bundle the canonical YAML into Android and seed `filesDir/config/config.yaml`

**Files:**
- Modify: `gen/android/app/build.gradle.kts`
- Modify: `gen/android/app/src/main/java/com/reus/gazeguard/BreakEngineConfig.kt`
- Modify: `gen/android/app/src/test/java/com/reus/gazeguard/BreakEngineConfigTest.kt`

- [ ] **Step 1: Write the failing Android unit tests for bootstrap path and no-overwrite behavior**

```kotlin
@Test
fun ensureConfigFileWritesDefaultsWhenMissing() {
    val root = createTempDir()
    val configFile = File(root, "config/config.yaml")

    val resolved = BreakEngineConfig.ensureConfigFile(configFile, "short_break_interval: 15\n")

    assertEquals(configFile.absolutePath, resolved.absolutePath)
    assertEquals("short_break_interval: 15\n", configFile.readText())
}

@Test
fun ensureConfigFileKeepsExistingContents() {
    val root = createTempDir()
    val configFile = File(root, "config/config.yaml")
    configFile.parentFile!!.mkdirs()
    configFile.writeText("short_break_interval: 33\n")

    BreakEngineConfig.ensureConfigFile(configFile, "short_break_interval: 15\n")

    assertEquals("short_break_interval: 33\n", configFile.readText())
}
```

- [ ] **Step 2: Run the Android unit tests to verify they fail because the helper does not exist**

Run: `./gradlew app:testDebugUnitTest --tests com.reus.gazeguard.BreakEngineConfigTest`
Expected: FAIL with unresolved reference errors for `ensureConfigFile`.

- [ ] **Step 3: Add the root `config/` directory as an Android assets source directory**

```kotlin
android {
    // existing config
    sourceSets.getByName("main") {
        assets.srcDirs("src/main/assets", "../../../config")
    }
}
```

- [ ] **Step 4: Replace asset-path loading with a file bootstrap helper in Kotlin**

```kotlin
object BreakEngineConfig {
    private const val CONFIG_ASSET_PATH = "defaults.yaml"
    private const val CONFIG_DIR_NAME = "config"
    private const val CONFIG_FILE_NAME = "config.yaml"

    fun ensureConfigFile(configFile: File, defaultYaml: String): File {
        configFile.parentFile?.mkdirs()
        if (!configFile.exists()) {
            configFile.writeText(defaultYaml)
        }
        return configFile
    }

    fun resolveConfigFile(context: Context): File {
        return File(File(context.filesDir, CONFIG_DIR_NAME), CONFIG_FILE_NAME)
    }

    fun loadSchedule(context: Context): Schedule {
        val defaultYaml = context.assets.open(CONFIG_ASSET_PATH).bufferedReader().use { it.readText() }
        val configFile = ensureConfigFile(resolveConfigFile(context), defaultYaml)
        return parseSchedule(configFile.readText())
    }
}
```

- [ ] **Step 5: Extend Android tests to cover the seeded file parser path**

```kotlin
@Test
fun parseScheduleReadsSeededConfigFileContents() {
    val root = createTempDir()
    val configFile = File(root, "config/config.yaml")
    BreakEngineConfig.ensureConfigFile(
        configFile,
        """
        short_break_interval: 15
        long_break_interval: 75
        pre_break_warning_time: 10
        """.trimIndent()
    )

    val schedule = BreakEngineConfig.parseSchedule(configFile.readText())

    assertEquals(15 * 60 * 1000L, schedule.breakIntervalMillis)
    assertEquals(10 * 1000L, schedule.preBreakWarningMillis)
}
```

- [ ] **Step 6: Run the Android unit tests to verify the new file-backed loader passes**

Run: `./gradlew app:testDebugUnitTest --tests com.reus.gazeguard.BreakEngineConfigTest`
Expected: PASS

- [ ] **Step 7: Commit the Android file-backed config loader**

```bash
git add gen/android/app/build.gradle.kts gen/android/app/src/main/java/com/reus/gazeguard/BreakEngineConfig.kt gen/android/app/src/test/java/com/reus/gazeguard/BreakEngineConfigTest.kt
git commit -m "feat: seed android config file from bundled defaults"
```

### Task 4: Remove stale generated-path references and run full verification

**Files:**
- Modify: `src/break_engine.rs`
- Modify: `gen/android/app/src/main/java/com/reus/gazeguard/BreakEngineConfig.kt`
- Test: `src/break_engine.rs`
- Test: `gen/android/app/src/test/java/com/reus/gazeguard/BreakEngineConfigTest.kt`

- [ ] **Step 1: Write the failing grep check by verifying stale generated references still exist**

Run: `rg -n "gen/android/app/src/main/assets/config/defaults.yaml|config/defaults.yaml" src gen/android/app/src/main/java`
Expected: output still includes stale generated-path references before cleanup.

- [ ] **Step 2: Remove stale generated-path constants and comments**

```rust
// before
Self::from_yaml(include_str!("../gen/android/app/src/main/assets/config/defaults.yaml"))

// after
Self::load_or_create_from_path(...)
```

```kotlin
// before
private const val CONFIG_ASSET_PATH = "config/defaults.yaml"

// after
private const val CONFIG_ASSET_PATH = "defaults.yaml"
```

- [ ] **Step 3: Run the Rust verification suite**

Run: `cargo test loads_yaml_defaults_shape --lib`
Expected: PASS

Run: `cargo test load_or_create_from_path_reads_seeded_yaml_file --lib`
Expected: PASS

Run: `cargo test ensure_config_file_ --lib`
Expected: PASS

- [ ] **Step 4: Run the Android verification suite**

Run: `cd gen/android && ./gradlew app:testDebugUnitTest --tests com.reus.gazeguard.BreakEngineConfigTest`
Expected: PASS

- [ ] **Step 5: Verify no runtime code still depends on generated defaults**

Run: `rg -n "gen/android/app/src/main/assets/config/defaults.yaml" src gen/android/app`
Expected: no matches

- [ ] **Step 6: Commit the cleanup and verification pass**

```bash
git add src/break_engine.rs gen/android/app/src/main/java/com/reus/gazeguard/BreakEngineConfig.kt gen/android/app/build.gradle.kts gen/android/app/src/test/java/com/reus/gazeguard/BreakEngineConfigTest.kt
git commit -m "refactor: remove generated defaults yaml runtime dependency"
```
