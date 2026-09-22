use crate::infrastructure::tray::{self, TrayAction};
use tauri::{Emitter, Manager};
pub fn init(app: &tauri::AppHandle) -> tauri::Result<()> {
    tray::init(app, |app, action| match action {
        TrayAction::Show => {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.unminimize();
                let _ = win.set_focus();
            }
        }
        TrayAction::Capture => {
            let app = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(e) = super::capture_actions::restart(&app, "default") {
                    let _ = app.emit_to("main", "desktop:error", e);
                }
            });
        }
        TrayAction::Note => {
            let app = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(e) = super::desktop_actions::create_note(&app, "default", "") {
                    let _ = app.emit_to("main", "desktop:error", e);
                }
            });
        }
        TrayAction::Quit => {
            if let Err(e) = super::desktop_actions::request_quit(app) {
                let _ = app.emit_to("main", "desktop:error", e);
            }
        }
    })
}
