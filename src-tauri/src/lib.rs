// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod documents;
mod logging;
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
            let started = std::time::Instant::now();
            log::info!("model_load_started");
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
            log::info!(
                "model_load_completed elapsed_ms={}",
                started.elapsed().as_millis()
            );
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
    let run_id = match logging::operation("ocr_create_pending", || {
        document_repository::create_pending(database, &image)
    }) {
        Ok(run_id) => run_id,
        Err(error) => {
            storage.remove_file(&image.absolute_path);
            return Err(error);
        }
    };

    let text = recognize_run(database, &run_id, || {
        model_dir(app).and_then(|directory| state.recognize(&directory, &image.absolute_path))
    })?;
    logging::operation("ocr_preview", || {
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
    })
}

fn recognize_run(
    database: &Database,
    run_id: &str,
    recognize: impl FnOnce() -> Result<String, String>,
) -> Result<String, String> {
    let started = std::time::Instant::now();
    log::info!("ocr_started run_id={run_id}");
    match recognize() {
        Ok(text) => {
            document_repository::complete_run(database, run_id, &text).inspect_err(|_| {
                log::error!("ocr_status_update_failed run_id={run_id} stage=complete");
            })?;
            log::info!(
                "ocr_completed run_id={run_id} elapsed_ms={}",
                started.elapsed().as_millis()
            );
            Ok(text)
        }
        Err(error) => {
            log::error!(
                "ocr_failed run_id={run_id} reason={} elapsed_ms={}",
                logging::reason(&error),
                started.elapsed().as_millis()
            );
            if let Err(update_error) = document_repository::fail_run(database, run_id, &error) {
                log::error!("ocr_status_update_failed run_id={run_id} stage=fail");
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
    let image = logging::operation("ocr_image_prepare", || {
        if !workspace::repository::exists(&database, &workspace_id)? {
            return Err("工作区不存在".to_string());
        }

        storage.store_path(&workspace_id, &PathBuf::from(path.trim()))
    })?;
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
    let image = logging::operation("ocr_image_bytes_prepare", || {
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

        storage.store_bytes(workspace_id, bytes)
    })?;
    recognize_stored_image(&app, &state, &database, &storage, image)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            logging::init(app.handle());
            #[cfg(desktop)]
            app.handle().plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                None,
            ))?;
            log::info!("app_start version={}", env!("CARGO_PKG_VERSION"));
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_global_shortcut::Builder::new().build())?;
            let data_dir = app.path().app_local_data_dir().inspect_err(|_| {
                log::error!("app_setup_failed stage=data_directory");
            })?;
            let database_dir = data_dir.join("database");
            fs::create_dir_all(&database_dir).inspect_err(|_| {
                log::error!("app_setup_failed stage=database_directory");
            })?;

            let database = Database::open(&database_dir.join("app.sqlite"))
                .inspect_err(|_| {
                    log::error!("app_setup_failed stage=database_open");
                })
                .map_err(std::io::Error::other)?;
            run_migrations(&database)
                .inspect_err(|_| {
                    log::error!("app_setup_failed stage=database_migration");
                })
                .map_err(std::io::Error::other)?;
            log::info!("database_ready");
            let storage = AppStorage::new(data_dir)
                .inspect_err(|_| {
                    log::error!("app_setup_failed stage=storage");
                })
                .map_err(std::io::Error::other)?;

            app.manage(database);
            app.manage(storage);
            log::info!("app_ready");
            Ok(())
        })
        .manage(OcrEngineState::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
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
        .unwrap_or_else(|_| {
            log::error!("app_run_failed");
            log::logger().flush();
            panic!("error while running tauri application");
        });
}

#[cfg(test)]
mod run_tests {
    use super::*;

    #[test]
    fn resource_failure_finishes_pending_run() {
        let database = Database::open(Path::new(":memory:")).unwrap();
        run_migrations(&database).unwrap();
        let image = StoredImage {
            id: uuid::Uuid::new_v4().to_string(),
            workspace_id: "default".into(),
            original_name: "private.png".into(),
            relative_path: "test.png".into(),
            absolute_path: PathBuf::from("test.png"),
            mime_type: "image/png".into(),
            byte_size: 1,
            width: 1,
            height: 1,
        };
        let run_id = document_repository::create_pending(&database, &image).unwrap();
        let error = "获取应用资源目录失败: private path".to_string();
        assert_eq!(
            recognize_run(&database, &run_id, || Err(error.clone())),
            Err(error.clone())
        );
        let (status, saved_error, finished): (String, String, Option<i64>) = database
            .connection()
            .unwrap()
            .query_row(
                "SELECT status, error_message, finished_at FROM ocr_runs WHERE id = ?1",
                [&run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(status, "failed");
        assert_eq!(saved_error, error);
        assert!(finished.is_some());
    }
}
