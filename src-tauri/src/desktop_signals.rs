#![allow(dead_code, clashing_extern_declarations)]

use std::fmt::Arguments;

use crate::logger::{self, LogLevel};
#[cfg(desktop)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScreenRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

const DESKTOP_SIGNALS_LOG_TARGET: &str = "desktop_signals";

fn log_desktop_signals(level: LogLevel, configured_level: LogLevel, args: Arguments<'_>) {
    logger::log(level, configured_level, DESKTOP_SIGNALS_LOG_TARGET, args);
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
fn desktop_window_snapshot(
    app: &tauri::AppHandle,
    configured_level: LogLevel,
) -> Option<WindowStateSnapshot> {
    let snapshot = app
        .get_webview_window("main")
        .map(|window| WindowStateSnapshot {
            fullscreen: window.is_fullscreen().unwrap_or(false),
            focused: window.is_focused().unwrap_or(true),
            minimized: window.is_minimized().unwrap_or(false),
            visible: window.is_visible().unwrap_or(true),
        });

    match snapshot {
        Some(snapshot) => {
            log_desktop_signals(
                LogLevel::Debug,
                configured_level,
                format_args!("fallback_window_snapshot={snapshot:?}"),
            );
            Some(snapshot)
        }
        None => {
            log_desktop_signals(
                LogLevel::Trace,
                configured_level,
                format_args!("fallback_window_snapshot unavailable: main window not found"),
            );
            None
        }
    }
}

#[cfg(desktop)]
fn desktop_signals_from_desktop_window(
    app: &tauri::AppHandle,
    configured_level: LogLevel,
) -> DesktopSignals {
    let signals = desktop_signals_from_window_state(desktop_window_snapshot(app, configured_level));
    log_desktop_signals(
        LogLevel::Debug,
        configured_level,
        format_args!("fallback_desktop_signals={signals:?}"),
    );
    signals
}

#[cfg(desktop)]
trait DesktopSignalProvider {
    fn collect(
        &self,
        app: &tauri::AppHandle,
        idle_threshold_seconds: u64,
        configured_level: LogLevel,
    ) -> DesktopSignals;
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

fn linux_idle_from_sources_logged(
    fallback: DesktopSignals,
    native_idle_active: Option<bool>,
    configured_level: LogLevel,
) -> DesktopSignals {
    match native_idle_active {
        Some(native_idle_active) => log_desktop_signals(
            LogLevel::Debug,
            configured_level,
            format_args!("native_idle_result={native_idle_active}"),
        ),
        None => log_desktop_signals(
            LogLevel::Trace,
            configured_level,
            format_args!(
                "native_idle_result unavailable; falling back to window idle_active={}",
                fallback.idle_active
            ),
        ),
    }

    let signals = linux_idle_from_sources(fallback, native_idle_active);
    log_desktop_signals(
        LogLevel::Debug,
        configured_level,
        format_args!("merged_desktop_signals={signals:?}"),
    );
    signals
}

fn windows_signals_from_sources(
    fallback: DesktopSignals,
    native_idle_active: Option<bool>,
    native_fullscreen_active: Option<bool>,
) -> DesktopSignals {
    DesktopSignals {
        fullscreen_active: native_fullscreen_active.unwrap_or(fallback.fullscreen_active),
        idle_active: native_idle_active.unwrap_or(fallback.idle_active),
    }
}

fn windows_signals_from_sources_logged(
    fallback: DesktopSignals,
    native_idle_active: Option<bool>,
    native_fullscreen_active: Option<bool>,
    configured_level: LogLevel,
) -> DesktopSignals {
    match native_idle_active {
        Some(native_idle_active) => log_desktop_signals(
            LogLevel::Debug,
            configured_level,
            format_args!("native_idle_result={native_idle_active}"),
        ),
        None => log_desktop_signals(
            LogLevel::Trace,
            configured_level,
            format_args!(
                "native_idle_result unavailable; falling back to window idle_active={}",
                fallback.idle_active
            ),
        ),
    }

    match native_fullscreen_active {
        Some(native_fullscreen_active) => log_desktop_signals(
            LogLevel::Debug,
            configured_level,
            format_args!("native_fullscreen_result={native_fullscreen_active}"),
        ),
        None => log_desktop_signals(
            LogLevel::Trace,
            configured_level,
            format_args!(
                "native_fullscreen_result unavailable; falling back to window fullscreen_active={}",
                fallback.fullscreen_active
            ),
        ),
    }

    let signals =
        windows_signals_from_sources(fallback, native_idle_active, native_fullscreen_active);
    log_desktop_signals(
        LogLevel::Debug,
        configured_level,
        format_args!("merged_desktop_signals={signals:?}"),
    );
    signals
}

fn screen_rect_covers_monitor(window: ScreenRect, monitor: ScreenRect) -> bool {
    window.left <= monitor.left
        && window.top <= monitor.top
        && window.right >= monitor.right
        && window.bottom >= monitor.bottom
}

fn windows_other_app_fullscreen_from_bounds(
    foreground_window: Option<isize>,
    app_window: Option<isize>,
    foreground_bounds: Option<ScreenRect>,
    monitor_bounds: Option<ScreenRect>,
) -> Option<bool> {
    let foreground_window = foreground_window?;

    if app_window.is_some_and(|app_window| app_window == foreground_window) {
        return Some(false);
    }

    let foreground_bounds = foreground_bounds?;
    let monitor_bounds = monitor_bounds?;
    Some(screen_rect_covers_monitor(
        foreground_bounds,
        monitor_bounds,
    ))
}

fn macos_signals_from_sources(
    fallback: DesktopSignals,
    native_idle_active: Option<bool>,
    native_fullscreen_active: Option<bool>,
) -> DesktopSignals {
    DesktopSignals {
        fullscreen_active: native_fullscreen_active.unwrap_or(fallback.fullscreen_active),
        idle_active: native_idle_active.unwrap_or(fallback.idle_active),
    }
}

fn macos_signals_from_sources_logged(
    fallback: DesktopSignals,
    native_idle_active: Option<bool>,
    native_fullscreen_active: Option<bool>,
    configured_level: LogLevel,
) -> DesktopSignals {
    match native_idle_active {
        Some(native_idle_active) => log_desktop_signals(
            LogLevel::Debug,
            configured_level,
            format_args!("native_idle_result={native_idle_active}"),
        ),
        None => log_desktop_signals(
            LogLevel::Trace,
            configured_level,
            format_args!(
                "native_idle_result unavailable; falling back to window idle_active={}",
                fallback.idle_active
            ),
        ),
    }

    match native_fullscreen_active {
        Some(native_fullscreen_active) => log_desktop_signals(
            LogLevel::Debug,
            configured_level,
            format_args!("native_fullscreen_result={native_fullscreen_active}"),
        ),
        None => log_desktop_signals(
            LogLevel::Trace,
            configured_level,
            format_args!(
                "native_fullscreen_result unavailable; falling back to window fullscreen_active={}",
                fallback.fullscreen_active
            ),
        ),
    }

    let signals =
        macos_signals_from_sources(fallback, native_idle_active, native_fullscreen_active);
    log_desktop_signals(
        LogLevel::Debug,
        configured_level,
        format_args!("merged_desktop_signals={signals:?}"),
    );
    signals
}

fn macos_other_app_fullscreen_from_bounds(
    frontmost_window: Option<usize>,
    app_window: Option<usize>,
    frontmost_bounds: Option<ScreenRect>,
    screen_bounds: Option<ScreenRect>,
) -> Option<bool> {
    let frontmost_window = frontmost_window?;

    if app_window.is_some_and(|app_window| app_window == frontmost_window) {
        return Some(false);
    }

    let frontmost_bounds = frontmost_bounds?;
    let screen_bounds = screen_bounds?;
    Some(screen_rect_covers_monitor(frontmost_bounds, screen_bounds))
}

#[cfg(all(desktop, target_os = "linux"))]
mod platform {
    use super::{
        desktop_signals_from_desktop_window, linux_idle_from_sources_logged,
        linux_native_idle_active_from_env, DesktopSignalProvider, DesktopSignals,
    };
    use crate::logger::LogLevel;

    pub(super) struct PlatformDesktopSignalProvider;

    fn native_idle_active() -> Option<bool> {
        linux_native_idle_active_from_env(
            std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
            std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
        )
    }

    impl DesktopSignalProvider for PlatformDesktopSignalProvider {
        fn collect(
            &self,
            app: &tauri::AppHandle,
            _idle_threshold_seconds: u64,
            configured_level: LogLevel,
        ) -> DesktopSignals {
            let fallback = desktop_signals_from_desktop_window(app, configured_level);
            linux_idle_from_sources_logged(fallback, native_idle_active(), configured_level)
        }
    }
}

#[cfg(all(desktop, target_os = "macos"))]
mod platform {
    use std::{ffi::c_void, ptr};

    use super::{
        desktop_signals_from_desktop_window, idle_active_from_seconds, log_desktop_signals,
        macos_other_app_fullscreen_from_bounds, macos_signals_from_sources_logged,
        DesktopSignalProvider, DesktopSignals, ScreenRect,
    };
    use crate::logger::LogLevel;
    use core_graphics::{display::CGDisplay, event_source::CGEventSourceStateID, geometry::CGRect};
    use tauri::Manager;

    pub(super) struct PlatformDesktopSignalProvider;
    pub(super) const ANY_INPUT_EVENT_TYPE: u32 = u32::MAX;
    const CG_WINDOW_LIST_ON_SCREEN_EXCLUDING_DESKTOP: u32 = (1 << 0) | (1 << 4);
    const K_CF_NUMBER_SINT32_TYPE: u32 = 3;
    const K_CF_NUMBER_SINT64_TYPE: u32 = 4;

    type CFArrayRef = *const c_void;
    type CFDictionaryRef = *const c_void;
    type CFNumberRef = *const c_void;
    type CFStringRef = *const c_void;
    type Sel = *const c_void;

    #[derive(Clone, Copy)]
    struct NativeWindowInfo {
        window_number: usize,
        bounds: ScreenRect,
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(
            state_id: CGEventSourceStateID,
            event_type: u32,
        ) -> f64;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFArrayGetCount(array: CFArrayRef) -> isize;
        fn CFArrayGetValueAtIndex(array: CFArrayRef, index: isize) -> *const c_void;
        fn CFDictionaryGetValueIfPresent(
            dictionary: CFDictionaryRef,
            key: *const c_void,
            value: *mut *const c_void,
        ) -> u8;
        fn CFNumberGetValue(number: CFNumberRef, number_type: u32, value: *mut c_void) -> bool;
        fn CFRelease(cf: *const c_void);
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        static kCGWindowBounds: CFStringRef;
        static kCGWindowLayer: CFStringRef;
        static kCGWindowNumber: CFStringRef;
        static kCGWindowOwnerPID: CFStringRef;

        fn CGWindowListCopyWindowInfo(option: u32, relative_to_window: u32) -> CFArrayRef;
        fn CGRectMakeWithDictionaryRepresentation(
            dictionary: CFDictionaryRef,
            rect: *mut CGRect,
        ) -> u8;
    }

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const i8) -> *mut c_void;
        fn sel_registerName(name: *const i8) -> Sel;
        fn objc_autoreleasePoolPush() -> *mut c_void;
        fn objc_autoreleasePoolPop(pool: *mut c_void);

        #[link_name = "objc_msgSend"]
        fn objc_msg_send_id(receiver: *mut c_void, op: Sel) -> *mut c_void;
        #[link_name = "objc_msgSend"]
        fn objc_msg_send_i32(receiver: *mut c_void, op: Sel) -> i32;
        #[link_name = "objc_msgSend"]
        fn objc_msg_send_isize(receiver: *mut c_void, op: Sel) -> isize;
    }

    fn native_idle_seconds() -> Option<f64> {
        Some(unsafe {
            CGEventSourceSecondsSinceLastEventType(
                CGEventSourceStateID::CombinedSessionState,
                ANY_INPUT_EVENT_TYPE,
            )
        })
    }

    fn selector(name: &'static [u8]) -> Sel {
        unsafe { sel_registerName(name.as_ptr().cast()) }
    }

    struct AutoreleasePool {
        pool: *mut c_void,
    }

    impl AutoreleasePool {
        fn new() -> Self {
            Self {
                pool: unsafe { objc_autoreleasePoolPush() },
            }
        }
    }

    impl Drop for AutoreleasePool {
        fn drop(&mut self) {
            unsafe { objc_autoreleasePoolPop(self.pool) };
        }
    }

    pub(super) fn with_autorelease_pool<T>(callback: impl FnOnce() -> T) -> T {
        let _pool = AutoreleasePool::new();
        callback()
    }

    fn native_idle_active(configured_level: LogLevel) -> Option<bool> {
        let idle_seconds = native_idle_seconds()?;
        let idle_active = idle_active_from_seconds(idle_seconds);
        log_desktop_signals(
            LogLevel::Debug,
            configured_level,
            format_args!(
                "native_idle_sample_seconds={idle_seconds:.3} native_idle_result={idle_active}"
            ),
        );
        Some(idle_active)
    }

    fn frontmost_application_pid() -> Option<i32> {
        let workspace_class = unsafe { objc_getClass(b"NSWorkspace\0".as_ptr().cast()) };
        if workspace_class.is_null() {
            return None;
        }

        let workspace =
            unsafe { objc_msg_send_id(workspace_class, selector(b"sharedWorkspace\0")) };
        if workspace.is_null() {
            return None;
        }

        let application =
            unsafe { objc_msg_send_id(workspace, selector(b"frontmostApplication\0")) };
        if application.is_null() {
            return None;
        }

        Some(unsafe { objc_msg_send_i32(application, selector(b"processIdentifier\0")) })
    }

    fn app_window_number(app: &tauri::AppHandle) -> Option<usize> {
        app.get_webview_window("main")
            .and_then(|window| window.ns_window().ok())
            .map(|handle| unsafe {
                objc_msg_send_isize(handle.cast(), selector(b"windowNumber\0")) as usize
            })
    }

    fn frontmost_window_info(frontmost_pid: i32) -> Option<NativeWindowInfo> {
        let window_list =
            unsafe { CGWindowListCopyWindowInfo(CG_WINDOW_LIST_ON_SCREEN_EXCLUDING_DESKTOP, 0) };
        if window_list.is_null() {
            return None;
        }

        let count = unsafe { CFArrayGetCount(window_list) };
        let mut result = None;

        for index in 0..count {
            let entry = unsafe { CFArrayGetValueAtIndex(window_list, index) as CFDictionaryRef };
            if entry.is_null() {
                continue;
            }

            if dictionary_i32(entry, unsafe { kCGWindowOwnerPID }) != Some(frontmost_pid) {
                continue;
            }

            if dictionary_i64(entry, unsafe { kCGWindowLayer }) != Some(0) {
                continue;
            }

            let Some(bounds) = dictionary_rect(entry, unsafe { kCGWindowBounds }) else {
                continue;
            };

            if bounds.left >= bounds.right || bounds.top >= bounds.bottom {
                continue;
            }

            let Some(window_number) = dictionary_i64(entry, unsafe { kCGWindowNumber }) else {
                continue;
            };

            if window_number < 0 {
                continue;
            }

            result = Some(NativeWindowInfo {
                window_number: window_number as usize,
                bounds,
            });
            break;
        }

        unsafe { CFRelease(window_list) };
        result
    }

    fn dictionary_value(dictionary: CFDictionaryRef, key: CFStringRef) -> Option<*const c_void> {
        let mut value = ptr::null();
        let present =
            unsafe { CFDictionaryGetValueIfPresent(dictionary, key.cast(), &mut value) } != 0;

        if present && !value.is_null() {
            Some(value)
        } else {
            None
        }
    }

    fn dictionary_i32(dictionary: CFDictionaryRef, key: CFStringRef) -> Option<i32> {
        let number = dictionary_value(dictionary, key)? as CFNumberRef;
        let mut value = 0i32;
        let ok = unsafe {
            CFNumberGetValue(
                number,
                K_CF_NUMBER_SINT32_TYPE,
                (&mut value as *mut i32).cast(),
            )
        };

        ok.then_some(value)
    }

    fn dictionary_i64(dictionary: CFDictionaryRef, key: CFStringRef) -> Option<i64> {
        let number = dictionary_value(dictionary, key)? as CFNumberRef;
        let mut value = 0i64;
        let ok = unsafe {
            CFNumberGetValue(
                number,
                K_CF_NUMBER_SINT64_TYPE,
                (&mut value as *mut i64).cast(),
            )
        };

        ok.then_some(value)
    }

    fn dictionary_rect(dictionary: CFDictionaryRef, key: CFStringRef) -> Option<ScreenRect> {
        let bounds = dictionary_value(dictionary, key)? as CFDictionaryRef;
        let mut rect = CGRect::default();
        let ok = unsafe { CGRectMakeWithDictionaryRepresentation(bounds, &mut rect) != 0 };

        ok.then_some(cg_rect_to_screen_rect(rect))
    }

    fn cg_rect_to_screen_rect(rect: CGRect) -> ScreenRect {
        ScreenRect {
            left: rect.origin.x.floor() as i32,
            top: rect.origin.y.floor() as i32,
            right: (rect.origin.x + rect.size.width).ceil() as i32,
            bottom: (rect.origin.y + rect.size.height).ceil() as i32,
        }
    }

    fn screen_bounds_for_window(window_bounds: ScreenRect) -> Option<ScreenRect> {
        let mut best_match = None;
        let mut best_overlap = 0i64;

        for display_id in CGDisplay::active_displays().ok()? {
            let bounds = cg_rect_to_screen_rect(CGDisplay::new(display_id).bounds());
            let overlap = intersection_area(window_bounds, bounds);
            if overlap > best_overlap {
                best_overlap = overlap;
                best_match = Some(bounds);
            }
        }

        best_match
    }

    fn intersection_area(first: ScreenRect, second: ScreenRect) -> i64 {
        let width = (first.right.min(second.right) - first.left.max(second.left)).max(0) as i64;
        let height = (first.bottom.min(second.bottom) - first.top.max(second.top)).max(0) as i64;
        width * height
    }

    fn native_fullscreen_active(
        app: &tauri::AppHandle,
        configured_level: LogLevel,
    ) -> Option<bool> {
        with_autorelease_pool(|| {
            let frontmost_pid = frontmost_application_pid()?;
            if frontmost_pid == std::process::id() as i32 {
                log_desktop_signals(
                    LogLevel::Debug,
                    configured_level,
                    format_args!(
                        "native_fullscreen_sample frontmost_pid={frontmost_pid} result=false reason=self_frontmost"
                    ),
                );
                return Some(false);
            }

            let frontmost_window = frontmost_window_info(frontmost_pid)?;
            let screen_bounds = screen_bounds_for_window(frontmost_window.bounds)?;
            let app_window = app_window_number(app);
            let native_fullscreen_active = macos_other_app_fullscreen_from_bounds(
                Some(frontmost_window.window_number),
                app_window,
                Some(frontmost_window.bounds),
                Some(screen_bounds),
            );

            if let Some(native_fullscreen_active) = native_fullscreen_active {
                log_desktop_signals(
                    LogLevel::Debug,
                    configured_level,
                    format_args!(
                        "native_fullscreen_sample frontmost_pid={frontmost_pid} frontmost_window={} app_window={app_window:?} frontmost_bounds={:?} screen_bounds={:?} native_fullscreen_result={native_fullscreen_active}",
                        frontmost_window.window_number,
                        frontmost_window.bounds,
                        screen_bounds,
                    ),
                );
            }

            native_fullscreen_active
        })
    }

    impl DesktopSignalProvider for PlatformDesktopSignalProvider {
        fn collect(
            &self,
            app: &tauri::AppHandle,
            idle_threshold_seconds: u64,
            configured_level: LogLevel,
        ) -> DesktopSignals {
            let fallback = desktop_signals_from_desktop_window(app, configured_level);
            let _ = idle_threshold_seconds;
            macos_signals_from_sources_logged(
                fallback,
                native_idle_active(configured_level),
                native_fullscreen_active(app, configured_level),
                configured_level,
            )
        }
    }
}

#[cfg(all(desktop, target_os = "windows"))]
mod platform {
    use super::{
        desktop_signals_from_desktop_window, idle_active_from_seconds,
        windows_other_app_fullscreen_from_bounds, windows_signals_from_sources_logged,
        DesktopSignalProvider, DesktopSignals, ScreenRect,
    };
    use crate::logger::LogLevel;
    use tauri::Manager;
    use windows_sys::Win32::Foundation::{HWND, RECT};
    use windows_sys::Win32::Graphics::Gdi::{
        MonitorFromWindow, HMONITOR, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows_sys::Win32::System::SystemInformation::GetTickCount;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowRect, IsIconic, IsWindowVisible,
    };

    pub(super) struct PlatformDesktopSignalProvider;

    fn rect_to_screen_rect(rect: RECT) -> ScreenRect {
        ScreenRect {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        }
    }

    fn main_window_handle(app: &tauri::AppHandle) -> Option<isize> {
        app.get_webview_window("main")
            .and_then(|window| window.hwnd().ok())
            .map(|handle| handle.0 as isize)
    }

    fn native_idle_seconds() -> Option<f64> {
        let mut last_input = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };

        let success = unsafe { GetLastInputInfo(&mut last_input) };
        if success == 0 {
            return None;
        }

        let now_ticks = unsafe { GetTickCount() };
        let elapsed_ms = now_ticks.wrapping_sub(last_input.dwTime);
        Some(f64::from(elapsed_ms) / 1000.0)
    }

    fn native_idle_active() -> Option<bool> {
        native_idle_seconds().map(idle_active_from_seconds)
    }

    fn window_bounds(hwnd: HWND) -> Option<ScreenRect> {
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        let success = unsafe { GetWindowRect(hwnd, &mut rect) };
        if success == 0 {
            return None;
        }

        Some(rect_to_screen_rect(rect))
    }

    fn monitor_bounds(monitor: HMONITOR) -> Option<ScreenRect> {
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            rcMonitor: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            rcWork: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            dwFlags: 0,
        };

        let success = unsafe {
            windows_sys::Win32::Graphics::Gdi::GetMonitorInfoW(
                monitor,
                &mut info as *mut MONITORINFO as *mut _,
            )
        };
        if success == 0 {
            return None;
        }

        Some(rect_to_screen_rect(info.rcMonitor))
    }

    fn native_fullscreen_active(app: &tauri::AppHandle) -> Option<bool> {
        let foreground_window = unsafe { GetForegroundWindow() };
        if foreground_window.is_null() {
            return None;
        }

        let is_visible = unsafe { IsWindowVisible(foreground_window) } != 0;
        let is_minimized = unsafe { IsIconic(foreground_window) } != 0;
        if !is_visible || is_minimized {
            return Some(false);
        }

        let monitor = unsafe { MonitorFromWindow(foreground_window, MONITOR_DEFAULTTONEAREST) };
        if monitor.is_null() {
            return None;
        }

        windows_other_app_fullscreen_from_bounds(
            Some(foreground_window as isize),
            main_window_handle(app),
            window_bounds(foreground_window),
            monitor_bounds(monitor),
        )
    }

    impl DesktopSignalProvider for PlatformDesktopSignalProvider {
        fn collect(
            &self,
            app: &tauri::AppHandle,
            _idle_threshold_seconds: u64,
            configured_level: LogLevel,
        ) -> DesktopSignals {
            let fallback = desktop_signals_from_desktop_window(app, configured_level);
            windows_signals_from_sources_logged(
                fallback,
                native_idle_active(),
                native_fullscreen_active(app),
                configured_level,
            )
        }
    }
}

#[cfg(all(
    desktop,
    not(target_os = "macos"),
    not(target_os = "linux"),
    not(target_os = "windows")
))]
mod platform {
    use super::{desktop_signals_from_desktop_window, DesktopSignalProvider, DesktopSignals};
    use crate::logger::LogLevel;

    pub(super) struct PlatformDesktopSignalProvider;

    impl DesktopSignalProvider for PlatformDesktopSignalProvider {
        fn collect(
            &self,
            app: &tauri::AppHandle,
            _idle_threshold_seconds: u64,
            configured_level: LogLevel,
        ) -> DesktopSignals {
            desktop_signals_from_desktop_window(app, configured_level)
        }
    }
}

#[cfg(desktop)]
pub fn collect_desktop_signals_with_level(
    app: &tauri::AppHandle,
    idle_threshold_seconds: u64,
    configured_level: LogLevel,
) -> DesktopSignals {
    platform::PlatformDesktopSignalProvider.collect(app, idle_threshold_seconds, configured_level)
}

#[cfg(not(desktop))]
pub fn collect_desktop_signals_with_level(
    _app: &tauri::AppHandle,
    _idle_threshold_seconds: u64,
    _configured_level: LogLevel,
) -> DesktopSignals {
    desktop_signals_without_window()
}

pub fn collect_desktop_signals(
    app: &tauri::AppHandle,
    idle_threshold_seconds: u64,
) -> DesktopSignals {
    collect_desktop_signals_with_level(app, idle_threshold_seconds, LogLevel::Off)
}

#[cfg(test)]
mod tests {
    use super::{
        desktop_signals_from_window_state, fallback_from_window_state, idle_active_from_seconds,
        linux_idle_from_sources, linux_native_idle_active_from_env, linux_prefers_wayland_session,
        linux_should_query_native_idle, macos_other_app_fullscreen_from_bounds,
        macos_signals_from_sources, screen_rect_covers_monitor,
        windows_other_app_fullscreen_from_bounds, windows_signals_from_sources, DesktopSignals,
        ScreenRect, WindowStateSnapshot,
    };

    #[cfg(all(desktop, target_os = "macos"))]
    use super::platform::with_autorelease_pool;
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
    fn idle_active_from_seconds_uses_elapsed_idle_seconds() {
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
        assert_eq!(
            linux_native_idle_active_from_env(Some("wayland"), None),
            None
        );
        assert_eq!(
            linux_native_idle_active_from_env(None, Some("wayland-0")),
            None
        );
    }

    #[test]
    fn windows_signals_from_sources_uses_native_values_when_present() {
        let fallback = DesktopSignals {
            fullscreen_active: false,
            idle_active: true,
        };

        let signals = windows_signals_from_sources(fallback, Some(false), Some(true));

        assert_eq!(
            signals,
            DesktopSignals {
                fullscreen_active: true,
                idle_active: false,
            }
        );
    }

    #[test]
    fn windows_signals_from_sources_falls_back_when_native_values_are_missing() {
        let fallback = DesktopSignals {
            fullscreen_active: true,
            idle_active: false,
        };

        assert_eq!(windows_signals_from_sources(fallback, None, None), fallback);
    }

    #[test]
    fn macos_signals_from_sources_prefers_native_values_when_present() {
        let fallback = DesktopSignals {
            fullscreen_active: false,
            idle_active: true,
        };

        let signals = macos_signals_from_sources(fallback, Some(false), Some(true));

        assert_eq!(
            signals,
            DesktopSignals {
                fullscreen_active: true,
                idle_active: false,
            }
        );
    }

    #[test]
    fn macos_signals_from_sources_falls_back_when_native_values_are_missing() {
        let fallback = DesktopSignals {
            fullscreen_active: true,
            idle_active: false,
        };

        assert_eq!(macos_signals_from_sources(fallback, None, None), fallback);
    }

    #[test]
    fn windows_fullscreen_helper_rejects_own_app_window() {
        let foreground = Some(10);
        let app = Some(10);
        let bounds = Some(ScreenRect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        });

        assert_eq!(
            windows_other_app_fullscreen_from_bounds(foreground, app, bounds, bounds),
            Some(false)
        );
    }

    #[test]
    fn macos_fullscreen_helper_rejects_own_app_window() {
        let screen = ScreenRect {
            left: 0,
            top: 0,
            right: 1728,
            bottom: 1117,
        };

        assert_eq!(
            macos_other_app_fullscreen_from_bounds(Some(42), Some(42), Some(screen), Some(screen)),
            Some(false)
        );
    }

    #[test]
    fn macos_fullscreen_helper_detects_foreign_window_covering_screen() {
        let screen = ScreenRect {
            left: 0,
            top: 0,
            right: 1728,
            bottom: 1117,
        };
        let window = ScreenRect {
            left: 0,
            top: 0,
            right: 1728,
            bottom: 1117,
        };

        assert_eq!(
            macos_other_app_fullscreen_from_bounds(Some(99), Some(42), Some(window), Some(screen)),
            Some(true)
        );
    }

    #[test]
    fn macos_fullscreen_helper_rejects_foreign_window_smaller_than_screen() {
        let screen = ScreenRect {
            left: 0,
            top: 0,
            right: 1728,
            bottom: 1117,
        };
        let window = ScreenRect {
            left: 20,
            top: 20,
            right: 1700,
            bottom: 1080,
        };

        assert_eq!(
            macos_other_app_fullscreen_from_bounds(Some(99), Some(42), Some(window), Some(screen)),
            Some(false)
        );
    }

    #[test]
    fn windows_fullscreen_helper_detects_foreground_window_covering_monitor() {
        let monitor = ScreenRect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
        let foreground = ScreenRect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };

        assert_eq!(
            windows_other_app_fullscreen_from_bounds(
                Some(20),
                Some(10),
                Some(foreground),
                Some(monitor),
            ),
            Some(true)
        );
    }

    #[test]
    fn windows_fullscreen_helper_rejects_non_covering_foreground_window() {
        let monitor = ScreenRect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
        let foreground = ScreenRect {
            left: 0,
            top: 0,
            right: 1910,
            bottom: 1040,
        };

        assert_eq!(
            windows_other_app_fullscreen_from_bounds(
                Some(20),
                Some(10),
                Some(foreground),
                Some(monitor),
            ),
            Some(false)
        );
    }

    #[test]
    fn windows_monitor_coverage_accepts_equal_bounds() {
        let monitor = ScreenRect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };

        assert!(screen_rect_covers_monitor(monitor, monitor));
    }

    #[cfg(all(desktop, target_os = "macos"))]
    #[test]
    fn macos_idle_query_uses_any_input_event_type_constant() {
        assert_eq!(ANY_INPUT_EVENT_TYPE, u32::MAX);
    }

    #[cfg(all(desktop, target_os = "macos"))]
    #[test]
    fn macos_autorelease_pool_helper_returns_closure_result() {
        assert_eq!(with_autorelease_pool(|| 42usize), 42usize);
    }
}
