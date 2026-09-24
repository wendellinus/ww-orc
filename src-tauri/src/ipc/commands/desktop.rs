use crate::{application::desktop_actions as actions, features::pins};
use tauri::WebviewWindow;

fn main_only(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err("此操作仅限主窗口".into())
    }
}

#[tauri::command]
pub fn list_desktop_items(
    app: tauri::AppHandle,
    window: WebviewWindow,
    workspace_id: String,
) -> Result<actions::DesktopItems, String> {
    main_only(&window)?;
    actions::list(&app, &workspace_id)
}

#[tauri::command]
pub async fn create_pin(
    app: tauri::AppHandle,
    window: WebviewWindow,
    image_id: String,
) -> Result<pins::model::Pin, String> {
    main_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || actions::create_pin(&app, &image_id))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn open_pin(
    app: tauri::AppHandle,
    window: WebviewWindow,
    id: String,
) -> Result<(), String> {
    main_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || actions::open(&app, &id))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn close_pin(
    app: tauri::AppHandle,
    window: WebviewWindow,
    id: String,
) -> Result<(), String> {
    main_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || actions::close(&app, &id))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn delete_pin(
    app: tauri::AppHandle,
    window: WebviewWindow,
    id: String,
) -> Result<(), String> {
    main_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || actions::delete(&app, &id))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn request_desktop_quit(app: tauri::AppHandle, window: WebviewWindow) -> Result<(), String> {
    main_only(&window)?;
    actions::request_quit(&app)
}

#[tauri::command]
pub async fn paste_clipboard_pin(
    app: tauri::AppHandle,
    window: WebviewWindow,
    workspace_id: String,
) -> Result<Option<pins::model::Pin>, String> {
    main_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || actions::paste_clipboard_pin(&app, &workspace_id))
        .await
        .map_err(|error| error.to_string())?
}
