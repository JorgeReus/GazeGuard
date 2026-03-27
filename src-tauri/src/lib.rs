#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod break_engine;

use serde::Serialize;
use serde_json::json;
use std::thread;
use std::time::Duration;
use std::sync::{Arc, Mutex, OnceLock};
use break_engine::{BreakEngine, BreakEngineConfig, BreakInfo, DisableOption, EnginePhase, EngineStatus};
use tauri::Manager;
use tauri::State;

#[cfg(desktop)]
use tauri::{menu::{Menu, MenuItem}, tray::TrayIconBuilder};

type SharedBreakEngine = Arc<Mutex<BreakEngine>>;

static SHARED_BREAK_ENGINE: OnceLock<Mutex<Option<SharedBreakEngine>>> = OnceLock::new();

#[derive(Debug, Serialize)]
struct BreakSchedule {
    break_interval_minutes: u64,
    pre_break_warning_seconds: u64,
    disable_options: Vec<DisableOption>,
}

#[derive(Debug, Serialize)]
struct AndroidBreakOverlaySnapshot {
    phase: String,
    remaining_seconds: u64,
    message: String,
    should_show_notification: bool,
    should_show_overlay: bool,
}

fn shared_break_engine_slot() -> &'static Mutex<Option<SharedBreakEngine>> {
    SHARED_BREAK_ENGINE.get_or_init(|| Mutex::new(None))
}

fn register_shared_break_engine(engine: SharedBreakEngine) {
    if let Ok(mut slot) = shared_break_engine_slot().lock() {
        *slot = Some(engine);
    }
}

fn get_shared_break_engine() -> Option<SharedBreakEngine> {
    shared_break_engine_slot()
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

fn break_overlay_snapshot_for_android() -> String {
    let Some(engine) = get_shared_break_engine() else {
        return json!({
            "phase": "unavailable",
            "remaining_seconds": 0,
            "message": "Break unavailable",
            "should_show_notification": false,
            "should_show_overlay": false
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
                            info.template_name.clone().unwrap_or_else(|| match info.kind {
                                break_engine::BreakKind::Long => "Take a Long Break".to_string(),
                                break_engine::BreakKind::Short => "Take a Short Break".to_string(),
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
            })
            .unwrap_or_else(|_| {
                json!({
                    "phase": "error",
                    "remaining_seconds": 0,
                    "message": "Break unavailable",
                    "should_show_notification": false,
                    "should_show_overlay": false
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
                "should_show_overlay": false
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

fn create_break_engine() -> SharedBreakEngine {
    let mut engine = BreakEngine::new(BreakEngineConfig::load());
    engine.start();
    let engine = Arc::new(Mutex::new(engine));
    register_shared_break_engine(engine.clone());
    engine
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
}

#[cfg(not(desktop))]
fn refresh_tray_title(_app: &tauri::AppHandle) {
}

#[cfg(desktop)]
fn spawn_tray_updater(app: tauri::AppHandle) {
    thread::spawn(move || loop {
        refresh_tray_title(&app);
        thread::sleep(Duration::from_secs(1));
    });
}

#[cfg(not(desktop))]
fn spawn_tray_updater(_app: tauri::AppHandle) {
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
    Ok(guard.status())
}

#[tauri::command]
fn get_break_schedule(state: State<'_, SharedBreakEngine>) -> Result<BreakSchedule, String> {
    let guard = state.lock().map_err(|_| "State lock poisoned")?;
    let config = guard.config();
    Ok(BreakSchedule {
        break_interval_minutes: config.break_interval,
        pre_break_warning_seconds: config.pre_break_warning_time,
        disable_options: config.disable_options.clone(),
    })
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
    Ok(guard.status())
}

#[tauri::command]
fn set_fullscreen_active(
    state: State<'_, SharedBreakEngine>,
    active: bool,
) -> Result<EngineStatus, String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    guard.set_fullscreen(active);
    Ok(guard.status())
}

#[tauri::command]
fn sync_desktop_window_state(
    app: tauri::AppHandle,
    state: State<'_, SharedBreakEngine>,
) -> Result<EngineStatus, String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;

    #[cfg(not(desktop))]
    let _ = &app;

    #[cfg(desktop)]
    let (fullscreen_active, idle_active) = app
        .get_webview_window("main")
        .map(|window| {
            let fullscreen = window.is_fullscreen().unwrap_or(false);
            let focused = window.is_focused().unwrap_or(true);
            let minimized = window.is_minimized().unwrap_or(false);
            let visible = window.is_visible().unwrap_or(true);
            (fullscreen, !focused || minimized || !visible)
        })
        .unwrap_or((false, false));

    #[cfg(not(desktop))]
    let (fullscreen_active, idle_active) = (false, false);

    guard.set_idle(idle_active);
    guard.set_fullscreen(fullscreen_active);
    Ok(guard.status())
}

#[tauri::command]
fn disable_reminders(
    state: State<'_, SharedBreakEngine>,
    seconds: u64,
) -> Result<EngineStatus, String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    guard.disable_for(seconds)
}

#[tauri::command]
fn skip_break(
    app: tauri::AppHandle,
    state: State<'_, SharedBreakEngine>,
) -> Result<(), String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    let skip_result = guard.skip_break();
    drop(guard);

    if skip_result.is_ok() {
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
        // Check if break window already exists
        if let Some(existing) = app.get_webview_window("break") {
            existing.close().ok();
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
            main_window.eval("window.location.href = 'break.html';")
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
            main_window.eval("window.location.href = 'index.html';")
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(create_break_engine());

    // Add android plugin
    // #[cfg(target_os = "android")]
    //  {
    //      builder = builder.plugin(mobile::init());
    //  }

    builder
        .setup(|app| {
            #[cfg(not(desktop))]
            let _ = &app;

            #[cfg(desktop)]
            {
                // Create tray menu
                let test_break = MenuItem::with_id(app, "test_break", "Test Break", true, None::<&str>)?;
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
                spawn_tray_updater(app.handle().clone());

                // Hide the main window on startup (tray only mode)
                if let Some(window) = app.get_webview_window("main") {
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
            get_current_break_info,
            set_idle_active,
            set_fullscreen_active,
            sync_desktop_window_state,
            disable_reminders,
            skip_break,
            complete_break
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{
        break_overlay_snapshot_for_android, create_break_engine, debug_engine_phase_for_android,
        force_break_now_for_android,
        format_tray_title, set_shared_break_engine_for_tests,
    };
    use crate::break_engine::{BreakKind, EnginePhase, EngineStatus};

    #[test]
    fn startup_engine_is_running() {
        let engine = create_break_engine();
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
            disable_options: Vec::new(),
        });

        assert_eq!(title, "14:32 short");
    }

    #[test]
    fn android_probe_reads_the_shared_engine_phase() {
        let engine = create_break_engine();
        set_shared_break_engine_for_tests(Some(engine.clone()));

        assert_eq!(debug_engine_phase_for_android(), "running");

        set_shared_break_engine_for_tests(None);
    }

    #[test]
    fn android_probe_can_force_the_shared_engine_into_break_phase() {
        let engine = create_break_engine();
        set_shared_break_engine_for_tests(Some(engine.clone()));

        assert_eq!(force_break_now_for_android(), "on_break");
        let mut guard = engine.lock().unwrap();
        let status = guard.status();
        assert!(matches!(status.phase, EnginePhase::OnBreak));
        assert!(status.current_break.is_some());

        set_shared_break_engine_for_tests(None);
    }

    #[test]
    fn android_overlay_snapshot_reports_active_break_state() {
        let engine = create_break_engine();
        set_shared_break_engine_for_tests(Some(engine.clone()));
        assert_eq!(force_break_now_for_android(), "on_break");

        let snapshot = break_overlay_snapshot_for_android();

        assert!(snapshot.contains("\"phase\":\"on_break\""));
        assert!(snapshot.contains("\"remaining_seconds\":15"));

        set_shared_break_engine_for_tests(None);
    }

    #[test]
    fn android_snapshot_reports_warning_delivery_state() {
        let engine = create_break_engine();
        set_shared_break_engine_for_tests(Some(engine.clone()));

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

        set_shared_break_engine_for_tests(None);
    }
}
