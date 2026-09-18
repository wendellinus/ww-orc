import { normalizeShortcut } from "./keys.ts";

export type GlobalBinding = { id: string; shortcut: string };
export type GlobalShortcutDriver = {
  register(shortcut: string, handler: (event: { state: string }) => void): Promise<void>;
  unregister(shortcut: string): Promise<void>;
};
export function nativeShortcut(value: string): string {
  const names: Record<string, string> = { Mod: "CommandOrControl", Ctrl: "Control", Meta: "Super" };
  return normalizeShortcut(value).split("+").map(part => names[part] ?? part).join("+");
}

// Serialize startup, saves and cleanup; acquire replacement keys before releasing old ones.
export class GlobalShortcuts {
  private bindings = new Map<string, string>();
  private queue: Promise<unknown> = Promise.resolve();
  private driver: GlobalShortcutDriver;
  private execute: (id: string) => void;
  constructor(driver: GlobalShortcutDriver, execute: (id: string) => void) {
    this.driver = driver;
    this.execute = execute;
  }
  replace(next: readonly GlobalBinding[], commit: () => void = () => {}): Promise<void> {
    const operation = this.queue.then(async () => {
      const desired = new Map<string, string>();
      for (const binding of next) {
        const key = nativeShortcut(binding.shortcut);
        if (desired.has(key)) throw new Error(`全局快捷键冲突：${binding.shortcut}`);
        desired.set(key, binding.id);
      }
      const added: string[] = [];
      const removed: string[] = [];
      const register = (key: string) => this.driver.register(key, event => {
        const id = this.bindings.get(key);
        if (event.state === "Pressed" && id) this.execute(id);
      });
      try {
        for (const key of desired.keys()) {
          if (this.bindings.has(key)) continue;
          await register(key);
          added.push(key);
        }
        for (const key of this.bindings.keys()) {
          if (desired.has(key)) continue;
          await this.driver.unregister(key);
          removed.push(key);
        }
        commit();
        this.bindings = desired;
      } catch (error) {
        const rollback = await Promise.allSettled([
          ...added.map(key => this.driver.unregister(key)), ...removed.map(register),
        ]);
        const failed = rollback.some(result => result.status === "rejected");
        throw new Error(`注册全局快捷键失败（可能被其他应用占用）：${String(error)}${failed ? "；恢复原绑定失败，请重启应用" : ""}`);
      }
    });
    this.queue = operation.catch(() => {});
    return operation;
  }
}
