import { isTauri } from "@tauri-apps/api/core";
import { register, unregister } from "@tauri-apps/plugin-global-shortcut";
import { GlobalShortcuts } from "../model/global-shortcuts";
import { useEffect, useLayoutEffect, useState } from "react";
import { CommandRegistry, type Command, type CommandContext, type CommandExecution } from "../model/registry";
import { emptyConfig, localShortcutStorage, parseConfig, resolveBindings, validateBindings, type ShortcutConfig, type ShortcutStorage } from "../model/config";
import { formatShortcut, type Platform } from "../model/keys";

export function useShortcuts(commands: readonly Command[], context: CommandContext, onError: (message: string) => void, storage: ShortcutStorage = localShortcutStorage) {
  const [platform] = useState<Platform>(() => /Mac|iPhone|iPad/.test(navigator.platform) ? "mac" : "other");
  const [registry] = useState(() => new CommandRegistry());
  const [native] = useState(() => isTauri() ? new GlobalShortcuts({ register, unregister }, id => { void registry.execute(id, "native"); }) : null);
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
    .filter(entry => entry.global && entry.shortcut)
    .map(entry => ({ id: entry.id, shortcut: entry.shortcut! }));
  const [startupBindings] = useState(() => globalBindings(initial.config));
  useEffect(() => {
    if (!native) return;
    let active = true;
    void native.replace(startupBindings).catch(error => { if (active) onError(String(error)); });
    return () => {
      active = false;
      void native.replace([]).catch(error => onError(String(error)));
    };
  // Settings updates go through saveConfig; do not unregister after a successful save.
  }, [native, startupBindings]);
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
    if (native) await native.replace(globalBindings(validated), commit);
    else commit();
  }
  return {
    entries, config, platform, saveConfig,
    execute: (id: string, source: CommandExecution["source"] = "button") => registry.execute(id, source),
    label: (id: string) => formatShortcut(entries.find((entry) => entry.id === id)?.shortcut ?? null, platform),
  };
}
