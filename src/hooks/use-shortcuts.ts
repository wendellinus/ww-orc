import { useEffect, useLayoutEffect, useState } from "react";
import { CommandRegistry, type Command, type CommandContext, type CommandExecution } from "@/lib/shortcuts/registry";
import { emptyConfig, localShortcutStorage, parseConfig, resolveBindings, validateBindings, type ShortcutConfig, type ShortcutStorage } from "@/lib/shortcuts/config";
import { formatShortcut, type Platform } from "@/lib/shortcuts/keys";

export function useShortcuts(commands: readonly Command[], context: CommandContext, onError: (message: string) => void, storage: ShortcutStorage = localShortcutStorage) {
  const [platform] = useState<Platform>(() => /Mac|iPhone|iPad/.test(navigator.platform) ? "mac" : "other");
  const [registry] = useState(() => new CommandRegistry());
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
  const entries = resolveBindings(commands, config, context.workspaceId);
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

  function saveConfig(next: ShortcutConfig) {
    const validated = parseConfig(next);
    validateBindings(commands, validated, platform);
    storage.save(validated); // Failure leaves both the current bindings and UI state intact.
    setConfig(validated);
  }
  return {
    entries, config, platform, saveConfig,
    execute: (id: string, source: CommandExecution["source"] = "button") => registry.execute(id, source),
    label: (id: string) => formatShortcut(entries.find((entry) => entry.id === id)?.shortcut ?? null, platform),
  };
}
