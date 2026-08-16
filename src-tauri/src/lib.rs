#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod break_engine;
mod config_file;
mod config_reload;
mod desktop_signals;
mod logger;

use break_engine::{
    BreakEngine, BreakEngineConfig, BreakEngineSnapshot, BreakInfo, DisableOption, EnginePhase,
    EngineStatus, PostponeOption,
};
use serde::Serialize;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
#[cfg(desktop)]
use tauri::Emitter;
use tauri::Manager;
use tauri::State;
#[cfg(desktop)]
use tauri_plugin_tracing::TracedProfilingExt;

#[cfg(desktop)]
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};

type SharedBreakEngine = Arc<Mutex<BreakEngine>>;

#[cfg(desktop)]
struct TrayUpdater {
    stop: Arc<(Mutex<bool>, Condvar)>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

#[cfg(desktop)]
impl TrayUpdater {
    fn start(app: tauri::AppHandle, engine: SharedBreakEngine) -> Self {
        let stop = Arc::new((Mutex::new(false), Condvar::new()));
        let worker_stop = stop.clone();
        let handle = thread::spawn(move || Self::run(app, engine, worker_stop));
        Self {
            stop,
            handle: Mutex::new(Some(handle)),
        }
    }

    fn run(app: tauri::AppHandle, engine: SharedBreakEngine, stop: Arc<(Mutex<bool>, Condvar)>) {
        loop {
            refresh_desktop_signals(&app, &engine);
            refresh_tray_title(&app);

            let (lock, wake) = &*stop;
            let stopped = lock
                .lock()
                .map(|stopped| {
                    let (stopped, _) = wake
                        .wait_timeout_while(stopped, Duration::from_secs(1), |stopped| !*stopped)
                        .expect("tray updater condvar poisoned");
                    *stopped
                })
                .unwrap_or(true);
            if stopped {
                break;
            }
        }
    }

    fn shutdown(&self) {
        let (lock, wake) = &*self.stop;
        if let Ok(mut stopped) = lock.lock() {
            *stopped = true;
            wake.notify_one();
        }
        if let Ok(mut handle) = self.handle.lock() {
            if let Some(handle) = handle.take() {
                let _ = handle.join();
            }
        }
    }
}

static SHARED_BREAK_ENGINE: OnceLock<Mutex<Option<SharedBreakEngine>>> = OnceLock::new();
static SNAPSHOT_APP_DATA_DIR: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
const SNAPSHOT_FILE_NAME: &str = "break-engine-snapshot.json";

#[derive(Debug, Serialize)]
struct BreakSchedule {
    break_interval_minutes: u64,
    pre_break_warning_seconds: u64,
    disable_options: Vec<DisableOption>,
    postpone_options: Vec<PostponeOption>,
}

#[derive(Debug, Serialize)]
struct AndroidBreakOverlaySnapshot {
    phase: String,
    remaining_seconds: u64,
    message: String,
    should_show_notification: bool,
    should_show_overlay: bool,
    can_postpone: bool,
    postpone_options: Vec<break_engine::PostponeOption>,
}

fn shared_break_engine_slot() -> &'static Mutex<Option<SharedBreakEngine>> {
    SHARED_BREAK_ENGINE.get_or_init(|| Mutex::new(None))
}

fn snapshot_app_data_dir_slot() -> &'static Mutex<Option<PathBuf>> {
    SNAPSHOT_APP_DATA_DIR.get_or_init(|| Mutex::new(None))
}

fn register_shared_break_engine(engine: SharedBreakEngine) {
    if let Ok(mut slot) = shared_break_engine_slot().lock() {
        *slot = Some(engine);
    }
}

fn register_snapshot_app_data_dir(path: PathBuf) {
    if let Ok(mut slot) = snapshot_app_data_dir_slot().lock() {
        *slot = Some(path);
    }
}

fn get_shared_break_engine() -> Option<SharedBreakEngine> {
    shared_break_engine_slot()
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
}

fn get_snapshot_app_data_dir() -> Option<PathBuf> {
    snapshot_app_data_dir_slot()
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
}

fn engine_phase_label(phase: &EnginePhase) -> &'static str {
    match phase {
        EnginePhase::Stopped => "stopped",
        EnginePhase::Running => "running",
        EnginePhase::Warning => "warning",
        EnginePhase::OnBreak => "on_break",
        EnginePhase::Disabled => "disabled",
    }
}

fn debug_engine_phase_for_android() -> String {
    let Some(engine) = get_shared_break_engine() else {
        return "unavailable".to_string();
    };

    engine
        .lock()
        .ok()
        .map(|mut guard| engine_phase_label(&guard.status().phase).to_string())
        .unwrap_or_else(|| "poisoned".to_string())
}

fn force_break_now_for_android() -> String {
    let Some(engine) = get_shared_break_engine() else {
        return "unavailable".to_string();
    };

    engine
        .lock()
        .ok()
        .map(|mut guard| {
            guard.begin_break_now();
            engine_phase_label(&guard.status().phase).to_string()
        })
        .unwrap_or_else(|| "poisoned".to_string())
}

fn postpone_break_for_android(seconds: u64) -> String {
    let Some(engine) = get_shared_break_engine() else {
        return "unavailable".to_string();
    };

    engine
        .lock()
        .ok()
        .map(
            |mut guard| match guard.postpone_break_with_override(Some(seconds)) {
                Ok(status) => engine_phase_label(&status.phase).to_string(),
                Err(error) => error,
            },
        )
        .unwrap_or_else(|| "poisoned".to_string())
}

fn break_overlay_snapshot_for_android() -> String {
    let Some(engine) = get_shared_break_engine() else {
        return json!({
            "phase": "unavailable",
            "remaining_seconds": 0,
            "message": "Break unavailable",
            "should_show_notification": false,
            "should_show_overlay": false,
            "can_postpone": false,
            "postpone_options": []
        })
        .to_string();
    };

    engine
        .lock()
        .ok()
        .map(|mut guard| {
            let status = guard.status();
            let phase = engine_phase_label(&status.phase).to_string();
            let remaining_seconds = status.seconds_remaining.unwrap_or(0);
            let (message, should_show_notification, should_show_overlay) = match status.phase {
                EnginePhase::Warning => (
                    format!("Break starts in {remaining_seconds} seconds"),
                    true,
                    false,
                ),
                EnginePhase::OnBreak => (
                    status
                        .current_break
                        .as_ref()
                        .map(|info| {
                            info.template_name
                                .clone()
                                .unwrap_or_else(|| match info.kind {
                                    break_engine::BreakKind::Long => {
                                        "Take a Long Break".to_string()
                                    }
                                    break_engine::BreakKind::Short => {
                                        "Take a Short Break".to_string()
                                    }
                                })
                        })
                        .unwrap_or_else(|| "Take a Break".to_string()),
                    true,
                    true,
                ),
                _ => ("Break ended".to_string(), false, false),
            };

            serde_json::to_string(&AndroidBreakOverlaySnapshot {
                phase,
                remaining_seconds,
                message,
                should_show_notification,
                should_show_overlay,
                can_postpone: status.can_postpone,
                postpone_options: guard.config().postpone_options.clone(),
            })
            .unwrap_or_else(|_| {
                json!({
                    "phase": "error",
                    "remaining_seconds": 0,
                    "message": "Break unavailable",
                    "should_show_notification": false,
                    "should_show_overlay": false,
                    "can_postpone": false,
                    "postpone_options": []
                })
                .to_string()
            })
        })
        .unwrap_or_else(|| {
            json!({
                "phase": "poisoned",
                "remaining_seconds": 0,
                "message": "Break unavailable",
                "should_show_notification": false,
                "should_show_overlay": false,
                "can_postpone": false,
                "postpone_options": []
            })
            .to_string()
        })
}

#[cfg(test)]
fn set_shared_break_engine_for_tests(engine: Option<SharedBreakEngine>) {
    if let Ok(mut slot) = shared_break_engine_slot().lock() {
        *slot = engine;
    }
}

#[cfg(test)]
fn set_snapshot_app_data_dir_for_tests(path: Option<PathBuf>) {
    if let Ok(mut slot) = snapshot_app_data_dir_slot().lock() {
        *slot = path;
    }
}

#[cfg(test)]
fn singleton_test_lock() -> &'static Mutex<()> {
    static SINGLETON_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    SINGLETON_TEST_LOCK.get_or_init(|| Mutex::new(()))
}

fn create_break_engine() -> SharedBreakEngine {
    let engine =
        create_break_engine_with_config(BreakEngineConfig::load(), None, unix_now_seconds());
    register_shared_break_engine(engine.clone());
    engine
}

fn runtime_config_path(app_data_dir: &Path) -> Result<PathBuf, String> {
    #[cfg(desktop)]
    let _ = app_data_dir;

    #[cfg(desktop)]
    {
        crate::config_file::desktop_config_path()
    }

    #[cfg(not(desktop))]
    {
        Ok(app_data_dir
            .join("config")
            .join(crate::config_file::CONFIG_FILE_NAME))
    }
}

fn load_runtime_break_engine_config(app_data_dir: &Path) -> Result<BreakEngineConfig, String> {
    let config_path = runtime_config_path(app_data_dir)?;
    BreakEngineConfig::load_or_create_from_path(
        &config_path,
        include_str!("../config/defaults.yaml"),
    )
}

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

    let mut guard = engine
        .lock()
        .map_err(|_| "State lock poisoned".to_string())?;
    guard.apply_config(config);
    Ok(schedule)
}

#[cfg(test)]
fn reload_runtime_config_for_tests(engine: SharedBreakEngine) -> Result<BreakSchedule, String> {
    let app_data_dir =
        get_snapshot_app_data_dir().ok_or_else(|| "App data dir unavailable".to_string())?;
    let path = runtime_config_path(app_data_dir.as_path())?;
    reload_runtime_config_into_engine(&engine, &path)
}

fn snapshot_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(SNAPSHOT_FILE_NAME)
}

fn unix_now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn load_snapshot(snapshot_file: &Path) -> Result<Option<BreakEngineSnapshot>, String> {
    let contents = match fs::read_to_string(snapshot_file) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };

    serde_json::from_str(&contents)
        .map(Some)
        .map_err(|error| error.to_string())
}

fn load_engine_from_disk(
    config: &BreakEngineConfig,
    app_data_dir: &Path,
    now_unix_seconds: u64,
) -> Option<BreakEngine> {
    if !config.persist_state {
        return None;
    }

    let snapshot_file = snapshot_path(app_data_dir);
    let snapshot = match load_snapshot(&snapshot_file) {
        Ok(Some(snapshot)) => snapshot,
        Ok(None) => return None,
        Err(error) => {
            eprintln!(
                "failed to load break engine snapshot from {}: {error}",
                snapshot_file.display()
            );
            return None;
        }
    };
    let elapsed_seconds = now_unix_seconds.saturating_sub(snapshot.saved_at_unix_seconds);
    let mut engine = BreakEngine::from_snapshot(config.clone(), snapshot);
    engine.restore_elapsed(elapsed_seconds);
    Some(engine)
}

fn create_break_engine_with_config(
    config: BreakEngineConfig,
    app_data_dir: Option<&Path>,
    now_unix_seconds: u64,
) -> SharedBreakEngine {
    let engine = app_data_dir
        .and_then(|path| load_engine_from_disk(&config, path, now_unix_seconds))
        .unwrap_or_else(|| {
            let mut engine = BreakEngine::new(config);
            engine.start();
            engine
        });

    Arc::new(Mutex::new(engine))
}

#[cfg(test)]
fn create_break_engine_for_tests(
    config: BreakEngineConfig,
    app_data_dir: &Path,
    now_unix_seconds: u64,
) -> SharedBreakEngine {
    create_break_engine_with_config(config, Some(app_data_dir), now_unix_seconds)
}

fn save_engine_snapshot(
    engine: &SharedBreakEngine,
    app_data_dir: &Path,
    now_unix_seconds: u64,
) -> Result<(), String> {
    let snapshot = {
        let mut guard = engine
            .lock()
            .map_err(|_| "State lock poisoned".to_string())?;
        if !guard.config().persist_state {
            let snapshot_file = snapshot_path(app_data_dir);
            match fs::remove_file(snapshot_file) {
                Ok(()) => return Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(error.to_string()),
            }
        }
        guard.snapshot(now_unix_seconds)
    };

    fs::create_dir_all(app_data_dir).map_err(|error| error.to_string())?;
    let snapshot_file = snapshot_path(app_data_dir);
    let payload = serde_json::to_vec(&snapshot).map_err(|error| error.to_string())?;
    fs::write(snapshot_file, payload).map_err(|error| error.to_string())
}

fn save_registered_engine_snapshot(now_unix_seconds: u64) -> Result<(), String> {
    let Some(engine) = get_shared_break_engine() else {
        return Ok(());
    };
    let Some(app_data_dir) = get_snapshot_app_data_dir() else {
        return Ok(());
    };

    save_engine_snapshot(&engine, app_data_dir.as_path(), now_unix_seconds)
}

fn format_duration(seconds: u64) -> String {
    let minutes = seconds / 60;
    let secs = seconds % 60;
    format!("{minutes}:{secs:02}")
}

fn format_tray_title(status: &EngineStatus) -> String {
    match status.phase {
        break_engine::EnginePhase::Running | break_engine::EnginePhase::Warning => {
            let remaining = format_duration(status.seconds_remaining.unwrap_or(0));
            let kind = match status.upcoming_break_kind {
                Some(break_engine::BreakKind::Short) => "short",
                Some(break_engine::BreakKind::Long) => "long",
                None => "break",
            };
            format!("{remaining} {kind}")
        }
        break_engine::EnginePhase::OnBreak => {
            let remaining = format_duration(status.seconds_remaining.unwrap_or(0));
            format!("{remaining} break")
        }
        break_engine::EnginePhase::Disabled => {
            let remaining = format_duration(status.seconds_remaining.unwrap_or(0));
            format!("{remaining} paused")
        }
        break_engine::EnginePhase::Stopped => "stopped".to_string(),
    }
}

#[cfg(desktop)]
fn refresh_tray_title(app: &tauri::AppHandle) {
    let Some(tray) = app.tray_by_id("main") else {
        return;
    };

    let title = app
        .state::<SharedBreakEngine>()
        .lock()
        .ok()
        .map(|mut engine| format_tray_title(&engine.status()))
        .unwrap_or_else(|| "GazeGuard".to_string());

    let _ = tray.set_title(Some(&title));
    let _ = tray.set_tooltip(Some(&title));
    let _ = save_registered_engine_snapshot(unix_now_seconds());
}

#[cfg(not(desktop))]
fn refresh_tray_title(_app: &tauri::AppHandle) {}

#[cfg(desktop)]
fn refresh_desktop_signals(app: &tauri::AppHandle, engine: &SharedBreakEngine) {
    let (idle_threshold_seconds, configured_log_level) = match engine.lock() {
        Ok(guard) => (
            guard.config().idle_time.saturating_mul(60),
            guard.config().log_level,
        ),
        Err(_) => return,
    };

    let signals = crate::desktop_signals::collect_desktop_signals_with_level(
        app,
        idle_threshold_seconds,
        configured_log_level,
    );

    if let Ok(mut guard) = engine.lock() {
        let status = apply_desktop_signals_to_engine(&mut guard, signals);
        crate::logger::log(
            crate::logger::LogLevel::Debug,
            configured_log_level,
            "desktop_signals",
            format_args!(
                "rust_updater_applied_signals={signals:?} phase={:?}",
                status.phase
            ),
        );
    }

    let _ = save_registered_engine_snapshot(unix_now_seconds());
}

#[cfg(desktop)]
fn spawn_config_watcher(
    app: tauri::AppHandle,
    engine: SharedBreakEngine,
    config_path: PathBuf,
    state: crate::config_reload::ConfigReloadState,
) {
    thread::spawn(move || {
        let mut last_seen = crate::config_reload::file_mtime(&config_path)
            .ok()
            .flatten();
        let mut last_emitted_error = None;

        loop {
            thread::sleep(Duration::from_millis(750));

            let current = match crate::config_reload::file_mtime(&config_path) {
                Ok(value) => value,
                Err(error) => {
                    if crate::config_reload::should_emit_error(&mut last_emitted_error, &error) {
                        let _ = app.emit("config-reload-error", json!({ "message": error }));
                    }
                    continue;
                }
            };

            if !crate::config_reload::should_reload(last_seen, current) {
                crate::config_reload::clear_emitted_error(&mut last_emitted_error);
                continue;
            }

            let result = BreakEngineConfig::load_or_create_from_path(
                &config_path,
                include_str!("../config/defaults.yaml"),
            );
            let reload = state.finish_reload(result);
            last_seen = crate::config_reload::refreshed_tracked_mtime(&config_path, current);

            match reload.outcome {
                crate::config_reload::ConfigReloadOutcome::Reloaded => {
                    match apply_reloaded_config(&engine, state.last_good_config()) {
                        Ok(()) => {
                            last_emitted_error = None;
                            let _ = app.emit("config-reload-success", json!({}));
                        }
                        Err(message) => {
                            if crate::config_reload::should_emit_error(
                                &mut last_emitted_error,
                                &message,
                            ) {
                                let _ =
                                    app.emit("config-reload-error", json!({ "message": message }));
                            }
                        }
                    }
                }
                crate::config_reload::ConfigReloadOutcome::Failed => {
                    let message = reload.message.unwrap_or_else(|| {
                        "Could not reload config.yaml. Using the last valid config.".to_string()
                    });
                    if crate::config_reload::should_emit_error(&mut last_emitted_error, &message) {
                        let _ = app.emit("config-reload-error", json!({ "message": message }));
                    }
                }
                crate::config_reload::ConfigReloadOutcome::Unchanged => {}
            }
        }
    });
}

fn apply_reloaded_config(
    engine: &SharedBreakEngine,
    config: BreakEngineConfig,
) -> Result<(), String> {
    let mut guard = engine
        .lock()
        .map_err(|_| "State lock poisoned".to_string())?;
    guard.apply_config(config);
    Ok(())
}

#[tauri::command]
fn start_break_timer(state: State<'_, SharedBreakEngine>) -> Result<EngineStatus, String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    Ok(guard.start())
}

#[tauri::command]
fn stop_break_timer(state: State<'_, SharedBreakEngine>) -> Result<EngineStatus, String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    Ok(guard.stop())
}

#[tauri::command]
fn get_engine_status(state: State<'_, SharedBreakEngine>) -> Result<EngineStatus, String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    let status = guard.status();
    drop(guard);
    let _ = save_registered_engine_snapshot(unix_now_seconds());
    Ok(status)
}

#[tauri::command]
fn get_break_schedule(state: State<'_, SharedBreakEngine>) -> Result<BreakSchedule, String> {
    let guard = state.lock().map_err(|_| "State lock poisoned")?;
    let config = guard.config();
    Ok(BreakSchedule {
        break_interval_minutes: config.break_interval,
        pre_break_warning_seconds: config.pre_break_warning_time,
        disable_options: config.disable_options.clone(),
        postpone_options: config.postpone_options.clone(),
    })
}

#[tauri::command]
fn reload_runtime_config(state: State<'_, SharedBreakEngine>) -> Result<BreakSchedule, String> {
    let app_data_dir =
        get_snapshot_app_data_dir().ok_or_else(|| "App data dir unavailable".to_string())?;
    let path = runtime_config_path(app_data_dir.as_path())?;
    let engine = state.inner().clone();
    reload_runtime_config_into_engine(&engine, &path)
}

#[tauri::command]
fn get_current_break_info(state: State<'_, SharedBreakEngine>) -> Result<BreakInfo, String> {
    let guard = state.lock().map_err(|_| "State lock poisoned")?;
    guard
        .current_break()
        .ok_or_else(|| "No active break is available.".to_string())
}

#[tauri::command]
fn set_idle_active(
    state: State<'_, SharedBreakEngine>,
    active: bool,
) -> Result<EngineStatus, String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    guard.set_idle(active);
    let status = guard.status();
    drop(guard);
    let _ = save_registered_engine_snapshot(unix_now_seconds());
    Ok(status)
}

#[tauri::command]
fn set_fullscreen_active(
    state: State<'_, SharedBreakEngine>,
    active: bool,
) -> Result<EngineStatus, String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    guard.set_fullscreen(active);
    let status = guard.status();
    drop(guard);
    let _ = save_registered_engine_snapshot(unix_now_seconds());
    Ok(status)
}

#[tauri::command]
fn apply_desktop_signals_to_engine(
    engine: &mut BreakEngine,
    signals: crate::desktop_signals::DesktopSignals,
) -> EngineStatus {
    engine.set_idle(signals.idle_active);
    engine.set_fullscreen(signals.fullscreen_active);
    engine.status()
}

#[tauri::command]
fn sync_desktop_window_state(
    app: tauri::AppHandle,
    state: State<'_, SharedBreakEngine>,
) -> Result<EngineStatus, String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    let idle_threshold_seconds = guard.config().idle_time.saturating_mul(60);
    let configured_log_level = guard.config().log_level;

    #[cfg(not(desktop))]
    let _ = &app;

    #[cfg(desktop)]
    crate::logger::log(
        crate::logger::LogLevel::Debug,
        configured_log_level,
        "desktop_signals",
        format_args!("configured_idle_threshold_seconds={idle_threshold_seconds}"),
    );

    #[cfg(desktop)]
    let signals = crate::desktop_signals::collect_desktop_signals_with_level(
        &app,
        idle_threshold_seconds,
        configured_log_level,
    );

    #[cfg(not(desktop))]
    let signals = crate::desktop_signals::DesktopSignals {
        fullscreen_active: false,
        idle_active: false,
    };

    #[cfg(desktop)]
    crate::logger::log(
        crate::logger::LogLevel::Debug,
        configured_log_level,
        "desktop_signals",
        format_args!("collected_desktop_signals={signals:?}"),
    );

    let status = apply_desktop_signals_to_engine(&mut guard, signals);

    #[cfg(desktop)]
    {
        let engine_snapshot = guard.snapshot(unix_now_seconds());
        crate::logger::log(
            crate::logger::LogLevel::Debug,
            configured_log_level,
            "desktop_signals",
            format_args!(
                "applied_engine_state idle_active={} fullscreen_active={} phase={:?}",
                engine_snapshot.idle_active, engine_snapshot.fullscreen, status.phase
            ),
        );
    }

    drop(guard);
    let _ = save_registered_engine_snapshot(unix_now_seconds());
    Ok(status)
}

#[tauri::command]
fn disable_reminders(
    state: State<'_, SharedBreakEngine>,
    seconds: u64,
) -> Result<EngineStatus, String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    let status = guard.disable_for(seconds)?;
    drop(guard);
    let _ = save_registered_engine_snapshot(unix_now_seconds());
    Ok(status)
}

#[tauri::command]
fn skip_break(app: tauri::AppHandle, state: State<'_, SharedBreakEngine>) -> Result<(), String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    let skip_result = guard.skip_break();
    drop(guard);

    if skip_result.is_ok() {
        let _ = save_registered_engine_snapshot(unix_now_seconds());
        close_break_window(app)
    } else {
        skip_result.map(|_| ())
    }
}

#[tauri::command]
fn complete_break(
    app: tauri::AppHandle,
    state: State<'_, SharedBreakEngine>,
) -> Result<(), String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    guard.complete_break()?;

    drop(guard);
    let _ = save_registered_engine_snapshot(unix_now_seconds());
    close_break_window(app)
}

#[tauri::command]
fn postpone_break(
    app: tauri::AppHandle,
    state: State<'_, SharedBreakEngine>,
    seconds: Option<u64>,
) -> Result<(), String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    guard.postpone_break_with_override(seconds)?;

    drop(guard);
    let _ = save_registered_engine_snapshot(unix_now_seconds());
    close_break_window(app)
}

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_reus_gazeguard_RustProbe_debugEnginePhase(
    mut env: jni::JNIEnv,
    _: jni::objects::JClass,
) -> jni::sys::jstring {
    let phase = debug_engine_phase_for_android();
    env.new_string(phase)
        .map(|value| value.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_reus_gazeguard_RustProbe_forceBreakNow(
    mut env: jni::JNIEnv,
    _: jni::objects::JClass,
) -> jni::sys::jstring {
    let phase = force_break_now_for_android();
    env.new_string(phase)
        .map(|value| value.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_reus_gazeguard_RustProbe_postponeBreak(
    mut env: jni::JNIEnv,
    _: jni::objects::JClass,
    seconds: jni::sys::jlong,
) -> jni::sys::jstring {
    let phase = postpone_break_for_android(seconds.max(0) as u64);
    env.new_string(phase)
        .map(|value| value.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_reus_gazeguard_RustProbe_breakOverlaySnapshot(
    mut env: jni::JNIEnv,
    _: jni::objects::JClass,
) -> jni::sys::jstring {
    let snapshot = break_overlay_snapshot_for_android();
    env.new_string(snapshot)
        .map(|value| value.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

// #[cfg(target_os = "android")]
// mod mobile;

#[tauri::command]
fn start_background_service(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(not(target_os = "android"))]
    let _ = &app;

    #[cfg(target_os = "android")]
    {
        if let Some(w) = app.get_webview_window("main") {
            w.eval("console.log('Rust: start_background_service invoked');")
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
fn stop_background_service(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(not(target_os = "android"))]
    let _ = &app;

    #[cfg(target_os = "android")]
    {
        if let Some(w) = app.get_webview_window("main") {
            w.eval("console.log('Rust: stop_background_service invoked');")
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
fn show_break_window(
    app: tauri::AppHandle,
    state: State<'_, SharedBreakEngine>,
) -> Result<(), String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    guard.begin_break_now();
    drop(guard);
    open_break_window(app)
}

fn open_break_window(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(desktop)]
    {
        // Reuse existing break window; closing/recreating same label can race.
        if let Some(existing) = app.get_webview_window("break") {
            existing.show().map_err(|e| e.to_string())?;
            existing.set_focus().map_err(|e| e.to_string())?;
            return Ok(());
        }

        // Try primary monitor first, fallback to first available monitor
        let monitor = app
            .primary_monitor()
            .ok()
            .flatten()
            .or_else(|| {
                app.available_monitors()
                    .ok()
                    .and_then(|monitors| monitors.into_iter().next())
            })
            .ok_or("No monitors found")?;

        let size = monitor.size();
        let position = monitor.position();

        let break_window = tauri::WebviewWindowBuilder::new(
            &app,
            "break",
            tauri::WebviewUrl::App("break.html".into()),
        )
        .title("Take a Break")
        .inner_size(size.width as f64, size.height as f64)
        .position(position.x as f64, position.y as f64)
        .fullscreen(true)
        .always_on_top(true)
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .focused(true)
        .build()
        .map_err(|e| e.to_string())?;

        break_window.set_focus().map_err(|e| e.to_string())?;
    }

    #[cfg(mobile)]
    {
        // On mobile, navigate the main window to the break page
        if let Some(main_window) = app.get_webview_window("main") {
            main_window
                .eval("window.location.href = 'break.html';")
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

#[tauri::command]
fn close_break_window(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(desktop)]
    {
        if let Some(window) = app.get_webview_window("break") {
            window.close().map_err(|e| e.to_string())?;
        }
    }

    #[cfg(mobile)]
    {
        // On mobile, navigate back to the main page
        if let Some(main_window) = app.get_webview_window("main") {
            main_window
                .eval("window.location.href = 'index.html';")
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

#[cfg(desktop)]
#[tauri::command]
fn start_cpu_profile(app: tauri::AppHandle) -> Result<(), String> {
    app.start_cpu_profile_traced().map_err(|error| error.to_string())
}

#[cfg(desktop)]
#[tauri::command]
fn stop_cpu_profile(app: tauri::AppHandle) -> Result<String, String> {
    app.stop_cpu_profile_traced()
        .map(|profile| profile.flamegraph_path.display().to_string())
        .map_err(|error| error.to_string())
}

#[cfg(not(desktop))]
#[tauri::command]
fn start_cpu_profile() -> Result<(), String> {
    Err("CPU profiling is only available on desktop".to_string())
}

#[cfg(not(desktop))]
#[tauri::command]
fn stop_cpu_profile() -> Result<String, String> {
    Err("CPU profiling is only available on desktop".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(desktop)]
    let tracing_level = if cfg!(debug_assertions) {
        tauri_plugin_tracing::LevelFilter::DEBUG
    } else {
        tauri_plugin_tracing::LevelFilter::INFO
    };

    let builder = tauri::Builder::default().plugin(tauri_plugin_shell::init());

    #[cfg(desktop)]
    let builder = builder.plugin(
            tauri_plugin_tracing::Builder::new()
                .with_max_level(tracing_level)
                .with_file_logging()
                .with_default_subscriber()
                .build(),
        );
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_tracing::init_profiling());

    // Add android plugin
    // #[cfg(target_os = "android")]
    //  {
    //      builder = builder.plugin(mobile::init());
    //  }

    builder
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            register_snapshot_app_data_dir(app_data_dir.clone());
            let initial_config =
                load_runtime_break_engine_config(app_data_dir.as_path()).map_err(|error| {
                    tauri::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, error))
                })?;
            let engine = create_break_engine_with_config(
                initial_config.clone(),
                Some(app_data_dir.as_path()),
                unix_now_seconds(),
            );
            let _ = app.manage(engine.clone());
            register_shared_break_engine(engine.clone());

            #[cfg(not(desktop))]
            let _ = &app;

            #[cfg(desktop)]
            {
                let config_path = runtime_config_path(app_data_dir.as_path()).map_err(|error| {
                    tauri::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, error))
                })?;
                spawn_config_watcher(
                    app.handle().clone(),
                    engine.clone(),
                    config_path,
                    crate::config_reload::ConfigReloadState::new(initial_config),
                );

                // Create tray menu
                let test_break =
                    MenuItem::with_id(app, "test_break", "Test Break", true, None::<&str>)?;
                let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
                let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

                let menu = Menu::with_items(app, &[&test_break, &settings, &quit])?;

                let _tray = TrayIconBuilder::with_id("main")
                    .icon(app.default_window_icon().unwrap().clone())
                    .title("15:00 short")
                    .menu(&menu)
                    .on_menu_event(|app, event| {
                        match event.id.as_ref() {
                            "test_break" => {
                                if let Ok(mut guard) = app.state::<SharedBreakEngine>().lock() {
                                    guard.begin_break_now();
                                }
                                let _ = open_break_window(app.clone());
                            }
                            "settings" => {
                                // Show settings window
                                if let Some(window) = app.get_webview_window("main") {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                } else {
                                    // Create main window if it doesn't exist
                                    let _ = tauri::WebviewWindowBuilder::new(
                                        app,
                                        "main",
                                        tauri::WebviewUrl::App("index.html".into()),
                                    )
                                    .title("GazeGuard Settings")
                                    .inner_size(400.0, 300.0)
                                    .build();
                                }
                            }
                            "quit" => {
                                app.exit(0);
                            }
                            _ => {}
                        }
                    })
                    .build(app)?;

                refresh_tray_title(app.handle());
                app.manage(TrayUpdater::start(app.handle().clone(), engine.clone()));

                // Hide the main window on startup (tray only mode)
                if let Some(window) = app.get_webview_window("main") {
                    #[cfg(target_os = "macos")]
                    {
                        let window_to_hide = window.clone();
                        window.on_window_event(move |event| {
                            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                                api.prevent_close();
                                let _ = window_to_hide.hide();
                            }
                        });
                    }
                    let _ = window.hide();
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            show_break_window,
            close_break_window,
            start_background_service,
            stop_background_service,
            start_break_timer,
            stop_break_timer,
            get_engine_status,
            get_break_schedule,
            reload_runtime_config,
            get_current_break_info,
            set_idle_active,
            set_fullscreen_active,
            sync_desktop_window_state,
            disable_reminders,
            skip_break,
            postpone_break,
            complete_break,
            start_cpu_profile,
            stop_cpu_profile
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            let _ = &app;
            if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
                #[cfg(desktop)]
                app.state::<TrayUpdater>().shutdown();
                if let Err(error) = save_registered_engine_snapshot(unix_now_seconds()) {
                    eprintln!("failed to save break engine snapshot on exit: {error}");
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::{
        apply_desktop_signals_to_engine, apply_reloaded_config, break_overlay_snapshot_for_android,
        create_break_engine, create_break_engine_for_tests, debug_engine_phase_for_android,
        force_break_now_for_android, format_tray_title, reload_runtime_config_for_tests,
        save_engine_snapshot, save_registered_engine_snapshot, set_shared_break_engine_for_tests,
        set_snapshot_app_data_dir_for_tests, singleton_test_lock, SharedBreakEngine,
        SNAPSHOT_FILE_NAME,
    };
    use crate::break_engine::{
        BreakEngine, BreakEngineConfig, BreakKind, EnginePhase, EngineStatus,
    };
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    use std::ffi::OsString;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex, MutexGuard};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct SharedEngineTestGuard;

    impl Drop for SharedEngineTestGuard {
        fn drop(&mut self) {
            set_shared_break_engine_for_tests(None);
            set_snapshot_app_data_dir_for_tests(None);
        }
    }

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn path(&self) -> &std::path::Path {
            self.path.as_path()
        }

        fn join(&self, child: &str) -> PathBuf {
            self.path.join(child)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    struct SingletonTestContext {
        _shared_engine: SharedEngineTestGuard,
        _lock: MutexGuard<'static, ()>,
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    struct HomeEnvGuard {
        previous_home: Option<OsString>,
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    impl HomeEnvGuard {
        fn swap(path: &std::path::Path) -> Self {
            let previous_home = std::env::var_os("HOME");
            unsafe {
                std::env::set_var("HOME", path);
            }
            Self { previous_home }
        }
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    impl Drop for HomeEnvGuard {
        fn drop(&mut self) {
            unsafe {
                match self.previous_home.as_ref() {
                    Some(home) => std::env::set_var("HOME", home),
                    None => std::env::remove_var("HOME"),
                }
            }
        }
    }

    fn test_config(persist_state: bool) -> BreakEngineConfig {
        let mut config = BreakEngineConfig::load();
        config.break_interval = 2;
        config.pre_break_warning_time = 5;
        config.persist_state = persist_state;
        config
    }

    fn unique_test_dir(name: &str) -> TestDir {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("gazeguard-{name}-{suffix}"));
        fs::create_dir_all(&path).unwrap();
        TestDir { path }
    }

    fn create_registered_break_engine_for_test() -> (SharedBreakEngine, SingletonTestContext) {
        let lock = singleton_test_lock().lock().unwrap();
        let engine = create_break_engine();
        set_shared_break_engine_for_tests(Some(engine.clone()));
        (
            engine,
            SingletonTestContext {
                _lock: lock,
                _shared_engine: SharedEngineTestGuard,
            },
        )
    }

    #[test]
    fn apply_desktop_signals_updates_engine_idle_and_fullscreen_state() {
        let mut engine = BreakEngine::new(BreakEngineConfig::load());
        engine.start();

        let status = apply_desktop_signals_to_engine(
            &mut engine,
            crate::desktop_signals::DesktopSignals {
                idle_active: true,
                fullscreen_active: true,
            },
        );

        assert!(matches!(status.phase, EnginePhase::Running));
        let snapshot = engine.snapshot(0);
        assert!(snapshot.idle_active);
        assert!(snapshot.fullscreen);
    }

    #[test]
    fn apply_desktop_signals_clears_idle_and_fullscreen_state() {
        let mut engine = BreakEngine::new(BreakEngineConfig::load());
        engine.start();
        engine.set_idle(true);
        engine.set_fullscreen(true);

        let status = apply_desktop_signals_to_engine(
            &mut engine,
            crate::desktop_signals::DesktopSignals {
                idle_active: false,
                fullscreen_active: false,
            },
        );

        assert!(matches!(status.phase, EnginePhase::Running));
        let snapshot = engine.snapshot(0);
        assert!(!snapshot.idle_active);
        assert!(!snapshot.fullscreen);
    }

    #[test]
    fn startup_engine_is_running() {
        let (engine, _shared_engine) = create_registered_break_engine_for_test();
        let mut engine = engine.lock().unwrap();

        assert!(matches!(engine.status().phase, EnginePhase::Running));
    }

    #[test]
    fn tray_title_shows_remaining_time_and_break_kind() {
        let title = format_tray_title(&EngineStatus {
            phase: EnginePhase::Running,
            seconds_remaining: Some(14 * 60 + 32),
            break_interval_minutes: 15,
            warning_seconds: 10,
            upcoming_break_kind: Some(BreakKind::Short),
            skip_limit_reached: false,
            postpone_reason: None,
            current_break: None,
            can_skip: true,
            can_postpone: false,
            disable_options: Vec::new(),
        });

        assert_eq!(title, "14:32 short");
    }

    #[test]
    fn android_probe_reads_the_shared_engine_phase() {
        let (_engine, _shared_engine) = create_registered_break_engine_for_test();

        assert_eq!(debug_engine_phase_for_android(), "running");
    }

    #[test]
    fn android_probe_can_force_the_shared_engine_into_break_phase() {
        let (engine, _shared_engine) = create_registered_break_engine_for_test();

        assert_eq!(force_break_now_for_android(), "on_break");
        let mut guard = engine.lock().unwrap();
        let status = guard.status();
        assert!(matches!(status.phase, EnginePhase::OnBreak));
        assert!(status.current_break.is_some());
    }

    #[test]
    fn android_overlay_snapshot_reports_active_break_state() {
        let (_engine, _shared_engine) = create_registered_break_engine_for_test();
        assert_eq!(force_break_now_for_android(), "on_break");

        let snapshot = break_overlay_snapshot_for_android();

        assert!(snapshot.contains("\"phase\":\"on_break\""));
        assert!(snapshot.contains("\"remaining_seconds\":15"));
        assert!(snapshot.contains("\"can_postpone\":true"));
        assert!(snapshot.contains("\"postpone_options\":["));
        assert!(snapshot.contains("\"seconds\":300"));
    }

    #[test]
    fn android_snapshot_reports_warning_delivery_state() {
        let (engine, _shared_engine) = create_registered_break_engine_for_test();

        {
            let mut guard = engine.lock().unwrap();
            let work_seconds = guard.config().break_interval * 60;
            let warning_seconds = guard.config().pre_break_warning_time;
            let status = guard.advance_by(work_seconds - warning_seconds);
            assert!(matches!(status.phase, EnginePhase::Warning));
        }

        let snapshot = break_overlay_snapshot_for_android();

        assert!(snapshot.contains("\"phase\":\"warning\""));
        assert!(snapshot.contains("\"remaining_seconds\":10"));
        assert!(snapshot.contains("\"should_show_notification\":true"));
        assert!(snapshot.contains("\"should_show_overlay\":false"));
        assert!(snapshot.contains("Break starts in 10 seconds"));
    }

    #[test]
    fn create_break_engine_restores_saved_snapshot_when_persist_state_is_enabled() {
        let config = test_config(true);
        let app_data_dir = unique_test_dir("restore-enabled");
        let saved_at = 1_000;
        let now = saved_at + 7;
        let mut source_engine = BreakEngine::new(config.clone());
        source_engine.start();
        source_engine.advance_by(30);

        let persisted_engine = Arc::new(Mutex::new(source_engine));
        save_engine_snapshot(&persisted_engine, app_data_dir.path(), saved_at).unwrap();

        let restored = create_break_engine_for_tests(config, app_data_dir.path(), now);
        let mut guard = restored.lock().unwrap();
        let status = guard.status();

        assert!(matches!(status.phase, EnginePhase::Running));
        assert_eq!(status.seconds_remaining, Some(83));
    }

    #[test]
    fn create_break_engine_ignores_saved_snapshot_when_persist_state_is_disabled() {
        let mut persisted_config = test_config(true);
        let app_data_dir = unique_test_dir("restore-disabled");
        let saved_at = 2_000;
        let now = saved_at + 9;
        let mut source_engine = BreakEngine::new(persisted_config.clone());
        source_engine.start();
        source_engine.advance_by(30);

        let persisted_engine = Arc::new(Mutex::new(source_engine));
        save_engine_snapshot(&persisted_engine, app_data_dir.path(), saved_at).unwrap();

        persisted_config.persist_state = false;
        let restored = create_break_engine_for_tests(persisted_config, app_data_dir.path(), now);
        let mut guard = restored.lock().unwrap();
        let status = guard.status();

        assert!(matches!(status.phase, EnginePhase::Running));
        assert_eq!(status.seconds_remaining, Some(120));
    }

    #[test]
    fn create_break_engine_falls_back_to_fresh_state_when_snapshot_is_corrupt() {
        let config = test_config(true);
        let app_data_dir = unique_test_dir("restore-corrupt");
        fs::write(app_data_dir.join(SNAPSHOT_FILE_NAME), "{not valid json").unwrap();

        let restored = create_break_engine_for_tests(config, app_data_dir.path(), 3_000);
        let mut guard = restored.lock().unwrap();
        let status = guard.status();

        assert!(matches!(status.phase, EnginePhase::Running));
        assert_eq!(status.seconds_remaining, Some(120));
    }

    #[test]
    fn save_engine_snapshot_removes_stale_snapshot_when_persistence_is_disabled() {
        let app_data_dir = unique_test_dir("clear-stale-snapshot");
        let saved_at = 4_000;
        let snapshot_file = app_data_dir.join(SNAPSHOT_FILE_NAME);

        let mut persisted_engine = BreakEngine::new(test_config(true));
        persisted_engine.start();
        persisted_engine.advance_by(30);
        let persisted_engine = Arc::new(Mutex::new(persisted_engine));
        save_engine_snapshot(&persisted_engine, app_data_dir.path(), saved_at).unwrap();
        assert!(snapshot_file.exists());

        let mut disabled_engine = BreakEngine::new(test_config(false));
        disabled_engine.start();
        disabled_engine.advance_by(45);
        let disabled_engine = Arc::new(Mutex::new(disabled_engine));
        save_engine_snapshot(&disabled_engine, app_data_dir.path(), saved_at + 5).unwrap();
        assert!(!snapshot_file.exists());

        let restored =
            create_break_engine_for_tests(test_config(true), app_data_dir.path(), saved_at + 10);
        let mut guard = restored.lock().unwrap();
        let status = guard.status();

        assert!(matches!(status.phase, EnginePhase::Running));
        assert_eq!(status.seconds_remaining, Some(120));
    }

    #[test]
    fn apply_reloaded_config_returns_error_when_engine_lock_is_poisoned() {
        let engine = Arc::new(Mutex::new(BreakEngine::new(BreakEngineConfig::load())));
        let poisoned = engine.clone();
        let _ = std::thread::spawn(move || {
            let _guard = poisoned.lock().unwrap();
            panic!("poison engine lock");
        })
        .join();

        let result = apply_reloaded_config(&engine, BreakEngineConfig::load());

        assert_eq!(result, Err("State lock poisoned".to_string()));
    }

    #[test]
    fn save_registered_engine_snapshot_uses_registered_state() {
        let _lock = singleton_test_lock().lock().unwrap();
        let _shared_engine = SharedEngineTestGuard;
        let app_data_dir = unique_test_dir("registered-shutdown-save");
        let saved_at = 5_000;
        let snapshot_file = app_data_dir.join(SNAPSHOT_FILE_NAME);

        let mut engine = BreakEngine::new(test_config(true));
        engine.start();
        engine.advance_by(20);
        let engine = Arc::new(Mutex::new(engine));
        set_shared_break_engine_for_tests(Some(engine));
        set_snapshot_app_data_dir_for_tests(Some(app_data_dir.path().to_path_buf()));

        save_registered_engine_snapshot(saved_at).unwrap();

        assert!(snapshot_file.exists());
    }

    #[test]
    fn save_registered_engine_snapshot_updates_runtime_without_shutdown() {
        let _lock = singleton_test_lock().lock().unwrap();
        let _shared_engine = SharedEngineTestGuard;
        let app_data_dir = unique_test_dir("registered-runtime-save");
        let saved_at = 6_000;
        let mut source_engine = BreakEngine::new(test_config(true));
        source_engine.start();
        source_engine.advance_by(30);
        let engine = Arc::new(Mutex::new(source_engine));
        set_shared_break_engine_for_tests(Some(engine.clone()));
        set_snapshot_app_data_dir_for_tests(Some(app_data_dir.path().to_path_buf()));

        save_registered_engine_snapshot(saved_at).unwrap();

        {
            let mut guard = engine.lock().unwrap();
            guard.advance_by(10);
        }
        save_registered_engine_snapshot(saved_at + 10).unwrap();

        let restored =
            create_break_engine_for_tests(test_config(true), app_data_dir.path(), saved_at + 10);
        let mut guard = restored.lock().unwrap();
        let status = guard.status();

        assert!(matches!(status.phase, EnginePhase::Running));
        assert_eq!(status.seconds_remaining, Some(80));
    }

    #[test]
    fn reload_runtime_config_command_updates_registered_engine() {
        let temp = unique_test_dir("reload-runtime-config");
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        let _home_guard = HomeEnvGuard::swap(temp.path());

        #[cfg(desktop)]
        let config_path = crate::config_file::desktop_config_path_from_home(temp.path());
        #[cfg(not(desktop))]
        let config_path = temp
            .path()
            .join("config")
            .join(crate::config_file::CONFIG_FILE_NAME);
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::write(
            &config_path,
            "short_break_interval: 15\nlong_break_interval: 75\nlong_break_duration: 60\npre_break_warning_time: 10\nshort_break_duration: 15\nstrict_break: false\n",
        )
        .unwrap();

        let engine = Arc::new(Mutex::new(BreakEngine::new(BreakEngineConfig::load())));
        set_shared_break_engine_for_tests(Some(engine.clone()));
        set_snapshot_app_data_dir_for_tests(Some(temp.path().to_path_buf()));

        let result = reload_runtime_config_for_tests(engine.clone()).unwrap();

        assert_eq!(result.break_interval_minutes, 15);
        let guard = engine.lock().unwrap();
        assert_eq!(guard.config().break_interval, 15);
        assert_eq!(guard.config().pre_break_warning_time, 10);
    }
}
