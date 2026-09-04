// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use paddle_ocr_rs::ocr_lite::OcrLite;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Manager;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

struct OcrEngineState(Mutex<Option<OcrLite>>);

impl Default for OcrEngineState {
    fn default() -> Self {
        Self(Mutex::new(None))
    }
}

impl OcrEngineState {
    fn recognize(&self, model_dir: &Path, image_path: &Path) -> Result<String, String> {
        let mut engine = self
            .0
            .lock()
            .map_err(|_| "OCR 引擎状态锁已中毒".to_string())?;

        if engine.is_none() {
            let det_path = model_dir.join("det.onnx");
            let cls_path = model_dir.join("cls.onnx");
            let rec_path = model_dir.join("rec.onnx");
            let dict_path = model_dir.join("dict.txt");
            for path in [&det_path, &cls_path, &rec_path, &dict_path] {
                if !path.is_file() {
                    return Err(format!(
                        "缺少 OCR 模型文件: {}。请把 det.onnx、cls.onnx、rec.onnx、dict.txt 放入 {}",
                        path.display(),
                        model_dir.display()
                    ));
                }
            }

            let det = det_path.to_string_lossy();
            let cls = cls_path.to_string_lossy();
            let rec = rec_path.to_string_lossy();
            // paddle-ocr-rs 将第 0 类作为 CTC 空白类，但 PaddleOCR 原始字典
            // 不包含这个占位符。生成到临时目录，避免修改打包后的只读资源。
            let raw_dict = fs::read(&dict_path).map_err(|e| format!("读取 OCR 字典失败: {e}"))?;
            let mut normalized_dict = b"#\n".to_vec();
            normalized_dict.extend(raw_dict);
            if !normalized_dict.ends_with(b"\n") {
                normalized_dict.push(b'\n');
            }
            let normalized_dict_path =
                std::env::temp_dir().join(format!("ww-ocr-dict-{}.txt", std::process::id()));
            fs::write(&normalized_dict_path, normalized_dict)
                .map_err(|e| format!("写入 OCR 临时字典失败: {e}"))?;
            let dict = normalized_dict_path.to_string_lossy();
            let mut ocr = OcrLite::new();
            ocr.init_models_with_dict(&det, &cls, &rec, &dict, 2)
                .map_err(|e| format!("加载 Rust OCR 模型失败: {e}"))?;
            *engine = Some(ocr);
        }

        let image_path = image_path.to_string_lossy();
        let result = engine
            .as_mut()
            .expect("OCR engine must exist")
            .detect_from_path(&image_path, 50, 1024, 0.5, 0.3, 1.6, true, false)
            .map_err(|e| format!("Rust OCR 识别失败: {e}"))?;

        Ok(result
            .text_blocks
            .iter()
            .map(|block| block.text.trim())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

#[derive(Serialize)]
struct OcrResult {
    text: String,
}

fn model_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let bundled = app
        .path()
        .resource_dir()
        .map_err(|e| format!("获取应用资源目录失败: {e}"))?
        .join("models");
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models");

    if bundled.is_dir() {
        Ok(bundled)
    } else {
        Ok(dev)
    }
}

/// 接收原生拖拽得到的图片路径 -> 由 Rust 直接加载 ONNX OCR 模型
#[tauri::command]
fn ocr_image(
    app: tauri::AppHandle,
    state: tauri::State<'_, OcrEngineState>,
    path: String,
) -> Result<OcrResult, String> {
    let image_path = PathBuf::from(path.trim());
    if !image_path.is_file() {
        return Err(format!("图片文件不存在: {}", image_path.display()));
    }

    // 首次请求时加载模型，后续请求复用同一个 Rust OCR 引擎。
    let text = state.recognize(&model_dir(&app)?, &image_path)?;

    // 仅授权本次识别的文件，供前端通过 convertFileSrc 加载预览。
    app.asset_protocol_scope()
        .allow_file(&image_path)
        .map_err(|e| format!("授权图片预览失败: {e}"))?;

    Ok(OcrResult { text })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(OcrEngineState::default())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, ocr_image])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
