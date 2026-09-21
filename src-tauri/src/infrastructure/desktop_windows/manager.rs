use super::repository::{self, WindowState};
use crate::infrastructure::persistence::Database;
use tauri::{
    Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
pub fn label(kind: &str, id: &str) -> Result<String, String> {
    if !["pin", "note"].contains(&kind) || uuid::Uuid::parse_str(id).is_err() {
        return Err("窗口对象无效".into());
    }
    Ok(format!("{kind}_{id}"))
}
pub fn save(window: &WebviewWindow, kind: &str, id: &str) -> Result<(), String> {
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let size = window.inner_size().map_err(|e| e.to_string())?;
    repository::save(
        &window.app_handle().state::<Database>(),
        kind,
        id,
        &WindowState {
            x: pos.x,
            y: pos.y,
            width: size.width,
            height: size.height,
            topmost: window.is_always_on_top().map_err(|e| e.to_string())?,
        },
    )
}
pub fn open(
    app: &tauri::AppHandle,
    kind: &str,
    id: &str,
    width: f64,
    height: f64,
) -> Result<WebviewWindow, String> {
    let name = label(kind, id)?;
    if let Some(win) = app.get_webview_window(&name) {
        if kind == "pin" {
            win.set_always_on_top(true).map_err(|e| e.to_string())?;
        }
        win.show().map_err(|e| e.to_string())?;
        win.set_focus().map_err(|e| e.to_string())?;
        return Ok(win);
    }
    let state = repository::get(&app.state::<Database>(), kind, id)?;
    let win = WebviewWindowBuilder::new(
        app,
        &name,
        WebviewUrl::App(format!("index.html?window={kind}&id={id}").into()),
    )
    .title(if kind == "pin" {
        "桌面贴图"
    } else {
        "便签"
    })
    .decorations(false)
    .transparent(kind == "pin")
    .shadow(false)
    .resizable(kind != "pin")
    .skip_taskbar(true)
    .visible(false)
    .inner_size(width, height)
    .min_inner_size(
        if kind == "pin" { 1.0 } else { 180.0 },
        if kind == "pin" { 1.0 } else { 120.0 },
    )
    .always_on_top(kind == "pin" || state.as_ref().is_some_and(|s| s.topmost))
    .center()
    .build()
    .map_err(|e| format!("创建窗口失败: {e}"))?;
    if let Some(s) = state {
        let monitors = win.available_monitors().map_err(|e| e.to_string())?;
        let visible = monitors.iter().any(|m| {
            let p = m.position();
            let z = m.size();
            s.x + 80 > p.x
                && s.y + 40 > p.y
                && s.x < p.x + z.width as i32
                && s.y < p.y + z.height as i32
        });
        if visible {
            win.set_position(PhysicalPosition::new(s.x, s.y))
                .map_err(|e| e.to_string())?;
        }
        win.set_size(PhysicalSize::new(s.width.min(3000), s.height.min(2200)))
            .map_err(|e| e.to_string())?;
    }
    win.show().map_err(|e| e.to_string())?;
    Ok(win)
}
