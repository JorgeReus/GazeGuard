use tauri::Manager;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesktopSignals {
    pub fullscreen_active: bool,
    pub idle_active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowStateSnapshot {
    pub fullscreen: bool,
    pub focused: bool,
    pub minimized: bool,
    pub visible: bool,
}

pub fn fallback_from_window_state(window: WindowStateSnapshot) -> DesktopSignals {
    DesktopSignals {
        fullscreen_active: window.fullscreen,
        idle_active: !window.focused || window.minimized || !window.visible,
    }
}

fn desktop_signals_without_window() -> DesktopSignals {
    DesktopSignals {
        fullscreen_active: false,
        idle_active: false,
    }
}

fn desktop_signals_from_window_state(window: Option<WindowStateSnapshot>) -> DesktopSignals {
    window
        .map(fallback_from_window_state)
        .unwrap_or_else(desktop_signals_without_window)
}

#[cfg(desktop)]
fn desktop_window_snapshot(app: &tauri::AppHandle) -> Option<WindowStateSnapshot> {
    app.get_webview_window("main").map(|window| WindowStateSnapshot {
        fullscreen: window.is_fullscreen().unwrap_or(false),
        focused: window.is_focused().unwrap_or(true),
        minimized: window.is_minimized().unwrap_or(false),
        visible: window.is_visible().unwrap_or(true),
    })
}

#[cfg(desktop)]
fn desktop_signals_from_desktop_window(app: &tauri::AppHandle) -> DesktopSignals {
    desktop_signals_from_window_state(desktop_window_snapshot(app))
}

const NATIVE_IDLE_SIGNAL_FLOOR_SECONDS: f64 = 1.0;

fn idle_active_from_seconds(idle_seconds: f64) -> bool {
    idle_seconds >= NATIVE_IDLE_SIGNAL_FLOOR_SECONDS
}

fn linux_prefers_wayland_session(
    xdg_session_type: Option<&str>,
    wayland_display: Option<&str>,
) -> bool {
    matches!(xdg_session_type, Some(value) if value.eq_ignore_ascii_case("wayland"))
        || wayland_display.is_some_and(|value| !value.trim().is_empty())
}

fn linux_native_idle_active_from_env(
    xdg_session_type: Option<&str>,
    wayland_display: Option<&str>,
) -> Option<bool> {
    if !linux_should_query_native_idle(xdg_session_type, wayland_display) {
        return None;
    }

    None
}

fn linux_should_query_native_idle(
    xdg_session_type: Option<&str>,
    wayland_display: Option<&str>,
) -> bool {
    linux_prefers_wayland_session(xdg_session_type, wayland_display)
}

fn linux_idle_from_sources(
    fallback: DesktopSignals,
    native_idle_active: Option<bool>,
) -> DesktopSignals {
    DesktopSignals {
        fullscreen_active: fallback.fullscreen_active,
        idle_active: native_idle_active.unwrap_or(fallback.idle_active),
    }
}

#[cfg(all(desktop, target_os = "linux"))]
mod platform {
    use super::{
        desktop_signals_from_desktop_window, linux_idle_from_sources,
        linux_native_idle_active_from_env, DesktopSignals,
    };

    fn native_idle_active() -> Option<bool> {
        linux_native_idle_active_from_env(
            std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
            std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
        )
    }

    pub fn collect(app: &tauri::AppHandle, _idle_threshold_seconds: u64) -> DesktopSignals {
        let fallback = desktop_signals_from_desktop_window(app);
        linux_idle_from_sources(fallback, native_idle_active())
    }
}

#[cfg(all(desktop, target_os = "macos"))]
mod platform {
    use super::{desktop_signals_from_desktop_window, idle_active_from_seconds, DesktopSignals};
    use core_graphics::event_source::CGEventSourceStateID;

    pub(super) const ANY_INPUT_EVENT_TYPE: u32 = u32::MAX;

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(
            state_id: CGEventSourceStateID,
            event_type: u32,
        ) -> f64;
    }

    fn native_idle_seconds() -> Option<f64> {
        Some(unsafe {
            CGEventSourceSecondsSinceLastEventType(
                CGEventSourceStateID::CombinedSessionState,
                ANY_INPUT_EVENT_TYPE,
            )
        })
    }

    pub fn collect(app: &tauri::AppHandle, idle_threshold_seconds: u64) -> DesktopSignals {
        let fallback = desktop_signals_from_desktop_window(app);
        let _ = idle_threshold_seconds;
        let idle_active = native_idle_seconds()
            .map(super::idle_active_from_seconds)
            .unwrap_or(fallback.idle_active);

        DesktopSignals {
            fullscreen_active: fallback.fullscreen_active,
            idle_active,
        }
    }
}

#[cfg(all(desktop, not(target_os = "macos"), not(target_os = "linux")))]
mod platform {
    use super::{desktop_signals_from_desktop_window, DesktopSignals};

    pub fn collect(app: &tauri::AppHandle, _idle_threshold_seconds: u64) -> DesktopSignals {
        desktop_signals_from_desktop_window(app)
    }
}

#[cfg(desktop)]
pub fn collect_desktop_signals(
    app: &tauri::AppHandle,
    idle_threshold_seconds: u64,
) -> DesktopSignals {
    platform::collect(app, idle_threshold_seconds)
}

#[cfg(not(desktop))]
pub fn collect_desktop_signals(
    _app: &tauri::AppHandle,
    _idle_threshold_seconds: u64,
) -> DesktopSignals {
    desktop_signals_without_window()
}

#[cfg(test)]
mod tests {
    use super::{
        desktop_signals_from_window_state, fallback_from_window_state, idle_active_from_seconds,
        linux_idle_from_sources, linux_native_idle_active_from_env, linux_prefers_wayland_session,
        linux_should_query_native_idle, DesktopSignals, WindowStateSnapshot,
    };

    #[cfg(all(desktop, target_os = "macos"))]
    use super::platform::ANY_INPUT_EVENT_TYPE;

    #[test]
    fn fallback_marks_idle_when_window_is_not_focused() {
        let signals = fallback_from_window_state(WindowStateSnapshot {
            fullscreen: false,
            focused: false,
            minimized: false,
            visible: true,
        });

        assert_eq!(
            signals,
            DesktopSignals {
                fullscreen_active: false,
                idle_active: true,
            }
        );
    }

    #[test]
    fn fallback_preserves_fullscreen_when_window_is_visible_and_focused() {
        let signals = fallback_from_window_state(WindowStateSnapshot {
            fullscreen: true,
            focused: true,
            minimized: false,
            visible: true,
        });

        assert_eq!(
            signals,
            DesktopSignals {
                fullscreen_active: true,
                idle_active: false,
            }
        );
    }

    #[test]
    fn fallback_marks_idle_when_window_is_minimized() {
        let signals = fallback_from_window_state(WindowStateSnapshot {
            fullscreen: false,
            focused: true,
            minimized: true,
            visible: true,
        });

        assert_eq!(
            signals,
            DesktopSignals {
                fullscreen_active: false,
                idle_active: true,
            }
        );
    }

    #[test]
    fn fallback_marks_idle_when_window_is_hidden() {
        let signals = fallback_from_window_state(WindowStateSnapshot {
            fullscreen: false,
            focused: true,
            minimized: false,
            visible: false,
        });

        assert_eq!(
            signals,
            DesktopSignals {
                fullscreen_active: false,
                idle_active: true,
            }
        );
    }

    #[test]
    fn no_window_defaults_to_false_false() {
        let signals = desktop_signals_from_window_state(None);

        assert_eq!(
            signals,
            DesktopSignals {
                fullscreen_active: false,
                idle_active: false,
            }
        );
    }

    #[test]
    fn idle_threshold_uses_elapsed_idle_seconds() {
        assert!(!idle_active_from_seconds(0.0));
        assert!(!idle_active_from_seconds(0.5));
        assert!(idle_active_from_seconds(1.0));
        assert!(idle_active_from_seconds(9.5));
    }

    #[test]
    fn native_idle_treats_active_input_as_not_idle_even_if_window_heuristic_would() {
        let fallback = fallback_from_window_state(WindowStateSnapshot {
            fullscreen: false,
            focused: false,
            minimized: false,
            visible: true,
        });

        assert!(fallback.idle_active);
        assert!(!idle_active_from_seconds(0.0));
    }

    #[test]
    fn native_idle_hands_threshold_timing_to_the_engine() {
        assert!(!idle_active_from_seconds(0.9));
        assert!(idle_active_from_seconds(1.0));
        assert!(idle_active_from_seconds(60.0));
    }

    #[test]
    fn linux_provider_uses_fallback_when_native_idle_is_unavailable() {
        let fallback = DesktopSignals {
            fullscreen_active: true,
            idle_active: false,
        };

        let signals = linux_idle_from_sources(fallback, None);

        assert_eq!(signals, fallback);
    }

    #[test]
    fn linux_provider_overrides_fallback_idle_when_native_idle_reports_inactive_input() {
        let fallback = DesktopSignals {
            fullscreen_active: false,
            idle_active: true,
        };

        let signals = linux_idle_from_sources(fallback, Some(false));

        assert_eq!(
            signals,
            DesktopSignals {
                fullscreen_active: false,
                idle_active: false,
            }
        );
    }

    #[test]
    fn linux_wayland_detection_accepts_wayland_session_type() {
        assert!(linux_prefers_wayland_session(Some("wayland"), None));
    }

    #[test]
    fn linux_wayland_detection_rejects_non_wayland_sessions() {
        assert!(!linux_prefers_wayland_session(Some("x11"), None));
        assert!(!linux_prefers_wayland_session(None, Some("")));
        assert!(!linux_prefers_wayland_session(None, None));
    }

    #[test]
    fn linux_native_idle_helper_falls_back_when_wayland_session_is_unavailable() {
        assert_eq!(linux_native_idle_active_from_env(Some("x11"), None), None);
        assert_eq!(linux_native_idle_active_from_env(None, Some("")), None);
        assert_eq!(linux_native_idle_active_from_env(None, None), None);
    }

    #[test]
    fn linux_native_idle_helper_is_ready_to_query_when_wayland_session_exists() {
        assert!(linux_should_query_native_idle(Some("wayland"), None));
        assert!(linux_should_query_native_idle(None, Some("wayland-0")));
        assert_eq!(linux_native_idle_active_from_env(Some("wayland"), None), None);
        assert_eq!(linux_native_idle_active_from_env(None, Some("wayland-0")), None);
    }

    #[cfg(all(desktop, target_os = "macos"))]
    #[test]
    fn macos_idle_query_uses_any_input_event_type_constant() {
        assert_eq!(ANY_INPUT_EVENT_TYPE, u32::MAX);
    }
}
