use super::Database;

const INITIAL_SCHEMA: &str = include_str!("../../../migrations/001_initial.sql");

pub fn run(database: &Database) -> Result<(), String> {
    let mut connection = database.connection()?;
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|error| format!("读取数据库版本失败: {error}"))?;

    if version > 4 {
        return Err("数据库版本高于当前应用支持版本".into());
    }

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

    if version < 2 {
        let tx = connection.transaction().map_err(|e| e.to_string())?;
        tx.execute_batch(include_str!("../../../migrations/002_desktop.sql"))
            .map_err(|e| format!("升级桌面功能数据库失败: {e}"))?;
        tx.pragma_update(None, "user_version", 2)
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }
    if version < 3 {
        let tx = connection.transaction().map_err(|e| e.to_string())?;
        tx.execute_batch(include_str!("../../../migrations/003_ocr_blocks.sql"))
            .map_err(|e| format!("升级 OCR 坐标数据库失败: {e}"))?;
        tx.pragma_update(None, "user_version", 3)
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }
    if version < 4 {
        let tx = connection.transaction().map_err(|e| e.to_string())?;
        tx.execute_batch(include_str!("../../../migrations/004_image_pixel_hash.sql"))
            .map_err(|e| format!("升级图片哈希索引失败: {e}"))?;
        tx.pragma_update(None, "user_version", 4)
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    #[test]
    fn upgrades_existing_database_without_losing_data() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        {
            let c = db.connection().unwrap();
            c.execute_batch(INITIAL_SCHEMA).unwrap();
            c.pragma_update(None, "user_version", 1).unwrap();
            c.execute(
                "UPDATE workspaces SET name='原有空间' WHERE id='default'",
                [],
            )
            .unwrap();
        }
        run(&db).unwrap();
        run(&db).unwrap();
        let c = db.connection().unwrap();
        let version: i64 = c
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, 4);
        let name: String = c
            .query_row("SELECT name FROM workspaces WHERE id='default'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(name, "原有空间");
    }
    #[test]
    fn image_references_prevent_deletion() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        run(&db).unwrap();
        let c = db.connection().unwrap();
        c.execute("INSERT INTO images(id,workspace_id,original_name,relative_path,mime_type,byte_size,width,height,created_at) VALUES('image','default','test','test.png','image/png',1,1,1,0)",[]).unwrap();
        c.execute("INSERT INTO pins(id,workspace_id,image_id,created_at) VALUES('pin','default','image',0)",[]).unwrap();
        assert!(c
            .execute("DELETE FROM images WHERE id='image'", [])
            .is_err());
        c.execute("DELETE FROM pins WHERE id='pin'", []).unwrap();
        assert_eq!(
            c.execute("DELETE FROM images WHERE id='image'", [])
                .unwrap(),
            1
        );
    }
}
