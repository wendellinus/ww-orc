# 功能架构

## 已实现功能

截图与框选、多显示器单屏区域选择、桌面贴图、贴图缩放与 OCR、便签颜色与自动保存、便签独立置顶、对象管理与删除确认、SQLite 持久化、窗口恢复、托盘、可配置全局快捷键。

版本：Tauri Rust 2.11.5、JS API 2.11.1、React 19.2.8、Tailwind CSS 4.3.3、XCap 0.9.8、image 0.25.10、rusqlite 0.40.2、paddle-ocr-rs 0.6.1、ort 2.0.0-rc.10。采用一条明确实现路线，不增加备用引擎。

## 模块依赖

```text
窗口界面 → 前端 features → IPC → application
                                ↙         ↘
                           features   infrastructure
```

application 组合功能；features 拥有模型和数据访问，不互相调用服务。共享图像类型是功能输入，不附加窗口和识别行为。infrastructure 负责数据库、文件资源定位、窗口及系统适配，不调用业务流程。IPC 解析参数并校验调用窗口归属，前端功能通过公开入口供 app 组装。

## 具体实现

| 功能 | 实现与隔离 |
| --- | --- |
| 图片 | image_assets 校验、保存和查询；被截图、贴图、OCR 共同引用 |
| 截图 | capture 拥有会话和显示器快照；coordinates 校验选区；application 调用 XCap 采集并组合后续动作 |
| 框选 | 每个显示器独立窗口，显示冻结画面和选区，不操作数据库 |
| 贴图 | pins 拥有图片引用、缩放和打开状态；透明无阴影纯图片窗口始终置顶，缩放同步物理尺寸，操作在右键菜单；窗口复用 desktop_windows |
| 便签 | notes 拥有正文、颜色、revision；前端 NoteAutosave 串行合并草稿写入 |
| OCR | 复用 engine/layout，识别在后台 spawn_blocking 执行；引擎 mutex 保证串行并复用模型 |
| OCR 历史 | documents 查询每张图片最新 run，不绑定图片必须识别 |
| 窗口 | desktop_windows 管理创建、几何状态和恢复；application 接入生命周期及对象打开状态 |
| 快捷键 | 复用既有主窗口快捷键注册系统，避免重复注册；新增截图和便签全局命令 |
| 托盘 | infrastructure 只输出操作枚举，application 调度功能 |
| 剪贴板 | Rust clipboard-manager 写截图；贴图通过受限权限复制 OCR 文字 |
| 存储 | SQLite 版本 2 + 原图文件，具体 SQL 归属对应 repository |

模型路径按构建模式明确选择：debug 为 src-tauri/models，release 为 resource_dir/models。缺失直接报错，不搜索其他位置。

## 操作流程

截图：隐藏可见主界面 → 采集各显示器 → 发布会话并释放锁 → 创建全部隐藏框选窗口 → 展示 → 确认选区 → 消费一次会话并清理窗口/预览 → 保存原图 → 贴图/OCR/复制。禁止持有会话锁等待 WebView 创建，避免多屏 IPC 初始化死锁。

便签：创建记录 → 创建独立窗口 → 读取正文和 revision → 延迟合并并串行保存 → 关闭前 flush → 保存窗口状态并关闭。失败保留草稿和窗口。

退出：通知全部对象 flush → 每个窗口确认自身保存成功 → 保存几何状态 → 退出，保持 is_open 供重启恢复。主窗口关闭只隐藏到托盘。

## 目录

```text
src/
  app/{App,window-router}.tsx
  app/ui/desktop-tools.tsx
  features/
    capture/{api,index}.ts
    notes/{api,index}.ts
    notes/model/autosave.ts
    notes/test/autosave.test.mjs
    pins/{api,index}.ts
    ocr/                       # 原有识别记录与 API
    workspace/                 # 原有工作区
    shortcuts/                 # 可配置局部和全局快捷键
  windows/
    capture/CaptureWindow.tsx
    pin/PinWindow.tsx
    note/NoteWindow.tsx
  shared/window/{api.ts,WindowBar.tsx}
  shared/styles/desktop.css
src-tauri/
  migrations/{001_initial,002_desktop}.sql
  models/
  capabilities/{default,desktop-windows,capture}.json
  src/
    lib.rs
    bootstrap.rs
    ipc/authorization.rs
    ipc/commands/{capture,desktop,ocr,documents,workspace}.rs
    application/{capture_actions,desktop_actions,tray_actions,ocr_actions,document_actions,workspace_actions}.rs
    features/
      capture/{mod,session,coordinates}.rs
      notes/{mod,model,repository}.rs
      pins/{mod,model,repository}.rs
      image_assets/{mod,storage,types,repository}.rs
      ocr/{mod,engine,layout}.rs
      documents/{mod,repository,types}.rs
      workspace/{mod,repository,types}.rs
    infrastructure/
      desktop_windows/{mod,manager,repository}.rs
      persistence/{mod,database,migrations}.rs
      paths.rs
      logging.rs
      tray.rs
```

## 本轮边界

跨显示器连续选区、图片标注、鼠标穿透、便签富文本、OCR 文字框叠层尚未提供。没有额外 OCR 引擎、云服务或 Python sidecar。使用说明见 desktop-tools.md。
