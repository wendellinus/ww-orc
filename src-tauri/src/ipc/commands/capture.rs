use crate::{
    application::capture_actions as actions,
    features::capture::{
        coordinates::Selection,
        session::{NativeCaptureBinding, NativeCaptureShortcut, SnapshotInfo},
    },
};
use std::str::FromStr;
use tauri::Manager;
use tauri_plugin_global_shortcut::Shortcut;
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
    workspace_id: Option<String>,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("仅主窗口可以配置截图快捷键".into());
    }
    let shortcut_id = shortcut
        .map(|value| {
            Shortcut::from_str(&value)
                .map(|shortcut| shortcut.id())
                .map_err(|error| format!("截图快捷键无效: {error}"))
        })
        .transpose()?;
    let state = app.state::<NativeCaptureShortcut>();
    let mut binding = state.0.lock().map_err(|_| "截图快捷键状态不可用")?;
    *binding = NativeCaptureBinding {
        shortcut_id,
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
