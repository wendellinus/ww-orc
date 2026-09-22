use image::RgbaImage;
use serde::Serialize;
use std::{collections::HashSet, sync::Mutex};
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotInfo {
    pub session_id: String,
    pub monitor_id: u32,
    pub monitor_x: i32,
    pub monitor_y: i32,
    pub width: u32,
    pub height: u32,
    pub desktop_x: i32,
    pub desktop_y: i32,
    pub desktop_width: u32,
    pub desktop_height: u32,
    pub window_regions: Vec<WindowRegion>,
}
pub struct Snapshot {
    pub info: SnapshotInfo,
    pub image: RgbaImage,
    pub x: i32,
    pub y: i32,
    pub scale: f64,
}
pub struct Session {
    pub id: String,
    pub workspace_id: String,
    pub snapshots: Vec<Snapshot>,
    // 各显示器在 Canvas 写入冻结画面后，向 Rust 报告一次就绪。
    pub ready_monitors: HashSet<u32>,
    pub windows_revealed: bool,
    pub shortcuts_registered: bool,
}
#[derive(Default)]
pub struct CaptureState(pub Mutex<Option<Session>>);
#[derive(Default)]
pub struct NativeCaptureBinding {
    pub shortcut_id: Option<u32>,
    pub workspace_id: Option<String>,
}
#[derive(Default)]
pub struct NativeCaptureShortcut(pub Mutex<NativeCaptureBinding>);
