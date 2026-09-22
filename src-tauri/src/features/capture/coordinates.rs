use serde::Deserialize;
#[derive(Clone, Deserialize)]
pub struct Selection {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
pub fn pixels(s: &Selection) -> Result<(i32, i32, u32, u32), String> {
    if [s.x, s.y, s.width, s.height].iter().any(|v| !v.is_finite())
        || s.width <= 0.0
        || s.height <= 0.0
    {
        return Err("截图选区无效".into());
    }
    let left = s.x.floor();
    let top = s.y.floor();
    let right = (s.x + s.width).ceil();
    let bottom = (s.y + s.height).ceil();
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scales_selection_and_checks_bounds() {
        assert_eq!(
            pixels(
                &Selection {
                    x: -500.0,
                    y: 200.0,
                    width: 1000.0,
                    height: 600.0
                }
            )
            .unwrap(),
            (-500, 200, 1000, 600)
        );
        assert!(pixels(
            &Selection {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0
            }
        )
        .is_err());
        assert!(pixels(
            &Selection {
                x: f64::NAN,
                y: 0.0,
                width: 1.0,
                height: 1.0
            }
        )
        .is_err());
    }
}
