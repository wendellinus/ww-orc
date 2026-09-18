use super::Database;

const INITIAL_SCHEMA: &str = include_str!("../../../migrations/001_initial.sql");

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
