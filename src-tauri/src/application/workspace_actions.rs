use crate::{features::image_assets::storage::AppStorage, infrastructure::persistence::Database};

use crate::features::workspace::{repository, types::Workspace};

pub fn list_workspaces(database: &Database) -> Result<Vec<Workspace>, String> {
    crate::infrastructure::logging::operation("list_workspaces", || repository::list(database))
}

pub fn create_workspace(database: &Database, name: String) -> Result<Workspace, String> {
    crate::infrastructure::logging::operation("create_workspace", || {
        repository::create(database, &name)
    })
}

pub fn rename_workspace(
    database: &Database,
    workspace_id: String,
    name: String,
) -> Result<(), String> {
    crate::infrastructure::logging::operation_with_id("rename_workspace", &workspace_id, || {
        repository::rename(database, &workspace_id, &name)
    })
}

pub fn delete_workspace(
    database: &Database,
    storage: &AppStorage,
    workspace_id: String,
) -> Result<(), String> {
    crate::infrastructure::logging::operation_with_id("delete_workspace", &workspace_id, || {
        let staged = storage.stage_workspace_deletion(&workspace_id)?;

        match repository::delete(database, &workspace_id) {
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
