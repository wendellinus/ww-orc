import { Menu } from "@base-ui/react/menu";
import { Ellipsis, FolderOpen, Pencil, Plus, Trash2 } from "lucide-react";

import { cn } from "@/shared/lib/utils";

import type { Workspace } from "../workspace.types";

type Props = {
  workspaces: readonly Workspace[];
  activeWorkspaceId: string | null;
  query: string;
  disabled: boolean;
  onSelect: (workspaceId: string) => void;
  onCreate: () => void;
  onRename: (workspace: Workspace) => void;
  onDelete: (workspace: Workspace) => void;
};

export function WorkspaceSidebar({
  workspaces,
  activeWorkspaceId,
  query,
  disabled,
  onSelect,
  onCreate,
  onRename,
  onDelete,
}: Props) {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const visibleWorkspaces = normalizedQuery
    ? workspaces.filter(
        (workspace) =>
          workspace.name.toLocaleLowerCase().includes(normalizedQuery) ||
          workspace.id.toLocaleLowerCase().includes(normalizedQuery),
      )
    : workspaces;

  return (
    <aside className="flex w-56 shrink-0 flex-col border-r bg-zinc-50/80 dark:bg-zinc-950/40">
      <div className="flex h-12 shrink-0 items-center justify-between pr-2 pl-3">
        <p className="text-xs font-semibold tracking-[0.12em] text-muted-foreground uppercase">
          工作空间
        </p>
        <span className="rounded-full bg-zinc-200 px-2 py-0.5 text-right text-[10px] font-medium text-zinc-600 tabular-nums dark:bg-zinc-800 dark:text-zinc-300">
          {workspaces.length}
        </span>
      </div>

      <nav className="min-h-0 flex-1 space-y-1 overflow-y-auto px-2 pb-2" aria-label="工作空间">
        {visibleWorkspaces.map((workspace) => {
          const active = workspace.id === activeWorkspaceId;
          return (
            <div
              key={workspace.id}
              className={cn(
                "group relative flex items-center rounded-lg pr-1 transition-colors",
                active
                  ? "bg-primary/16 text-foreground shadow-xs"
                  : "text-muted-foreground hover:bg-zinc-200/70 hover:text-foreground dark:hover:bg-zinc-800/70",
              )}
            >
              <button
                type="button"
                disabled={disabled}
                onClick={() => onSelect(workspace.id)}
                className="flex min-w-0 flex-1 items-center gap-2.5 rounded-lg py-2.5 pr-1 pl-2.5 text-left text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50"
              >
                <FolderOpen
                  className={cn(
                    "size-4 shrink-0",
                    active ? "text-primary" : "text-muted-foreground",
                  )}
                  aria-hidden="true"
                />
                <span className="min-w-0 flex-1 truncate font-medium">{workspace.name}</span>
                <span
                  className="min-w-7 shrink-0 text-right text-xs text-muted-foreground tabular-nums transition-opacity group-hover:opacity-0 group-focus-within:opacity-0 group-has-data-popup-open:opacity-0"
                  aria-label={`${workspace.imageCount} 张图片`}
                  title={`${workspace.imageCount} 张图片`}
                >
                  {workspace.imageCount}
                </span>
              </button>

              <Menu.Root>
                <Menu.Trigger
                  disabled={disabled}
                  className="pointer-events-none absolute top-1/2 right-1 grid size-7 -translate-y-1/2 cursor-pointer place-items-center rounded-md text-muted-foreground opacity-0 outline-none transition group-hover:pointer-events-auto group-hover:opacity-100 group-focus-within:pointer-events-auto group-focus-within:opacity-100 hover:bg-zinc-300/80 hover:text-foreground active:bg-zinc-400/50 focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50 data-popup-open:pointer-events-auto data-popup-open:bg-zinc-300/80 data-popup-open:text-foreground data-popup-open:opacity-100 dark:hover:bg-zinc-700 dark:active:bg-zinc-600 dark:data-popup-open:bg-zinc-700"
                  aria-label={`${workspace.name}的更多操作`}
                >
                  <Ellipsis className="size-4" aria-hidden="true" />
                </Menu.Trigger>
                <Menu.Portal>
                  <Menu.Positioner sideOffset={6} align="end" className="z-50 outline-none">
                    <Menu.Popup className="min-w-36 origin-(--transform-origin) rounded-lg border bg-popover p-1 text-popover-foreground shadow-lg outline-none">
                      <Menu.Item
                        onClick={() => onRename(workspace)}
                        className="flex cursor-default items-center gap-2 rounded-md px-2.5 py-2 text-sm outline-none data-highlighted:bg-muted"
                      >
                        <Pencil className="size-4" aria-hidden="true" />
                        重命名
                      </Menu.Item>
                      <Menu.Item
                        disabled={workspace.id === "default"}
                        onClick={() => onDelete(workspace)}
                        className="flex cursor-default items-center gap-2 rounded-md px-2.5 py-2 text-sm text-destructive outline-none data-disabled:opacity-40 data-highlighted:bg-destructive/10"
                      >
                        <Trash2 className="size-4" aria-hidden="true" />
                        删除
                      </Menu.Item>
                    </Menu.Popup>
                  </Menu.Positioner>
                </Menu.Portal>
              </Menu.Root>
            </div>
          );
        })}

        {visibleWorkspaces.length === 0 ? (
          <p className="px-3 py-8 text-center text-xs leading-5 text-muted-foreground">
            没有匹配的工作空间
          </p>
        ) : null}
        <div className="pt-2">
          <button
            type="button"
            disabled={disabled}
            onClick={onCreate}
            className="flex h-10 w-full items-center justify-center gap-2 rounded-lg border border-dashed border-zinc-300 bg-white/60 text-sm font-medium text-muted-foreground outline-none transition-colors hover:border-primary/60 hover:bg-primary/8 hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50 dark:border-zinc-700 dark:bg-zinc-900/50"
          >
            <Plus className="size-4" aria-hidden="true" />
            新建空间
          </button>
        </div>
      </nav>
    </aside>
  );
}
