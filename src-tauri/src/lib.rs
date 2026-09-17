// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod documents;
mod ocr_layout;
mod persistence;
mod workspace;

use documents::{
    repository as document_repository,
    storage::AppStorage,
    types::{OcrDocument, StoredImage},
};
use paddle_ocr_rs::ocr_lite::OcrLite;
use persistence::{run_migrations, unix_timestamp, Database};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::Manager;

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
            let raw_dict =
                fs::read(&dict_path).map_err(|error| format!("读取 OCR 字典失败: {error}"))?;
            let mut normalized_dict = b"#\n".to_vec();
            normalized_dict.extend(raw_dict);
            if !normalized_dict.ends_with(b"\n") {
                normalized_dict.push(b'\n');
            }
            let normalized_dict_path =
                std::env::temp_dir().join(format!("ww-ocr-dict-{}.txt", std::process::id()));
            fs::write(&normalized_dict_path, normalized_dict)
                .map_err(|error| format!("写入 OCR 临时字典失败: {error}"))?;
            let dict = normalized_dict_path.to_string_lossy();
            let mut ocr = OcrLite::new();
            ocr.init_models_with_dict(&det, &cls, &rec, &dict, 2)
                .map_err(|error| format!("加载 Rust OCR 模型失败: {error}"))?;
            *engine = Some(ocr);
        }

        let image_path = image_path.to_string_lossy();
        let result = engine
            .as_mut()
            .expect("OCR engine must exist")
            .detect_from_path(&image_path, 50, 1024, 0.5, 0.3, 1.6, true, false)
            .map_err(|error| format!("Rust OCR 识别失败: {error}"))?;

        Ok(ocr_layout::format_text(&result.text_blocks))
    }
}

fn model_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let bundled = app
        .path()
        .resource_dir()
        .map_err(|error| format!("获取应用资源目录失败: {error}"))?
        .join("models");
    let development = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models");

    if bundled.is_dir() {
        Ok(bundled)
    } else {
        Ok(development)
    }
}

fn recognize_stored_image(
    app: &tauri::AppHandle,
    state: &OcrEngineState,
    database: &Database,
    storage: &AppStorage,
    image: StoredImage,
) -> Result<OcrDocument, String> {
    let run_id = match document_repository::create_pending(database, &image) {
        Ok(run_id) => run_id,
        Err(error) => {
            storage.remove_file(&image.absolute_path);
            return Err(error);
        }
    };

    match state.recognize(&model_dir(app)?, &image.absolute_path) {
        Ok(text) => {
            document_repository::complete_run(database, &run_id, &text)?;
            app.asset_protocol_scope()
                .allow_file(&image.absolute_path)
                .map_err(|error| format!("授权图片预览失败: {error}"))?;

            Ok(OcrDocument {
                image_id: image.id,
                workspace_id: image.workspace_id,
                file_name: image.original_name,
                image_path: image.absolute_path.to_string_lossy().into_owned(),
                status: "completed".to_string(),
                text,
                error_message: None,
                created_at: unix_timestamp()?,
            })
        }
        Err(error) => {
            if let Err(update_error) = document_repository::fail_run(database, &run_id, &error) {
                return Err(format!("{error}；同时记录失败状态时出错: {update_error}"));
            }
            Err(error)
        }
    }
}

#[tauri::command]
fn ocr_image(
    app: tauri::AppHandle,
    state: tauri::State<'_, OcrEngineState>,
    database: tauri::State<'_, Database>,
    storage: tauri::State<'_, AppStorage>,
    workspace_id: String,
    path: String,
) -> Result<OcrDocument, String> {
    if !workspace::repository::exists(&database, &workspace_id)? {
        return Err("工作区不存在".to_string());
    }

    let image = storage.store_path(&workspace_id, &PathBuf::from(path.trim()))?;
    recognize_stored_image(&app, &state, &database, &storage, image)
}

#[tauri::command]
fn ocr_image_bytes(
    app: tauri::AppHandle,
    state: tauri::State<'_, OcrEngineState>,
    database: tauri::State<'_, Database>,
    storage: tauri::State<'_, AppStorage>,
    request: tauri::ipc::Request,
) -> Result<OcrDocument, String> {
    let workspace_id = request
        .headers()
        .get("x-workspace-id")
        .ok_or_else(|| "缺少工作区 ID".to_string())?
        .to_str()
        .map_err(|_| "工作区 ID 格式无效".to_string())?;

    if !workspace::repository::exists(&database, workspace_id)? {
        return Err("工作区不存在".to_string());
    }

    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("剪贴板图片传输格式无效".to_string());
    };

    let image = storage.store_bytes(workspace_id, bytes)?;
    recognize_stored_image(&app, &state, &database, &storage, image)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_local_data_dir()?;
            let database_dir = data_dir.join("database");
            fs::create_dir_all(&database_dir)?;

            let database =
                Database::open(&database_dir.join("app.sqlite")).map_err(std::io::Error::other)?;
            run_migrations(&database).map_err(std::io::Error::other)?;
            let storage = AppStorage::new(data_dir).map_err(std::io::Error::other)?;

            app.manage(database);
            app.manage(storage);
            Ok(())
        })
        .manage(OcrEngineState::default())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            workspace::commands::list_workspaces,
            workspace::commands::create_workspace,
            workspace::commands::rename_workspace,
            workspace::commands::delete_workspace,
            documents::commands::list_documents,
            documents::commands::delete_document,
            ocr_image,
            ocr_image_bytes,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
