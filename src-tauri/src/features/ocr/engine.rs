use paddle_ocr_rs::ocr_lite::OcrLite;
use std::{fs, path::Path, sync::Mutex};

use super::types::{OcrOutput, OcrPoint, OcrTextBlock};

const DEFAULT_MAX_SIDE: u32 = 1024;
const SMALL_IMAGE_MAX_SIDE: u32 = 2048;
const SMALL_IMAGE_TARGET_HEIGHT: u32 = 96;
const MAX_UPSCALE: f32 = 3.0;

const DEFAULT_BOX_SCORE_THRESHOLD: f32 = 0.5;
const DEFAULT_BOX_THRESHOLD: f32 = 0.3;
const RETRY_BOX_SCORE_THRESHOLD: f32 = 0.35;
const RETRY_BOX_THRESHOLD: f32 = 0.2;
const ANGLE_ROLLBACK_THRESHOLD: f32 = 0.8;

pub struct OcrEngineState(Mutex<Option<OcrLite>>);

impl Default for OcrEngineState {
    fn default() -> Self {
        Self(Mutex::new(None))
    }
}

impl OcrEngineState {
    pub fn recognize(&self, model_dir: &Path, image_path: &Path) -> Result<OcrOutput, String> {
        let mut engine = self
            .0
            .lock()
            .map_err(|_| "OCR 引擎状态锁已中毒".to_string())?;

        if engine.is_none() {
            let started = std::time::Instant::now();
            log::info!("model_load_started");
            let det_path = model_dir.join("det.onnx");
            let cls_path = model_dir.join("cls.onnx");
            let rec_path = model_dir.join("rec.onnx");
            let dict_path = model_dir.join("dict.txt");
            for path in [&det_path, &cls_path, &rec_path, &dict_path] {
                if !path.is_file() {
                    return Err(format!(
                        "缺少 OCR 模型文件: {}。请把 det.onnx、cls.onnx、rec.onnx、dict.txt 放入 {}",
                        path.display(),
                        model_dir.display()
                    ));
                }
            }

            let det = det_path.to_string_lossy();
            let cls = cls_path.to_string_lossy();
            let rec = rec_path.to_string_lossy();
            // paddle-ocr-rs 将第 0 类作为 CTC 空白类，但 PaddleOCR 原始字典
            // 不包含这个占位符。生成到临时目录，避免修改打包后的只读资源。
            let raw_dict =
                fs::read(&dict_path).map_err(|error| format!("读取 OCR 字典失败: {error}"))?;
            let mut normalized_dict = b"#\n".to_vec();
            normalized_dict.extend(raw_dict);
            if !normalized_dict.ends_with(b"\n") {
                normalized_dict.push(b'\n');
            }
            let normalized_dict_path =
                std::env::temp_dir().join(format!("ww-ocr-dict-{}.txt", std::process::id()));
            fs::write(&normalized_dict_path, normalized_dict)
                .map_err(|error| format!("写入 OCR 临时字典失败: {error}"))?;
            let dict = normalized_dict_path.to_string_lossy();
            let mut ocr = OcrLite::new();
            ocr.init_models_with_dict(&det, &cls, &rec, &dict, 2)
                .map_err(|error| format!("加载 Rust OCR 模型失败: {error}"))?;
            *engine = Some(ocr);
            log::info!(
                "model_load_completed elapsed_ms={}",
                started.elapsed().as_millis()
            );
        }

        let source = image::open(image_path)
            .map_err(|error| format!("Rust OCR 读取图片失败: {error}"))?
            .to_rgb8();
        let source_width = source.width();
        let source_height = source.height();
        let scale = preprocessing_scale(source_width, source_height);
        let prepared = if scale > 1.0 {
            let width = ((source_width as f32 * scale).round() as u32).max(1);
            let height = ((source_height as f32 * scale).round() as u32).max(1);
            image::imageops::resize(
                &source,
                width,
                height,
                image::imageops::FilterType::Lanczos3,
            )
        } else {
            source
        };
        let max_side = if source_height < SMALL_IMAGE_TARGET_HEIGHT {
            SMALL_IMAGE_MAX_SIDE
        } else {
            DEFAULT_MAX_SIDE
        };
        log::info!(
            "ocr_input width={source_width} height={source_height} prepared_width={} prepared_height={} scale={scale:.3}",
            prepared.width(),
            prepared.height()
        );

        let ocr = engine.as_mut().expect("OCR engine must exist");
        let mut result = ocr
            .detect_angle_rollback(
                &prepared,
                50,
                max_side,
                DEFAULT_BOX_SCORE_THRESHOLD,
                DEFAULT_BOX_THRESHOLD,
                1.6,
                true,
                false,
                ANGLE_ROLLBACK_THRESHOLD,
            )
            .map_err(|error| format!("Rust OCR 识别失败: {error}"))?;

        if !contains_text(&result.text_blocks) {
            log::info!("ocr_retry reason=no_text lower_detection_thresholds=true");
            result = ocr
                .detect_angle_rollback(
                    &prepared,
                    50,
                    max_side,
                    RETRY_BOX_SCORE_THRESHOLD,
                    RETRY_BOX_THRESHOLD,
                    1.6,
                    true,
                    false,
                    ANGLE_ROLLBACK_THRESHOLD,
                )
                .map_err(|error| format!("Rust OCR 重试失败: {error}"))?;
        }

        log::info!(
            "ocr_detection_completed block_count={}",
            result.text_blocks.len()
        );

        let text = super::layout::format_text(&result.text_blocks);
        let blocks = result
            .text_blocks
            .iter()
            .filter_map(|block| {
                let text = block.text.trim();
                if text.is_empty() {
                    return None;
                }
                Some(OcrTextBlock {
                    text: text.to_string(),
                    box_points: block
                        .box_points
                        .iter()
                        .map(|point| OcrPoint {
                            x: point.x as f64 / scale as f64,
                            y: point.y as f64 / scale as f64,
                        })
                        .collect(),
                })
            })
            .collect();

        Ok(OcrOutput { text, blocks })
    }
}

fn preprocessing_scale(width: u32, height: u32) -> f32 {
    if width == 0 || height == 0 || height >= SMALL_IMAGE_TARGET_HEIGHT {
        return 1.0;
    }

    let height_scale = SMALL_IMAGE_TARGET_HEIGHT as f32 / height as f32;
    let side_scale = SMALL_IMAGE_MAX_SIDE as f32 / width.max(height) as f32;
    height_scale.min(side_scale).min(MAX_UPSCALE).max(1.0)
}

fn contains_text(blocks: &[paddle_ocr_rs::ocr_result::TextBlock]) -> bool {
    blocks.iter().any(|block| !block.text.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enlarges_short_screenshots_without_exceeding_limits() {
        assert!((preprocessing_scale(363, 36) - 96.0 / 36.0).abs() < f32::EPSILON);
        assert!((preprocessing_scale(928, 29) - 2048.0 / 928.0).abs() < f32::EPSILON);
        assert_eq!(preprocessing_scale(100, 20), MAX_UPSCALE);
        assert_eq!(preprocessing_scale(4096, 30), 1.0);
    }

    #[test]
    fn leaves_normal_screenshots_at_original_size() {
        assert_eq!(preprocessing_scale(800, 200), 1.0);
        assert_eq!(preprocessing_scale(0, 20), 1.0);
        assert_eq!(preprocessing_scale(20, 0), 1.0);
    }
}
