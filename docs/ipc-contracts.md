# IPC 契约

所有 DTO 字段为 camelCase，时间为 Unix 秒，错误统一以 String 返回。桌面贴图窗口由 Rust/Win32 管理，不暴露 WebView 权限。

## 已注册接口

| Command                                          | 输入                                    | 输出与调用方                |
| ------------------------------------------------ | --------------------------------------- | --------------------------- |
| list/create/rename/delete_workspace              | 原有参数                                | 工作区接口；仅 main         |
| list_documents / delete_document                 | workspaceId、imageId                    | OCR 历史；仅 main           |
| ocr_image / ocr_image_bytes / ocr_existing_image | 对应图片参数                            | OcrDocument；仅 main        |
| start/cancel_capture                             | 对应截图参数                            | 截图会话；按窗口校验        |
| configure_native_capture                         | shortcut、pasteShortcut、showShortcut、workspaceId | 持久化 Rust 启动快捷键；仅 main |
| get_capture_snapshot / get_capture_preview       | sessionId、monitorId                    | 截图数据；匹配 capture 窗口 |
| finish_capture                                   | sessionId、monitorId、selection、action | imageId；匹配 capture 窗口  |
| list_desktop_items                               | workspaceId                             | pins；仅 main               |
| create_pin                                       | imageId                                 | Pin；仅 main                |
| paste_clipboard_pin                              | workspaceId                             | Pin 或 null；仅 main        |
| open_pin / close_pin / delete_pin                | id                                      | void；仅 main               |
| request_desktop_quit                             | 无                                      | void；仅 main               |

selection 是相对显示器快照的比例坐标，Rust 校验后映射为原图像素。action 固定为 pin/ocr/copy。

## 事件

- desktop:changed：刷新主界面贴图列表。
- ocr:changed：payload 为 workspaceId，刷新 OCR 历史。
- desktop:error：原生窗口或后台操作失败时发给 main。
- capture:pin-selection：截图中触发贴图快捷键时固定当前有效选区。
- capture:dispose：截图完成或取消后，通知所有框选 WebView 销毁自身。

事件不传输大图片。贴图复制、OCR、缩放、拖动和关闭均在原生窗口线程处理。

## 权限

main 保留应用命令权限；capture WebView 只具有截图所需权限。原生贴图不是 WebView，因此没有文件、IPC 或浏览器权限面。
