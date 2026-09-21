use crate::{
    features::{image_assets::storage::AppStorage, ocr::engine::OcrEngineState},
    infrastructure::{
        logging,
        persistence::{run_migrations, Database},
    },
    ipc,
};
use std::fs;
use tauri::Manager;

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
            crate::application::tray_actions::init(app.handle())?;
            let handle = app.handle().clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(error) = crate::application::desktop_actions::restore(&handle) {
                    log::error!("desktop_restore_failed");
                    use tauri::Emitter;
                    let _ = handle.emit_to(
                        "main",
                        "desktop:error",
                        format!("恢复桌面窗口失败：{error}"),
                    );
                }
            });
            log::info!("app_ready");
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .manage(crate::features::capture::session::CaptureState::default())
        .manage(crate::application::desktop_actions::QuitState::default())
        .manage(OcrEngineState::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            ipc::commands::workspace::list_workspaces,
            ipc::commands::workspace::create_workspace,
            ipc::commands::workspace::rename_workspace,
            ipc::commands::workspace::delete_workspace,
            ipc::commands::documents::list_documents,
            ipc::commands::documents::delete_document,
            ipc::commands::ocr::ocr_image,
            ipc::commands::ocr::ocr_image_bytes,
            ipc::commands::desktop::list_desktop_items,
            ipc::commands::desktop::create_note,
            ipc::commands::desktop::get_note,
            ipc::commands::desktop::update_note,
            ipc::commands::desktop::create_pin,
            ipc::commands::desktop::paste_clipboard_pin,
            ipc::commands::desktop::copy_pin_image,
            ipc::commands::desktop::get_pin,
            ipc::commands::desktop::open_desktop_object,
            ipc::commands::desktop::close_desktop_object,
            ipc::commands::desktop::delete_desktop_object,
            ipc::commands::desktop::set_object_topmost,
            ipc::commands::desktop::update_pin_zoom,
            ipc::commands::desktop::recognize_pin,
            ipc::commands::desktop::window_ready_to_quit,
            ipc::commands::desktop::cancel_desktop_quit,
            ipc::commands::desktop::request_desktop_quit,
            ipc::commands::capture::start_capture,
            ipc::commands::capture::get_capture_snapshot,
            ipc::commands::capture::cancel_capture,
            ipc::commands::capture::finish_capture,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|_| {
            log::error!("app_run_failed");
            log::logger().flush();
            panic!("error while running tauri application");
        });
}
