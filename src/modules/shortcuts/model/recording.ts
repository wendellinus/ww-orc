import { normalizeShortcut, type KeyInput, type Platform } from "./keys.ts";

type RecordingKeyInput = KeyInput & {
  repeat?: boolean;
  isComposing?: boolean;
  keyCode?: number;
  getModifierState?: (key: string) => boolean;
};

export type RecordedShortcut =
  | { kind: "ignore" }
  | { kind: "modifiers"; shortcut: string }
  | { kind: "binding"; shortcut: string };

export function readRecordedShortcut(event: RecordingKeyInput, platform: Platform): RecordedShortcut {
  if (event.repeat || event.isComposing || event.keyCode === 229 ||
      event.getModifierState?.("AltGraph") || ["AltGraph", "Dead", "Process", "Unidentified"].includes(event.key)) {
    return { kind: "ignore" };
  }
  const modifiers: string[] = [];
  // Both physical primary modifiers must stay explicit: Mod+Ctrl/Meta is ambiguous.
  if (event.ctrlKey) modifiers.push(platform === "other" && !event.metaKey ? "Mod" : "Ctrl");
  if (event.metaKey) modifiers.push(platform === "mac" && !event.ctrlKey ? "Mod" : "Meta");
  if (event.altKey) modifiers.push("Alt");
  if (event.shiftKey) modifiers.push("Shift");
  if (["Control", "Meta", "OS", "Alt", "Shift"].includes(event.key)) {
    return { kind: "modifiers", shortcut: modifiers.join("+") };
  }
  const key = ["Slash", "Comma", "Period", "Minus", "Equal"].includes(event.code)
    ? event.code : event.key === " " ? "Space" : event.key;
  return { kind: "binding", shortcut: normalizeShortcut([...modifiers, key].join("+")) };
}