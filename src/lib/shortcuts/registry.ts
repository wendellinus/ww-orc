import { matchesShortcut, type KeyInput, type Platform } from "./keys.ts";
import { type CommandDefinition } from "./config.ts";

export type CommandContext = Readonly<{ workspaceId: string | null; scopes: readonly string[] }>;
export type CommandExecution = CommandContext & { signal: AbortSignal; source: "keyboard" | "button" | "native" };
export type Command = CommandDefinition & {
  allowInEditable?: boolean;
  enabled?: (context: CommandContext) => boolean;
  run: (context: CommandExecution) => void | Promise<void>;
};
export type ShortcutEvent = KeyInput & {
  repeat: boolean; isComposing: boolean; keyCode: number; defaultPrevented: boolean;
  getModifierState?: (key: string) => boolean;
  preventDefault(): void;
};

// UI, key events and a future native global-shortcut adapter share this dispatcher.
export class CommandRegistry {
  private commands: readonly Command[] = [];
  private context: CommandContext = { workspaceId: null, scopes: ["app"] };
  private pending = new Map<string, AbortController>();
  private onError: (message: string) => void = () => {};

  update(commands: readonly Command[], context: CommandContext, onError: (message: string) => void) {
    this.commands = commands;
    this.context = { workspaceId: context.workspaceId, scopes: [...context.scopes] };
    this.onError = onError;
  }
  private pendingKey(command: Command, context: CommandContext) {
    return JSON.stringify([command.scope === "app" ? null : context.workspaceId, command.id]);
  }
  canExecute(command: Command): boolean {
    return this.context.scopes.includes(command.scope) &&
      (command.scope === "app" || command.scope === "dialog" || this.context.workspaceId !== null) &&
      !this.pending.has(this.pendingKey(command, this.context)) && (command.enabled?.(this.context) ?? true);
  }
  async execute(id: string, source: CommandExecution["source"] = "button"): Promise<boolean> {
    const command = this.commands.find((candidate) => candidate.id === id);
    if (!command || !this.canExecute(command)) return false;
    const controller = new AbortController();
    const context: CommandExecution = { ...this.context, scopes: [...this.context.scopes], signal: controller.signal, source };
    const key = this.pendingKey(command, context);
    const reportError = this.onError;
    this.pending.set(key, controller);
    try {
      // Invoke before the first await to preserve browser clipboard user activation.
      await command.run(context);
      return true;
    } catch (error) {
      if (!controller.signal.aborted) reportError(`${command.title}失败：${String(error)}`);
      return false;
    } finally {
      if (this.pending.get(key) === controller) this.pending.delete(key);
    }
  }
  handleKeydown(event: ShortcutEvent, editable: boolean, platform: Platform): boolean {
    if (event.defaultPrevented || event.repeat || event.isComposing || event.keyCode === 229 || event.getModifierState?.("AltGraph")) return false;
    // An inner scope owns a matching key even while its command is disabled.
    for (const scope of [...this.context.scopes].reverse()) {
      const command = this.commands.find((candidate) => candidate.scope === scope && candidate.shortcut && matchesShortcut(candidate.shortcut, event, platform));
      if (!command) continue;
      if ((editable && !command.allowInEditable) || !this.canExecute(command)) return false;
      event.preventDefault();
      void this.execute(command.id, "keyboard");
      return true;
    }
    return false;
  }
  cancelAll() {
    for (const controller of this.pending.values()) controller.abort();
    this.pending.clear();
  }
}
