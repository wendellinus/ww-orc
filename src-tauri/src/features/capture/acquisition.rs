use super::session::{Snapshot, SnapshotInfo, WindowRegion};

#[derive(Clone, Copy)]
struct WindowBounds {
    id: u32,
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
            if window.pid().ok() == Some(std::process::id())
                || window.is_minimized().unwrap_or(true)
            {
                return None;
            }
            let bounds = WindowBounds {
                id: window.id().ok()?,
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
            let right = (i64::from(window.x) + i64::from(window.width)).min(monitor_right);
            let bottom = (i64::from(window.y) + i64::from(window.height)).min(monitor_bottom);
            (right > left && bottom > top).then_some(WindowRegion {
                window_id: window.id,
                x: (left - monitor_left) as u32,
                y: (top - monitor_top) as u32,
                width: (right - left) as u32,
                height: (bottom - top) as u32,
            })
        })
        .collect()
}

pub fn capture_desktop(session_id: &str) -> Result<Vec<Snapshot>, String> {
    let window_capture = std::thread::spawn(visible_windows);
    let monitors = xcap::Monitor::all().map_err(|error| format!("获取显示器失败: {error}"))?;
    if monitors.is_empty() {
        return Err("未找到显示器".into());
    }
    let monitor_ids = monitors
        .iter()
        .map(|monitor| monitor.id().map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut snapshots = std::thread::scope(|scope| {
        let captures = monitor_ids
            .into_iter()
            .map(|monitor_id| {
                let session_id = session_id.to_string();
                scope.spawn(move || -> Result<Snapshot, String> {
                    let monitor = xcap::Monitor::all()
                        .map_err(|error| format!("获取显示器失败: {error}"))?
                        .into_iter()
                        .find(|monitor| monitor.id().ok() == Some(monitor_id))
                        .ok_or("显示器已断开")?;
                    let monitor_x = monitor.x().map_err(|error| error.to_string())?;
                    let monitor_y = monitor.y().map_err(|error| error.to_string())?;
                    let scale = monitor.scale_factor().map_err(|error| error.to_string())? as f64;
                    let mut image = monitor
                        .capture_image()
                        .map_err(|error| format!("屏幕截图失败: {error}"))?;
                    for pixel in image.pixels_mut() {
                        pixel.0[3] = u8::MAX;
                    }
                    let mut preview = std::io::Cursor::new(Vec::new());
                    image
                        .write_to(&mut preview, image::ImageFormat::Png)
                        .map_err(|error| format!("编码截图预览失败: {error}"))?;
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
                        preview_png: preview.into_inner(),
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
    let desktop_left = snapshots
        .iter()
        .map(|snapshot| i64::from(snapshot.x))
        .min()
        .ok_or("未找到显示器")?;
    let desktop_top = snapshots
        .iter()
        .map(|snapshot| i64::from(snapshot.y))
        .min()
        .ok_or("未找到显示器")?;
    let desktop_right = snapshots
        .iter()
        .map(|snapshot| i64::from(snapshot.x) + i64::from(snapshot.info.width))
        .max()
        .ok_or("未找到显示器")?;
    let desktop_bottom = snapshots
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
    for snapshot in &mut snapshots {
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
    Ok(snapshots)
}
