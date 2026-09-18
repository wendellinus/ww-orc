use tauri::State;

use crate::{documents::storage::AppStorage, persistence::Database};

use super::{repository, types::Workspace};

#[tauri::command]
pub fn list_workspaces(database: State<'_, Database>) -> Result<Vec<Workspace>, String> {
    crate::logging::operation("list_workspaces", || repository::list(&database))
}

#[tauri::command]
pub fn create_workspace(database: State<'_, Database>, name: String) -> Result<Workspace, String> {
    crate::logging::operation("create_workspace", || repository::create(&database, &name))
}

#[tauri::command]
pub fn rename_workspace(
    database: State<'_, Database>,
    workspace_id: String,
    name: String,
) -> Result<(), String> {
    crate::logging::operation_with_id("rename_workspace", &workspace_id, || {
        repository::rename(&database, &workspace_id, &name)
    })
}

#[tauri::command]
pub fn delete_workspace(
    database: State<'_, Database>,
    storage: State<'_, AppStorage>,
    workspace_id: String,
) -> Result<(), String> {
    crate::logging::operation_with_id("delete_workspace", &workspace_id, || {
        let staged = storage.stage_workspace_deletion(&workspace_id)?;

        match repository::delete(&database, &workspace_id) {
            Ok(()) => {
                if let Some(staged) = staged {
                    staged.finalize().map_err(|error| {
                        format!("工作区记录已删除，但图片文件清理失败: {error}")
                    })?;
                }
                Ok(())
            }
            Err(error) => {
                if let Some(staged) = staged {
                    if let Err(restore_error) = staged.restore() {
                        return Err(format!("{error}；同时恢复待删除文件失败: {restore_error}"));
                    }
                }
                Err(error)
            }
        }
    })
}
