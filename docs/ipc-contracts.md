# IPC 契约

所有 DTO 字段为 camelCase，时间为 Unix 秒。当前错误统一以 String 返回。系统操作只在 Rust 执行，子窗口不能调用主窗口管理接口。

## 已注册接口

| Command | 输入 | 输出与调用方 |
| --- | --- | --- |
| list/create/rename/delete_workspace | 原有参数 | 原接口；仅 main |
| list_documents / delete_document | workspaceId、imageId | 原接口；仅 main |
| ocr_image / ocr_image_bytes | 原有文件路径或 Raw bytes | OcrDocument；仅 main |
| start_capture | workspaceId | sessionId；仅 main |
| get_capture_snapshot | sessionId、monitorId | SnapshotInfo；匹配 capture 窗口 |
| cancel_capture | sessionId | void；main 或会话内 capture |
| finish_capture | sessionId、monitorId、selection、action | imageId；匹配 capture 窗口 |
| list_desktop_items | workspaceId | notes/pins；仅 main |
| create_note | workspaceId、text | Note；仅 main |
| get_note / update_note | id；更新加 text/color/revision | Note；main 或绑定 note 窗口 |
| paste_clipboard_pin | workspaceId | Pin 或 null；main；截图中通知固定选区，否则读取剪贴板图片 |
| copy_pin_image | id | void；main 或对应贴图；复制原图 |
| create_pin | imageId | Pin；仅 main |
| get_pin | id | PinView（图片地址和尺寸）；main 或绑定 pin |
| update_pin_zoom | id、zoom | void；main 或绑定 pin |
| recognize_pin | id | 识别文字；main 或绑定 pin |
| open_desktop_object / delete_desktop_object | kind、id | void；仅 main |
| close_desktop_object | kind、id | void；main 或绑定对象 |
| set_object_topmost | kind、id、enabled | void；main 或绑定对象；贴图不可取消置顶 |
| request_desktop_quit | 无 | void；仅 main |
| window_ready_to_quit | 无 | void；note/pin 确认自身保存完成 |
| cancel_desktop_quit | message | void；note/pin 保存失败时取消退出 |

selection 是相对显示器快照的比例坐标 x/y/width/height，范围 0 至 1。Rust 检查有限数值和范围，再向外取整为原图像素。action 固定为 pin/ocr/copy。

## 事件

- desktop:changed：主界面管理列表刷新。
- ocr:changed：payload 为 workspaceId，主界面刷新对应 OCR 历史。
- desktop:error：发给 main 显示明确失败原因。
- capture:pin-selection：全局贴图快捷键通知截图窗口固定有效选区。
- desktop:flush-object：退出前通知对象窗口提交草稿及缩放。

窗口卸载释放事件监听器。app 状态改变时通过查询取得当前数据，不在事件中传大图片。

## 权限

main 保留既有权限。note/pin 只具有 core 默认状态/事件、标题拖动、复制文字权限；capture 只具有 core 默认权限。置顶、关闭及识别通过验证对象归属的 Rust command 执行，不向子窗口授予任意系统窗口或文件操作权限。

窗口路由使用 Tauri 实际 label，不信任 URL 中的角色参数。
