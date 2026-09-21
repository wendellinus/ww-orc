use image::RgbaImage;
use serde::Serialize;
use std::{path::PathBuf, sync::Mutex};
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotInfo {
    pub session_id: String,
    pub monitor_id: u32,
    pub image_path: String,
    pub width: u32,
    pub height: u32,
}
pub struct Snapshot {
    pub info: SnapshotInfo,
    pub image: RgbaImage,
    pub path: PathBuf,
    pub x: i32,
    pub y: i32,
    pub scale: f64,
}
pub struct Session {
    pub id: String,
    pub workspace_id: String,
    pub snapshots: Vec<Snapshot>,
}
#[derive(Default)]
pub struct CaptureState(pub Mutex<Option<Session>>);
