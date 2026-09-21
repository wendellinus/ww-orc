use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};
pub enum TrayAction {
    Show,
    Capture,
    Note,
    Quit,
}
pub fn init(
    app: &tauri::AppHandle,
    action: impl Fn(&tauri::AppHandle, TrayAction) + Send + Sync + 'static,
) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "打开主窗口", true, None::<&str>)?;
    let capture = MenuItem::with_id(app, "capture", "截图贴图", true, None::<&str>)?;
    let note = MenuItem::with_id(app, "note", "新建便签", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &capture, &note, &quit])?;
    let mut builder = TrayIconBuilder::new()
        .tooltip("WW OCR · 截图、贴图与便签")
        .menu(&menu);
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder
        .on_menu_event(move |app, event| {
            let item = match event.id.as_ref() {
                "show" => TrayAction::Show,
                "capture" => TrayAction::Capture,
                "note" => TrayAction::Note,
                "quit" => TrayAction::Quit,
                _ => return,
            };
            action(app, item);
        })
        .build(app)?;
    Ok(())
}
