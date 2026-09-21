use crate::{
    features::{
        capture::{
            coordinates::{pixels, Selection},
            session::{CaptureState, Session, Snapshot, SnapshotInfo},
        },
        image_assets::{self, storage::AppStorage},
    },
    infrastructure::persistence::Database,
};
use image::ImageFormat;
use std::fs;

use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_clipboard_manager::ClipboardExt;
struct RestoreMain(Option<tauri::WebviewWindow>);
impl Drop for RestoreMain {
    fn drop(&mut self) {
        if let Some(win) = self.0.take() {
            let _ = win.show();
        }
    }
}
fn clean(app: &tauri::AppHandle, session: Session) {
    for snap in session.snapshots {
        if let Some(w) =
            app.get_webview_window(&format!("capture_{}_{}", session.id, snap.info.monitor_id))
        {
            let _ = w.destroy();
        }
        if fs::remove_file(&snap.path).is_err() {
            log::warn!("capture_snapshot_cleanup_failed");
        }
    }
}
pub fn start(app: &tauri::AppHandle, workspace: &str) -> Result<String, String> {
    let state = app.state::<CaptureState>();
    let mut guard = state.0.lock().map_err(|_| "截图会话锁不可用")?;
    if guard.is_some() {
        return Err("已有截图正在进行，请完成或按 Esc 取消".into());
    }
    if !crate::features::workspace::repository::exists(&app.state::<Database>(), workspace)? {
        return Err("工作区不存在".into());
    }
    let main = app.get_webview_window("main");
    let was_visible = main
        .as_ref()
        .map(|w| w.is_visible())
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or(false);
    let restore = RestoreMain(if was_visible { main } else { None });
    if let Some(win) = restore.0.as_ref() {
        win.hide().map_err(|e| e.to_string())?;
        std::thread::sleep(std::time::Duration::from_millis(120));
    }
    let id = uuid::Uuid::new_v4().to_string();
    let mut session = Session {
        id: id.clone(),
        workspace_id: workspace.into(),
        snapshots: Vec::new(),
    };
    let result = (|| -> Result<(), String> {
        let monitors = xcap::Monitor::all().map_err(|e| format!("获取显示器失败: {e}"))?;
        if monitors.is_empty() {
            return Err("未找到显示器".into());
        }
        for monitor in monitors {
            let monitor_id = monitor.id().map_err(|e| e.to_string())?;
            let image = monitor
                .capture_image()
                .map_err(|e| format!("屏幕截图失败: {e}"))?;
            let path = app.state::<AppStorage>().capture_path(&id, monitor_id);
            image
                .save_with_format(&path, ImageFormat::Png)
                .map_err(|e| format!("保存截图预览失败: {e}"))?;
            let snap = Snapshot {
                info: SnapshotInfo {
                    session_id: id.clone(),
                    monitor_id,
                    image_path: path.to_string_lossy().into_owned(),
                    width: image.width(),
                    height: image.height(),
                },
                image,
                path,
                x: monitor.x().map_err(|e| e.to_string())?,
                y: monitor.y().map_err(|e| e.to_string())?,
                scale: monitor.scale_factor().map_err(|e| e.to_string())? as f64,
            };
            session.snapshots.push(snap);
        }
        Ok(())
    })();
    if let Err(error) = result {
        clean(app, session);
        return Err(error);
    }
    drop(restore);
    let previews = session
        .snapshots
        .iter()
        .map(|s| (s.info.clone(), s.path.clone(), s.x, s.y, s.scale))
        .collect::<Vec<_>>();
    *guard = Some(session);
    drop(guard);
    let windows = (|| -> Result<Vec<tauri::WebviewWindow>, String> {
        let mut windows = Vec::new();
        for (info, path, x, y, scale) in previews {
            app.asset_protocol_scope()
                .allow_file(&path)
                .map_err(|e| e.to_string())?;
            let name = format!("capture_{}_{}", id, info.monitor_id);
            let win = WebviewWindowBuilder::new(
                app,
                &name,
                WebviewUrl::App(
                    format!(
                        "index.html?window=capture&session={id}&monitor={}",
                        info.monitor_id
                    )
                    .into(),
                ),
            )
            .title("框选截图 · Esc 取消")
            .decorations(false)
            .skip_taskbar(true)
            .always_on_top(true)
            .resizable(false)
            .closable(false)
            .visible(false)
            .inner_size(info.width as f64 / scale, info.height as f64 / scale)
            .build()
            .map_err(|e| e.to_string())?;
            win.set_position(PhysicalPosition::new(x, y))
                .map_err(|e| e.to_string())?;
            win.set_size(PhysicalSize::new(info.width, info.height))
                .map_err(|e| e.to_string())?;
            windows.push(win);
        }
        Ok(windows)
    })();
    match windows {
        Ok(windows) => {
            for win in windows {
                if let Err(e) = win.show() {
                    let _ = cancel(app, "main", &id);
                    return Err(e.to_string());
                }
            }
        }
        Err(e) => {
            let _ = cancel(app, "main", &id);
            return Err(e);
        }
    }
    Ok(id)
}
pub fn snapshot(
    app: &tauri::AppHandle,
    window: &str,
    session_id: &str,
    monitor_id: u32,
) -> Result<SnapshotInfo, String> {
    if window != format!("capture_{session_id}_{monitor_id}") {
        return Err("无权访问该截图窗口".into());
    }
    let state = app.state::<CaptureState>();
    let guard = state.0.lock().map_err(|_| "截图会话锁不可用")?;
    let session = guard
        .as_ref()
        .filter(|s| s.id == session_id)
        .ok_or("截图会话已结束")?;
    session
        .snapshots
        .iter()
        .find(|s| s.info.monitor_id == monitor_id)
        .map(|s| s.info.clone())
        .ok_or_else(|| "显示器截图不存在".into())
}
pub fn cancel(app: &tauri::AppHandle, window: &str, session_id: &str) -> Result<(), String> {
    let state = app.state::<CaptureState>();
    let mut guard = state.0.lock().map_err(|_| "截图会话锁不可用")?;
    let s = guard
        .as_ref()
        .filter(|s| s.id == session_id)
        .ok_or("截图会话已结束")?;
    if window != "main"
        && !s
            .snapshots
            .iter()
            .any(|m| window == format!("capture_{}_{}", s.id, m.info.monitor_id))
    {
        return Err("无权取消该截图".into());
    }
    let session = guard.take().ok_or("截图会话已结束")?;
    drop(guard);
    clean(app, session);
    Ok(())
}
pub fn finish(
    app: &tauri::AppHandle,
    window: &str,
    session_id: &str,
    monitor_id: u32,
    selection: Selection,
    action: &str,
) -> Result<String, String> {
    if !["pin", "ocr", "copy"].contains(&action) {
        return Err("截图操作无效".into());
    }
    if window != format!("capture_{session_id}_{monitor_id}") {
        return Err("无权操作该截图".into());
    }
    let state = app.state::<CaptureState>();
    let mut guard = state.0.lock().map_err(|_| "截图会话锁不可用")?;
    let session = guard
        .as_ref()
        .filter(|s| s.id == session_id)
        .ok_or("截图会话已结束")?;
    let snap = session
        .snapshots
        .iter()
        .find(|s| s.info.monitor_id == monitor_id)
        .ok_or("显示器不存在")?;
    let (x, y, w, h) = pixels(&selection, snap.image.width(), snap.image.height())?;
    let image = image::imageops::crop_imm(&snap.image, x, y, w, h).to_image();
    let pin_position = PhysicalPosition::new(snap.x + x as i32, snap.y + y as i32);
    let workspace = session.workspace_id.clone();
    let session = guard.take().ok_or("截图会话已结束")?;
    drop(guard);
    clean(app, session);
    let storage = app.state::<AppStorage>();
    let stored = storage.store_rgba(&workspace, &image)?;
    if let Err(e) =
        image_assets::repository::insert(&*app.state::<Database>().connection()?, &stored)
    {
        storage.remove_file(&stored.absolute_path);
        return Err(e);
    }
    let id = stored.id.clone();
    let result = match action {
        "pin" => super::desktop_actions::create_pin(app, &id).and_then(|pin| {
            let label = crate::infrastructure::desktop_windows::manager::label("pin", &pin.id)?;
            let win = app.get_webview_window(&label).ok_or("贴图窗口未创建")?;
            win.set_position(pin_position).map_err(|e| e.to_string())?;
            crate::infrastructure::desktop_windows::manager::save(&win, "pin", &pin.id)
        }),
        "ocr" => super::ocr_actions::recognize_asset(app, &id).map(|_| ()),
        "copy" => {
            let tauri_image = tauri::image::Image::new_owned(image.into_raw(), w, h);
            app.clipboard()
                .write_image(&tauri_image)
                .map_err(|e| e.to_string())
        }
        _ => unreachable!(),
    };
    super::desktop_actions::changed(app);
    let _ = app.emit("ocr:changed", &workspace);
    result?;
    Ok(id)
}
