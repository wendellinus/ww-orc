use super::session::{NativeCaptureBinding, NativeCaptureShortcut};
use crate::infrastructure::persistence::Database;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tauri::Manager;
use tauri_plugin_global_shortcut::{GlobalShortcut, GlobalShortcutExt, Shortcut};

const SETTINGS_KEY: &str = "native_shortcuts.v1";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeShortcutConfig {
    pub capture_shortcut: Option<String>,
    pub paste_shortcut: Option<String>,
    pub show_shortcut: Option<String>,
    pub workspace_id: Option<String>,
}

impl Default for NativeShortcutConfig {
    fn default() -> Self {
        Self {
            capture_shortcut: Some("F1".into()),
            paste_shortcut: Some("F3".into()),
            show_shortcut: Some("CommandOrControl+Shift+O".into()),
            workspace_id: Some("default".into()),
        }
    }
}

pub fn restore(app: &tauri::AppHandle) -> Result<(), String> {
    let serialized = app
        .state::<Database>()
        .connection()?
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [SETTINGS_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("读取原生快捷键设置失败: {error}"))?;
    let config = match serialized {
        Some(value) => serde_json::from_str(&value).unwrap_or_else(|error| {
            log::warn!("native_shortcut_settings_invalid reason={error}");
            NativeShortcutConfig::default()
        }),
        None => NativeShortcutConfig::default(),
    };
    apply(app, config, false)
}

pub fn configure(app: &tauri::AppHandle, config: NativeShortcutConfig) -> Result<(), String> {
    apply(app, config, true)
}

fn parse(value: Option<&str>, label: &str) -> Result<Option<Shortcut>, String> {
    value
        .map(|value| {
            Shortcut::from_str(value).map_err(|error| format!("{label}快捷键无效: {error}"))
        })
        .transpose()
}

fn apply(
    app: &tauri::AppHandle,
    config: NativeShortcutConfig,
    persist: bool,
) -> Result<(), String> {
    let capture = parse(config.capture_shortcut.as_deref(), "截图")?;
    let paste = parse(config.paste_shortcut.as_deref(), "贴图")?;
    let show = parse(config.show_shortcut.as_deref(), "唤起")?;
    let desired = [capture, paste, show];
    for (index, shortcut) in desired.iter().enumerate() {
        if let Some(shortcut) = shortcut {
            if desired[..index]
                .iter()
                .flatten()
                .any(|other| other.id() == shortcut.id())
            {
                return Err("原生全局快捷键冲突".into());
            }
        }
    }

    let state = app.state::<NativeCaptureShortcut>();
    let mut binding = state.0.lock().map_err(|_| "截图快捷键状态不可用")?;
    let previous = [
        binding.shortcut,
        binding.paste_shortcut,
        binding.show_shortcut,
    ];
    let manager = app.global_shortcut();
    let mut added = Vec::new();
    let mut active = desired;
    for index in 0..active.len() {
        let Some(next) = active[index] else {
            continue;
        };
        if previous
            .into_iter()
            .flatten()
            .any(|current| current.id() == next.id())
        {
            continue;
        }
        if manager.is_registered(next) {
            if let Err(error) = manager.unregister(next) {
                if persist {
                    rollback(manager, &added, &[]);
                    return Err(error.to_string());
                }
                log::warn!("native_shortcut_skipped shortcut={next:?} reason={error}");
                active[index] = None;
                continue;
            }
        }
        if let Err(error) = manager.register(next) {
            if persist {
                rollback(manager, &added, &[]);
                return Err(error.to_string());
            }
            log::warn!("native_shortcut_skipped shortcut={next:?} reason={error}");
            active[index] = None;
            continue;
        }
        added.push(next);
    }

    let mut removed = Vec::new();
    for current in previous.into_iter().flatten() {
        if desired
            .into_iter()
            .flatten()
            .any(|next| next.id() == current.id())
        {
            continue;
        }
        if let Err(error) = manager.unregister(current) {
            rollback(manager, &added, &removed);
            return Err(error.to_string());
        }
        removed.push(current);
    }

    if persist {
        let serialized = serde_json::to_string(&config)
            .map_err(|error| format!("序列化原生快捷键设置失败: {error}"))?;
        let result = app.state::<Database>().connection()?.execute(
            "INSERT INTO app_settings(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![SETTINGS_KEY, serialized],
        );
        if let Err(error) = result {
            rollback(manager, &added, &removed);
            return Err(format!("保存原生快捷键设置失败: {error}"));
        }
    }

    *binding = NativeCaptureBinding {
        shortcut: active[0],
        paste_shortcut: active[1],
        show_shortcut: active[2],
        workspace_id: config.workspace_id,
    };
    Ok(())
}

fn rollback<R: tauri::Runtime>(
    manager: &GlobalShortcut<R>,
    added: &[Shortcut],
    removed: &[Shortcut],
) {
    for shortcut in added {
        let _ = manager.unregister(*shortcut);
    }
    for shortcut in removed {
        let _ = manager.register(*shortcut);
    }
}
