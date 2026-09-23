use crate::{
    application::capture_actions as actions,
    features::capture::{
        composition::Selection,
        session::{NativeCaptureBinding, NativeCaptureShortcut, SnapshotInfo},
    },
};
use std::str::FromStr;
use tauri::Manager;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
#[tauri::command]
pub async fn start_capture(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    workspace_id: String,
) -> Result<String, String> {
    if window.label() != "main" {
        return Err("仅主窗口可以启动截图".into());
    }
    tauri::async_runtime::spawn_blocking(move || actions::start(&app, &workspace_id))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn configure_native_capture(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    shortcut: Option<String>,
    show_shortcut: Option<String>,
    workspace_id: Option<String>,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("仅主窗口可以配置截图快捷键".into());
    }
    let shortcut = shortcut
        .map(|value| Shortcut::from_str(&value).map_err(|error| format!("截图快捷键无效: {error}")))
        .transpose()?;
    let show_shortcut = show_shortcut
        .map(|value| Shortcut::from_str(&value).map_err(|error| format!("唤起快捷键无效: {error}")))
        .transpose()?;
    let state = app.state::<NativeCaptureShortcut>();
    let mut binding = state.0.lock().map_err(|_| "截图快捷键状态不可用")?;
    let manager = app.global_shortcut();
    let previous = [binding.shortcut, binding.show_shortcut];
    let desired = [shortcut, show_shortcut];
    let mut added = Vec::new();
    for next in desired.into_iter().flatten() {
        if previous
            .into_iter()
            .flatten()
            .any(|current| current.id() == next.id())
        {
            continue;
        }
        // The plugin owns the authoritative in-process registry. Reconcile a
        // stale entry before registering so repeated webview setup is idempotent.
        if manager.is_registered(next) {
            manager.unregister(next).map_err(|error| error.to_string())?;
        }
        if let Err(error) = manager.register(next) {
            for shortcut in added {
                let _ = manager.unregister(shortcut);
            }
            return Err(error.to_string());
        }
        added.push(next);
    }
    let mut removed = Vec::new();
    for current in previous.into_iter().flatten() {
        if desired
            .into_iter()
            .flatten()
            .any(|next| next.id() == current.id())
        {
            continue;
        }
        if let Err(error) = manager.unregister(current) {
            for shortcut in removed {
                let _ = manager.register(shortcut);
            }
            for shortcut in added {
                let _ = manager.unregister(shortcut);
            }
            return Err(error.to_string());
        }
        removed.push(current);
    }
    *binding = NativeCaptureBinding {
        shortcut,
        show_shortcut,
        workspace_id,
    };
    Ok(())
}
#[tauri::command]
pub fn get_capture_snapshot(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    session_id: String,
    monitor_id: u32,
) -> Result<SnapshotInfo, String> {
    actions::snapshot(&app, window.label(), &session_id, monitor_id)
}
#[tauri::command]
pub fn get_capture_preview(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    session_id: String,
    monitor_id: u32,
) -> Result<tauri::ipc::Response, String> {
    actions::preview(&app, window.label(), &session_id, monitor_id)
}
#[tauri::command]
pub fn capture_host_ready(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    monitor_id: u32,
) -> Result<Option<String>, String> {
    actions::host_session(&app, window.label(), monitor_id)
}
#[tauri::command]
pub fn capture_window_ready(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    session_id: String,
    monitor_id: u32,
) -> Result<(), String> {
    actions::ready(&app, &window, &session_id, monitor_id)
}
#[tauri::command]
pub fn cancel_capture(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    session_id: String,
) -> Result<(), String> {
    actions::cancel(&app, window.label(), &session_id)
}
#[tauri::command]
pub async fn finish_capture(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    session_id: String,
    monitor_id: u32,
    selection: Selection,
    action: String,
) -> Result<String, String> {
    let label = window.label().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let result = actions::finish(&app, &label, &session_id, monitor_id, selection, &action);
        if let Err(ref error) = result {
            use tauri::Emitter;
            let _ = app.emit_to("main", "desktop:error", error);
        }
        result
    })
    .await
    .map_err(|e| e.to_string())?
}
