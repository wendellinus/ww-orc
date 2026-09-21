use super::model::Note;
use crate::infrastructure::persistence::{unix_timestamp, Database};
use rusqlite::{params, OptionalExtension};
fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Note> {
    Ok(Note {
        id: r.get(0)?,
        workspace_id: r.get(1)?,
        text: r.get(2)?,
        color: r.get(3)?,
        revision: r.get(4)?,
        is_open: r.get(5)?,
        created_at: r.get(6)?,
        updated_at: r.get(7)?,
    })
}
pub fn create(db: &Database, workspace: &str, text: &str) -> Result<Note, String> {
    if text.len() > 1_000_000 {
        return Err("便签内容不能超过 1 MB".into());
    }
    let id = uuid::Uuid::new_v4().to_string();
    let now = unix_timestamp()?;
    db.connection()?
        .execute(
            "INSERT INTO notes(id,workspace_id,text,created_at,updated_at) VALUES(?1,?2,?3,?4,?4)",
            params![id, workspace, text, now],
        )
        .map_err(|e| format!("创建便签失败: {e}"))?;
    get(db, &id)
}
pub fn get(db: &Database, id: &str) -> Result<Note, String> {
    db.connection()?.query_row("SELECT id,workspace_id,text,color,revision,is_open,created_at,updated_at FROM notes WHERE id=?1",[id],row).optional().map_err(|e|e.to_string())?.ok_or_else(||"便签不存在".into())
}
pub fn list(db: &Database, workspace: &str) -> Result<Vec<Note>, String> {
    let c = db.connection()?;
    let mut s=c.prepare("SELECT id,workspace_id,text,color,revision,is_open,created_at,updated_at FROM notes WHERE workspace_id=?1 ORDER BY updated_at DESC").map_err(|e|e.to_string())?;
    let rows = s.query_map([workspace], row).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
pub fn update(
    db: &Database,
    id: &str,
    text: &str,
    color: &str,
    revision: i64,
) -> Result<Note, String> {
    if text.len() > 1_000_000 {
        return Err("便签内容不能超过 1 MB".into());
    }
    if !["amber", "mint", "blue", "rose"].contains(&color) {
        return Err("便签颜色无效".into());
    }
    let changed=db.connection()?.execute("UPDATE notes SET text=?2,color=?3,revision=revision+1,updated_at=?4 WHERE id=?1 AND revision=?5",params![id,text,color,unix_timestamp()?,revision]).map_err(|e|e.to_string())?;
    if changed == 0 {
        return Err("便签已被修改或删除，请重新打开后再编辑".into());
    }
    get(db, id)
}
pub fn set_open(db: &Database, id: &str, open: bool) -> Result<(), String> {
    db.connection()?
        .execute("UPDATE notes SET is_open=?2 WHERE id=?1", params![id, open])
        .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn delete(db: &Database, id: &str) -> Result<(), String> {
    let mut c = db.connection()?;
    let tx = c.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "DELETE FROM window_states WHERE object_kind='note' AND object_id=?1",
        [id],
    )
    .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM notes WHERE id=?1", [id])
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    #[test]
    fn saves_check_revision_and_reject_invalid_color() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        crate::infrastructure::persistence::run_migrations(&db).unwrap();
        let note = create(&db, "default", "草稿").unwrap();
        let saved = update(&db, &note.id, "正文", "mint", 0).unwrap();
        assert_eq!(saved.revision, 1);
        assert!(update(&db, &note.id, "覆盖", "blue", 0).is_err());
        assert_eq!(get(&db, &note.id).unwrap().text, "正文");
        assert!(update(&db, &note.id, "正文", "invalid", 1).is_err());
        set_open(&db, &note.id, false).unwrap();
        assert!(!get(&db, &note.id).unwrap().is_open);
    }
}
