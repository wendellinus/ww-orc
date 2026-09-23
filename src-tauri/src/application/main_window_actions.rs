use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use tauri::{Manager, WebviewWindowBuilder};

const MAIN_WINDOW_IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);

#[derive(Default)]
pub struct MainWindowLifecycle {
    generation: AtomicU64,
}

fn cancel_pending_destroy(app: &tauri::AppHandle) {
    app.state::<MainWindowLifecycle>()
        .generation
        .fetch_add(1, Ordering::AcqRel);
}

pub fn show(app: &tauri::AppHandle) -> Result<(), String> {
    cancel_pending_destroy(app);
    let window = if let Some(window) = app.get_webview_window("main") {
        window
    } else {
        let config = app
            .config()
            .app
            .windows
            .iter()
            .find(|config| config.label == "main")
            .ok_or("主窗口配置不存在")?
            .clone();
        WebviewWindowBuilder::from_config(app, &config)
            .map_err(|error| format!("读取主窗口配置失败: {error}"))?
            .build()
            .map_err(|error| format!("创建主窗口失败: {error}"))?
    };
    window.show().map_err(|error| error.to_string())?;
    window.unminimize().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

pub fn hide_to_tray(app: &tauri::AppHandle, window: &tauri::Window) -> Result<(), String> {
    window.hide().map_err(|error| error.to_string())?;
    schedule_idle_destroy(app);
    Ok(())
}

fn schedule_idle_destroy(app: &tauri::AppHandle) {
    let lifecycle = app.state::<MainWindowLifecycle>();
    let ticket = lifecycle.generation.fetch_add(1, Ordering::AcqRel) + 1;
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(MAIN_WINDOW_IDLE_TIMEOUT);
        let lifecycle = app.state::<MainWindowLifecycle>();
        if lifecycle.generation.load(Ordering::Acquire) != ticket {
            return;
        }
        let capture_active = app
            .state::<crate::features::capture::session::CaptureState>()
            .0
            .lock()
            .map(|session| session.is_some())
            .unwrap_or(true);
        if capture_active {
            schedule_idle_destroy(&app);
            return;
        }
        let Some(window) = app.get_webview_window("main") else {
            return;
        };
        if window.is_visible().unwrap_or(true) {
            return;
        }
        if let Err(error) = crate::application::capture_actions::retain_native_shortcut(&app) {
            log::error!("main_window_shortcut_cleanup_failed reason={error}");
            return;
        }
        if let Err(error) = window.destroy() {
            log::error!("main_window_idle_destroy_failed reason={error}");
        } else {
            log::info!("main_window_idle_destroyed");
        }
    });
}
