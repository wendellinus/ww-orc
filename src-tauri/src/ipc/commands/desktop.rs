use crate::{
    application::{desktop_actions as actions, ocr_actions},
    features::{notes, pins},
    infrastructure::{desktop_windows::manager, persistence::Database},
};
use tauri::{Emitter, Manager};
fn main_only(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err("此操作仅限主窗口".into())
    }
}
fn own(window: &tauri::WebviewWindow, kind: &str, id: &str) -> Result<(), String> {
    if window.label() == "main" || window.label() == manager::label(kind, id)? {
        Ok(())
    } else {
        Err("无权操作其他窗口".into())
    }
}
#[tauri::command]
pub fn list_desktop_items(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    workspace_id: String,
) -> Result<actions::DesktopItems, String> {
    main_only(&window)?;
    actions::list(&app, &workspace_id)
}
#[tauri::command]
pub async fn create_note(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    workspace_id: String,
    text: String,
) -> Result<notes::model::Note, String> {
    main_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || actions::create_note(&app, &workspace_id, &text))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn get_note(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    id: String,
) -> Result<notes::model::Note, String> {
    own(&window, "note", &id)?;
    notes::repository::get(&app.state::<Database>(), &id)
}
#[tauri::command]
pub fn update_note(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    id: String,
    text: String,
    color: String,
    revision: i64,
) -> Result<notes::model::Note, String> {
    own(&window, "note", &id)?;
    let note = notes::repository::update(&app.state::<Database>(), &id, &text, &color, revision)?;
    actions::changed(&app);
    Ok(note)
}
#[tauri::command]
pub async fn create_pin(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    image_id: String,
) -> Result<pins::model::Pin, String> {
    main_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || actions::create_pin(&app, &image_id))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn get_pin(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    id: String,
) -> Result<actions::PinView, String> {
    own(&window, "pin", &id)?;
    actions::pin_view(&app, &id)
}
#[tauri::command]
pub async fn open_desktop_object(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    kind: String,
    id: String,
) -> Result<(), String> {
    main_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || actions::open(&app, &kind, &id))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn close_desktop_object(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    kind: String,
    id: String,
) -> Result<(), String> {
    own(&window, &kind, &id)?;
    tauri::async_runtime::spawn_blocking(move || actions::close(&app, &kind, &id))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn delete_desktop_object(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    kind: String,
    id: String,
) -> Result<(), String> {
    main_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || actions::delete(&app, &kind, &id))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn set_object_topmost(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    kind: String,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    own(&window, &kind, &id)?;
    if kind == "pin" && !enabled {
        return Err("贴图始终置顶".into());
    }
    let win = app
        .get_webview_window(&manager::label(&kind, &id)?)
        .ok_or("窗口未打开")?;
    win.set_always_on_top(enabled).map_err(|e| e.to_string())?;
    manager::save(&win, &kind, &id)
}
#[tauri::command]
pub fn update_pin_zoom(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    id: String,
    zoom: f64,
) -> Result<(), String> {
    own(&window, "pin", &id)?;
    pins::repository::zoom(&app.state::<Database>(), &id, zoom)
}
#[tauri::command]
pub async fn recognize_pin(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    id: String,
) -> Result<String, String> {
    own(&window, "pin", &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let pin = pins::repository::get(&app.state::<Database>(), &id)?;
        let result = ocr_actions::recognize_asset(&app, &pin.image_id)?;
        let _ = app.emit("ocr:changed", &pin.workspace_id);
        Ok(result.text)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn window_ready_to_quit(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<(), String> {
    if !window.label().starts_with("note_") && !window.label().starts_with("pin_") {
        return Err("仅对象窗口可以确认保存".into());
    }
    actions::window_ready_to_quit(&app, window.label())
}
#[tauri::command]
pub async fn cancel_desktop_quit(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    message: String,
) -> Result<(), String> {
    if !window.label().starts_with("note_") && !window.label().starts_with("pin_") {
        return Err("仅便签可取消保存退出".into());
    }
    tauri::async_runtime::spawn_blocking(move || actions::cancel_quit(&app, &message))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn request_desktop_quit(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<(), String> {
    main_only(&window)?;
    actions::request_quit(&app)
}

#[tauri::command]
pub async fn paste_clipboard_pin(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    workspace_id: String,
) -> Result<Option<pins::model::Pin>, String> {
    main_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || actions::paste_clipboard_pin(&app, &workspace_id))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn copy_pin_image(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    id: String,
) -> Result<(), String> {
    own(&window, "pin", &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        use tauri_plugin_clipboard_manager::ClipboardExt;
        let view = actions::pin_view(&app, &id)?;
        let image = image::open(view.image_path)
            .map_err(|e| e.to_string())?
            .into_rgba8();
        let (width, height) = image.dimensions();
        app.clipboard()
            .write_image(&tauri::image::Image::new_owned(
                image.into_raw(),
                width,
                height,
            ))
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
