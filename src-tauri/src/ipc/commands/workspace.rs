use crate::{
    application::workspace_actions,
    features::{image_assets::storage::AppStorage, workspace::types::Workspace},
    infrastructure::persistence::Database,
};
use tauri::State;

#[tauri::command]
pub fn list_workspaces(database: State<'_, Database>) -> Result<Vec<Workspace>, String> {
    workspace_actions::list_workspaces(&database)
}
#[tauri::command]
pub fn create_workspace(database: State<'_, Database>, name: String) -> Result<Workspace, String> {
    workspace_actions::create_workspace(&database, name)
}
#[tauri::command]
pub fn rename_workspace(
    database: State<'_, Database>,
    workspace_id: String,
    name: String,
) -> Result<(), String> {
    workspace_actions::rename_workspace(&database, workspace_id, name)
}
#[tauri::command]
pub fn delete_workspace(
    database: State<'_, Database>,
    storage: State<'_, AppStorage>,
    workspace_id: String,
) -> Result<(), String> {
    workspace_actions::delete_workspace(&database, &storage, workspace_id)
}
