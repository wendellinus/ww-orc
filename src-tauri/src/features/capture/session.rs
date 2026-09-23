use image::RgbaImage;
use serde::Serialize;
use std::{collections::HashSet, sync::Mutex};
use tauri_plugin_global_shortcut::Shortcut;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowRegion {
    pub window_id: u32,
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
    pub preview_png: Vec<u8>,
    pub x: i32,
    pub y: i32,
    pub scale: f64,
}

pub enum CapturePhase {
    Acquiring,
    AwaitingPreview { ready_monitors: HashSet<u32> },
    Selecting,
    Finishing,
}

pub struct Session {
    pub id: String,
    pub workspace_id: String,
    pub snapshots: Vec<Snapshot>,
    pub shortcuts_registered: bool,
    phase: CapturePhase,
}

impl Session {
    pub fn new(id: String, workspace_id: String) -> Self {
        Self {
            id,
            workspace_id,
            snapshots: Vec::new(),
            shortcuts_registered: false,
            phase: CapturePhase::Acquiring,
        }
    }

    pub fn install_snapshots(&mut self, snapshots: Vec<Snapshot>) -> Result<(), String> {
        if !matches!(self.phase, CapturePhase::Acquiring) {
            return Err("截图会话阶段无效".into());
        }
        if snapshots.is_empty() {
            return Err("未找到显示器".into());
        }
        self.snapshots = snapshots;
        self.phase = CapturePhase::AwaitingPreview {
            ready_monitors: HashSet::new(),
        };
        Ok(())
    }

    pub fn mark_preview_ready(&mut self, monitor_id: u32) -> Result<bool, String> {
        if !self
            .snapshots
            .iter()
            .any(|snapshot| snapshot.info.monitor_id == monitor_id)
        {
            return Err("显示器截图不存在".into());
        }
        match &mut self.phase {
            CapturePhase::AwaitingPreview { ready_monitors } => {
                ready_monitors.insert(monitor_id);
                let all_ready = ready_monitors.len() == self.snapshots.len();
                if all_ready {
                    self.phase = CapturePhase::Selecting;
                }
                Ok(all_ready)
            }
            CapturePhase::Selecting => Ok(false),
            CapturePhase::Acquiring | CapturePhase::Finishing => Err("截图会话尚未准备完成".into()),
        }
    }

    pub fn ensure_selecting(&self) -> Result<(), String> {
        match self.phase {
            CapturePhase::Selecting => Ok(()),
            CapturePhase::Finishing => Err("截图正在完成".into()),
            CapturePhase::Acquiring | CapturePhase::AwaitingPreview { .. } => {
                Err("截图预览尚未准备完成".into())
            }
        }
    }

    pub fn begin_finishing(&mut self) -> Result<(), String> {
        self.ensure_selecting()?;
        self.phase = CapturePhase::Finishing;
        Ok(())
    }
}

#[derive(Default)]
pub struct CaptureState(pub Mutex<Option<Session>>);

#[derive(Default)]
pub struct NativeCaptureBinding {
    pub shortcut: Option<Shortcut>,
    pub show_shortcut: Option<Shortcut>,
    pub workspace_id: Option<String>,
}

#[derive(Default)]
pub struct NativeCaptureShortcut(pub Mutex<NativeCaptureBinding>);
