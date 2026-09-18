import {
  Copy,
  Minus,
  Pin,
  Search,
  Square,
  X,
} from "lucide-react";

import { SettingsMenu } from "./settings-menu";

import { cn } from "@/shared/lib/utils";
import { Button } from "@/shared/ui/button";

type Props = {
  desktop: boolean;
  maximized: boolean;
  alwaysOnTop: boolean;
  searchQuery: string;
  onSearchChange: (value: string) => void;
  onOpenShortcuts: () => void;
  onError: (message: string) => void;
  onToggleAlwaysOnTop: () => void;
  onWindowAction: (action: "minimize" | "toggleMaximize" | "close") => void;
};

export function WindowTitleBar({
  desktop,
  maximized,
  alwaysOnTop,
  searchQuery,
  onSearchChange,
  onOpenShortcuts,
  onError,
  onToggleAlwaysOnTop,
  onWindowAction,
}: Props) {
  return (
    <header data-tauri-drag-region className="flex h-14 shrink-0 items-center gap-3 border-b border-zinc-300/70 bg-zinc-100/95 px-3 dark:border-zinc-700 dark:bg-zinc-900/95">
      <div
        className="flex min-w-44 flex-1 items-center self-stretch select-none"
        data-tauri-drag-region
      >
        <div className="pointer-events-none flex items-center gap-2.5">
          <span className="grid size-7 place-items-center rounded-md bg-primary font-heading text-xs font-bold text-primary-foreground shadow-sm">
            W
          </span>
          <div className="leading-none">
            <p className="font-heading text-sm font-semibold tracking-tight">ww-ocr</p>
            <p className="mt-1 text-[10px] font-medium tracking-[0.16em] text-muted-foreground uppercase">
              workspace
            </p>
          </div>
        </div>
      </div>

      <label className="relative hidden w-72 shrink sm:block" htmlFor="app-search">
        <Search
          className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
          aria-hidden="true"
        />
        <span className="sr-only">搜索工作空间和识别内容</span>
        <input
          id="app-search"
          value={searchQuery}
          onChange={(event) => onSearchChange(event.target.value)}
          placeholder="搜索空间名称、ID 或关键字"
          className="h-9 w-full rounded-lg border border-zinc-300 bg-white/85 pr-3 pl-9 text-sm shadow-xs outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/40 dark:border-zinc-700 dark:bg-zinc-950/70"
        />
      </label>

      <div data-tauri-drag-region className="flex flex-1 items-center self-stretch justify-end gap-1" role="group" aria-label="窗口操作">
        <SettingsMenu desktop={desktop} onOpenShortcuts={onOpenShortcuts} onError={onError} />
        <Button
          size="icon-sm"
          variant="ghost"
          className="border-0 text-muted-foreground hover:bg-primary/10 hover:text-primary aria-pressed:bg-primary/15 aria-pressed:text-primary"
          aria-pressed={alwaysOnTop}
          disabled={!desktop}
          onClick={onToggleAlwaysOnTop}
          aria-label="窗口置顶"
          title={alwaysOnTop ? "取消置顶" : "窗口置顶"}
        >
          <Pin
            className={cn(alwaysOnTop && "fill-current")}
            aria-hidden="true"
          />
        </Button>
        <Button
          size="icon-sm"
          variant="ghost"
          className="border-0 text-muted-foreground hover:bg-zinc-200 hover:text-foreground dark:hover:bg-zinc-800"
          disabled={!desktop}
          onClick={() => onWindowAction("minimize")}
          aria-label="最小化"
          title="最小化"
        >
          <Minus aria-hidden="true" />
        </Button>
        <Button
          size="icon-sm"
          variant="ghost"
          className="border-0 text-muted-foreground hover:bg-zinc-200 hover:text-foreground dark:hover:bg-zinc-800"
          disabled={!desktop}
          onClick={() => onWindowAction("toggleMaximize")}
          aria-label={maximized ? "还原窗口" : "最大化"}
          title={maximized ? "还原窗口" : "最大化"}
        >
          {maximized ? <Copy aria-hidden="true" /> : <Square aria-hidden="true" />}
        </Button>
        <Button
          size="icon-sm"
          variant="ghost"
          className="window-close-button border-0 text-muted-foreground"
          disabled={!desktop}
          onClick={() => onWindowAction("close")}
          aria-label="关闭窗口"
          title="关闭窗口"
        >
          <X aria-hidden="true" />
        </Button>
      </div>
    </header>
  );
}
