#[cfg(windows)]
pub mod manager;

#[cfg(not(windows))]
pub mod manager {
    use std::path::PathBuf;

    use super::repository::WindowState;

    #[derive(Clone)]
    pub struct PinWindowSpec {
        pub id: String,
        pub workspace_id: String,
        pub image_id: String,
        pub image_path: PathBuf,
        pub width: u32,
        pub height: u32,
        pub zoom: f64,
        pub position: Option<(i32, i32)>,
        pub saved: Option<WindowState>,
    }

    pub fn open(_: &tauri::AppHandle, _: PinWindowSpec) -> Result<(), String> {
        Err("原生贴图目前仅支持 Windows".into())
    }

    pub fn close(_: &tauri::AppHandle, _: String) -> Result<(), String> {
        Err("原生贴图目前仅支持 Windows".into())
    }

    pub fn persist_all(_: &tauri::AppHandle) -> Result<(), String> {
        Ok(())
    }
}

pub mod repository;
