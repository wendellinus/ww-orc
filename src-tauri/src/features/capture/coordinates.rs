use serde::Deserialize;
#[derive(Clone, Deserialize)]
pub struct Selection {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
pub fn pixels(s: &Selection, width: u32, height: u32) -> Result<(u32, u32, u32, u32), String> {
    if [s.x, s.y, s.width, s.height].iter().any(|v| !v.is_finite())
        || s.x < 0.0
        || s.y < 0.0
        || s.width <= 0.0
        || s.height <= 0.0
        || s.x + s.width > 1.000001
        || s.y + s.height > 1.000001
    {
        return Err("截图选区无效".into());
    }
    let x = (s.x * width as f64).floor() as u32;
    let y = (s.y * height as f64).floor() as u32;
    let right = ((s.x + s.width) * width as f64).ceil().min(width as f64) as u32;
    let bottom = ((s.y + s.height) * height as f64).ceil().min(height as f64) as u32;
    if right <= x || bottom <= y {
        return Err("截图选区太小".into());
    }
    Ok((x, y, right - x, bottom - y))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scales_selection_and_checks_bounds() {
        assert_eq!(
            pixels(
                &Selection {
                    x: 0.25,
                    y: 0.2,
                    width: 0.5,
                    height: 0.6
                },
                2000,
                1000
            )
            .unwrap(),
            (500, 200, 1000, 600)
        );
        assert!(pixels(
            &Selection {
                x: 0.9,
                y: 0.0,
                width: 0.2,
                height: 1.0
            },
            100,
            100
        )
        .is_err());
        assert!(pixels(
            &Selection {
                x: f64::NAN,
                y: 0.0,
                width: 1.0,
                height: 1.0
            },
            100,
            100
        )
        .is_err());
    }
}
