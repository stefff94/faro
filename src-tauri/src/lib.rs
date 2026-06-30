pub mod classify;
pub mod git;
pub mod http;
pub mod model;
pub mod source;
pub mod store;
pub mod transcript;
pub mod window_geom;

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{Emitter, Manager, PhysicalPosition};

use crate::http::{OnChange, SharedStore};
use crate::store::SessionStore;

fn position_right_edge(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    if let Some(monitor) = window.current_monitor()? {
        let screen = monitor.size();
        let win = window.outer_size()?;
        let x = (screen.width as i32) - (win.width as i32);
        let y = ((screen.height as i32) - (win.height as i32)) / 4; // upper-ish
        window.set_position(PhysicalPosition::new(x.max(0), y.max(0)))?;
    }
    Ok(())
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Last content-fit reported by the frontend, used for cursor hit-testing.
struct ContentFitState(Mutex<crate::window_geom::ContentFit>);

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct FitArgs {
    win_w: f64,
    win_h: f64,
    content_x: f64,
    content_y: f64,
    content_w: f64,
    content_h: f64,
}

/// Resize the window to hug the painted widget and re-anchor it to the right edge,
/// then remember the content rect so cursor hit-testing targets only painted pixels.
#[tauri::command]
fn resize_to_content(
    window: tauri::WebviewWindow,
    fit_state: tauri::State<ContentFitState>,
    fit: FitArgs,
) -> tauri::Result<()> {
    use tauri::{LogicalSize, PhysicalPosition};

    window.set_size(LogicalSize::new(fit.win_w, fit.win_h))?;
    *fit_state.0.lock().unwrap() = crate::window_geom::ContentFit {
        x: fit.content_x,
        y: fit.content_y,
        w: fit.content_w,
        h: fit.content_h,
    };

    if let Some(monitor) = window.current_monitor()? {
        let screen = monitor.size();
        // Use the logical size we just set (not outer_size(), which may still reflect
        // the pre-resize frame on macOS before the compositor applies the change).
        let scale = window.scale_factor()?;
        let win_w_phys = (fit.win_w * scale).round() as i32;
        let win_h_phys = (fit.win_h * scale).round() as i32;
        let (x, y) = crate::window_geom::right_edge_position(
            screen.width as i32,
            screen.height as i32,
            win_w_phys,
            win_h_phys,
        );
        window.set_position(PhysicalPosition::new(x, y))?;
    }
    Ok(())
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// Returns the global cursor position in physical pixels (y=0 at top of primary display).
/// Uses CoreGraphics CGEvent which reports coordinates with top-left origin.
#[cfg(target_os = "macos")]
fn cursor_pos_physical(scale: f64) -> Option<(i32, i32)> {
    use std::ffi::c_void;

    #[repr(C)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventCreate(source: *const c_void) -> *mut c_void;
        fn CGEventGetLocation(event: *const c_void) -> CGPoint;
        fn CFRelease(cf: *const c_void);
    }

    unsafe {
        let evt = CGEventCreate(std::ptr::null());
        if evt.is_null() {
            return None;
        }
        let pt = CGEventGetLocation(evt);
        CFRelease(evt);
        // CGEventGetLocation returns logical points; multiply by scale factor for physical pixels
        Some(((pt.x * scale) as i32, (pt.y * scale) as i32))
    }
}

/// Returns the global cursor position in physical pixels (top-left origin) on Windows.
/// `GetCursorPos` already reports PHYSICAL screen pixels when the process is
/// per-monitor DPI aware (Tauri/WebView2 declares PerMonitorV2), so — unlike the
/// CoreGraphics path — we do NOT multiply by `scale`. `scale` is accepted only to
/// keep the signature symmetric with the macOS implementation.
#[cfg(target_os = "windows")]
fn cursor_pos_physical(_scale: f64) -> Option<(i32, i32)> {
    #[repr(C)]
    struct POINT {
        x: i32,
        y: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetCursorPos(point: *mut POINT) -> i32; // BOOL: nonzero on success
    }

    unsafe {
        let mut pt = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut pt) == 0 {
            return None;
        }
        Some((pt.x, pt.y))
    }
}

#[tauri::command]
fn cursor_in_window(window: tauri::WebviewWindow, fit_state: tauri::State<ContentFitState>) -> bool {
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (&window, &fit_state);
        return false;
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let scale = window.scale_factor().unwrap_or(1.0);
        let Some((cx, cy)) = cursor_pos_physical(scale) else {
            return false;
        };
        let Ok(pos) = window.outer_position() else {
            return false;
        };
        let fit = *fit_state.0.lock().unwrap();
        let rect = crate::window_geom::content_rect_physical(pos.x, pos.y, fit, scale);
        crate::window_geom::point_in_rect(cx, cy, rect)
    }
}

#[tauri::command]
fn set_cursor_passthrough(window: tauri::WebviewWindow, passthrough: bool) {
    let _ = window.set_ignore_cursor_events(passthrough);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            cursor_in_window,
            set_cursor_passthrough,
            resize_to_content
        ])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            app.manage(ContentFitState(Mutex::new(
                crate::window_geom::ContentFit { x: 0.0, y: 0.0, w: 0.0, h: 0.0 },
            )));
            // Show on every Space/desktop. We intentionally do NOT request
            // fullScreenAuxiliary: the widget yields to fullscreen apps.
            let _ = window.set_visible_on_all_workspaces(true);
            position_right_edge(&window)?;
            // Start click-through; the frontend polling loop re-enables cursor events
            // when the cursor enters the window bounds (cursor_in_window command).
            let _ = window.set_ignore_cursor_events(true);

            // Create shared store
            let store: SharedStore = Arc::new(Mutex::new(SessionStore::new()));

            // Wire on_change callback to emit "sessions-updated" Tauri event
            let app_handle = app.handle().clone();
            let on_change: OnChange = Arc::new(move |snap| {
                let _ = app_handle.emit("sessions-updated", snap);
            });

            // Spawn axum server on 127.0.0.1:8765
            let router = crate::http::router(store.clone(), on_change.clone());
            tauri::async_runtime::spawn(async move {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:8765")
                    .await
                    .expect("failed to bind 127.0.0.1:8765");
                axum::serve(listener, router)
                    .await
                    .expect("axum server error");
            });

            // Spawn stale ticker: every 5s, mark sessions stale with TTL=90s; purge after 30min
            const STALE_TTL_MS: i64 = 90_000;
            const PURGE_TTL_MS: i64 = 30 * 60_000;
            let store_ticker = store.clone();
            let on_change_ticker = on_change.clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(5));
                loop {
                    interval.tick().await;
                    let snap = {
                        let mut s = store_ticker.lock().unwrap();
                        let c1 = s.mark_stale(STALE_TTL_MS, now_ms());
                        let c2 = s.purge(PURGE_TTL_MS, now_ms());
                        if c1 || c2 { Some(s.snapshot()) } else { None }
                    };
                    if let Some(snap) = snap {
                        on_change_ticker(snap);
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
