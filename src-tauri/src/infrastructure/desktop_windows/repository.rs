use crate::infrastructure::persistence::Database;
use rusqlite::{params, OptionalExtension};
#[derive(Clone)]
pub struct WindowState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub topmost: bool,
}
pub fn get(db: &Database, kind: &str, id: &str) -> Result<Option<WindowState>, String> {
    db.connection()?.query_row("SELECT x,y,width,height,topmost FROM window_states WHERE object_kind=?1 AND object_id=?2",params![kind,id],|r|Ok(WindowState{x:r.get(0)?,y:r.get(1)?,width:r.get(2)?,height:r.get(3)?,topmost:r.get(4)?})).optional().map_err(|e|e.to_string())
}
pub fn save(db: &Database, kind: &str, id: &str, state: &WindowState) -> Result<(), String> {
    db.connection()?.execute("INSERT INTO window_states(object_kind,object_id,x,y,width,height,topmost) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(object_kind,object_id) DO UPDATE SET x=excluded.x,y=excluded.y,width=excluded.width,height=excluded.height,topmost=excluded.topmost",params![kind,id,state.x,state.y,state.width,state.height,state.topmost]).map_err(|e|e.to_string())?;
    Ok(())
}
