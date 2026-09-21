#[cfg(test)]
use crate::infrastructure::persistence::run_migrations;
use crate::{
    features::{
        documents::{repository as document_repository, types::OcrDocument},
        image_assets::{storage::AppStorage, types::StoredImage},
        ocr::engine::OcrEngineState,
    },
    infrastructure::{
        logging,
        paths::model_dir,
        persistence::{unix_timestamp, Database},
    },
};
#[cfg(test)]
use std::path::{Path, PathBuf};
use tauri::Manager;

pub fn recognize_stored_image(
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

pub fn recognize_path(
    app: &tauri::AppHandle,
    state: &OcrEngineState,
    database: &Database,
    storage: &AppStorage,
    workspace_id: &str,
    path: &std::path::Path,
) -> Result<OcrDocument, String> {
    let image = logging::operation("ocr_image_prepare", || {
        ensure_workspace(database, workspace_id)?;
        storage.store_path(workspace_id, path)
    })?;
    recognize_stored_image(app, state, database, storage, image)
}

pub fn recognize_bytes(
    app: &tauri::AppHandle,
    state: &OcrEngineState,
    database: &Database,
    storage: &AppStorage,
    workspace_id: &str,
    bytes: &[u8],
) -> Result<OcrDocument, String> {
    let image = logging::operation("ocr_image_bytes_prepare", || {
        ensure_workspace(database, workspace_id)?;
        storage.store_bytes(workspace_id, bytes)
    })?;
    recognize_stored_image(app, state, database, storage, image)
}

fn ensure_workspace(database: &Database, workspace_id: &str) -> Result<(), String> {
    if !crate::features::workspace::repository::exists(database, workspace_id)? {
        return Err("工作区不存在".to_string());
    }
    Ok(())
}

pub fn recognize_asset(app: &tauri::AppHandle, image_id: &str) -> Result<OcrDocument, String> {
    let database = app.state::<Database>();
    let storage = app.state::<AppStorage>();
    let image = crate::features::image_assets::repository::get(&database, &storage, image_id)?;
    let run_id = document_repository::create_existing_run(&database, &image)?;
    recognize_run(&database, &run_id, || {
        model_dir(app).and_then(|dir| {
            app.state::<OcrEngineState>()
                .recognize(&dir, &image.absolute_path)
        })
    })?;
    let record = document_repository::list(&database, &image.workspace_id)?
        .into_iter()
        .find(|r| r.image_id == image_id)
        .ok_or("识别记录不存在")?;
    super::document_actions::to_document(app, &storage, record)
}
