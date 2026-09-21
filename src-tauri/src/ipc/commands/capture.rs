use crate::{
    application::capture_actions as actions,
    features::capture::{coordinates::Selection, session::SnapshotInfo},
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
pub fn get_capture_snapshot(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    session_id: String,
    monitor_id: u32,
) -> Result<SnapshotInfo, String> {
    actions::snapshot(&app, window.label(), &session_id, monitor_id)
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
