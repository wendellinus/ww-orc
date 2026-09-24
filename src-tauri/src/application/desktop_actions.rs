use crate::{
    features::{
        image_assets::{self, storage::AppStorage},
        pins,
    },
    infrastructure::{
        desktop_windows::{manager, repository as window_repository},
        persistence::Database,
    },
};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopItems {
    pub pins: Vec<pins::model::Pin>,
}

pub fn changed(app: &tauri::AppHandle) {
    let _ = app.emit("desktop:changed", ());
}

pub fn list(app: &tauri::AppHandle, workspace: &str) -> Result<DesktopItems, String> {
    Ok(DesktopItems {
        pins: pins::repository::list(&app.state::<Database>(), workspace)?,
    })
}

fn pin_spec(
    app: &tauri::AppHandle,
    id: &str,
    position: Option<(i32, i32)>,
) -> Result<manager::PinWindowSpec, String> {
    let database = app.state::<Database>();
    let pin = pins::repository::get(&database, id)?;
    let image =
        image_assets::repository::get(&database, &app.state::<AppStorage>(), &pin.image_id)?;
    Ok(manager::PinWindowSpec {
        id: pin.id,
        workspace_id: pin.workspace_id,
        image_id: pin.image_id,
        image_path: image.absolute_path,
        width: image.width.try_into().map_err(|_| "贴图宽度无效")?,
        height: image.height.try_into().map_err(|_| "贴图高度无效")?,
        zoom: pin.zoom,
        position,
        saved: window_repository::get(&database, "pin", id)?,
    })
}

pub fn create_pin(app: &tauri::AppHandle, image_id: &str) -> Result<pins::model::Pin, String> {
    create_pin_at(app, image_id, None)
}

pub fn create_pin_at(
    app: &tauri::AppHandle,
    image_id: &str,
    position: Option<(i32, i32)>,
) -> Result<pins::model::Pin, String> {
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
    if let Err(error) = manager::open(app, pin_spec(app, &pin.id, position)?) {
        pins::repository::set_open(&app.state::<Database>(), &pin.id, false)?;
        return Err(error);
    }
    pins::repository::set_open(&app.state::<Database>(), &pin.id, true)?;
    changed(app);
    pins::repository::get(&app.state::<Database>(), &pin.id)
}

pub fn open(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
    manager::open(app, pin_spec(app, id, None)?)?;
    pins::repository::set_open(&app.state::<Database>(), id, true)?;
    changed(app);
    Ok(())
}

pub fn close(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
    let pin = pins::repository::get(&app.state::<Database>(), id)?;
    if pin.is_open {
        manager::close(app, id.to_string())?;
    }
    pins::repository::set_open(&app.state::<Database>(), id, false)?;
    changed(app);
    Ok(())
}

pub fn delete(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
    let pin = pins::repository::get(&app.state::<Database>(), id)?;
    if pin.is_open {
        manager::close(app, id.to_string())?;
    }
    pins::repository::delete(&app.state::<Database>(), id)?;
    changed(app);
    Ok(())
}

pub fn restore(app: &tauri::AppHandle) -> Result<(), String> {
    let workspaces = crate::features::workspace::repository::list(&app.state::<Database>())?;
    for workspace in workspaces {
        for pin in pins::repository::list(&app.state::<Database>(), &workspace.id)?
            .into_iter()
            .filter(|pin| pin.is_open)
        {
            open(app, &pin.id)?;
        }
    }
    Ok(())
}

#[derive(Default)]
pub struct QuitState(pub AtomicBool);

pub fn request_quit(app: &tauri::AppHandle) -> Result<(), String> {
    manager::persist_all(app)?;
    app.state::<QuitState>().0.store(true, Ordering::Release);
    app.exit(0);
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
            .map_err(|error| error.to_string())?;
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
        open(app, &pin.id)?;
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
