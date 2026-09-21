use super::{storage::AppStorage, types::StoredImage};
use crate::infrastructure::persistence::{unix_timestamp, Database};
use rusqlite::{params, Connection, OptionalExtension};
pub fn insert(connection: &Connection, image: &StoredImage) -> Result<(), String> {
    connection.execute("INSERT INTO images(id,workspace_id,original_name,relative_path,mime_type,byte_size,width,height,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![image.id,image.workspace_id,image.original_name,image.relative_path,image.mime_type,image.byte_size,image.width,image.height,unix_timestamp()?]).map_err(|e|format!("保存图片记录失败: {e}"))?;
    Ok(())
}
pub fn get(db: &Database, storage: &AppStorage, id: &str) -> Result<StoredImage, String> {
    let c = db.connection()?;
    let data=c.query_row("SELECT workspace_id,original_name,relative_path,mime_type,byte_size,width,height FROM images WHERE id=?1",[id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,i64>(4)?,r.get::<_,i64>(5)?,r.get::<_,i64>(6)?))).optional().map_err(|e|e.to_string())?.ok_or_else(||"图片不存在".to_string())?;
    Ok(StoredImage {
        id: id.into(),
        workspace_id: data.0,
        original_name: data.1,
        absolute_path: storage.absolute_path(&data.2)?,
        relative_path: data.2,
        mime_type: data.3,
        byte_size: data.4,
        width: data.5,
        height: data.6,
    })
}
