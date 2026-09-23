import { invoke, isTauri } from "@tauri-apps/api/core";
import { register, unregister } from "@tauri-apps/plugin-global-shortcut";
import { GlobalShortcuts, nativeShortcut } from "../model/global-shortcuts";
import { useEffect, useLayoutEffect, useState } from "react";
import { CommandRegistry, type Command, type CommandContext, type CommandExecution } from "../model/registry";
import { emptyConfig, localShortcutStorage, parseConfig, resolveBindings, validateBindings, type ShortcutConfig, type ShortcutStorage } from "../model/config";
import { formatShortcut, type Platform } from "../model/keys";

export function useShortcuts(commands: readonly Command[], context: CommandContext, onError: (message: string) => void, storage: ShortcutStorage = localShortcutStorage) {
  const [platform] = useState<Platform>(() => /Mac|iPhone|iPad/.test(navigator.platform) ? "mac" : "other");
  const [registry] = useState(() => new CommandRegistry());
  const [native] = useState(() => isTauri() ? new GlobalShortcuts(
    { register, unregister },
    id => {
      // 截图由 Rust 全局处理器直接执行，其余全局命令仍进入前端注册表。
      if (id !== "capture.start") void registry.execute(id, "native");
    },
  ) : null);
  const [initial] = useState(() => {
    try {
      const config = parseConfig(storage.load());
      validateBindings(commands, config, platform);
      return { config, error: null };
    } catch (error) {
      return { config: emptyConfig(), error: `读取快捷键设置失败，已使用默认值：${String(error)}` };
    }
  });
  const [config, setConfig] = useState(initial.config);
  validateBindings(commands, config, platform);
  const entries = resolveBindings(commands, config, context.workspaceId).map(entry => ({ ...entry, global: native ? entry.global : false }));
  const globalBindings = (value: ShortcutConfig) => resolveBindings(commands, value, null)
    .filter(entry => entry.global && entry.shortcut && !["capture.start", "window.show"].includes(entry.id))
    .map(entry => ({ id: entry.id, shortcut: entry.shortcut! }));
  const configureNativeCapture = (value: ShortcutConfig, workspaceId: string | null) => {
    const bindings = resolveBindings(commands, value, null);
    const shortcut = bindings.find(entry => entry.id === "capture.start")?.shortcut;
    const showShortcut = bindings.find(entry => entry.id === "window.show")?.shortcut;
    return invoke<void>("configure_native_capture", {
      shortcut: shortcut ? nativeShortcut(shortcut) : null,
      showShortcut: showShortcut ? nativeShortcut(showShortcut) : null,
      workspaceId,
    });
  };
  const [startupBindings] = useState(() => globalBindings(initial.config));
  useEffect(() => {
    if (!native) return;
    let active = true;
    void configureNativeCapture(initial.config, context.workspaceId)
      .then(() => native.replace(startupBindings))
      .catch(error => { if (active) onError(String(error)); });
    return () => {
      active = false;
      void native.replace([]).catch(error => onError(String(error)));
    };
  // Settings updates go through saveConfig; do not unregister after a successful save.
  }, [native, startupBindings]);
  useEffect(() => {
    if (!native) return;
    void configureNativeCapture(config, context.workspaceId)
      .catch(error => onError(String(error)));
  }, [native, config, context.workspaceId]);
  // Publish committed callbacks only. OCR/loading renders don't rebind the listener.
  useLayoutEffect(() => { registry.update(entries, context, onError); });
  useEffect(() => {
    if (initial.error) onError(initial.error);
    const onKeydown = (event: KeyboardEvent) => {
      const editable = event.composedPath().some((target) => target instanceof HTMLElement &&
        (target.isContentEditable || target.matches("input, textarea, select, [role='textbox']")));
      registry.handleKeydown(event, editable, platform);
    };
    window.addEventListener("keydown", onKeydown);
    return () => {
      window.removeEventListener("keydown", onKeydown);
      registry.cancelAll();
    };
    // onError is published separately; there is one subscription per app shell.
  }, [registry, platform, initial]);

  async function saveConfig(next: ShortcutConfig) {
    const validated = parseConfig(next);
    validateBindings(commands, validated, platform);
    const commit = () => {
      storage.save(validated);
      setConfig(validated);
    };
    if (native) {
      // 先同步 Rust 的匹配键，再替换系统注册；失败时恢复旧匹配键，
      // 保持配置存储、系统快捷键和 Rust 执行入口的一致性。
      await configureNativeCapture(validated, context.workspaceId);
      try {
        await native.replace(globalBindings(validated), commit);
      } catch (error) {
        await configureNativeCapture(config, context.workspaceId).catch(() => {});
        throw error;
      }
    } else commit();
  }
  return {
    entries, config, platform, saveConfig,
    execute: (id: string, source: CommandExecution["source"] = "button") => registry.execute(id, source),
    label: (id: string) => formatShortcut(entries.find((entry) => entry.id === id)?.shortcut ?? null, platform),
  };
}
