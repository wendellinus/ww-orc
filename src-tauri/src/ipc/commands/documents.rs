use crate::{
    application::document_actions,
    features::{documents::types::OcrDocument, image_assets::storage::AppStorage},
    infrastructure::persistence::Database,
};
use tauri::State;

#[tauri::command]
pub fn list_documents(
    app: tauri::AppHandle,
    database: State<'_, Database>,
    storage: State<'_, AppStorage>,
    workspace_id: String,
) -> Result<Vec<OcrDocument>, String> {
    document_actions::list_documents(app, &database, &storage, workspace_id)
}

#[tauri::command]
pub fn delete_document(
    database: State<'_, Database>,
    storage: State<'_, AppStorage>,
    workspace_id: String,
    image_id: String,
) -> Result<(), String> {
    document_actions::delete_document(&database, &storage, workspace_id, image_id)
}
