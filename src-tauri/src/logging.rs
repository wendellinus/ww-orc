//! Diagnostic logs contain fixed stages and generated IDs, never user data.
use log::LevelFilter;
use std::time::Instant;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

fn builder(file: bool) -> tauri_plugin_log::Builder {
    let mut targets = Vec::new();
    if cfg!(debug_assertions) {
        targets.push(Target::new(TargetKind::Stdout));
    }
    if file {
        targets.push(
            Target::new(TargetKind::LogDir {
                file_name: Some("ww-ocr".into()),
            })
            .filter(|metadata| metadata.target().starts_with("ww_ocr_lib")),
        );
    }
    tauri_plugin_log::Builder::new()
        .level(LevelFilter::Warn)
        .level_for(
            "ww_ocr_lib",
            if cfg!(debug_assertions) {
                LevelFilter::Debug
            } else {
                LevelFilter::Info
            },
        )
        .max_file_size(5 * 1024 * 1024)
        .rotation_strategy(RotationStrategy::KeepSome(5))
        .targets(targets)
}

pub fn init(app: &tauri::AppHandle) {
    // split allows file initialization failure to fall back without failing app setup.
    let configured = builder(true).split(app);
    let file_available = configured.is_ok();
    let configured = configured.or_else(|_| builder(false).split(app));
    if let Ok((plugin, level, logger)) = configured {
        if tauri_plugin_log::attach_logger(level, logger).is_ok() {
            if app.plugin(plugin).is_err() {
                log::warn!("logging_plugin_registration_failed");
            }
            if !file_available {
                log::warn!("logging_file_unavailable");
            }
        }
    }
    std::panic::set_hook(Box::new(|info| {
        // Panic payloads may contain paths or user data. Only record source location.
        if let Some(location) = info.location() {
            log::error!("panic source={} line={}", location.file(), location.line());
        } else {
            log::error!("panic");
        }
        log::logger().flush();
    }));
}

pub fn operation<T>(
    stage: &'static str,
    action: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    operation_with_id(stage, "", action)
}

pub fn operation_with_id<T>(
    stage: &'static str,
    id: &str,
    action: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    // IDs cross IPC: parse and normalize rather than printing arbitrary input.
    let id = if id == "default" {
        "default".to_string()
    } else {
        uuid::Uuid::parse_str(id)
            .map(|id| id.to_string())
            .unwrap_or_else(|_| "none".into())
    };
    let start = Instant::now();
    let result = action();
    match &result {
        Ok(_) => log::info!(
            "operation_completed stage={stage} id={id} elapsed_ms={}",
            start.elapsed().as_millis()
        ),
        Err(error) => log::error!(
            "operation_failed stage={stage} id={id} reason={} elapsed_ms={}",
            reason(error),
            start.elapsed().as_millis()
        ),
    }
    result
}

/// A whitelist prevents raw errors (including paths and user content) entering logs.
pub fn reason(error: &str) -> &'static str {
    if error.contains("；同时恢复待删除文件失败:") {
        return "file_restore_failed";
    }
    let prefix = error.split(':').next().unwrap_or(error);
    match prefix {
        "缺少 OCR 模型文件" => "model_missing",
        "获取应用资源目录失败" => "resource_directory_failed",
        "读取 OCR 字典失败" => "dictionary_read_failed",
        "写入 OCR 临时字典失败" => "dictionary_write_failed",
        "加载 Rust OCR 模型失败" => "model_load_failed",
        "Rust OCR 识别失败" => "inference_failed",
        "工作区不存在" => "workspace_missing",
        "打开图片失败" | "读取图片信息失败" => "image_read_failed",
        "识别图片格式失败" | "无法读取剪贴板图片" | "解析剪贴板图片失败" => {
            "image_decode_failed"
        }
        "工作区记录已删除，但图片文件清理失败" | "图片记录已删除，但文件清理失败" => {
            "file_cleanup_failed"
        }
        _ => "operation_error",
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn errors_never_expose_raw_content() {
        assert_eq!(
            super::reason("缺少 OCR 模型文件: C:/private/secret.onnx"),
            "model_missing"
        );
        assert_eq!(
            super::reason("private user text\nforged log"),
            "operation_error"
        );
    }
}
