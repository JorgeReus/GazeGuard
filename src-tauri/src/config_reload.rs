#![allow(dead_code)]

use crate::break_engine::BreakEngineConfig;
use std::path::Path;
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
                ConfigReloadResult {
                    outcome: ConfigReloadOutcome::Reloaded,
                    message: None,
                }
            }
            Err(_) => {
                let message =
                    "Could not reload config.yaml. Using the last valid config.".to_string();
                *self.last_error.lock().unwrap() = Some(message.clone());
                ConfigReloadResult {
                    outcome: ConfigReloadOutcome::Failed,
                    message: Some(message),
                }
            }
        }
    }
}

pub fn file_mtime(path: &Path) -> Result<Option<SystemTime>, String> {
    match std::fs::metadata(path) {
        Ok(metadata) => metadata
            .modified()
            .map(Some)
            .map_err(|error| error.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

pub fn should_reload(previous: Option<SystemTime>, current: Option<SystemTime>) -> bool {
    current != previous
}

pub fn should_emit_error(last_emitted_error: &mut Option<String>, message: &str) -> bool {
    if last_emitted_error.as_deref() == Some(message) {
        return false;
    }

    *last_emitted_error = Some(message.to_string());
    true
}

pub fn clear_emitted_error(last_emitted_error: &mut Option<String>) {
    *last_emitted_error = None;
}

pub fn refreshed_tracked_mtime(path: &Path, fallback: Option<SystemTime>) -> Option<SystemTime> {
    file_mtime(path).ok().flatten().or(fallback)
}

#[cfg(test)]
mod tests {
    use super::{
        clear_emitted_error, refreshed_tracked_mtime, should_emit_error, should_reload,
        ConfigReloadOutcome, ConfigReloadState,
    };
    use crate::break_engine::BreakEngineConfig;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn path(&self) -> &Path {
            self.path.as_path()
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn unique_test_dir(name: &str) -> TestDir {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("gazeguard-config-reload-{name}-{suffix}"));
        fs::create_dir_all(&path).unwrap();
        TestDir { path }
    }

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
        let updated =
            BreakEngineConfig::load_or_create_from_path(&config_path, "short_break_interval: 1\n")
                .unwrap();

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
        assert_eq!(
            result.message.as_deref(),
            Some("Could not reload config.yaml. Using the last valid config.")
        );
    }

    #[test]
    fn should_reload_when_existing_config_file_is_deleted() {
        let previous = Some(UNIX_EPOCH + Duration::from_secs(1));

        assert!(should_reload(previous, None));
    }

    #[test]
    fn duplicate_watcher_errors_are_suppressed_until_message_changes() {
        let mut last_emitted_error = None;

        assert!(should_emit_error(
            &mut last_emitted_error,
            "metadata failed"
        ));
        assert_eq!(last_emitted_error.as_deref(), Some("metadata failed"));

        assert!(!should_emit_error(
            &mut last_emitted_error,
            "metadata failed"
        ));
        assert!(should_emit_error(&mut last_emitted_error, "reload failed"));
        assert_eq!(last_emitted_error.as_deref(), Some("reload failed"));
    }

    #[test]
    fn healthy_poll_clears_emitted_error_so_same_message_can_surface_again() {
        let mut last_emitted_error = Some("metadata failed".to_string());

        clear_emitted_error(&mut last_emitted_error);

        assert_eq!(last_emitted_error, None);
        assert!(should_emit_error(
            &mut last_emitted_error,
            "metadata failed"
        ));
    }

    #[test]
    fn refreshed_tracked_mtime_uses_recreated_file_state_after_reload() {
        let temp = unique_test_dir("refresh-tracked-mtime");
        let config_path = temp.path().join("config.yaml");
        let deleted_state = None;

        std::fs::write(&config_path, "short_break_interval: 1\n").unwrap();

        let refreshed = refreshed_tracked_mtime(&config_path, deleted_state);

        assert!(refreshed.is_some());
        assert_ne!(refreshed, deleted_state);
    }
}
