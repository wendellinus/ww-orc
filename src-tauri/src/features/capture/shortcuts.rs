#[derive(Clone, Copy)]
pub enum CaptureShortcutAction {
    Cancel,
    Confirm,
    Pin,
}

#[cfg(desktop)]
const CAPTURE_SHORTCUTS: [(&str, CaptureShortcutAction); 5] = [
    ("Escape", CaptureShortcutAction::Cancel),
    ("Control+C", CaptureShortcutAction::Confirm),
    ("Enter", CaptureShortcutAction::Confirm),
    ("Control+V", CaptureShortcutAction::Pin),
    ("Control+T", CaptureShortcutAction::Pin),
];

#[cfg(desktop)]
pub fn register(
    app: &tauri::AppHandle,
    handler: fn(&tauri::AppHandle, CaptureShortcutAction),
) -> Result<(), String> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

    let manager = app.global_shortcut();
    let mut registered = Vec::new();
    for (shortcut, action) in CAPTURE_SHORTCUTS {
        let result = manager.on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                handler(app, action);
            }
        });
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
pub fn register(
    _app: &tauri::AppHandle,
    _handler: fn(&tauri::AppHandle, CaptureShortcutAction),
) -> Result<(), String> {
    Ok(())
}

#[cfg(desktop)]
pub fn unregister(app: &tauri::AppHandle) {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let manager = app.global_shortcut();
    for (shortcut, _) in CAPTURE_SHORTCUTS {
        if manager.is_registered(shortcut) {
            let _ = manager.unregister(shortcut);
        }
    }
}

#[cfg(not(desktop))]
pub fn unregister(_app: &tauri::AppHandle) {}
