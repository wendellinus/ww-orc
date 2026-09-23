use crate::{
    features::{
        image_assets::{self, storage::AppStorage},
        notes, pins,
    },
    infrastructure::{desktop_windows::manager, persistence::Database},
};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager, WindowEvent};
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopItems {
    pub notes: Vec<notes::model::Note>,
    pub pins: Vec<pins::model::Pin>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinView {
    pub pin: pins::model::Pin,
    pub image_path: String,
    pub width: i64,
    pub height: i64,
}
pub fn changed(app: &tauri::AppHandle) {
    let _ = app.emit("desktop:changed", ());
}
pub fn list(app: &tauri::AppHandle, workspace: &str) -> Result<DesktopItems, String> {
    let db = app.state::<Database>();
    Ok(DesktopItems {
        notes: notes::repository::list(&db, workspace)?,
        pins: pins::repository::list(&db, workspace)?,
    })
}
pub fn pin_view(app: &tauri::AppHandle, id: &str) -> Result<PinView, String> {
    let db = app.state::<Database>();
    let pin = pins::repository::get(&db, id)?;
    let image = image_assets::repository::get(&db, &app.state::<AppStorage>(), &pin.image_id)?;
    app.asset_protocol_scope()
        .allow_file(&image.absolute_path)
        .map_err(|e| e.to_string())?;
    Ok(PinView {
        pin,
        image_path: image.absolute_path.to_string_lossy().into_owned(),
        width: image.width,
        height: image.height,
    })
}
pub fn create_note(
    app: &tauri::AppHandle,
    workspace: &str,
    text: &str,
) -> Result<notes::model::Note, String> {
    let note = notes::repository::create(&app.state::<Database>(), workspace, text)?;
    if let Err(e) = open(app, "note", &note.id) {
        notes::repository::set_open(&app.state::<Database>(), &note.id, false)?;
        return Err(e);
    }
    changed(app);
    Ok(note)
}
pub fn create_pin(app: &tauri::AppHandle, image_id: &str) -> Result<pins::model::Pin, String> {
    let image = image_assets::repository::get(
        &app.state::<Database>(),
        &app.state::<AppStorage>(),
        image_id,
    )?;
    let pin = match pins::repository::get_by_image(
        &app.state::<Database>(),
        &image.workspace_id,
        image_id,
    )? {
        Some(pin) => pin,
        None => pins::repository::create(&app.state::<Database>(), &image.workspace_id, image_id)?,
    };
    if let Err(e) = open(app, "pin", &pin.id) {
        pins::repository::set_open(&app.state::<Database>(), &pin.id, false)?;
        return Err(e);
    }
    changed(app);
    Ok(pin)
}
pub fn open(app: &tauri::AppHandle, kind: &str, id: &str) -> Result<(), String> {
    let name = manager::label(kind, id)?;
    let existing = app.get_webview_window(&name).is_some();
    let (width, height) = if kind == "pin" {
        let view = pin_view(app, id)?;
        (view.width as f64, view.height as f64)
    } else {
        notes::repository::get(&app.state::<Database>(), id)?;
        (320.0, 340.0)
    };
    let win = manager::open(
        app,
        kind,
        id,
        if kind == "pin" {
            width
        } else {
            width.clamp(240.0, 900.0)
        },
        if kind == "pin" {
            height
        } else {
            height.clamp(180.0, 700.0)
        },
    )?;
    if kind == "pin" {
        pins::repository::set_open(&app.state::<Database>(), id, true)?;
    } else {
        notes::repository::set_open(&app.state::<Database>(), id, true)?;
    }
    if !existing {
        let handle = app.clone();
        let object = id.to_string();
        let role = kind.to_string();
        let label = name.clone();
        let saved = Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
        win.on_window_event(move |event| match event {
            WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
                if let Ok(mut last) = saved.lock() {
                    if last.elapsed() >= Duration::from_millis(250) {
                        if let Some(w) = handle.get_webview_window(&label) {
                            if manager::save(&w, &role, &object).is_err() {
                                log::error!("window_state_save_failed");
                            }
                        }
                        *last = Instant::now();
                    }
                }
            }
            WindowEvent::CloseRequested { .. } => {
                if let Some(w) = handle.get_webview_window(&label) {
                    let _ = manager::save(&w, &role, &object);
                }
            }
            WindowEvent::Destroyed => {
                if handle
                    .state::<QuitState>()
                    .1
                    .load(std::sync::atomic::Ordering::Acquire)
                {
                    return;
                }
                let db = handle.state::<Database>();
                let result = if role == "pin" {
                    pins::repository::set_open(&db, &object, false)
                } else {
                    notes::repository::set_open(&db, &object, false)
                };
                if result.is_err() {
                    log::error!("window_close_state_failed");
                }
                changed(&handle);
            }
            _ => {}
        });
    }
    manager::save(&win, kind, id)?;
    changed(app);
    Ok(())
}
pub fn close(app: &tauri::AppHandle, kind: &str, id: &str) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(&manager::label(kind, id)?) {
        manager::save(&win, kind, id)?;
        win.destroy().map_err(|e| e.to_string())?;
    }
    if kind == "pin" {
        pins::repository::set_open(&app.state::<Database>(), id, false)?;
    } else {
        notes::repository::set_open(&app.state::<Database>(), id, false)?;
    }
    changed(app);
    Ok(())
}
pub fn delete(app: &tauri::AppHandle, kind: &str, id: &str) -> Result<(), String> {
    close(app, kind, id)?;
    if kind == "pin" {
        pins::repository::delete(&app.state::<Database>(), id)?;
    } else {
        notes::repository::delete(&app.state::<Database>(), id)?;
    }
    changed(app);
    Ok(())
}
pub fn restore(app: &tauri::AppHandle) -> Result<(), String> {
    let workspaces = crate::features::workspace::repository::list(&app.state::<Database>())?;
    for workspace in workspaces {
        let items = list(app, &workspace.id)?;
        for note in items.notes.into_iter().filter(|n| n.is_open) {
            open(app, "note", &note.id)?;
        }
        for pin in items.pins.into_iter().filter(|p| p.is_open) {
            open(app, "pin", &pin.id)?;
        }
    }
    Ok(())
}

#[derive(Default)]
pub struct QuitState(
    pub Mutex<Option<std::collections::HashSet<String>>>,
    pub std::sync::atomic::AtomicBool,
);
pub fn request_quit(app: &tauri::AppHandle) -> Result<(), String> {
    let windows = app.webview_windows();
    let notes = windows
        .keys()
        .filter(|l| l.starts_with("note_") || l.starts_with("pin_"))
        .cloned()
        .collect::<std::collections::HashSet<_>>();
    if notes.is_empty() {
        persist_geometry(app)?;
        app.state::<QuitState>()
            .1
            .store(true, std::sync::atomic::Ordering::Release);
        app.exit(0);
        return Ok(());
    }
    let state = app.state::<QuitState>();
    {
        let mut guard = state.0.lock().map_err(|_| "退出状态不可用")?;
        if guard.is_some() {
            return Err("正在保存便签，请稍候".into());
        }
        *guard = Some(notes.clone());
    }
    for id in notes {
        app.emit_to(id, "desktop:flush-object", ())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub fn window_ready_to_quit(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
    let state = app.state::<QuitState>();
    let mut guard = state.0.lock().map_err(|_| "退出状态不可用")?;
    if let Some(waiting) = guard.as_mut() {
        waiting.remove(id);
        if waiting.is_empty() {
            persist_geometry(app)?;
            app.state::<QuitState>()
                .1
                .store(true, std::sync::atomic::Ordering::Release);
            drop(guard);
            app.exit(0);
        }
    }
    Ok(())
}
pub fn cancel_quit(app: &tauri::AppHandle, message: &str) -> Result<(), String> {
    *app.state::<QuitState>()
        .0
        .lock()
        .map_err(|_| "退出状态不可用")? = None;
    crate::application::main_window_actions::show(app)?;
    app.emit_to("main", "desktop:error", message)
        .map_err(|e| e.to_string())
}

fn persist_geometry(app: &tauri::AppHandle) -> Result<(), String> {
    for (name, win) in app.webview_windows() {
        if let Some(id) = name.strip_prefix("note_") {
            manager::save(&win, "note", id)?;
        }
        if let Some(id) = name.strip_prefix("pin_") {
            manager::save(&win, "pin", id)?;
        }
    }
    Ok(())
}

pub fn paste_clipboard_pin(
    app: &tauri::AppHandle,
    workspace: &str,
) -> Result<Option<pins::model::Pin>, String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let capturing = app
        .state::<crate::features::capture::session::CaptureState>()
        .0
        .lock()
        .map_err(|_| "截图会话锁不可用")?
        .is_some();
    if capturing {
        app.emit("capture:pin-selection", ())
            .map_err(|e| e.to_string())?;
        return Ok(None);
    }
    let clipboard = app
        .clipboard()
        .read_image()
        .map_err(|_| "剪贴板中没有图片")?;
    let rgba = image::RgbaImage::from_raw(
        clipboard.width(),
        clipboard.height(),
        clipboard.rgba().to_vec(),
    )
    .ok_or("剪贴板图片数据无效")?;
    let pixel_sha256 = image_assets::storage::pixel_sha256(&rgba);
    if let Some(pin) = find_pin_with_hash(app, workspace, &pixel_sha256)? {
        open(app, "pin", &pin.id)?;
        changed(app);
        return Ok(Some(pin));
    }
    let storage = app.state::<AppStorage>();
    let mut stored = storage.store_rgba(workspace, &rgba)?;
    stored.original_name = "剪贴板图片.png".into();
    if let Err(error) =
        image_assets::repository::insert(&*app.state::<Database>().connection()?, &stored)
    {
        storage.remove_file(&stored.absolute_path);
        return Err(error);
    }
    let pin = create_pin(app, &stored.id)?;
    let _ = app.emit("ocr:changed", workspace);
    Ok(Some(pin))
}

fn find_pin_with_hash(
    app: &tauri::AppHandle,
    workspace: &str,
    expected_hash: &str,
) -> Result<Option<pins::model::Pin>, String> {
    let database = app.state::<Database>();
    if let Some(pin) = pins::repository::get_by_pixel_sha256(&database, workspace, expected_hash)? {
        return Ok(Some(pin));
    }

    // 旧数据库记录没有哈希。只在首次遇到时解码并回填，后续全部走索引。
    let storage = app.state::<AppStorage>();
    for pin in pins::repository::list(&database, workspace)? {
        let Ok(stored) = image_assets::repository::get(&database, &storage, &pin.image_id) else {
            continue;
        };
        if stored.pixel_sha256.is_some() {
            continue;
        }
        let Ok(actual) = image::open(&stored.absolute_path).map(|image| image.to_rgba8()) else {
            continue;
        };
        let actual_hash = image_assets::storage::pixel_sha256(&actual);
        image_assets::repository::set_pixel_sha256(&database, &stored.id, &actual_hash)?;
        if actual_hash == expected_hash {
            return Ok(Some(pin));
        }
    }
    Ok(None)
}
