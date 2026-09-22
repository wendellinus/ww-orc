use image::{Rgba, RgbaImage};
use serde::Deserialize;

use super::session::Snapshot;

#[derive(Clone, Deserialize)]
pub struct Selection {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

fn pixels(selection: &Selection) -> Result<(i32, i32, u32, u32), String> {
    if [selection.x, selection.y, selection.width, selection.height]
        .iter()
        .any(|value| !value.is_finite())
        || selection.width <= 0.0
        || selection.height <= 0.0
    {
        return Err("截图选区无效".into());
    }
    let left = selection.x.floor();
    let top = selection.y.floor();
    let right = (selection.x + selection.width).ceil();
    let bottom = (selection.y + selection.height).ceil();
    if left < i32::MIN as f64
        || top < i32::MIN as f64
        || right > i32::MAX as f64
        || bottom > i32::MAX as f64
    {
        return Err("截图选区超出桌面范围".into());
    }
    let x = left as i32;
    let y = top as i32;
    let width = (right - left) as u32;
    let height = (bottom - top) as u32;
    if width < 2 || height < 2 {
        return Err("截图选区太小".into());
    }
    Ok((x, y, width, height))
}

pub struct ComposedCapture {
    pub image: RgbaImage,
    pub x: i32,
    pub y: i32,
}

pub fn compose_selection(
    snapshots: &[Snapshot],
    selection: &Selection,
) -> Result<ComposedCapture, String> {
    let (x, y, width, height) = pixels(selection)?;
    let right = i64::from(x) + i64::from(width);
    let bottom = i64::from(y) + i64::from(height);
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
    if i64::from(x) < desktop_left
        || i64::from(y) < desktop_top
        || right > desktop_right
        || bottom > desktop_bottom
    {
        return Err("截图选区超出虚拟桌面范围".into());
    }

    let mut image = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 255]));
    let mut intersects_display = false;
    for snapshot in snapshots {
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
        image::imageops::replace(&mut image, &crop, left - i64::from(x), top - i64::from(y));
    }
    if !intersects_display {
        return Err("截图选区不在任何显示器内".into());
    }
    Ok(ComposedCapture { image, x, y })
}
