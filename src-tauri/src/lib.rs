pub mod classify;
pub mod git;
pub mod http;
pub mod model;
pub mod source;
pub mod store;
pub mod transcript;

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

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            position_right_edge(&window)?;
            // The drawer only occupies the right strip; let clicks pass through the
            // transparent area. Re-enabled implicitly where the webview paints opaque
            // content is not automatic — Phase 1 keeps it simple: ignore cursor events
            // globally only when collapsed is a Phase 2 refinement. For now, size the
            // window to the drawer so there is little dead space.
            let _ = window.set_ignore_cursor_events(false);

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
