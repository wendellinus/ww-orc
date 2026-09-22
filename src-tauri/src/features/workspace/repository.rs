use crate::infrastructure::persistence::{unix_timestamp, Database};
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use super::types::Workspace;

pub fn list(database: &Database) -> Result<Vec<Workspace>, String> {
    let connection = database.connection()?;
    let mut statement = connection
        .prepare(
            "SELECT id, name, created_at, updated_at,
                (SELECT COUNT(DISTINCT image_id)
                 FROM ocr_runs
                 WHERE workspace_id = workspaces.id)
             FROM workspaces
             ORDER BY updated_at DESC, created_at DESC",
        )
        .map_err(|error| format!("准备工作区查询失败: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok(Workspace {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
                image_count: row.get(4)?,
            })
        })
        .map_err(|error| format!("查询工作区失败: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取工作区失败: {error}"))
}

pub fn exists(database: &Database, workspace_id: &str) -> Result<bool, String> {
    let connection = database.connection()?;
    connection
        .query_row(
            "SELECT 1 FROM workspaces WHERE id = ?1",
            params![workspace_id],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(|error| format!("检查工作区失败: {error}"))
}

pub fn create(database: &Database, name: &str) -> Result<Workspace, String> {
    let name = validate_name(name)?;
    let timestamp = unix_timestamp()?;
    let workspace = Workspace {
        id: Uuid::new_v4().to_string(),
        name,
        created_at: timestamp,
        updated_at: timestamp,
        image_count: 0,
    };

    let connection = database.connection()?;
    connection
        .execute(
            "INSERT INTO workspaces (id, name, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                workspace.id,
                workspace.name,
                workspace.created_at,
                workspace.updated_at
            ],
        )
        .map_err(|error| format!("创建工作区失败: {error}"))?;

    Ok(workspace)
}

pub fn rename(database: &Database, workspace_id: &str, name: &str) -> Result<(), String> {
    let name = validate_name(name)?;
    let timestamp = unix_timestamp()?;
    let connection = database.connection()?;
    let changed = connection
        .execute(
            "UPDATE workspaces SET name = ?1, updated_at = ?2 WHERE id = ?3",
            params![name, timestamp, workspace_id],
        )
        .map_err(|error| format!("重命名工作区失败: {error}"))?;

    if changed == 0 {
        return Err("工作区不存在".to_string());
    }

    Ok(())
}

pub fn delete(database: &Database, workspace_id: &str) -> Result<(), String> {
    if workspace_id == "default" {
        return Err("默认工作区不能删除".to_string());
    }

    let connection = database.connection()?;
    let changed = connection
        .execute(
            "DELETE FROM workspaces WHERE id = ?1",
            params![workspace_id],
        )
        .map_err(|error| format!("删除工作区失败: {error}"))?;

    if changed == 0 {
        return Err("工作区不存在".to_string());
    }

    Ok(())
}

fn validate_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("工作区名称不能为空".to_string());
    }
    if name.chars().count() > 80 {
        return Err("工作区名称不能超过 80 个字符".to_string());
    }
    Ok(name.to_string())
}
