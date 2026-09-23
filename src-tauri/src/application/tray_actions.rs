use crate::infrastructure::tray::{self, TrayAction};
use tauri::Emitter;
pub fn init(app: &tauri::AppHandle) -> tauri::Result<()> {
    tray::init(app, |app, action| match action {
        TrayAction::Show => {
            let app = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(error) = super::main_window_actions::show(&app) {
                    log::error!("main_window_show_failed reason={error}");
                }
            });
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
