export type Platform = "mac" | "other";
export type KeyInput = Pick<KeyboardEvent, "key" | "code" | "ctrlKey" | "metaKey" | "altKey" | "shiftKey">;
const modifiers = ["Mod", "Ctrl", "Meta", "Alt", "Shift"];
const namedKeys = ["Escape", "Enter", "Tab", "Space", "Backspace", "Delete", "Home", "End", "PageUp", "PageDown", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Slash", "Comma", "Period", "Minus", "Equal"];

export function normalizeShortcut(value: string): string {
  const parts = value.trim().split("+").map((part) => part.trim());
  const rawKey = parts.pop() || "";
  const key = /^[a-z0-9]$/i.test(rawKey) ? rawKey.toUpperCase()
    : /^f([1-9]|1[0-2])$/i.test(rawKey) ? rawKey.toUpperCase()
    : namedKeys.find((name) => name.toLowerCase() === rawKey.toLowerCase());
  const mods = parts.map((part) => modifiers.find((mod) => mod.toLowerCase() === part.toLowerCase()));
  if (!key || mods.some((mod) => !mod) || new Set(mods).size !== mods.length ||
      (mods.includes("Mod") && (mods.includes("Ctrl") || mods.includes("Meta")))) {
    throw new Error(`无效快捷键：${value}。示例：Mod+Shift+Y、Mod+Slash。`);
  }
  if (!mods.some((mod) => mod !== "Shift") && !/^F\d+$/.test(key) && key !== "Escape") {
    throw new Error("请使用 Ctrl、Meta、Alt 或 Mod 组合键，避免影响文字输入和键盘导航。");
  }
  const result = [...modifiers.filter((mod) => mods.includes(mod)), key].join("+");
  if (key === "F12" || ((mods.includes("Ctrl") || mods.includes("Mod")) && mods.includes("Shift") && ["I", "J", "C"].includes(key))) {
    throw new Error("该组合键由应用的开发者工具保护逻辑保留，请选择其他按键。");
  }
  return result;
}

export function shortcutSignature(value: string, platform: Platform): string {
  return normalizeShortcut(value).split("+").map((part) => part === "Mod" ? (platform === "mac" ? "Meta" : "Ctrl") : part)
    .sort().join("+");
}

export function matchesShortcut(value: string, event: KeyInput, platform: Platform): boolean {
  const parts = normalizeShortcut(value).split("+");
  const key = parts.pop()!;
  const mods = new Set(parts.map((part) => part === "Mod" ? (platform === "mac" ? "Meta" : "Ctrl") : part));
  if (event.ctrlKey !== mods.has("Ctrl") || event.metaKey !== mods.has("Meta") ||
      event.altKey !== mods.has("Alt") || event.shiftKey !== mods.has("Shift")) return false;
  // Physical punctuation codes avoid Shift changing '/' to '?'. Letters follow the user's layout.
  if (["Slash", "Comma", "Period", "Minus", "Equal"].includes(key)) return event.code === key;
  return (event.key === " " ? "SPACE" : event.key.toUpperCase()) === key.toUpperCase();
}

export function formatShortcut(value: string | null, platform: Platform): string {
  if (!value) return "未设置";
  return value.split("+").map((part) => {
    if (part === "Mod") return platform === "mac" ? "⌘" : "Ctrl";
    return ({ Meta: "⌘", Slash: "/", Comma: ",", Period: ".", Minus: "-", Equal: "=", Escape: "Esc" } as Record<string, string>)[part] || part;
  }).join("+");
}
