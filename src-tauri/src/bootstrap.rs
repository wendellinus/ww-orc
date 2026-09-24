use crate::{
    features::{image_assets::storage::AppStorage, ocr::engine::OcrEngineState},
    infrastructure::{
        logging,
        persistence::{run_migrations, Database},
    },
    ipc,
};
use std::{fs, time::Instant};
use tauri::Manager;

pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        if let Err(error) = crate::application::main_window_actions::show(app) {
            log::error!("main_window_single_instance_show_failed reason={error}");
        }
    }));

    builder
        .setup(|app| {
            let setup_started = Instant::now();
            logging::init(app.handle());
            #[cfg(desktop)]
            app.handle().plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                None,
            ))?;
            log::info!("app_start version={}", env!("CARGO_PKG_VERSION"));
            #[cfg(desktop)]
            app.handle().plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_handler(|app, shortcut, event| {
                        use tauri::Emitter;
                        use tauri_plugin_global_shortcut::ShortcutState;
                        // Windows 注册时已使用 MOD_NOREPEAT，不能再依赖 Released
                        // 维护额外按键锁；焦点切换期间 Released 可能延迟或丢失。
                        if event.state != ShortcutState::Pressed {
                            return;
                        }
                        enum NativeAction {
                            Capture(String),
                            Paste(String),
                            Show,
                        }
                        let binding =
                            app.state::<crate::features::capture::session::NativeCaptureShortcut>();
                        let action = binding.0.lock().ok().and_then(|binding| {
                            let workspace = binding
                                .workspace_id
                                .clone()
                                .unwrap_or_else(|| "default".into());
                            if binding
                                .shortcut
                                .as_ref()
                                .map(tauri_plugin_global_shortcut::Shortcut::id)
                                == Some(shortcut.id())
                            {
                                Some(NativeAction::Capture(workspace))
                            } else if binding
                                .paste_shortcut
                                .as_ref()
                                .map(tauri_plugin_global_shortcut::Shortcut::id)
                                == Some(shortcut.id())
                            {
                                Some(NativeAction::Paste(workspace))
                            } else if binding
                                .show_shortcut
                                .as_ref()
                                .map(tauri_plugin_global_shortcut::Shortcut::id)
                                == Some(shortcut.id())
                            {
                                Some(NativeAction::Show)
                            } else {
                                None
                            }
                        });
                        let Some(action) = action else { return };
                        let handle = app.clone();
                        tauri::async_runtime::spawn_blocking(move || {
                            let result = match action {
                                NativeAction::Capture(workspace_id) => {
                                    crate::application::capture_actions::toggle(
                                        &handle,
                                        &workspace_id,
                                    )
                                }
                                NativeAction::Paste(workspace_id) => {
                                    crate::application::desktop_actions::paste_clipboard_pin(
                                        &handle,
                                        &workspace_id,
                                    )
                                    .map(|_| ())
                                }
                                NativeAction::Show => {
                                    crate::application::main_window_actions::show(&handle)
                                }
                            };
                            if let Err(error) = result {
                                log::error!("native_shortcut_failed reason={error}");
                                let _ = handle.emit_to("main", "desktop:error", error);
                            }
                        });
                    })
                    .build(),
            )?;
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
            if let Err(error) = crate::features::capture::native_shortcuts::restore(app.handle()) {
                log::error!("native_shortcut_restore_failed reason={error}");
            }
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
            log::info!(
                "app_ready elapsed_ms={}",
                setup_started.elapsed().as_millis()
            );
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    if let Err(error) = crate::application::main_window_actions::hide_to_tray(
                        window.app_handle(),
                        window,
                    ) {
                        log::error!("main_window_hide_failed reason={error}");
                    }
                }
            }
        })
        .manage(crate::features::capture::session::CaptureState::default())
        .manage(crate::features::capture::session::NativeCaptureShortcut::default())
        .manage(crate::application::desktop_actions::QuitState::default())
        .manage(crate::application::main_window_actions::MainWindowLifecycle::default())
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
            ipc::commands::ocr::ocr_existing_image,
            ipc::commands::desktop::list_desktop_items,
            ipc::commands::desktop::create_pin,
            ipc::commands::desktop::paste_clipboard_pin,
            ipc::commands::desktop::open_pin,
            ipc::commands::desktop::close_pin,
            ipc::commands::desktop::delete_pin,
            ipc::commands::desktop::request_desktop_quit,
            ipc::commands::capture::start_capture,
            ipc::commands::capture::configure_native_capture,
            ipc::commands::capture::get_capture_snapshot,
            ipc::commands::capture::get_capture_preview,
            ipc::commands::capture::capture_host_ready,
            ipc::commands::capture::capture_window_ready,
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
