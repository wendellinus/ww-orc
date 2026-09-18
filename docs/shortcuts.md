# 快捷键与命令系统

当前架构是 React 19.2.8 + Tauri 2：React 负责界面和交互，Rust `ocr_image` 负责图片识别。快捷键层接入应用壳，桌面端使用 Tauri 2 官方 global-shortcut 插件注册系统全局快捷键。

## 已接入的功能

| 命令 ID | 默认组合键 | 作用域 |
| --- | --- | --- |
| `ocr.copyText` | `Mod+Shift+Y` | workspace |
| `ocr.focusResult` | `Mod+Shift+E` | workspace |
| `window.toggleAlwaysOnTop` | `Mod+Shift+P` | app（全局） |
| `window.show` | `Mod+Shift+O` | app（全局） |

Mod 在 Windows/Linux 对应 Ctrl，在 macOS 对应 Meta（⌘）。点击标题栏键盘按钮打开设置。当前提供复制识别内容、聚焦识别结果、窗口置顶和唤起主窗口四项快捷键。保存后立即生效，刷新/重新打开应用后恢复。设置仅支持按键录制：点击快捷键显示框，再按下组合键；Esc、Tab、再次点击快捷键框或窗口失焦会取消录制并保留原绑定。点击快捷键框内的 × 可禁用绑定；录制、清除和恢复默认只修改草稿，点击保存后才生效。冲突、无效按键和保留键会提示错误，并继续等待重新录制。

复制和置顶操作通过命令入口执行，共用可用条件和进行中去重。设置入口与最小化、最大化、关闭窗口通过标题栏按钮操作，不注册快捷键。浏览器预览中原生窗口命令不可用。当前拖图 OCR 流程继续使用已有的 `runOcr`。

## 模块边界

```text
App 命令定义 ─────────────┐
按钮 / 键盘 / 原生适配器 → CommandRegistry → 命令处理器 → 业务服务
                         ↑
        默认绑定 + 用户覆盖 + 工作区覆盖
                         ↑
                 ShortcutStorage
```

- `src/features/shortcuts/model/keys.ts`：组合键规范化、精确修饰键匹配、平台映射、显示格式。
- `src/features/shortcuts/model/recording.ts`：录制按键的跨平台修饰键映射、物理标点编码及 IME、AltGraph、长按过滤。
- `src/features/shortcuts/model/config.ts`：版本化配置、持久化接口、作用域内冲突检测和覆盖合并。
- `src/features/shortcuts/model/registry.ts`：命令可用性、作用域路由、执行上下文快照、异步去重和取消。
- `src/features/shortcuts/hooks/use-shortcuts.ts`：应用壳中的单一键盘订阅；提交后更新命令回调，卸载时解绑并发出取消信号。只在应用壳调用一次，不在各个面板重复挂载。
- `src/features/shortcuts/ui/shortcut-settings.tsx`：录制用户级绑定、清除与恢复默认、显示录制和保存错误；原生模态对话框负责焦点限制与返回。

命令的 `id` 是稳定的业务标识，不应因文案或默认按键改变而变化。命令数组本身就是注册清单，功能模块可以导出各自的命令，在应用壳合并注册；移除条目即停止后续调度。

作用域按外到内传入，例如 `["app", "workspace", "result"]`，匹配时由内向外查找。同一作用域内禁止重复组合键；不同作用域允许覆盖。内层命令被禁用时不会穿透执行外层同键命令。弹窗打开时使用 `["dialog"]`，隔离背景命令。`app` 表示应用级作用域，不代表操作系统全局快捷键。

输入框和 contenteditable 默认不触发命令，可通过 `allowInEditable` 明确放行。忽略已消费事件、IME 组合输入、keyCode 229、按键长按重复和 AltGraph。保留现有生产环境 DevTools 拦截逻辑，改键时拒绝其保留组合键。

## 工作区隔离接入

当前已支持默认工作区以及用户创建的多个工作区。活动工作区 ID 由应用壳传入快捷键上下文，图片和 OCR 查询同时携带该 ID；切换工作区后仅加载该工作区的记录。

每次执行都固定 `workspaceId`、scopes、source 和 AbortSignal。命令等待期间切换工作区，不改变已经执行任务的上下文。`app` 命令在全应用去重，其他命令按工作区和命令 ID 去重。

业务处理器必须使用执行上下文中的 ID 写入结果，而不是 await 后读取当前选中的工作区；上下文快照本身不会隔离业务存储。删除工作区时，业务服务也必须拒绝后续写入或取消其任务。

示意接入：

```ts
const captureCommand: Command = {
  id: "capture.ocr",
  title: "截图并识别",
  scope: "workspace",
  shortcut: "Mod+Shift+X",
  enabled: ({ workspaceId }) => workspaceId !== null && captureService.available,
  async run({ workspaceId, signal }) {
    if (workspaceId === null) return;
    const image = await captureService.capture({ signal });
    if (!image || signal.aborted) return;
    const result = await ocrService.recognize(image, { signal });
    if (signal.aborted) return;
    await workspaceRepository.appendResult(workspaceId, result);
  },
};
```

这里的服务是未来实现接口的示意，当前尚不存在。AbortSignal 需要处理器和服务协作检查；它不能自动中止现有 Rust OCR。卸载应用壳时会发出取消信号，正常工作区切换不会自动取消任务。

## 持久化接入

快捷键设置继续保存在 localStorage，键为 `ww-ocr.shortcuts.v1`；活动工作区使用 `ww-ocr.active-workspace.v1`。工作区、图片元数据和 OCR 结果由 Rust 写入 SQLite，原图保存在 Tauri 的应用本地数据目录：

```json
{
  "version": 1,
  "user": { "ocr.copyText": "Mod+K" },
  "workspaces": { "research": { "ocr.copyText": null } }
}
```

覆盖优先级：默认值 → 用户 → 指定工作区。缺省继承，null 明确禁用。设置面板目前编辑用户级覆盖；工作区级覆盖已有数据结构和解析支持，后续由工作区设置面板接入。保留未注册命令的配置，便于功能按需装载。

读取失败时提示错误并使用默认值，不自动覆盖原数据；保存失败时不更改生效配置，并让编辑面板保留草稿。配置接口目前是同步的，可替换成内存缓存适配器；若接入 Tauri 文件或 SQLite 的异步 API，应增加启动加载和异步保存流程，不能直接把异步函数塞进现有同步接口。当前不支持跨窗口实时同步。

## 截图 OCR、搜索接入顺序

1. 已建立稳定 workspaceId、工作区 repository，并让 OCR 写入发起任务的工作区。
2. 已接入 SQLite 与原图文件持久化，支持创建、切换、重命名和删除工作区。
3. 后续实现截图服务，注册 `capture.ocr` 命令并复用 OCR 服务。
4. 后续实现搜索服务/面板，注册 `search.open` 等命令，区分工作区搜索和应用级搜索。
5. 后台截图命令设置 `global: true`，复用现有全局快捷键适配器。原生注册失败应保留旧绑定并显示错误；收到事件后调度 `shortcuts.execute("capture.ocr", "native")`。必须明确后台截图目标工作区和应用唤醒策略。

已注册窗口置顶、唤起主窗口两项系统全局快捷键。应用运行且主窗口最小化或失焦时仍可触发；关闭窗口退出应用后不再响应。弹窗打开时隔离命令。

全局命令设置 `global: true`，使用用户级绑定，不随工作区切换改变；系统注册失败或持久化失败会回滚，错误显示在设置中。前端仅响应 Pressed，避免按下和释放执行两次，也不会重复走应用内键盘监听。浏览器预览不注册系统快捷键。

后续截图贴图、便签使用独立置顶窗口，并以 `capture.pin`、`note.create` 等全局命令接入；功能尚未实现，不预占按键。新窗口不要挂载主窗口的全局快捷键订阅。关闭后驻留、托盘、截图捕获及多窗口权限需随具体功能实现。

## 验证

- `pnpm test`：使用 Node 22.18+ 的 TypeScript 类型擦除运行核心行为测试，无额外测试依赖。
- `pnpm build`：TypeScript 检查及生产构建。
- 浏览器验证：生产构建下检查打开/关闭、焦点限制、冲突、保留键、改键、刷新恢复、禁用、恢复默认、存储失败以及窄窗口布局。浏览器预览无法验证 Tauri 原生窗口行为。
- 样式规范：使用项目安装的 Tailwind 4.3.3 `canonicalizeCandidates` 检查设置面板类名，无规范化建议；未运行 VS Code 语言服务诊断。参考 [Tailwind 4.3 宽度规范](https://tailwindcss.com/docs/width)、[最大高度规范](https://tailwindcss.com/docs/max-height) 和 [React useLayoutEffect](https://react.dev/reference/react/useLayoutEffect)。
