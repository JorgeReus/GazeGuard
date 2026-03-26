#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod break_engine;

use serde::Serialize;
use std::sync::Mutex;
use break_engine::{BreakEngine, BreakEngineConfig, BreakInfo, DisableOption, EngineStatus};
use tauri::Manager;
use tauri::State;

#[cfg(desktop)]
use tauri::{menu::{Menu, MenuItem}, tray::TrayIconBuilder};

#[derive(Debug, Serialize)]
struct BreakSchedule {
    break_interval_minutes: u64,
    pre_break_warning_seconds: u64,
    disable_options: Vec<DisableOption>,
}

#[tauri::command]
fn start_break_timer(state: State<'_, Mutex<BreakEngine>>) -> Result<EngineStatus, String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    Ok(guard.start())
}

#[tauri::command]
fn stop_break_timer(state: State<'_, Mutex<BreakEngine>>) -> Result<EngineStatus, String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    Ok(guard.stop())
}

#[tauri::command]
fn get_engine_status(state: State<'_, Mutex<BreakEngine>>) -> Result<EngineStatus, String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    Ok(guard.status())
}

#[tauri::command]
fn get_break_schedule(state: State<'_, Mutex<BreakEngine>>) -> Result<BreakSchedule, String> {
    let guard = state.lock().map_err(|_| "State lock poisoned")?;
    let config = guard.config();
    Ok(BreakSchedule {
        break_interval_minutes: config.break_interval,
        pre_break_warning_seconds: config.pre_break_warning_time,
        disable_options: config.disable_options.clone(),
    })
}

#[tauri::command]
fn get_current_break_info(state: State<'_, Mutex<BreakEngine>>) -> Result<BreakInfo, String> {
    let guard = state.lock().map_err(|_| "State lock poisoned")?;
    guard
        .current_break()
        .ok_or_else(|| "No active break is available.".to_string())
}

#[tauri::command]
fn set_idle_active(
    state: State<'_, Mutex<BreakEngine>>,
    active: bool,
) -> Result<EngineStatus, String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    guard.set_idle(active);
    Ok(guard.status())
}

#[tauri::command]
fn set_fullscreen_active(
    state: State<'_, Mutex<BreakEngine>>,
    active: bool,
) -> Result<EngineStatus, String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    guard.set_fullscreen(active);
    Ok(guard.status())
}

#[tauri::command]
fn sync_desktop_window_state(
    app: tauri::AppHandle,
    state: State<'_, Mutex<BreakEngine>>,
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
    state: State<'_, Mutex<BreakEngine>>,
    seconds: u64,
) -> Result<EngineStatus, String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    guard.disable_for(seconds)
}

#[tauri::command]
fn skip_break(
    app: tauri::AppHandle,
    state: State<'_, Mutex<BreakEngine>>,
) -> Result<(), String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    guard.skip_break()?;

    drop(guard);
    close_break_window(app)
}

#[tauri::command]
fn complete_break(
    app: tauri::AppHandle,
    state: State<'_, Mutex<BreakEngine>>,
) -> Result<(), String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    guard.complete_break()?;

    drop(guard);
    close_break_window(app)
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
    state: State<'_, Mutex<BreakEngine>>,
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
        .manage(Mutex::new(BreakEngine::new(BreakEngineConfig::load())));

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

                let _tray = TrayIconBuilder::new()
                    .icon(app.default_window_icon().unwrap().clone())
                    .menu(&menu)
                    .on_menu_event(|app, event| {
                        match event.id.as_ref() {
                            "test_break" => {
                                if let Ok(mut guard) = app.state::<Mutex<BreakEngine>>().lock() {
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
