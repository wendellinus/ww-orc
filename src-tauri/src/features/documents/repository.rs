use crate::infrastructure::persistence::{unix_timestamp, Database};
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use super::types::DocumentRecord;
use crate::features::image_assets::types::StoredImage;

const OCR_ENGINE_VERSION: &str = "paddle-ocr-rs/0.6.1";

pub fn create_pending(database: &Database, image: &StoredImage) -> Result<String, String> {
    let timestamp = unix_timestamp()?;
    let run_id = Uuid::new_v4().to_string();
    let mut connection = database.connection()?;
    let transaction = connection
        .transaction()
        .map_err(|error| format!("启动图片保存事务失败: {error}"))?;

    transaction
        .execute(
            "INSERT INTO images (
                id, workspace_id, original_name, relative_path, mime_type,
                byte_size, width, height, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                &image.id,
                &image.workspace_id,
                &image.original_name,
                &image.relative_path,
                &image.mime_type,
                image.byte_size,
                image.width,
                image.height,
                timestamp,
            ],
        )
        .map_err(|error| format!("保存图片记录失败: {error}"))?;

    transaction
        .execute(
            "INSERT INTO ocr_runs (
                id, workspace_id, image_id, status, engine_version, created_at
             ) VALUES (?1, ?2, ?3, 'pending', ?4, ?5)",
            params![
                &run_id,
                &image.workspace_id,
                &image.id,
                OCR_ENGINE_VERSION,
                timestamp,
            ],
        )
        .map_err(|error| format!("创建识别记录失败: {error}"))?;

    transaction
        .commit()
        .map_err(|error| format!("提交图片保存事务失败: {error}"))?;

    Ok(run_id)
}

pub fn complete_run(database: &Database, run_id: &str, text: &str) -> Result<(), String> {
    update_run(database, run_id, "completed", Some(text), None)
}

pub fn fail_run(database: &Database, run_id: &str, message: &str) -> Result<(), String> {
    update_run(database, run_id, "failed", None, Some(message))
}

fn update_run(
    database: &Database,
    run_id: &str,
    status: &str,
    text: Option<&str>,
    error_message: Option<&str>,
) -> Result<(), String> {
    let timestamp = unix_timestamp()?;
    let connection = database.connection()?;
    let changed = connection
        .execute(
            "UPDATE ocr_runs
             SET status = ?1, text = ?2, error_message = ?3, finished_at = ?4
             WHERE id = ?5",
            params![status, text, error_message, timestamp, run_id],
        )
        .map_err(|error| format!("更新识别记录失败: {error}"))?;

    if changed == 0 {
        return Err("识别记录不存在".to_string());
    }
    Ok(())
}

pub fn list(database: &Database, workspace_id: &str) -> Result<Vec<DocumentRecord>, String> {
    let connection = database.connection()?;
    let mut statement = connection
        .prepare(
            "SELECT
                i.id,
                i.workspace_id,
                i.original_name,
                i.relative_path,
                COALESCE(r.status, 'pending'),
                COALESCE(r.text, ''),
                r.error_message,
                i.created_at
             FROM images i
             LEFT JOIN ocr_runs r ON r.id = (
                SELECT latest.id
                FROM ocr_runs latest
                WHERE latest.workspace_id = i.workspace_id
                  AND latest.image_id = i.id
                ORDER BY latest.created_at DESC, latest.id DESC
                LIMIT 1
             )
             WHERE i.workspace_id = ?1
             ORDER BY i.created_at ASC, i.rowid ASC",
        )
        .map_err(|error| format!("准备图片查询失败: {error}"))?;

    let rows = statement
        .query_map(params![workspace_id], |row| {
            Ok(DocumentRecord {
                image_id: row.get(0)?,
                workspace_id: row.get(1)?,
                original_name: row.get(2)?,
                relative_path: row.get(3)?,
                status: row.get(4)?,
                text: row.get(5)?,
                error_message: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .map_err(|error| format!("查询图片失败: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取图片失败: {error}"))
}

pub fn relative_path(
    database: &Database,
    workspace_id: &str,
    image_id: &str,
) -> Result<String, String> {
    let connection = database.connection()?;
    connection
        .query_row(
            "SELECT relative_path
             FROM images
             WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id, image_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("查询图片路径失败: {error}"))?
        .ok_or_else(|| "图片不存在".to_string())
}

pub fn delete(database: &Database, workspace_id: &str, image_id: &str) -> Result<(), String> {
    let connection = database.connection()?;
    let changed = connection
        .execute(
            "DELETE FROM images WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id, image_id],
        )
        .map_err(|error| format!("删除图片记录失败: {error}"))?;

    if changed == 0 {
        return Err("图片不存在".to_string());
    }
    Ok(())
}
