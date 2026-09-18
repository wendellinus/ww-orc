use paddle_ocr_rs::ocr_lite::OcrLite;
use std::{fs, path::Path, sync::Mutex};

pub struct OcrEngineState(Mutex<Option<OcrLite>>);

impl Default for OcrEngineState {
    fn default() -> Self {
        Self(Mutex::new(None))
    }
}

impl OcrEngineState {
    pub fn recognize(&self, model_dir: &Path, image_path: &Path) -> Result<String, String> {
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

        let image_path = image_path.to_string_lossy();
        let result = engine
            .as_mut()
            .expect("OCR engine must exist")
            .detect_from_path(&image_path, 50, 1024, 0.5, 0.3, 1.6, true, false)
            .map_err(|error| format!("Rust OCR 识别失败: {error}"))?;

        Ok(super::layout::format_text(&result.text_blocks))
    }
}
