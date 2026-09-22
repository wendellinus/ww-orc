use super::model::Pin;
use crate::infrastructure::persistence::{unix_timestamp, Database};
use rusqlite::{params, OptionalExtension};
fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Pin> {
    Ok(Pin {
        id: r.get(0)?,
        workspace_id: r.get(1)?,
        image_id: r.get(2)?,
        zoom: r.get(3)?,
        is_open: r.get(4)?,
        created_at: r.get(5)?,
    })
}
pub fn create(db: &Database, workspace: &str, image: &str) -> Result<Pin, String> {
    let id = uuid::Uuid::new_v4().to_string();
    db.connection()?
        .execute(
            "INSERT INTO pins(id,workspace_id,image_id,created_at) VALUES(?1,?2,?3,?4)",
            params![id, workspace, image, unix_timestamp()?],
        )
        .map_err(|e| e.to_string())?;
    get(db, &id)
}
pub fn get(db: &Database, id: &str) -> Result<Pin, String> {
    db.connection()?
        .query_row(
            "SELECT id,workspace_id,image_id,zoom,is_open,created_at FROM pins WHERE id=?1",
            [id],
            row,
        )
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "贴图不存在".into())
}
pub fn get_by_image(db: &Database, workspace: &str, image: &str) -> Result<Option<Pin>, String> {
    db.connection()?
        .query_row(
            "SELECT id,workspace_id,image_id,zoom,is_open,created_at
             FROM pins
             WHERE workspace_id=?1 AND image_id=?2
             ORDER BY created_at ASC
             LIMIT 1",
            params![workspace, image],
            row,
        )
        .optional()
        .map_err(|e| e.to_string())
}
pub fn get_by_pixel_sha256(
    db: &Database,
    workspace: &str,
    pixel_sha256: &str,
) -> Result<Option<Pin>, String> {
    db.connection()?
        .query_row(
            "SELECT p.id,p.workspace_id,p.image_id,p.zoom,p.is_open,p.created_at
             FROM pins p
             JOIN images i ON i.id=p.image_id
             WHERE p.workspace_id=?1 AND i.pixel_sha256=?2
             ORDER BY p.created_at ASC
             LIMIT 1",
            params![workspace, pixel_sha256],
            row,
        )
        .optional()
        .map_err(|e| e.to_string())
}
pub fn list(db: &Database, workspace: &str) -> Result<Vec<Pin>, String> {
    let c = db.connection()?;
    let mut s=c.prepare("SELECT id,workspace_id,image_id,zoom,is_open,created_at FROM pins WHERE workspace_id=?1 ORDER BY created_at DESC").map_err(|e|e.to_string())?;
    let rows = s.query_map([workspace], row).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
pub fn set_open(db: &Database, id: &str, open: bool) -> Result<(), String> {
    db.connection()?
        .execute("UPDATE pins SET is_open=?2 WHERE id=?1", params![id, open])
        .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn zoom(db: &Database, id: &str, value: f64) -> Result<(), String> {
    if !value.is_finite() || !(0.1..=5.0).contains(&value) {
        return Err("缩放必须在 10% 到 500% 之间".into());
    }
    db.connection()?
        .execute("UPDATE pins SET zoom=?2 WHERE id=?1", params![id, value])
        .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn delete(db: &Database, id: &str) -> Result<(), String> {
    let mut c = db.connection()?;
    let tx = c.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "DELETE FROM window_states WHERE object_kind='pin' AND object_id=?1",
        [id],
    )
    .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM pins WHERE id=?1", [id])
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}
