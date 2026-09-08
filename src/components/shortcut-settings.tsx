import { useEffect, useRef, useState } from "react";
import { X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { normalizeShortcut, formatShortcut, type Platform } from "@/lib/shortcuts/keys";
import { type Command } from "@/lib/shortcuts/registry";
import { type ShortcutConfig } from "@/lib/shortcuts/config";

type Props = {
  commands: readonly Command[];
  config: ShortcutConfig;
  platform: Platform;
  onSave: (config: ShortcutConfig) => void;
  onClose: () => void;
};

export function ShortcutSettings({ commands, config, platform, onSave, onClose }: Props) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const [draft, setDraft] = useState<Record<string, string>>(() => Object.fromEntries(commands.map((command) => [command.id,
    Object.prototype.hasOwnProperty.call(config.user, command.id) ? config.user[command.id] ?? "" : command.shortcut ?? "",
  ])));
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    const dialog = dialogRef.current!;
    const previousFocus = document.activeElement;
    dialog.showModal();
    return () => {
      dialog.close();
      if (previousFocus instanceof HTMLElement && previousFocus.isConnected) previousFocus.focus();
    };
  }, []);

  function save() {
    try {
      const user = { ...config.user };
      for (const command of commands) {
        const binding = draft[command.id].trim() ? normalizeShortcut(draft[command.id]) : null;
        if (binding === (command.shortcut ?? null)) delete user[command.id];
        else user[command.id] = binding;
      }
      onSave({ ...config, user });
      onClose();
    } catch (cause) {
      setError(String(cause instanceof Error ? cause.message : cause));
    }
  }

  return (
    <dialog
      ref={dialogRef}
      aria-labelledby="shortcut-title"
      aria-describedby="shortcut-description"
      className="m-auto max-h-[85dvh] w-[calc(100%-2rem)] max-w-xl overflow-auto rounded-xl border bg-background p-0 text-foreground shadow-xl backdrop:bg-black/40"
      onCancel={(event) => { event.preventDefault(); onClose(); }}
    >
      <form onSubmit={(event) => { event.preventDefault(); save(); }}>
        <header className="flex items-center justify-between border-b p-4">
          <h2 id="shortcut-title" className="font-semibold">快捷键设置</h2>
          <Button type="button" size="icon-sm" variant="ghost" onClick={onClose} aria-label="关闭快捷键设置"><X aria-hidden="true" /></Button>
        </header>
        <div className="space-y-4 p-4">
          <p id="shortcut-description" className="text-sm leading-6 text-muted-foreground">
            输入组合键，如 Mod+Shift+Y。Mod 在 Windows 上表示 Ctrl，在 macOS 上表示 ⌘。留空可禁用；快捷键仅在应用内生效。
          </p>
          <div className="space-y-3">
            {commands.map((command) => (
              <div key={command.id} className="flex flex-wrap items-center justify-between gap-2">
                <label htmlFor={`shortcut-${command.id}`} className="text-sm">
                  {command.title}
                  <span className="mt-1 block text-xs text-muted-foreground">默认：{formatShortcut(command.shortcut ?? null, platform)}</span>
                </label>
                <input
                  id={`shortcut-${command.id}`}
                  value={draft[command.id]}
                  onChange={(event) => { setDraft((previous) => ({ ...previous, [command.id]: event.target.value })); setError(null); }}
                  placeholder="未设置"
                  autoComplete="off"
                  spellCheck={false}
                  className="h-9 w-48 rounded-md border bg-transparent px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
                />
              </div>
            ))}
          </div>
          {error ? <p role="alert" className="text-sm text-destructive">{error}</p> : null}
        </div>
        <footer className="flex flex-wrap items-center justify-between gap-3 border-t p-4">
          <Button type="button" variant="ghost" onClick={() => {
            setDraft(Object.fromEntries(commands.map((command) => [command.id, command.shortcut ?? ""])));
            setError(null);
          }}>恢复默认</Button>
          <div className="flex gap-2">
            <Button type="button" variant="outline" onClick={onClose}>取消</Button>
            <Button type="submit">保存</Button>
          </div>
        </footer>
      </form>
    </dialog>
  );
}
