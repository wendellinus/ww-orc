# Tauri + React + Typescript

This template should help get you started developing with Tauri, React and Typescript in Vite.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## 前端目录约定

```text
src/
├─ app/                 # 应用壳、全局样式与模块组装
├─ modules/             # 按业务能力组织
│  └─ <module>/
│     ├─ model/         # 领域状态和纯业务逻辑
│     ├─ hooks/         # React 接入
│     ├─ ui/            # 模块专属界面
│     ├─ test/          # 该模块的测试
│     └─ index.ts       # 模块公共入口
└─ shared/              # 无业务归属的通用代码和基础 UI
```

应用层只通过模块的 `index.ts` 使用公开能力。模块内部可使用相对路径，但其他模块不应直接导入其 `model/`、`hooks/` 或 `ui/`。只有真正跨业务复用的代码才能进入 `shared/`。

测试与所属业务模块放在一起，统一命名为 `src/modules/<module>/test/*.test.mjs`，由 `pnpm test` 自动发现。不要重新建立根级 `tests/`，也不要为空的未来功能预建目录；当前工作空间与 OCR 分别由 `modules/workspace`、`modules/ocr` 提供公共入口。

Tauri/Rust 后端继续位于 `src-tauri/`，其业务模块使用 Rust 原生的 `mod.rs` 或同名模块文件组织。

## 快捷键系统

快捷键使用方式、模块边界与后续工作区 / 持久化 / 截图 OCR / 搜索接入见 [设计文档](docs/shortcuts.md)。

## 设置

点击顶部齿轮图标打开设置下拉菜单：

- **开机启动**：勾选后在登录系统时启动应用，取消勾选即可关闭。使用 Tauri 官方 autostart 插件，读取系统实际状态；仅桌面应用可用。
- **快捷键设置**：打开快捷键配置窗口。

开机启动建议在安装后的应用中开启，系统启动项绑定当前应用可执行文件路径。
