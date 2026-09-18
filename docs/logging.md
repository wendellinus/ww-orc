# Rust 日志

配置集中在 `src-tauri/src/logging.rs`，使用 tauri-plugin-log 2.9.1 和 log。

- 开发：应用 debug，第三方 warn；终端和文件输出。
- 发布：应用 info；只写文件。第三方日志不写文件，避免其原始错误泄露路径或内容。
- 文件：Tauri `app_log_dir` 下 `ww-ocr.log`。Windows 为 `%LOCALAPPDATA%/com.huangw.ww-ocr/logs`。
- 单文件达到 5 MiB 后轮转，保留最多 5 个历史文件（另有当前文件）。阈值不是精确的硬容量限制，单条记录可能越过阈值。
- 时间采用插件默认 UTC。应用日志包含阶段、耗时、固定错误分类，OCR 使用内部生成的 run_id。
- 不记录原始错误、完整路径、图片字节、OCR 全文、剪贴板内容、用户名称；新增错误分类必须返回固定字符串。
- command/业务边界记录最终结果；repository/storage 返回错误，不重复打印。读取列表目前也记录结果和耗时。
- OCR 历史仍由 SQLite ocr_runs 管理，日志不参与业务状态判断。资源目录解析失败也会将 pending 更新为 failed。
- 文件初始化失败时降级到开发终端（发布版无输出），不阻止启动；运行期写入错误不会作为业务错误返回。文件系统不可用时无法保证落盘。
- panic 只记录源码位置并尽力 flush，不记录 payload。启动失败只记录阶段。进程被强杀或原生崩溃不保证捕获。
- 未接入前端日志，未开放 log 插件前端权限，也未转发到 WebView。

调整级别、轮转大小和保留数量请修改集中配置；暂不提供 UI 或环境变量开关。
