use super::Database;

const INITIAL_SCHEMA: &str = r#"
CREATE TABLE workspaces (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL CHECK (length(trim(name)) > 0),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE images (
    id            TEXT PRIMARY KEY,
    workspace_id  TEXT NOT NULL,
    original_name TEXT NOT NULL,
    relative_path TEXT NOT NULL UNIQUE,
    mime_type     TEXT NOT NULL,
    byte_size     INTEGER NOT NULL CHECK (byte_size >= 0),
    width         INTEGER NOT NULL CHECK (width > 0),
    height        INTEGER NOT NULL CHECK (height > 0),
    created_at    INTEGER NOT NULL,
    UNIQUE (workspace_id, id),
    FOREIGN KEY (workspace_id)
        REFERENCES workspaces(id)
        ON DELETE CASCADE
);

CREATE TABLE ocr_runs (
    id             TEXT PRIMARY KEY,
    workspace_id   TEXT NOT NULL,
    image_id       TEXT NOT NULL,
    status         TEXT NOT NULL CHECK (status IN ('pending', 'completed', 'failed')),
    text           TEXT,
    error_message  TEXT,
    engine_version TEXT NOT NULL,
    created_at     INTEGER NOT NULL,
    finished_at    INTEGER,
    FOREIGN KEY (workspace_id, image_id)
        REFERENCES images(workspace_id, id)
        ON DELETE CASCADE
);

CREATE INDEX images_workspace_created_idx
    ON images(workspace_id, created_at DESC);

CREATE INDEX ocr_runs_image_created_idx
    ON ocr_runs(workspace_id, image_id, created_at DESC);

INSERT INTO workspaces (id, name, created_at, updated_at)
VALUES ('default', '默认工作区', unixepoch(), unixepoch());
"#;

pub fn run(database: &Database) -> Result<(), String> {
    let mut connection = database.connection()?;
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|error| format!("读取数据库版本失败: {error}"))?;

    if version < 1 {
        let transaction = connection
            .transaction()
            .map_err(|error| format!("启动数据库迁移失败: {error}"))?;

        transaction
            .execute_batch(INITIAL_SCHEMA)
            .map_err(|error| format!("创建数据库结构失败: {error}"))?;
        transaction
            .pragma_update(None, "user_version", 1)
            .map_err(|error| format!("更新数据库版本失败: {error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("提交数据库迁移失败: {error}"))?;
    }

    Ok(())
}
