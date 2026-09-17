import { normalizeShortcut, shortcutSignature, type Platform } from "./keys.ts";

export type BindingOverrides = Record<string, string | null>;
export type ShortcutConfig = { version: 1; user: BindingOverrides; workspaces: Record<string, BindingOverrides> };
export interface ShortcutStorage {
  load(): ShortcutConfig;
  save(config: ShortcutConfig): void;
}
export const STORAGE_KEY = "ww-ocr.shortcuts.v1";
export const emptyConfig = (): ShortcutConfig => ({ version: 1, user: {}, workspaces: {} });

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
function parseOverrides(value: unknown): BindingOverrides {
  if (!isRecord(value)) throw new Error("快捷键配置格式错误");
  return Object.fromEntries(Object.entries(value).map(([id, shortcut]) => {
    if (shortcut !== null && typeof shortcut !== "string") throw new Error("快捷键配置格式错误");
    return [id, shortcut === null ? null : normalizeShortcut(shortcut)];
  }));
}
export function parseConfig(value: unknown): ShortcutConfig {
  if (!isRecord(value) || value.version !== 1 || !isRecord(value.workspaces)) throw new Error("不支持的快捷键配置版本或格式");
  return {
    version: 1,
    user: parseOverrides(value.user),
    workspaces: Object.fromEntries(Object.entries(value.workspaces).map(([id, overrides]) => [id, parseOverrides(overrides)])),
  };
}
export const localShortcutStorage: ShortcutStorage = {
  load() {
    const value = localStorage.getItem(STORAGE_KEY);
    return value === null ? emptyConfig() : parseConfig(JSON.parse(value));
  },
  save(config) { localStorage.setItem(STORAGE_KEY, JSON.stringify(config)); },
};

export type CommandDefinition = { id: string; title: string; scope: string; shortcut?: string | null };
export function resolveBindings<T extends CommandDefinition>(commands: readonly T[], config: ShortcutConfig, workspaceId: string | null) {
  const overrides = { ...config.user, ...(workspaceId === null ? {} : config.workspaces[workspaceId]) };
  return commands.map((command) => ({ ...command, shortcut: Object.prototype.hasOwnProperty.call(overrides, command.id)
    ? overrides[command.id] : command.shortcut ?? null }));
}
export function validateBindings(commands: readonly CommandDefinition[], config: ShortcutConfig, platform: Platform): void {
  const ids = new Set<string>();
  for (const command of commands) {
    if (ids.has(command.id)) throw new Error(`命令 ID 重复：${command.id}`);
    ids.add(command.id);
  }
  for (const workspaceId of [null, ...Object.keys(config.workspaces)]) {
    const occupied = new Map<string, string>();
    for (const command of resolveBindings(commands, config, workspaceId)) {
      if (!command.shortcut) continue;
      const key = `${command.scope}:${shortcutSignature(command.shortcut, platform)}`;
      const previous = occupied.get(key);
      if (previous) throw new Error(`“${command.title}”与“${previous}”的快捷键冲突${workspaceId === null ? "" : `（工作区 ${workspaceId}）`}。`);
      occupied.set(key, command.title);
    }
  }
}
