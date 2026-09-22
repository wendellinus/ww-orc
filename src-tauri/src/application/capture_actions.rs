use crate::{
    features::{
        capture::{
            coordinates::{pixels, Selection},
            session::{CaptureState, Session, Snapshot, SnapshotInfo, WindowRegion},
        },
        image_assets::{self, storage::AppStorage},
    },
    infrastructure::persistence::Database,
};
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_clipboard_manager::ClipboardExt;

#[cfg(desktop)]
#[derive(Clone, Copy)]
enum CaptureShortcutAction {
    Cancel,
    Copy,
    Pin,
}

#[cfg(desktop)]
const CAPTURE_SHORTCUTS: [(&str, CaptureShortcutAction); 5] = [
    ("Escape", CaptureShortcutAction::Cancel),
    ("Control+C", CaptureShortcutAction::Copy),
    ("Enter", CaptureShortcutAction::Copy),
    ("Control+V", CaptureShortcutAction::Pin),
    ("Control+T", CaptureShortcutAction::Pin),
];

#[derive(Clone, Copy)]
struct WindowBounds {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

fn visible_windows() -> Vec<WindowBounds> {
    let Ok(windows) = xcap::Window::all() else {
        log::warn!("capture_window_enumeration_failed");
        return Vec::new();
    };
    windows
        .into_iter()
        .filter_map(|window| {
            // 本进程窗口不作为窗口吸附目标；像素捕获仍保留当前可见桌面，
            // 这样截图层出现前后画面保持一致。
            if window.pid().ok() == Some(std::process::id()) {
                return None;
            }
            if window.is_minimized().unwrap_or(true) {
                return None;
            }
            let bounds = WindowBounds {
                x: window.x().ok()?,
                y: window.y().ok()?,
                width: window.width().ok()?,
                height: window.height().ok()?,
            };
            (bounds.width > 1 && bounds.height > 1).then_some(bounds)
        })
        .collect()
}

fn regions_for_monitor(
    windows: &[WindowBounds],
    monitor_x: i32,
    monitor_y: i32,
    monitor_width: u32,
    monitor_height: u32,
) -> Vec<WindowRegion> {
    let monitor_left = i64::from(monitor_x);
    let monitor_top = i64::from(monitor_y);
    let monitor_right = monitor_left + i64::from(monitor_width);
    let monitor_bottom = monitor_top + i64::from(monitor_height);
    windows
        .iter()
        .filter_map(|window| {
            let left = i64::from(window.x).max(monitor_left);
            let top = i64::from(window.y).max(monitor_top);
            let right =
                (i64::from(window.x) + i64::from(window.width)).min(monitor_right);
            let bottom =
                (i64::from(window.y) + i64::from(window.height)).min(monitor_bottom);
            (right > left && bottom > top).then_some(WindowRegion {
                x: (left - monitor_left) as u32,
                y: (top - monitor_top) as u32,
                width: (right - left) as u32,
                height: (bottom - top) as u32,
            })
        })
        .collect()
}

fn capture_label(monitor_id: u32) -> String {
    format!("capture_{monitor_id}")
}

#[cfg(windows)]
fn configure_capture_window(window: &tauri::WebviewWindow) -> Result<(), String> {
    use windows::Win32::{
        Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED,
        },
        UI::WindowsAndMessaging::{
            GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE,
            SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
            SWP_NOZORDER, WS_EX_NOACTIVATE,
        },
    };

    let handle = window.hwnd().map_err(|error| error.to_string())?;
    unsafe {
        let style = GetWindowLongPtrW(handle, GWL_EXSTYLE);
        SetWindowLongPtrW(
            handle,
            GWL_EXSTYLE,
            style | WS_EX_NOACTIVATE.0 as isize,
        );
        SetWindowPos(
            handle,
            None,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED
                | SWP_NOACTIVATE
                | SWP_NOMOVE
                | SWP_NOSIZE
                | SWP_NOZORDER,
        )
        .map_err(|error| format!("设置无焦点截图窗口失败: {error}"))?;
        let disabled = 1i32;
        DwmSetWindowAttribute(
            handle,
            DWMWA_TRANSITIONS_FORCEDISABLED,
            (&disabled as *const i32).cast(),
            std::mem::size_of_val(&disabled) as u32,
        )
        .map_err(|error| format!("关闭截图窗口动画失败: {error}"))?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn configure_capture_window(_window: &tauri::WebviewWindow) -> Result<(), String> {
    Ok(())
}

fn ensure_window(
    app: &tauri::AppHandle,
    monitor_id: u32,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    scale: f64,
) -> Result<tauri::WebviewWindow, String> {
    let label = capture_label(monitor_id);
    let window = if let Some(window) = app.get_webview_window(&label) {
        window
    } else {
        WebviewWindowBuilder::new(
            app,
            &label,
            WebviewUrl::App(
                format!("index.html?window=capture&monitor={monitor_id}").into(),
            ),
        )
        .title("框选截图 · Esc 取消")
        .decorations(false)
        // 左侧显示器使用负坐标；关闭 DWM 阴影后，窗口外框与 Canvas 原点完全重合。
        .shadow(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .resizable(false)
        .closable(false)
        .focused(false)
        .focusable(false)
        .visible(false)
        .inner_size(width as f64 / scale, height as f64 / scale)
        .build()
        .map_err(|error| error.to_string())?
    };
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|error| error.to_string())?;
    window
        .set_size(PhysicalSize::new(width, height))
        .map_err(|error| error.to_string())?;
    configure_capture_window(&window)?;
    Ok(window)
}

pub fn warmup(app: &tauri::AppHandle) -> Result<(), String> {
    for monitor in xcap::Monitor::all().map_err(|error| error.to_string())? {
        let monitor_id = monitor.id().map_err(|error| error.to_string())?;
        ensure_window(
            app,
            monitor_id,
            monitor.x().map_err(|error| error.to_string())?,
            monitor.y().map_err(|error| error.to_string())?,
            monitor.width().map_err(|error| error.to_string())?,
            monitor.height().map_err(|error| error.to_string())?,
            monitor
                .scale_factor()
                .map_err(|error| error.to_string())? as f64,
        )?;
    }
    Ok(())
}

#[cfg(windows)]
fn hide_capture_windows(
    windows: &[tauri::WebviewWindow],
) {
    use windows::Win32::{
        Graphics::Dwm::DwmFlush,
        UI::WindowsAndMessaging::{
            BeginDeferWindowPos, DeferWindowPos, EndDeferWindowPos, SWP_HIDEWINDOW,
            SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
        },
    };

    let handles = windows
        .iter()
        .filter_map(|window| window.hwnd().ok())
        .collect::<Vec<_>>();

    let hidden_as_group = (|| -> Result<(), windows::core::Error> {
        if handles.is_empty() {
            return Ok(());
        }
        let mut batch = unsafe { BeginDeferWindowPos(handles.len() as i32)? };
        let flags =
            SWP_HIDEWINDOW | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER;
        for handle in handles {
            batch = unsafe {
                DeferWindowPos(batch, handle, None, 0, 0, 0, 0, flags)?
            };
        }
        unsafe { EndDeferWindowPos(batch) }
    })()
    .is_ok();

    if !hidden_as_group {
        log::warn!("capture_group_hide_failed");
        for window in windows {
            let _ = window.hide();
        }
    }

    // hide/EndDeferWindowPos 只提交窗口变化；等 DWM 真正呈现隐藏结果后，
    // 前端才能清空最后一帧，否则深色 WebView 背景可能短暂进入合成画面。
    unsafe {
        let _ = DwmFlush();
    }

    // Win32 批量隐藏绕过了 tao 的 WindowFlags::VISIBLE。窗口已经退出
    // DWM 合成后再走一次 Tauri hide，仅同步运行时状态，确保下次 show()
    // 会真正重新显示预热窗口；此时不会再暴露 WebView 清理帧。
    for window in windows {
        let _ = window.hide();
    }
}

#[cfg(not(windows))]
fn hide_capture_windows(
    windows: &[tauri::WebviewWindow],
) {
    for window in windows {
        let _ = window.hide();
    }
}

fn clean(app: &tauri::AppHandle, session: Session) {
    let Session {
        snapshots,
        shortcuts_registered,
        ..
    } = session;
    let windows = snapshots
        .iter()
        .filter_map(|snapshot| {
            app.get_webview_window(&capture_label(snapshot.info.monitor_id))
        })
        .collect::<Vec<_>>();

    hide_capture_windows(&windows);
    // 此时窗口已经退出桌面合成，可以安全释放 Canvas 和预览像素。
    for window in windows {
        let _ = window.emit("capture:reset", ());
    }
    if shortcuts_registered {
        unregister_capture_shortcuts(app);
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

#[cfg(desktop)]
fn register_capture_shortcuts(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_global_shortcut::{
        GlobalShortcutExt, ShortcutState,
    };

    let manager = app.global_shortcut();
    let mut registered = Vec::new();
    for (shortcut, action) in CAPTURE_SHORTCUTS {
        let result = manager.on_shortcut(
            shortcut,
            move |app, _shortcut, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }
                match action {
                    CaptureShortcutAction::Cancel => {
                        let app = app.clone();
                        tauri::async_runtime::spawn_blocking(move || {
                            if let Err(error) = cancel_active(&app) {
                                let _ = app.emit_to("main", "desktop:error", error);
                            }
                        });
                    }
                    CaptureShortcutAction::Copy => {
                        let _ = app.emit("capture:copy-selection", ());
                    }
                    CaptureShortcutAction::Pin => {
                        let _ = app.emit("capture:pin-selection", ());
                    }
                }
            },
        );
        if let Err(error) = result {
            for registered_shortcut in registered {
                let _ = manager.unregister(registered_shortcut);
            }
            return Err(format!("注册截图操作快捷键失败: {error}"));
        }
        registered.push(shortcut);
    }
    Ok(())
}

#[cfg(not(desktop))]
fn register_capture_shortcuts(_app: &tauri::AppHandle) -> Result<(), String> {
    Ok(())
}

#[cfg(desktop)]
fn unregister_capture_shortcuts(app: &tauri::AppHandle) {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let manager = app.global_shortcut();
    for (shortcut, _) in CAPTURE_SHORTCUTS {
        if manager.is_registered(shortcut) {
            let _ = manager.unregister(shortcut);
        }
    }
}

#[cfg(not(desktop))]
fn unregister_capture_shortcuts(_app: &tauri::AppHandle) {}

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
    let mut session = Session {
        id: id.clone(),
        workspace_id: workspace.into(),
        snapshots: Vec::new(),
        ready_monitors: Default::default(),
        windows_revealed: false,
        shortcuts_registered: false,
    };
    let result = (|| -> Result<(), String> {
        let window_capture = std::thread::spawn(visible_windows);
        let monitors = xcap::Monitor::all().map_err(|e| format!("获取显示器失败: {e}"))?;
        if monitors.is_empty() {
            return Err("未找到显示器".into());
        }
        let monitor_ids = monitors
            .iter()
            .map(|monitor| monitor.id().map_err(|error| error.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        session.snapshots = std::thread::scope(|scope| {
            let captures = monitor_ids
                .into_iter()
                .map(|monitor_id| {
                    let session_id = id.clone();
                    scope.spawn(move || -> Result<Snapshot, String> {
                        let monitor = xcap::Monitor::all()
                            .map_err(|e| format!("获取显示器失败: {e}"))?
                            .into_iter()
                            .find(|monitor| monitor.id().ok() == Some(monitor_id))
                            .ok_or("显示器已断开")?;
                        let monitor_x = monitor.x().map_err(|e| e.to_string())?;
                        let monitor_y = monitor.y().map_err(|e| e.to_string())?;
                        let scale =
                            monitor.scale_factor().map_err(|e| e.to_string())? as f64;
                        let mut image = monitor
                            .capture_image()
                            .map_err(|e| format!("屏幕截图失败: {e}"))?;
                        // 桌面抓屏应被视为不透明图像。部分 Windows 捕获后端返回的
                        // alpha 没有显示语义，Canvas 若照用会让背景看起来半透明。
                        for pixel in image.pixels_mut() {
                            pixel.0[3] = u8::MAX;
                        }
                        Ok(Snapshot {
                            info: SnapshotInfo {
                                session_id,
                                monitor_id,
                                monitor_x,
                                monitor_y,
                                width: image.width(),
                                height: image.height(),
                                desktop_x: 0,
                                desktop_y: 0,
                                desktop_width: 0,
                                desktop_height: 0,
                                window_regions: Vec::new(),
                            },
                            image,
                            x: monitor_x,
                            y: monitor_y,
                            scale,
                        })
                    })
                })
                .collect::<Vec<_>>();
            captures
                .into_iter()
                .map(|capture| {
                    capture
                        .join()
                        .map_err(|_| "屏幕截图线程异常退出".to_string())?
                })
                .collect::<Result<Vec<_>, _>>()
        })?;
        let windows = window_capture.join().unwrap_or_else(|_| {
            log::warn!("capture_window_enumeration_panicked");
            Vec::new()
        });
        let desktop_left = session
            .snapshots
            .iter()
            .map(|snapshot| i64::from(snapshot.x))
            .min()
            .ok_or("未找到显示器")?;
        let desktop_top = session
            .snapshots
            .iter()
            .map(|snapshot| i64::from(snapshot.y))
            .min()
            .ok_or("未找到显示器")?;
        let desktop_right = session
            .snapshots
            .iter()
            .map(|snapshot| i64::from(snapshot.x) + i64::from(snapshot.info.width))
            .max()
            .ok_or("未找到显示器")?;
        let desktop_bottom = session
            .snapshots
            .iter()
            .map(|snapshot| i64::from(snapshot.y) + i64::from(snapshot.info.height))
            .max()
            .ok_or("未找到显示器")?;
        let desktop_x = i32::try_from(desktop_left).map_err(|_| "虚拟桌面坐标超出范围")?;
        let desktop_y = i32::try_from(desktop_top).map_err(|_| "虚拟桌面坐标超出范围")?;
        let desktop_width =
            u32::try_from(desktop_right - desktop_left).map_err(|_| "虚拟桌面宽度超出范围")?;
        let desktop_height =
            u32::try_from(desktop_bottom - desktop_top).map_err(|_| "虚拟桌面高度超出范围")?;
        for snapshot in &mut session.snapshots {
            snapshot.info.desktop_x = desktop_x;
            snapshot.info.desktop_y = desktop_y;
            snapshot.info.desktop_width = desktop_width;
            snapshot.info.desktop_height = desktop_height;
            snapshot.info.window_regions = regions_for_monitor(
                &windows,
                snapshot.x,
                snapshot.y,
                snapshot.info.width,
                snapshot.info.height,
            );
        }
        Ok(())
    })();
    if let Err(error) = result {
        clean(app, session);
        return Err(error);
    }
    let previews = session
        .snapshots
        .iter()
        .map(|s| (s.info.clone(), s.x, s.y, s.scale))
        .collect::<Vec<_>>();
    if let Err(error) = register_capture_shortcuts(app) {
        clean(app, session);
        return Err(error);
    }
    session.shortcuts_registered = true;
    *guard = Some(session);
    drop(guard);
    let windows = (|| -> Result<(), String> {
        for (info, x, y, scale) in previews {
            let window = ensure_window(
                app,
                info.monitor_id,
                x,
                y,
                info.width,
                info.height,
                scale,
            )?;
            window
                .emit("capture:start", &id)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    })();
    match windows {
        Ok(_) => {}
        Err(e) => {
            let _ = cancel(app, "main", &id);
            return Err(e);
        }
    }
    Ok(id)
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
    let expected_label = capture_label(monitor_id);
    if window.label() != expected_label {
        return Err("无权显示该截图窗口".into());
    }
    let state = app.state::<CaptureState>();
    let mut guard = state.0.lock().map_err(|_| "截图会话锁不可用")?;
    let session = guard
        .as_mut()
        .filter(|session| session.id == session_id)
        .ok_or("截图会话已结束")?;
    if !session
        .snapshots
        .iter()
        .any(|snapshot| snapshot.info.monitor_id == monitor_id)
    {
        return Err("显示器截图不存在".into());
    }
    if session.windows_revealed {
        return Ok(());
    }
    session.ready_monitors.insert(monitor_id);
    if session.ready_monitors.len() != session.snapshots.len() {
        return Ok(());
    }

    // 多屏显示屏障：全部 Canvas 都写入同一时刻的冻结画面后才统一显示。
    session.windows_revealed = true;
    let labels = session
        .snapshots
        .iter()
        .map(|snapshot| capture_label(snapshot.info.monitor_id))
        .collect::<Vec<_>>();
    drop(guard);

    for label in labels {
        app.get_webview_window(&label)
            .ok_or("截图窗口不存在")?
            .show()
            .map_err(|error| error.to_string())?;
    }
    // 覆盖层使用 WS_EX_NOACTIVATE，原应用从截图开始到结束始终保持焦点。
    Ok(())
}
pub fn preview(
    app: &tauri::AppHandle,
    window: &str,
    session_id: &str,
    monitor_id: u32,
) -> Result<tauri::ipc::Response, String> {
    if window != capture_label(monitor_id) {
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
        .map(|snapshot| snapshot.image.as_raw().clone())
        .ok_or("显示器截图不存在")?;
    Ok(tauri::ipc::Response::new(bytes))
}
pub fn host_session(
    app: &tauri::AppHandle,
    window: &str,
    monitor_id: u32,
) -> Result<Option<String>, String> {
    if window != capture_label(monitor_id) {
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
    if window != capture_label(monitor_id) {
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
            .any(|m| window == capture_label(m.info.monitor_id))
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
    if window != capture_label(monitor_id) {
        return Err("无权操作该截图".into());
    }
    let state = app.state::<CaptureState>();
    let mut guard = state.0.lock().map_err(|_| "截图会话锁不可用")?;
    let session = guard
        .as_ref()
        .filter(|s| s.id == session_id)
        .ok_or("截图会话已结束")?;
    if !session
        .snapshots
        .iter()
        .any(|snapshot| snapshot.info.monitor_id == monitor_id)
    {
        return Err("显示器不存在".into());
    }
    let (x, y, w, h) = pixels(&selection)?;
    let right = i64::from(x) + i64::from(w);
    let bottom = i64::from(y) + i64::from(h);
    let desktop_left = session
        .snapshots
        .iter()
        .map(|snapshot| i64::from(snapshot.x))
        .min()
        .ok_or("未找到显示器")?;
    let desktop_top = session
        .snapshots
        .iter()
        .map(|snapshot| i64::from(snapshot.y))
        .min()
        .ok_or("未找到显示器")?;
    let desktop_right = session
        .snapshots
        .iter()
        .map(|snapshot| i64::from(snapshot.x) + i64::from(snapshot.info.width))
        .max()
        .ok_or("未找到显示器")?;
    let desktop_bottom = session
        .snapshots
        .iter()
        .map(|snapshot| i64::from(snapshot.y) + i64::from(snapshot.info.height))
        .max()
        .ok_or("未找到显示器")?;
    if i64::from(x) < desktop_left
        || i64::from(y) < desktop_top
        || right > desktop_right
        || bottom > desktop_bottom
    {
        return Err("截图选区超出虚拟桌面范围".into());
    }
    let mut image = image::RgbaImage::from_pixel(w, h, image::Rgba([0, 0, 0, 255]));
    let mut intersects_display = false;
    for snapshot in &session.snapshots {
        let left = i64::from(x).max(i64::from(snapshot.x));
        let top = i64::from(y).max(i64::from(snapshot.y));
        let clip_right = right.min(i64::from(snapshot.x) + i64::from(snapshot.info.width));
        let clip_bottom = bottom.min(i64::from(snapshot.y) + i64::from(snapshot.info.height));
        if clip_right <= left || clip_bottom <= top {
            continue;
        }
        intersects_display = true;
        let crop = image::imageops::crop_imm(
            &snapshot.image,
            (left - i64::from(snapshot.x)) as u32,
            (top - i64::from(snapshot.y)) as u32,
            (clip_right - left) as u32,
            (clip_bottom - top) as u32,
        )
        .to_image();
        image::imageops::replace(
            &mut image,
            &crop,
            left - i64::from(x),
            top - i64::from(y),
        );
    }
    if !intersects_display {
        return Err("截图选区不在任何显示器内".into());
    }
    let pin_position = PhysicalPosition::new(x, y);
    let workspace = session.workspace_id.clone();
    let session = guard.take().ok_or("截图会话已结束")?;
    drop(guard);
    clean(app, session);

    if action == "copy" {
        let tauri_image = tauri::image::Image::new_owned(image.into_raw(), w, h);
        app.clipboard()
            .write_image(&tauri_image)
            .map_err(|e| e.to_string())?;
        return Ok(String::new());
    }

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
        "copy" => unreachable!(),
        _ => unreachable!(),
    };
    super::desktop_actions::changed(app);
    let _ = app.emit("ocr:changed", &workspace);
    result?;
    Ok(id)
}
