use tauri::Manager;

use crate::infrastructure::persistence::Database;

use crate::features::{
    documents::{
        repository,
        types::{DocumentRecord, OcrDocument},
    },
    image_assets::storage::AppStorage,
};

pub fn list_documents(
    app: tauri::AppHandle,
    database: &Database,
    storage: &AppStorage,
    workspace_id: String,
) -> Result<Vec<OcrDocument>, String> {
    crate::infrastructure::logging::operation_with_id("list_documents", &workspace_id, || {
        repository::list(database, &workspace_id)?
            .into_iter()
            .map(|record| to_document(&app, storage, record))
            .collect()
    })
}

pub fn delete_document(
    database: &Database,
    storage: &AppStorage,
    workspace_id: String,
    image_id: String,
) -> Result<(), String> {
    crate::infrastructure::logging::operation_with_id("delete_document", &image_id, || {
        let count: i64 = database
            .connection()?
            .query_row(
                "SELECT count(*) FROM pins WHERE image_id=?1",
                [&image_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if count > 0 {
            return Err("这张图片正在被贴图引用，请先删除对应贴图".into());
        }
        let relative_path = repository::relative_path(database, &workspace_id, &image_id)?;
        let staged = storage.stage_document_deletion(&relative_path)?;

        match repository::delete(database, &workspace_id, &image_id) {
            Ok(()) => {
                if let Some(staged) = staged {
                    staged
                        .finalize()
                        .map_err(|error| format!("图片记录已删除，但文件清理失败: {error}"))?;
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

pub fn to_document(
    app: &tauri::AppHandle,
    storage: &AppStorage,
    record: DocumentRecord,
) -> Result<OcrDocument, String> {
    let blocks = serde_json::from_str(&record.blocks_json)
        .map_err(|error| format!("读取识别坐标失败: {error}"))?;
    let absolute_path = storage.absolute_path(&record.relative_path)?;
    if absolute_path.is_file() {
        app.asset_protocol_scope()
            .allow_file(&absolute_path)
            .map_err(|error| format!("授权图片预览失败: {error}"))?;
    }

    Ok(OcrDocument {
        image_id: record.image_id,
        workspace_id: record.workspace_id,
        file_name: record.original_name,
        image_path: absolute_path.to_string_lossy().into_owned(),
        status: record.status,
        text: record.text,
        blocks,
        error_message: record.error_message,
        created_at: record.created_at,
    })
}
