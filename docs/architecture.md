# 功能架构

## 已实现功能

截图与框选、多显示器单屏区域选择、Win32 原生桌面贴图、贴图缩放与 OCR、对象管理与删除确认、SQLite 持久化、窗口恢复、托盘、可配置全局快捷键。

版本以依赖声明和锁文件为准：Tauri 2、React 19、Tailwind CSS 4.3.3、XCap 0.9.8、image 0.25、rusqlite 0.40.2、paddle-ocr-rs 0.6.1、ort 2.0.0-rc.10、windows 0.61.3。

## 模块依赖

```text
主界面 → 前端 features → IPC → application
                                ↙         ↘
                           features   infrastructure
```

application 组合业务流程；features 拥有模型和数据访问；infrastructure 负责数据库、文件资源、原生窗口和系统适配。

## 性能边界

保留 Tauri/React 负责工作区、设置和 OCR 历史等低频管理界面；主 WebView 按需创建。托盘、全局快捷键、贴图和其他高频系统交互放在 Rust/Win32。Paddle/ONNX Runtime 继续作为唯一 OCR 引擎，除非真实数据集证明替换后准确率不下降。截图框选迁移原生前，指针状态按动画帧合并，跨显示器事件只保留最新值，避免高频 IPC 堆积。
## 具体实现

| 功能     | 实现                                                                                               |
| -------- | -------------------------------------------------------------------------------------------------- |
| 图片     | image_assets 校验、保存和查询，被截图、贴图、OCR 共同引用                                          |
| 截图     | capture 管理会话和显示器快照；XCap 采集，WebView 框选层按需创建，完成或取消后立即销毁              |
| 贴图     | pins 保存图片引用、缩放和打开状态；Windows 使用独立消息线程创建 layered HWND，不创建 WebView2      |
| 原生渲染 | Rust image 解码 RGBA；滚轮期间用 Triangle 重采样，停稳后从原始预乘像素执行 Lanczos3 重采样，再由 UpdateLayeredWindow 合成 |
| 贴图交互 | 左键拖动；滚轮以固定左上角缩放，限制为 0.1–3 倍、最大边 8192、最大 32MP；双击/Esc 关闭；右键复制/OCR/恢复比例 |
| OCR      | engine/layout 在 spawn_blocking 中执行；引擎串行复用模型                                           |
| 窗口状态 | window_states 保存贴图位置、尺寸和置顶状态；退出保留 is_open，启动时恢复                           |
| 剪贴板   | Rust clipboard-manager 读写图片和 OCR 文字                                                         |
| 存储     | SQLite 元数据与应用数据目录中的原图文件                                                            |

## 启动路径

进程先初始化日志、SQLite、托盘和 Rust 全局快捷键，不创建主 WebView2。F1 截图、F3 剪贴板贴图和 Ctrl+Shift+O 唤起立即可用；打开主界面时才创建 React/WebView2，隐藏超过 5 分钟后销毁。OCR 引擎仍在首次识别时惰性初始化。
## 操作流程

截图贴图：隐藏界面 → 捕获显示器 → 按需创建框选 WebView → 保存原图 → 原生线程解码并创建 HWND → 销毁框选 WebView。

剪贴板贴图：读取 RGBA → 像素哈希去重 → 保存 PNG → 创建原生贴图。已存在相同贴图时直接唤起。

退出：原生线程同步保存所有贴图几何状态 → 标记退出 → 退出进程。用户主动关闭贴图时设置 is_open=false。

## 目录

```text
src/
  app/{App,window-router}.tsx
  app/ui/desktop-tools.tsx
  features/{capture,pins,ocr,workspace,shortcuts}/
  windows/capture/CaptureWindow.tsx
  shared/styles/desktop.css
src-tauri/
  migrations/
  models/
  capabilities/{default,capture}.json
  src/
    ipc/commands/{capture,desktop,ocr,documents,workspace}.rs
    application/{capture_actions,desktop_actions,tray_actions,ocr_actions}.rs
    features/{capture,pins,image_assets,ocr,documents,workspace}/
    infrastructure/
      desktop_windows/{manager,repository}.rs
      persistence/
```

## 边界

原生贴图当前仅在 Windows 构建；跨显示器连续选区、图片标注、鼠标穿透和 OCR 文字框叠层尚未提供。历史数据库中的 notes 表保留用于升级兼容，应用已不再提供便签功能。
