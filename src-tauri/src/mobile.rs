use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime, Manager,
};

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("gazeguard")
        .invoke_handler(tauri::generate_handler![
            start_background_service,
            stop_background_service,
        ])
        .build()
}

#[tauri::command]
fn start_background_service<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        // Call via JavaScript bridge to MainActivity
        if let Some(window) = app.get_webview_window("main") {
            window.eval(
                r#"
                if (window.startBreakService) {
                    window.startBreakService();
                }
                "#
            ).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
fn stop_background_service<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        if let Some(window) = app.get_webview_window("main") {
            window.eval(
                r#"
                if (window.stopBreakService) {
                    window.stopBreakService();
                }
                "#
            ).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
