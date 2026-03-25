#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;
use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use tauri::State;

#[cfg(desktop)]
use tauri::{menu::{Menu, MenuItem}, tray::TrayIconBuilder};

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
enum BreakKind {
    Short,
    Long,
}

#[derive(Debug, Serialize)]
struct BreakInfo {
    kind: BreakKind,
    duration_seconds: u64,
    mandatory: bool,
}

#[derive(Debug, Serialize)]
struct BreakSchedule {
    break_interval_minutes: u64,
}

#[derive(Debug)]
struct BreakConfig {
    skip_limit: u8,            // 2
    short_duration_seconds: u64,
    long_duration_seconds: u64,
    long_every_n_shorts: u8,
    break_interval_minutes: u64,
}

#[derive(Debug)]
struct BreakState {
    config: BreakConfig,
    skip_streak: u8,
    shorts_since_long: u8,
    // optional: remember what break is currently being shown
    current: Option<BreakInfo>,
}

impl BreakState {
    fn new() -> Self {
        let safe_eyes = SafeEyesConfig::load();
        Self {
            config: BreakConfig::from_safe_eyes(&safe_eyes),
            skip_streak: 0,
            shorts_since_long: 0,
            current: None,
        }
    }

    fn compute_next_break(&mut self) -> BreakInfo {
        let mandatory = self.skip_streak >= self.config.skip_limit;

        let kind = if self.shorts_since_long >= self.config.long_every_n_shorts {
            self.shorts_since_long = 0;
            BreakKind::Long
        } else {
            self.shorts_since_long += 1;
            BreakKind::Short
        };

        let duration_seconds = match kind {
            BreakKind::Short => self.config.short_duration_seconds,
            BreakKind::Long => self.config.long_duration_seconds,
        };

        let info = BreakInfo {
            kind,
            duration_seconds,
            mandatory,
        };

        self.current = Some(BreakInfo {
            kind: info.kind,
            duration_seconds: info.duration_seconds,
            mandatory: info.mandatory,
        });

        info
    }
}

impl BreakConfig {
    fn from_safe_eyes(config: &SafeEyesConfig) -> Self {
        Self {
            skip_limit: if config.strict_break { 0 } else { 2 },
            short_duration_seconds: config.short_break_duration,
            long_duration_seconds: config.long_break_duration,
            long_every_n_shorts: config.no_of_short_breaks_per_long_break,
            break_interval_minutes: config.break_interval,
        }
    }
}

#[derive(Debug, Deserialize)]
struct SafeEyesConfig {
    break_interval: u64,
    long_break_duration: u64,
    no_of_short_breaks_per_long_break: u8,
    short_break_duration: u64,
    strict_break: bool,
}

impl SafeEyesConfig {
    fn load() -> Self {
        serde_json::from_str(include_str!("../gen/android/app/src/main/assets/config/safeeyes.json"))
            .expect("safeeyes config should be valid JSON")
    }
}

#[cfg(test)]
mod tests {
    use super::{BreakConfig, BreakKind, BreakState, SafeEyesConfig};

    #[test]
    fn loads_safe_eyes_json_shape() {
        let config = SafeEyesConfig::load();
        let break_config = BreakConfig::from_safe_eyes(&config);

        assert_eq!(config.break_interval, 15);
        assert_eq!(break_config.break_interval_minutes, 15);
        assert_eq!(break_config.short_duration_seconds, 15);
        assert_eq!(break_config.long_duration_seconds, 60);
        assert_eq!(break_config.long_every_n_shorts, 5);
    }

    #[test]
    fn uses_safe_eyes_break_distribution() {
        let mut state = BreakState::new();

        for index in 1..=5 {
            let info = state.compute_next_break();
            assert!(matches!(info.kind, BreakKind::Short), "break {index} should be short");
            assert_eq!(info.duration_seconds, 15, "break {index} should last 15 seconds");
        }

        let info = state.compute_next_break();
        assert!(matches!(info.kind, BreakKind::Long), "6th break should be long");
        assert_eq!(info.duration_seconds, 60, "long break should last 60 seconds");
    }
}

#[tauri::command]
fn get_next_break_info(state: State<'_, Mutex<BreakState>>) -> Result<BreakInfo, String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    Ok(guard.compute_next_break())
}

#[tauri::command]
fn get_break_schedule(state: State<'_, Mutex<BreakState>>) -> Result<BreakSchedule, String> {
    let guard = state.lock().map_err(|_| "State lock poisoned")?;
    Ok(BreakSchedule {
        break_interval_minutes: guard.config.break_interval_minutes,
    })
}

#[tauri::command]
fn skip_break(
    app: tauri::AppHandle,
    state: State<'_, Mutex<BreakState>>,
) -> Result<(), String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;

    let mandatory = guard
        .current
        .as_ref()
        .map(|b| b.mandatory)
        .unwrap_or(false);

    if mandatory {
        return Err("This break is mandatory; skipping is disabled.".into());
    }

    guard.skip_streak = guard.skip_streak.saturating_add(1);

    drop(guard);
    close_break_window(app)
}

#[tauri::command]
fn complete_break(
    app: tauri::AppHandle,
    state: State<'_, Mutex<BreakState>>,
) -> Result<(), String> {
    let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
    guard.skip_streak = 0;
    guard.current = None;

    drop(guard);
    close_break_window(app)
}

// #[cfg(target_os = "android")]
// mod mobile;

#[tauri::command]
fn start_background_service(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        // Just a proof that the command exists and is executed.
        // We'll wire AndroidBridge after we confirm invocation works.
        if let Some(w) = app.get_webview_window("main") {
            w.eval("console.log('Rust: start_background_service invoked');")
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
fn stop_background_service(app: tauri::AppHandle) -> Result<(), String> {
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
fn show_break_window(app: tauri::AppHandle) -> Result<(), String> {
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
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(Mutex::new(BreakState::new()));

    // Add android plugin
    // #[cfg(target_os = "android")]
    //  {
    //      builder = builder.plugin(mobile::init());
    //  }

    builder
        .setup(|app| {
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
                                let _ = show_break_window(app.clone());
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
            get_break_schedule,
            get_next_break_info,
            skip_break,
            complete_break
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
