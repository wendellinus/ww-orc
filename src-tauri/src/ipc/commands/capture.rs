use crate::{
    application::capture_actions as actions,
    features::capture::{
        composition::Selection,
        native_shortcuts::{self, NativeShortcutConfig},
        session::SnapshotInfo,
    },
};

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
    paste_shortcut: Option<String>,
    show_shortcut: Option<String>,
    workspace_id: Option<String>,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("仅主窗口可以配置截图快捷键".into());
    }
    native_shortcuts::configure(
        &app,
        NativeShortcutConfig {
            capture_shortcut: shortcut,
            paste_shortcut,
            show_shortcut,
            workspace_id,
        },
    )
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
