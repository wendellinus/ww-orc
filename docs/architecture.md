# 功能架构

## 当前状态

本轮落地现有代码的模块隔离，并设计截图、贴图、便签的接口与数据模型。已迁移工作区、图片存储、OCR 引擎与排版、OCR 历史、数据库、日志、前端快捷键。屏幕采集、贴图、便签、多窗口恢复、OCR 队列和 Rust 全局操作调度尚未实现；不创建空实现代表功能完成。

锁定版本：Tauri Rust 2.11.5、JS API 2.11.1、React 19.2.8、Tailwind CSS 4.3.3、image 0.25.10、rusqlite 0.40.2、paddle-ocr-rs 0.6.1、ort 2.0.0-rc.10。本次不改依赖和引擎，不增加 fallback。

## 依赖与隔离

```text
窗口界面 → 前端 features 公共入口 → IPC → application
                                         ↙        ↘
                                    features   infrastructure
```

IPC 只处理传输参数和 Tauri State。application 组合功能、控制顺序和清理。features 拥有业务模型、服务和 repository，不互相调用服务。infrastructure 管理系统适配，不调用业务流程。共享图片类型可以被多个功能引用，但不能包含 OCR 或窗口行为。

前端使用 index.ts 暴露功能入口，app 负责组装，功能内部组件不能跨模块导入。当前 documents 指 OCR 历史，不是办公文档模块。

## 功能方案

| 功能 | 实现 | 边界 |
| --- | --- | --- |
| 图片资源 | image_assets，image + Rust 文件读写 | 校验、保存、裁剪、读取、受引用约束的清理 |
| 截图 | capture，XCap | 采集显示器快照、管理会话，输出图片资源 |
| 框选 | capture 窗口 | 显示冻结背景并报告选区，Rust 校验和裁剪 |
| 贴图 | pins + Tauri WebviewWindow | 图片引用、缩放和打开状态；复用窗口管理器 |
| 便签 | notes + Tauri WebviewWindow | 正文、颜色、revision 和打开状态 |
| OCR | 现有 paddle-ocr-rs + ort | 引擎只识别，layout 整理阅读顺序；不复制文字或创建窗口 |
| OCR 历史 | documents | 查询记录；应用层协调记录和文件删除 |
| 窗口 | desktop_windows | 创建、关闭、置顶、穿透、位置、尺寸、启动恢复 |
| 剪贴板 | clipboard-manager Rust API | 图片和文字读写；图片编码复用 image_assets |
| 全局快捷键 | global-shortcut Rust API | 触发统一应用操作；前端保留编辑场景局部快捷键 |
| 托盘 | Tauri Rust tray API | 调用同一应用操作，不重复业务流程 |
| 存储 | rusqlite + 图片文件 | 各功能拥有 SQL，共享连接与迁移 |
| 导入导出 | dialog + image_assets | 对话框只选择路径，Rust 处理文件 |

## 图片复用

截图、剪贴板和文件导入均创建 ImageAsset，现有对应 StoredImage。复用 ID、工作区、原名、内部路径、格式、字节数和原图物理像素尺寸。新功能通过 imageId 操作，不让前端传任意内部路径。

前端通过授权 asset 地址显示图片，大图不通过事件反复发送 Base64。贴图缩放不修改原图；裁剪、OCR、复制和导出使用原图。

下一步把 images 元数据插入从 OCR pending 事务拆出至 image_assets repository。图片独立入库，OCR 仅引用已存在图片。保留 images 表，不建立同用途重复表。

## 截图

应用操作 → 采集所有显示器 → 打开冻结背景框选窗口 → 确认区域 → Rust 裁剪保存 → 关闭会话窗口 → 执行贴图/OCR/复制/保存。

状态为 capturing → selecting → completed/cancelled/failed。一次只允许一个截图会话，重复启动返回 busy；确认只允许一次，任何终态都清理临时资源。

第一版每个显示器一个框选窗口，不支持跨屏连续选区。快照记录物理原点（允许负值）、尺寸、缩放因子。前端报告窗口内逻辑像素选区，Rust 转换为快照物理像素并检查范围。桌面窗口坐标不能直接当图片裁剪坐标。

## 贴图、便签和窗口

窗口角色为 main/capture/pin/note/ocr。label 由 Rust 用固定前缀和 UUID 创建，对象与窗口绑定，前端不能传任意 label。

贴图默认置顶、无边框、隐藏任务栏图标；便签默认不置顶，可独立切换。窗口模块保存位置、尺寸、置顶；pins 保存缩放；notes 保存正文和颜色，不合并两者业务对象。

位置和尺寸保存逻辑坐标与显示器标识，恢复时校正到当前显示器可访问区域。关闭对象窗口保存状态并标记关闭，管理页可重新打开；删除是独立操作。截图和 OCR 结果窗口不参与启动恢复。main 关闭后隐藏到托盘，显式退出才结束进程。

贴图穿透不持久化，启用前确保 Rust 快捷键和托盘恢复入口可用。正文短延迟自动保存，关闭前提交最后草稿，保存失败不丢弃草稿。位置与尺寸事件节流保存，关闭保存最终状态。

## OCR

复用现有 paddle-ocr-rs 检测、方向分类和识别管线，不重写模型前后处理。engine 管理惰性加载实例，layout 负责文字排版。模型位置按构建模式明确选择：debug 使用 src-tauri/models，release 使用 Tauri resource_dir/models。每种构建只检查一个位置，目录或文件缺失直接报错，不查备用路径。

后续增加单工作线程和有界队列，串行识别；首次任务加载模型，随后复用。command 返回 jobId，事件通知状态，查询取得持久化结果。输出扩展为全文、文字块、多边形、置信度和模型版本，文字框使用原图物理像素。

排队任务取消立即结束；运行中取消不强制中断底层推理，完成后丢弃结果并记录取消。关闭结果窗口只退订事件，不取消任务。复制、转便签和窗口展示由 application 组合。

## 真实目录

```text
src/
  app/                         # 当前主窗口应用壳
  features/{ocr,workspace,shortcuts}/
  shared/{ui,lib}/
src-tauri/
  migrations/001_initial.sql
  models/                      # 现有打包模型，保持资源路径
  src/
    lib.rs                     # 唯一公开入口
    bootstrap.rs               # 初始化、插件、command 注册
    ipc/commands/{ocr,documents,workspace}.rs
    application/{ocr_actions,document_actions,workspace_actions}.rs
    features/
      image_assets/{mod,storage,types}.rs
      ocr/{mod,engine,layout}.rs
      documents/{mod,repository,types}.rs
      workspace/{mod,repository,types}.rs
    infrastructure/
      persistence/{mod,database,migrations}.rs
      paths.rs
      logging.rs
docs/{architecture,ipc-contracts,data-model}.md
```

## 新功能目录（实现时建立）

```text
src/app/window-router.ts
src/windows/{main,capture,pin,note,ocr}/
src/features/{capture,pins,notes,settings}/
src/shared/{contracts,ipc,window}/
src-tauri/src/application/{capture_actions,image_actions,note_actions,restore}.rs
src-tauri/src/features/capture/{mod,service,session,coordinates,xcap_backend}.rs
src-tauri/src/features/image_assets/{model,service,repository}.rs
src-tauri/src/features/pins/{mod,model,service,repository}.rs
src-tauri/src/features/notes/{mod,model,service,repository}.rs
src-tauri/src/features/ocr/{worker,model,service,repository}.rs
src-tauri/src/features/settings/{mod,model,service,repository}.rs
src-tauri/src/infrastructure/desktop_windows/{mod,manager,state,repository}.rs
src-tauri/src/infrastructure/{clipboard,shortcuts,tray}.rs
src-tauri/src/ipc/{contracts,events}.rs
src-tauri/capabilities/{main,capture,pin,note,ocr}.json
```

## 实施顺序与验收

1. 本轮：现有功能分层，保持 IPC 与数据库兼容；编译和现有测试通过。
2. 图片独立入库、引用约束；验证导入无需 OCR、事务失败清理。
3. 窗口管理和贴图；验证独立置顶、缩放、关闭和启动恢复。
4. 截图会话；验证不同 DPI、负坐标、取消、重复确认和裁剪边界。
5. 便签；验证自动保存、关闭提交、revision 冲突和恢复。
6. OCR 队列与文字框；验证模型复用、任务状态与图像坐标映射。
7. Rust 全局调度与托盘；移除前端全局快捷键注册，防止重复注册。

只在实际存在多实现需求时引入 trait，不建立通用 Service/Repository 框架，不设置备用引擎或静默切换实现。
