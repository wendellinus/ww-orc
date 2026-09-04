# OCR 模型目录

应用现在由 Rust 直接调用 ONNX OCR 引擎，不再启动 Python。运行前需要把下面三个文件放在本目录：

- det.onnx：文本检测模型
- cls.onnx：文字方向分类模型
- rec.onnx：中文文本识别模型
- dict.txt：与识别模型匹配的字符字典。程序会自动补齐 `paddle-ocr-rs` 需要的 CTC 空白占位符。

推荐使用同一版本 PaddleOCR / PaddleOCR-rs 的中文检测模型、识别模型和字典，并确保模型已经转换为 ONNX。开发模式和打包后的应用都会从 models 目录加载这些文件。
