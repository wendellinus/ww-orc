use crate::{
    application::ocr_actions,
    features::{
        documents::types::OcrDocument, image_assets::storage::AppStorage,
        ocr::engine::OcrEngineState,
    },
    infrastructure::persistence::Database,
};

#[tauri::command]
pub fn ocr_image(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<'_, OcrEngineState>,
    database: tauri::State<'_, Database>,
    storage: tauri::State<'_, AppStorage>,
    workspace_id: String,
    path: String,
) -> Result<OcrDocument, String> {
    crate::ipc::authorization::main_only(&window)?;
    ocr_actions::recognize_path(
        &app,
        &state,
        &database,
        &storage,
        &workspace_id,
        std::path::Path::new(path.trim()),
    )
}

#[tauri::command]
pub fn ocr_image_bytes(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<'_, OcrEngineState>,
    database: tauri::State<'_, Database>,
    storage: tauri::State<'_, AppStorage>,
    request: tauri::ipc::Request,
) -> Result<OcrDocument, String> {
    crate::ipc::authorization::main_only(&window)?;
    let workspace_id = request
        .headers()
        .get("x-workspace-id")
        .ok_or_else(|| "缺少工作区 ID".to_string())?
        .to_str()
        .map_err(|_| "工作区 ID 格式无效".to_string())?;
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("剪贴板图片传输格式无效".to_string());
    };
    ocr_actions::recognize_bytes(&app, &state, &database, &storage, workspace_id, bytes)
}
