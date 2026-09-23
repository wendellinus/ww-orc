use crate::{
    features::{
        capture::{
            acquisition,
            composition::{self, Selection},
            platform_window,
            session::{CaptureState, Session, SnapshotInfo},
            shortcuts::{self, CaptureShortcutAction},
        },
        image_assets::{self, storage::AppStorage},
    },
    infrastructure::persistence::Database,
};
use tauri::{Emitter, Manager, PhysicalPosition};
use tauri_plugin_clipboard_manager::ClipboardExt;

fn clean(app: &tauri::AppHandle, session: Session) {
    platform_window::hide_and_reset(app, &session.snapshots);
    if session.shortcuts_registered {
        shortcuts::unregister(app);
    }
}

fn cancel_active(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<CaptureState>();
    let mut guard = state.0.lock().map_err(|_| "截图会话锁不可用")?;
    let Some(session) = guard.take() else {
        return Ok(());
    };
    drop(guard);
    clean(app, session);
    Ok(())
}

fn handle_shortcut(app: &tauri::AppHandle, action: CaptureShortcutAction) {
    match action {
        CaptureShortcutAction::Cancel => {
            let app = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(error) = cancel_active(&app) {
                    let _ = app.emit_to("main", "desktop:error", error);
                }
            });
        }
        CaptureShortcutAction::Confirm => {
            let _ = app.emit("capture:copy-selection", ());
        }
        CaptureShortcutAction::Pin => {
            let _ = app.emit("capture:pin-selection", ());
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

    let id = uuid::Uuid::new_v4().to_string();
    let mut session = Session::new(id.clone(), workspace.to_string());
    let snapshots = match acquisition::capture_desktop(&id) {
        Ok(snapshots) => snapshots,
        Err(error) => {
            clean(app, session);
            return Err(error);
        }
    };
    if let Err(error) = session.install_snapshots(snapshots) {
        clean(app, session);
        return Err(error);
    }
    let window_specs = platform_window::specs(&session.snapshots);
    if let Err(error) = shortcuts::register(app, handle_shortcut) {
        clean(app, session);
        return Err(error);
    }
    session.shortcuts_registered = true;
    *guard = Some(session);
    drop(guard);

    if let Err(error) = platform_window::prepare_and_start(app, &window_specs, &id) {
        let _ = cancel(app, "main", &id);
        return Err(error);
    }
    Ok(id)
}

pub fn warmup(app: &tauri::AppHandle) -> Result<(), String> {
    platform_window::warmup(app)
}

#[cfg(desktop)]
pub fn retain_native_shortcut(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let shortcuts = app
        .state::<crate::features::capture::session::NativeCaptureShortcut>()
        .0
        .lock()
        .map_err(|_| "截图快捷键状态不可用")
        .map(|binding| [binding.shortcut, binding.show_shortcut])?;
    let manager = app.global_shortcut();
    manager
        .unregister_all()
        .map_err(|error| error.to_string())?;
    for shortcut in shortcuts.into_iter().flatten() {
        manager
            .register(shortcut)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(not(desktop))]
pub fn retain_native_shortcut(_app: &tauri::AppHandle) -> Result<(), String> {
    Ok(())
}

pub fn toggle(app: &tauri::AppHandle, workspace: &str) -> Result<(), String> {
    let state = app.state::<CaptureState>();
    let mut guard = state.0.lock().map_err(|_| "截图会话锁不可用")?;
    if let Some(session) = guard.take() {
        drop(guard);
        clean(app, session);
        return Ok(());
    }
    drop(guard);
    start(app, workspace).map(|_| ())
}

pub fn restart(app: &tauri::AppHandle, workspace: &str) -> Result<(), String> {
    let state = app.state::<CaptureState>();
    let mut guard = state.0.lock().map_err(|_| "截图会话锁不可用")?;
    let previous = guard.take();
    drop(guard);
    if let Some(session) = previous {
        clean(app, session);
    }
    start(app, workspace).map(|_| ())
}

pub fn ready(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    session_id: &str,
    monitor_id: u32,
) -> Result<(), String> {
    if window.label() != platform_window::label(monitor_id) {
        return Err("无权显示该截图窗口".into());
    }
    let state = app.state::<CaptureState>();
    let mut guard = state.0.lock().map_err(|_| "截图会话锁不可用")?;
    let session = guard
        .as_mut()
        .filter(|session| session.id == session_id)
        .ok_or("截图会话已结束")?;
    if !session.mark_preview_ready(monitor_id)? {
        return Ok(());
    }
    let specs = platform_window::specs(&session.snapshots);
    drop(guard);
    platform_window::reveal_specs(app, &specs)
}

pub fn preview(
    app: &tauri::AppHandle,
    window: &str,
    session_id: &str,
    monitor_id: u32,
) -> Result<tauri::ipc::Response, String> {
    if window != platform_window::label(monitor_id) {
        return Err("无权访问该截图预览".into());
    }
    let state = app.state::<CaptureState>();
    let guard = state.0.lock().map_err(|_| "截图会话锁不可用")?;
    let session = guard
        .as_ref()
        .filter(|session| session.id == session_id)
        .ok_or("截图会话已结束")?;
    let bytes = session
        .snapshots
        .iter()
        .find(|snapshot| snapshot.info.monitor_id == monitor_id)
        .map(|snapshot| snapshot.preview_png.clone())
        .ok_or("显示器截图不存在")?;
    Ok(tauri::ipc::Response::new(bytes))
}

pub fn host_session(
    app: &tauri::AppHandle,
    window: &str,
    monitor_id: u32,
) -> Result<Option<String>, String> {
    if window != platform_window::label(monitor_id) {
        return Err("无权初始化该截图窗口".into());
    }
    let state = app.state::<CaptureState>();
    let guard = state.0.lock().map_err(|_| "截图会话锁不可用")?;
    Ok(guard.as_ref().and_then(|session| {
        session
            .snapshots
            .iter()
            .any(|snapshot| snapshot.info.monitor_id == monitor_id)
            .then(|| session.id.clone())
    }))
}

pub fn snapshot(
    app: &tauri::AppHandle,
    window: &str,
    session_id: &str,
    monitor_id: u32,
) -> Result<SnapshotInfo, String> {
    if window != platform_window::label(monitor_id) {
        return Err("无权访问该截图窗口".into());
    }
    let state = app.state::<CaptureState>();
    let guard = state.0.lock().map_err(|_| "截图会话锁不可用")?;
    let session = guard
        .as_ref()
        .filter(|session| session.id == session_id)
        .ok_or("截图会话已结束")?;
    session
        .snapshots
        .iter()
        .find(|snapshot| snapshot.info.monitor_id == monitor_id)
        .map(|snapshot| snapshot.info.clone())
        .ok_or_else(|| "显示器截图不存在".into())
}

pub fn cancel(app: &tauri::AppHandle, window: &str, session_id: &str) -> Result<(), String> {
    let state = app.state::<CaptureState>();
    let mut guard = state.0.lock().map_err(|_| "截图会话锁不可用")?;
    let session = guard
        .as_ref()
        .filter(|session| session.id == session_id)
        .ok_or("截图会话已结束")?;
    if window != "main"
        && !session
            .snapshots
            .iter()
            .any(|snapshot| window == platform_window::label(snapshot.info.monitor_id))
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
    if window != platform_window::label(monitor_id) {
        return Err("无权操作该截图".into());
    }
    let state = app.state::<CaptureState>();
    let mut guard = state.0.lock().map_err(|_| "截图会话锁不可用")?;
    let session = guard
        .as_mut()
        .filter(|session| session.id == session_id)
        .ok_or("截图会话已结束")?;
    session.ensure_selecting()?;
    if !session
        .snapshots
        .iter()
        .any(|snapshot| snapshot.info.monitor_id == monitor_id)
    {
        return Err("显示器不存在".into());
    }
    let composed = composition::compose_selection(&session.snapshots, &selection)?;
    let workspace = session.workspace_id.clone();
    session.begin_finishing()?;
    let session = guard.take().ok_or("截图会话已结束")?;
    drop(guard);
    let mut pending_session = Some(session);
    if action != "pin" {
        clean(
            app,
            pending_session
                .take()
                .expect("截图会话清理状态应当存在"),
        );
    }

    let result = (|| -> Result<String, String> {
        let width = composed.image.width();
        let height = composed.image.height();
        if action == "copy" {
            let image = tauri::image::Image::new_owned(composed.image.into_raw(), width, height);
            app.clipboard()
                .write_image(&image)
                .map_err(|error| error.to_string())?;
            return Ok(String::new());
        }

        let pin_position = PhysicalPosition::new(composed.x, composed.y);
        let storage = app.state::<AppStorage>();
        let stored = storage.store_rgba(&workspace, &composed.image)?;
        if let Err(error) =
            image_assets::repository::insert(&*app.state::<Database>().connection()?, &stored)
        {
            storage.remove_file(&stored.absolute_path);
            return Err(error);
        }
        let id = stored.id.clone();
        let action_result = match action {
            "pin" => super::desktop_actions::create_pin(app, &id).and_then(|pin| {
                let label =
                    crate::infrastructure::desktop_windows::manager::label("pin", &pin.id)?;
                let window = app.get_webview_window(&label).ok_or("贴图窗口未创建")?;
                window
                    .set_position(pin_position)
                    .map_err(|error| error.to_string())?;
                crate::infrastructure::desktop_windows::manager::save(&window, "pin", &pin.id)
            }),
            "ocr" => super::ocr_actions::recognize_asset(app, &id).map(|_| ()),
            "copy" => unreachable!(),
            _ => unreachable!(),
        };
        super::desktop_actions::changed(app);
        let _ = app.emit("ocr:changed", &workspace);
        action_result?;
        Ok(id)
    })();

    if action == "pin" && result.is_ok() {
        let state = app.state::<CaptureState>();
        let restore_result = state
            .0
            .lock()
            .map_err(|_| "截图会话锁不可用".to_string())
            .and_then(|mut guard| {
                if guard.is_some() {
                    return Err("截图会话状态冲突".to_string());
                }
                *guard = pending_session.take();
                Ok(())
            });
        if let Err(error) = restore_result {
            if let Some(session) = pending_session {
                clean(app, session);
            }
            return Err(error);
        }
        return result;
    }

    if let Some(session) = pending_session {
        clean(app, session);
    }
    result
}
