use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

use super::session::Snapshot;

static CAPTURE_WINDOW_CREATE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Clone, Copy)]
pub struct CaptureWindowSpec {
    pub monitor_id: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
}

pub fn specs(snapshots: &[Snapshot]) -> Vec<CaptureWindowSpec> {
    snapshots
        .iter()
        .map(|snapshot| CaptureWindowSpec {
            monitor_id: snapshot.info.monitor_id,
            x: snapshot.x,
            y: snapshot.y,
            width: snapshot.info.width,
            height: snapshot.info.height,
            scale: snapshot.scale,
        })
        .collect()
}

pub fn label(monitor_id: u32) -> String {
    format!("capture_{monitor_id}")
}

#[cfg(windows)]
fn configure(window: &tauri::WebviewWindow) -> Result<(), String> {
    use windows::Win32::{
        Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED},
        UI::WindowsAndMessaging::{
            GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, SWP_FRAMECHANGED,
            SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_EX_NOACTIVATE,
        },
    };

    let handle = window.hwnd().map_err(|error| error.to_string())?;
    unsafe {
        let style = GetWindowLongPtrW(handle, GWL_EXSTYLE);
        SetWindowLongPtrW(handle, GWL_EXSTYLE, style | WS_EX_NOACTIVATE.0 as isize);
        SetWindowPos(
            handle,
            None,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
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
fn configure(_window: &tauri::WebviewWindow) -> Result<(), String> {
    Ok(())
}

pub fn ensure(
    app: &tauri::AppHandle,
    monitor_id: u32,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    scale: f64,
) -> Result<tauri::WebviewWindow, String> {
    let _create_guard = CAPTURE_WINDOW_CREATE_LOCK
        .lock()
        .map_err(|_| "截图窗口创建锁不可用")?;
    let window_label = label(monitor_id);
    let window = if let Some(window) = app.get_webview_window(&window_label) {
        window
    } else {
        WebviewWindowBuilder::new(
            app,
            &window_label,
            WebviewUrl::App(format!("index.html?window=capture&monitor={monitor_id}").into()),
        )
        .title("框选截图 · Esc 取消")
        .decorations(false)
        .shadow(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .resizable(false)
        .closable(false)
        .focused(false)
        .focusable(false)
        .transparent(true)
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
    configure(&window)?;
    Ok(window)
}

pub fn reveal_specs(app: &tauri::AppHandle, windows: &[CaptureWindowSpec]) -> Result<(), String> {
    for spec in windows {
        app.get_webview_window(&label(spec.monitor_id))
            .ok_or("截图窗口不存在")?
            .show()
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn prepare_and_start(
    app: &tauri::AppHandle,
    windows: &[CaptureWindowSpec],
    session_id: &str,
) -> Result<(), String> {
    for spec in windows {
        let window = ensure(
            app,
            spec.monitor_id,
            spec.x,
            spec.y,
            spec.width,
            spec.height,
            spec.scale,
        )?;
        window
            .emit("capture:start", session_id)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn hide_and_reset(app: &tauri::AppHandle, snapshots: &[Snapshot]) {
    let windows = snapshots
        .iter()
        .filter_map(|snapshot| app.get_webview_window(&label(snapshot.info.monitor_id)))
        .collect::<Vec<_>>();
    hide(&windows);
    for window in windows {
        let _ = window.emit("capture:dispose", ());
    }
}

#[cfg(windows)]
fn hide(windows: &[tauri::WebviewWindow]) {
    use windows::Win32::{
        Graphics::Dwm::DwmFlush,
        UI::WindowsAndMessaging::{
            BeginDeferWindowPos, DeferWindowPos, EndDeferWindowPos, SWP_HIDEWINDOW, SWP_NOACTIVATE,
            SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
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
        let flags = SWP_HIDEWINDOW | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER;
        for handle in handles {
            batch = unsafe { DeferWindowPos(batch, handle, None, 0, 0, 0, 0, flags)? };
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
    unsafe {
        let _ = DwmFlush();
    }
    for window in windows {
        let _ = window.hide();
    }
}

#[cfg(not(windows))]
fn hide(windows: &[tauri::WebviewWindow]) {
    for window in windows {
        let _ = window.hide();
    }
}
