import { useEffect, useRef, useState } from "react";
import { Menu } from "@base-ui/react/menu";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { Check, Keyboard, LoaderCircle, Power, Settings } from "lucide-react";

type Props = {
  desktop: boolean;
  onOpenShortcuts: () => void;
  onError: (message: string) => void;
};

const itemClassName = "flex cursor-default items-center gap-2 rounded-md px-2.5 py-2 text-sm outline-none data-disabled:opacity-50 data-highlighted:bg-muted";

export function SettingsMenu({ desktop, onOpenShortcuts, onError }: Props) {
  const [enabled, setEnabled] = useState(false);
  const [ready, setReady] = useState(false);
  const [pending, setPending] = useState(false);
  const inFlight = useRef(false);

  useEffect(() => {
    if (!desktop) return;
    let disposed = false;
    isEnabled().then((value) => {
      if (!disposed) {
        setEnabled(value);
        setReady(true);
      }
    }).catch((cause) => {
      if (!disposed) onError(`读取开机启动设置失败：${String(cause)}`);
    });
    return () => { disposed = true; };
  }, [desktop, onError]);

  async function refresh() {
    if (!desktop || inFlight.current) return;
    inFlight.current = true;
    setPending(true);
    try {
      setEnabled(await isEnabled());
      setReady(true);
    } catch (cause) {
      setReady(false);
      onError(`读取开机启动设置失败：${String(cause)}`);
    } finally {
      inFlight.current = false;
      setPending(false);
    }
  }

  async function toggle(value: boolean) {
    if (!desktop || !ready || inFlight.current) return;
    inFlight.current = true;
    setPending(true);
    try {
      await (value ? enable() : disable());
      const actual = await isEnabled();
      setEnabled(actual);
      if (actual !== value) onError("开机启动设置未生效，请重试。");
    } catch (cause) {
      setReady(false);
      onError(`设置开机启动失败：${String(cause)}`);
    } finally {
      inFlight.current = false;
      setPending(false);
    }
  }

  return (
    <Menu.Root onOpenChange={(open) => { if (open) void refresh(); }}>
      <Menu.Trigger
        aria-label="设置"
        title="设置"
        className="grid size-8 shrink-0 cursor-pointer place-items-center rounded-md text-muted-foreground outline-none hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring data-popup-open:bg-muted data-popup-open:text-foreground"
      >
        <Settings className="size-4" aria-hidden="true" />
      </Menu.Trigger>
      <Menu.Portal>
        <Menu.Positioner align="end" sideOffset={6} className="z-50 outline-none">
          <Menu.Popup className="min-w-52 origin-(--transform-origin) rounded-lg border bg-popover p-1 text-popover-foreground shadow-lg outline-none">
            <Menu.CheckboxItem
              className={itemClassName}
              checked={enabled}
              disabled={!desktop || !ready || pending}
              onCheckedChange={(value) => void toggle(value)}
            >
              <Power className="size-4" aria-hidden="true" />
              <span className="flex-1">开机启动</span>
              {pending ? (
                <LoaderCircle className="size-4 animate-spin" aria-hidden="true" />
              ) : (
                <Menu.CheckboxItemIndicator>
                  <Check className="size-4" aria-hidden="true" />
                </Menu.CheckboxItemIndicator>
              )}
            </Menu.CheckboxItem>
            <Menu.Item className={itemClassName} onClick={onOpenShortcuts}>
              <Keyboard className="size-4" aria-hidden="true" />
              <span>快捷键设置</span>
            </Menu.Item>
          </Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.Root>
  );
}
