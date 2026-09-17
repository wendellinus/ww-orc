use tauri::State;

use crate::{documents::storage::AppStorage, persistence::Database};

use super::{repository, types::Workspace};

#[tauri::command]
pub fn list_workspaces(database: State<'_, Database>) -> Result<Vec<Workspace>, String> {
    repository::list(&database)
}

#[tauri::command]
pub fn create_workspace(database: State<'_, Database>, name: String) -> Result<Workspace, String> {
    repository::create(&database, &name)
}

#[tauri::command]
pub fn rename_workspace(
    database: State<'_, Database>,
    workspace_id: String,
    name: String,
) -> Result<(), String> {
    repository::rename(&database, &workspace_id, &name)
}

#[tauri::command]
pub fn delete_workspace(
    database: State<'_, Database>,
    storage: State<'_, AppStorage>,
    workspace_id: String,
) -> Result<(), String> {
    let staged = storage.stage_workspace_deletion(&workspace_id)?;

    match repository::delete(&database, &workspace_id) {
        Ok(()) => {
            if let Some(staged) = staged {
                staged
                    .finalize()
                    .map_err(|error| format!("工作区记录已删除，但图片文件清理失败: {error}"))?;
            }
            Ok(())
        }
        Err(error) => {
            if let Some(staged) = staged {
                staged.restore()?;
            }
            Err(error)
        }
    }
}
