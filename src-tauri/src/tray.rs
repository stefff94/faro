//! System-tray control surface for the chromeless overlay.

use tauri::menu::{CheckMenuItemBuilder, Menu, MenuItem, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri_plugin_autostart::ManagerExt;

/// Build the tray icon and menu. Called once from setup().
pub fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let handle = app.handle();

    let autostart_on = handle.autolaunch().is_enabled().unwrap_or(false);

    let status = MenuItemBuilder::with_id("status", bash_status())
        .enabled(false)
        .build(app)?;
    let autostart = CheckMenuItemBuilder::with_id("autostart", "Avvio automatico al login")
        .checked(autostart_on)
        .build(app)?;
    let reinstall: MenuItem<_> = MenuItemBuilder::with_id("reinstall", "Ripristina hook").build(app)?;
    let update: MenuItem<_> = MenuItemBuilder::with_id("update", "Controlla aggiornamenti").build(app)?;
    let quit: MenuItem<_> = MenuItemBuilder::with_id("quit", "Esci").build(app)?;

    let menu = Menu::with_items(app, &[&status, &autostart, &reinstall, &update, &quit])?;

    TrayIconBuilder::with_id("faro-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Faro")
        .menu(&menu)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "quit" => app.exit(0),
            "reinstall" => {
                let _ = crate::do_register(app);
            }
            "update" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    crate::check_and_update(app).await;
                });
            }
            "autostart" => {
                let mgr = app.autolaunch();
                if mgr.is_enabled().unwrap_or(false) {
                    let _ = mgr.disable();
                } else {
                    let _ = mgr.enable();
                }
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn bash_status() -> &'static str {
    let ok = std::process::Command::new("bash").arg("--version").output().is_ok();
    if ok { "● hook attivi" } else { "⚠ Git Bash non trovato — hook non operativi" }
}

#[cfg(not(target_os = "windows"))]
fn bash_status() -> &'static str {
    "● hook attivi"
}
