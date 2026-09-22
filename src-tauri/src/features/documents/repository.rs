use crate::infrastructure::persistence::{unix_timestamp, Database};
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use super::types::DocumentRecord;
use crate::features::image_assets::types::StoredImage;
use crate::features::ocr::types::OcrTextBlock;

const OCR_ENGINE_VERSION: &str = "paddle-ocr-rs/0.6.1";

pub fn create_pending(database: &Database, image: &StoredImage) -> Result<String, String> {
    let timestamp = unix_timestamp()?;
    let run_id = Uuid::new_v4().to_string();
    let mut connection = database.connection()?;
    let transaction = connection
        .transaction()
        .map_err(|error| format!("启动图片保存事务失败: {error}"))?;

    crate::features::image_assets::repository::insert(&transaction, image)?;

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

pub fn create_existing_run(database: &Database, image: &StoredImage) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    database.connection()?.execute("INSERT INTO ocr_runs(id,workspace_id,image_id,status,engine_version,created_at) VALUES(?1,?2,?3,'pending',?4,?5)",params![id,image.workspace_id,image.id,OCR_ENGINE_VERSION,unix_timestamp()?]).map_err(|e|e.to_string())?;
    Ok(id)
}

pub fn complete_run(
    database: &Database,
    run_id: &str,
    text: &str,
    blocks: &[OcrTextBlock],
) -> Result<(), String> {
    let blocks_json =
        serde_json::to_string(blocks).map_err(|error| format!("序列化识别坐标失败: {error}"))?;
    update_run(
        database,
        run_id,
        "completed",
        Some(text),
        Some(&blocks_json),
        None,
    )
}

pub fn fail_run(database: &Database, run_id: &str, message: &str) -> Result<(), String> {
    update_run(database, run_id, "failed", None, None, Some(message))
}

fn update_run(
    database: &Database,
    run_id: &str,
    status: &str,
    text: Option<&str>,
    blocks_json: Option<&str>,
    error_message: Option<&str>,
) -> Result<(), String> {
    let timestamp = unix_timestamp()?;
    let connection = database.connection()?;
    let changed = connection
        .execute(
            "UPDATE ocr_runs
             SET status = ?1,
                 text = ?2,
                 error_message = ?3,
                 blocks_json = COALESCE(?4, blocks_json),
                 finished_at = ?5
             WHERE id = ?6",
            params![status, text, error_message, blocks_json, timestamp, run_id],
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
                COALESCE(r.status, 'unrecognized'),
                COALESCE(r.text, ''),
                COALESCE(r.blocks_json, '[]'),
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
               AND r.id IS NOT NULL
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
                blocks_json: row.get(6)?,
                error_message: row.get(7)?,
                created_at: row.get(8)?,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::persistence::run_migrations;
    use std::path::Path;

    #[test]
    fn cached_images_stay_out_of_ocr_history_and_workspace_count() {
        let database = Database::open(Path::new(":memory:")).unwrap();
        run_migrations(&database).unwrap();
        let connection = database.connection().unwrap();
        connection
            .execute_batch(
                "INSERT INTO images
                    (id, workspace_id, original_name, relative_path, mime_type, byte_size, width, height, created_at)
                 VALUES
                    ('cached', 'default', 'cached.png', 'cached.png', 'image/png', 1, 10, 10, 1),
                    ('recorded', 'default', 'recorded.png', 'recorded.png', 'image/png', 1, 10, 10, 2);
                 INSERT INTO ocr_runs
                    (id, workspace_id, image_id, status, text, engine_version, created_at, finished_at)
                 VALUES
                    ('run', 'default', 'recorded', 'completed', 'text', 'test', 2, 2);",
            )
            .unwrap();
        drop(connection);

        let documents = list(&database, "default").unwrap();
        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].image_id, "recorded");

        let workspaces = crate::features::workspace::repository::list(&database).unwrap();
        assert_eq!(workspaces[0].image_count, 1);
    }
}
