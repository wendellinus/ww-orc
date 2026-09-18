# IPC 契约

## 当前已注册接口

本次重构保持名称、参数和返回值不变。DTO 字段为 camelCase，时间为 Unix 秒，现有错误仍为 String。

| Command | 输入 | 输出 |
| --- | --- | --- |
| list_workspaces | 无 | Workspace[] |
| create_workspace | name | Workspace |
| rename_workspace | workspaceId、name | void |
| delete_workspace | workspaceId | void |
| list_documents | workspaceId | OcrDocument[] |
| delete_document | workspaceId、imageId | void |
| ocr_image | workspaceId、path | OcrDocument |
| ocr_image_bytes | Raw PNG/JPEG bytes，x-workspace-id header | OcrDocument |

OcrDocument：imageId、workspaceId、fileName、imagePath、status、text、errorMessage、createdAt。imagePath 仅用于现有授权预览。

## 新功能目标契约（尚未注册）

新功能以 ID 引用资源和对象，不接收任意窗口 label 或内部资源路径。统一错误为 {code,message}，code 包括 not_found/invalid_input/busy/conflict/storage/capture/ocr/window。旧接口迁移时前后端同步更新，不增加兼容回退层。

| Command | 输入 | 输出 |
| --- | --- | --- |
| import_image | workspaceId；文件对话框选择路径 | ImageAssetDto |
| paste_image | workspaceId | ImageAssetDto |
| get_image | imageId | 元数据和受限 asset 地址 |
| copy_image / export_image | imageId；导出目标由保存对话框获取 | void |
| start_capture | workspaceId、action | sessionId |
| confirm_capture | sessionId、monitorId、逻辑像素选区 | imageId |
| cancel_capture | sessionId | void |
| create_pin | imageId | PinDto |
| open_pin / close_pin / delete_pin | pinId | void |
| update_pin_view | pinId、zoom | PinDto |
| create_note | workspaceId、text、color | NoteDto |
| update_note | noteId、text、color、revision | NoteDto |
| open_note / close_note / delete_note | noteId | void |
| list_pins / list_notes | workspaceId | 对象数组 |
| set_object_topmost | objectKind、objectId、enabled | 实际窗口状态 |
| set_pin_click_through | pinId、enabled | 实际窗口状态 |
| enqueue_ocr | imageId | jobId |
| get_ocr_job / cancel_ocr_job | jobId | OcrJobDto |
| copy_ocr_text | jobId | void |
| create_note_from_ocr | jobId | NoteDto |

capture action 固定为 pin/ocr/copy/save。note 更新使用 revision 条件写入，旧版本返回 conflict，不覆盖新内容。

## 事件和权限

目标事件 capture:state-changed、ocr:job-changed 包含 ID、状态和递增 revision。OCR 完成只发送结果 ID，不发送图片。前端先订阅再查询，忽略旧 revision，窗口卸载释放订阅。

command 验证调用窗口角色与绑定对象；capability 不替代业务授权。capture 只操作本会话，pin/note 只操作绑定对象，main 拥有管理能力。权限按角色配置，不给全部窗口通配文件访问、任意窗口操作或 shell 权限。
