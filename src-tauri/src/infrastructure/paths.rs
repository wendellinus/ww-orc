use std::path::PathBuf;
#[cfg(not(debug_assertions))]
use tauri::Manager;

pub fn model_dir(_app: &tauri::AppHandle) -> Result<PathBuf, String> {
    // Build mode selects one resource location; never search alternative paths.
    #[cfg(debug_assertions)]
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models");
    #[cfg(not(debug_assertions))]
    let directory = _app
        .path()
        .resource_dir()
        .map_err(|error| format!("获取应用资源目录失败: {error}"))?
        .join("models");
    if !directory.is_dir() {
        return Err(format!("OCR 模型目录不存在: {}", directory.display()));
    }
    Ok(directory)
}
