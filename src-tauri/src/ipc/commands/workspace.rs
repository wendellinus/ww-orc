use crate::{
    application::workspace_actions,
    features::{image_assets::storage::AppStorage, workspace::types::Workspace},
    infrastructure::persistence::Database,
};
use tauri::State;

#[tauri::command]
pub fn list_workspaces(
    window: tauri::WebviewWindow,
    database: State<'_, Database>,
) -> Result<Vec<Workspace>, String> {
    crate::ipc::authorization::main_only(&window)?;
    workspace_actions::list_workspaces(&database)
}
#[tauri::command]
pub fn create_workspace(
    window: tauri::WebviewWindow,
    database: State<'_, Database>,
    name: String,
) -> Result<Workspace, String> {
    crate::ipc::authorization::main_only(&window)?;
    workspace_actions::create_workspace(&database, name)
}
#[tauri::command]
pub fn rename_workspace(
    window: tauri::WebviewWindow,
    database: State<'_, Database>,
    workspace_id: String,
    name: String,
) -> Result<(), String> {
    crate::ipc::authorization::main_only(&window)?;
    workspace_actions::rename_workspace(&database, workspace_id, name)
}
#[tauri::command]
pub fn delete_workspace(
    window: tauri::WebviewWindow,
    database: State<'_, Database>,
    storage: State<'_, AppStorage>,
    workspace_id: String,
) -> Result<(), String> {
    crate::ipc::authorization::main_only(&window)?;
    workspace_actions::delete_workspace(&database, &storage, workspace_id)
}
