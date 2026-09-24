import { useCallback, useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { X } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { startCapture } from "@/features/capture";
import { createPin, pasteClipboardPin } from "@/features/pins";
import { DesktopTools } from "./ui/desktop-tools";

import {
  deleteDocument,
  listDocuments,
  OcrFeed,
  recognizeImage,
  recognizeImageBytes,
  recognizeExistingImage,
  type OcrResult,
} from "@/features/ocr";
import {
  ShortcutSettings,
  useShortcuts,
  type Command,
} from "@/features/shortcuts";
import {
  createWorkspace,
  deleteWorkspace,
  listWorkspaces,
  renameWorkspace,
  WorkspaceSidebar,
  type Workspace,
} from "@/features/workspace";
import { Button } from "@/shared/ui/button";

import "./App.css";
import { WindowTitleBar } from "./ui/window-title-bar";
import {
  LibraryActionDialog,
  type LibraryAction,
} from "./ui/library-action-dialog";

const IMG_RE = /\.(png|jpe?g)$/i;
const MAX_CLIPBOARD_IMAGE_BYTES = 20 * 1024 * 1024;
const ACTIVE_WORKSPACE_KEY = "ww-ocr.active-workspace.v1";
const ALWAYS_ON_TOP_KEY = "ww-ocr.always-on-top.v1";

function isEditablePasteTarget(event: ClipboardEvent) {
  return event
    .composedPath()
    .some(
      (target) =>
        target instanceof HTMLElement &&
        (target.isContentEditable ||
          target.matches("input, textarea, select, [role='textbox']")),
    );
}

function App() {
  const desktop = isTauri();
  const busy = useRef(false);
  const dropZoneRef = useRef<HTMLDivElement>(null);
  const resultRef = useRef<HTMLParagraphElement>(null);
  const draggedImagePath = useRef<string | null>(null);

  const [desktopManagerOpen, setDesktopManagerOpen] = useState(false);
  const captureBusy = useRef(false);
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const [libraryAction, setLibraryAction] = useState<LibraryAction | null>(
    null,
  );
  const [alwaysOnTop, setAlwaysOnTop] = useState(false);
  const [maximized, setMaximized] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [loading, setLoading] = useState(false);
  const [libraryLoading, setLibraryLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [copiedImageId, setCopiedImageId] = useState<string | null>(null);
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [activeWorkspaceId, setActiveWorkspaceId] = useState<string | null>(
    null,
  );
  const [documents, setDocuments] = useState<OcrResult[]>([]);
  const [loadedWorkspaceId, setLoadedWorkspaceId] = useState<string | null>(
    null,
  );
  const [selectedImageId, setSelectedImageId] = useState<string | null>(null);

  const activeWorkspace =
    workspaces.find((workspace) => workspace.id === activeWorkspaceId) ?? null;
  const selectedDocument =
    loadedWorkspaceId === activeWorkspaceId
      ? (documents.find((document) => document.imageId === selectedImageId) ??
        documents[documents.length - 1] ??
        null)
      : null;

  function isInsideDropZone(position: { x: number; y: number }) {
    const rect = dropZoneRef.current?.getBoundingClientRect();
    if (!rect) return false;

    const scaleFactor = window.devicePixelRatio || 1;
    const x = position.x / scaleFactor;
    const y = position.y / scaleFactor;
    return (
      x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom
    );
  }

  const runOcr = useCallback(
    async (path: string) => {
      if (busy.current) return;
      if (!activeWorkspaceId) {
        setError("请先选择工作空间。");
        return;
      }

      if (libraryLoading || loadedWorkspaceId !== activeWorkspaceId) {
        setError("正在加载工作空间，请稍后再识别。");
        return;
      }

      busy.current = true;
      setLoading(true);
      setError(null);
      setCopiedImageId(null);
      try {
        const result = await recognizeImage(activeWorkspaceId, path);
        setDocuments((current) => [
          ...current.filter((item) => item.imageId !== result.imageId),
          result,
        ]);
        setSelectedImageId(result.imageId);
        setError(result.errorMessage);
      } catch (cause) {
        setError(String(cause));
      } finally {
        try {
          setWorkspaces(await listWorkspaces());
        } catch (cause) {
          setError(`刷新空间图片数量失败：${String(cause)}`);
        }
        busy.current = false;
        setLoading(false);
      }
    },
    [activeWorkspaceId, libraryLoading, loadedWorkspaceId],
  );

  const runPastedOcr = useCallback(
    async (file: File) => {
      if (busy.current) return;
      if (!activeWorkspaceId) {
        setError("请先选择工作空间。");
        return;
      }
      if (file.size > MAX_CLIPBOARD_IMAGE_BYTES) {
        setError("剪贴板图片不能超过 20 MB。");
        return;
      }

      if (libraryLoading || loadedWorkspaceId !== activeWorkspaceId) {
        setError("正在加载工作空间，请稍后再识别。");
        return;
      }

      busy.current = true;
      setLoading(true);
      setError(null);
      setCopiedImageId(null);
      try {
        const bytes = new Uint8Array(await file.arrayBuffer());
        const result = await recognizeImageBytes(activeWorkspaceId, bytes);
        setDocuments((current) => [
          ...current.filter((item) => item.imageId !== result.imageId),
          result,
        ]);
        setSelectedImageId(result.imageId);
        setError(result.errorMessage);
      } catch (cause) {
        setError(String(cause));
      } finally {
        try {
          setWorkspaces(await listWorkspaces());
        } catch (cause) {
          setError(`刷新空间图片数量失败：${String(cause)}`);
        }
        busy.current = false;
        setLoading(false);
      }
    },
    [activeWorkspaceId, libraryLoading, loadedWorkspaceId],
  );

  useEffect(() => {
    if (!desktop) return;

    let disposed = false;
    setLibraryLoading(true);
    void listWorkspaces()
      .then((items) => {
        if (disposed) return;
        setWorkspaces(items);
        let savedWorkspaceId: string | null = null;
        try {
          savedWorkspaceId = localStorage.getItem(ACTIVE_WORKSPACE_KEY);
        } catch (cause) {
          setError("恢复工作空间失败：" + String(cause));
        }
        const initialWorkspace =
          items.find((item) => item.id === savedWorkspaceId) ?? items[0];
        setActiveWorkspaceId(initialWorkspace?.id ?? null);
      })
      .catch((cause) => {
        if (!disposed) setError(`加载工作空间失败：${String(cause)}`);
      })
      .finally(() => {
        if (!disposed) setLibraryLoading(false);
      });

    return () => {
      disposed = true;
    };
  }, [desktop]);

  useEffect(() => {
    if (!desktop || !activeWorkspaceId) {
      setLoadedWorkspaceId(null);
      setDocuments([]);
      setSelectedImageId(null);
      return;
    }

    let disposed = false;
    setLoadedWorkspaceId(null);
    setDocuments([]);
    setSelectedImageId(null);
    setLibraryLoading(true);
    void listDocuments(activeWorkspaceId)
      .then((items) => {
        if (disposed) return;
        setDocuments(items);
        setLoadedWorkspaceId(activeWorkspaceId);
        setSelectedImageId(items[items.length - 1]?.imageId ?? null);
      })
      .catch((cause) => {
        if (!disposed) setError(`加载图片记录失败：${String(cause)}`);
      })
      .finally(() => {
        if (!disposed) setLibraryLoading(false);
      });

    return () => {
      disposed = true;
    };
  }, [activeWorkspaceId, desktop]);

  useEffect(() => {
    if (!desktop || shortcutsOpen || libraryAction) return;

    const handlePaste = (event: ClipboardEvent) => {
      if (isEditablePasteTarget(event)) return;
      const imageItem = Array.from(event.clipboardData?.items ?? []).find(
        (item) => item.kind === "file" && item.type.startsWith("image/"),
      );
      if (!imageItem) return;

      event.preventDefault();
      if (busy.current) {
        setError("正在识别图片，请稍后再粘贴。");
        return;
      }
      const file = imageItem.getAsFile();
      if (!file) {
        setError("无法读取剪贴板图片，请重新复制后再试。");
        return;
      }
      void runPastedOcr(file);
    };

    window.addEventListener("paste", handlePaste);
    return () => window.removeEventListener("paste", handlePaste);
  }, [desktop, runPastedOcr, shortcutsOpen, libraryAction]);

  useEffect(() => {
    if (!desktop) return;
    let disposed = false;
    let unlistenResize: (() => void) | undefined;
    const win = getCurrentWindow();
    async function restoreAlwaysOnTop() {
      let savedAlwaysOnTop = false;
      try {
        savedAlwaysOnTop = localStorage.getItem(ALWAYS_ON_TOP_KEY) === "true";
      } catch (cause) {
        if (!disposed) setError(`读取置顶设置失败：${String(cause)}`);
      }
      await win.setAlwaysOnTop(savedAlwaysOnTop);
      if (!disposed) setAlwaysOnTop(savedAlwaysOnTop);
    }
    void restoreAlwaysOnTop().catch((cause) => {
      if (!disposed) setError(`恢复置顶设置失败：${String(cause)}`);
    });
    void Promise.all([win.isMaximized(), win.setResizable(true)])
      .then(([isMaximized]) => {
        if (!disposed) setMaximized(isMaximized);
      })
      .catch((cause) => {
        if (!disposed) setError(`读取窗口状态失败：${String(cause)}`);
      });
    void win
      .onResized(() => {
        void win
          .isMaximized()
          .then((value) => {
            if (!disposed) setMaximized(value);
          })
          .catch((cause) => {
            if (!disposed) setError(`读取窗口状态失败：${String(cause)}`);
          });
      })
      .then((unlisten) => {
        if (disposed) unlisten();
        else unlistenResize = unlisten;
      })
      .catch((cause) => {
        if (!disposed) setError(`监听窗口状态失败：${String(cause)}`);
      });

    return () => {
      disposed = true;
      unlistenResize?.();
    };
  }, [desktop]);

  useEffect(() => {
    if (!desktop) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        if (disposed) return;
        if (event.payload.type === "enter") {
          const [path] = event.payload.paths;
          draggedImagePath.current =
            event.payload.paths.length === 1 && path && IMG_RE.test(path)
              ? path
              : null;
          setDragging(
            isInsideDropZone(event.payload.position) &&
              !busy.current &&
              Boolean(draggedImagePath.current),
          );
          return;
        }
        if (event.payload.type === "over") {
          setDragging(
            isInsideDropZone(event.payload.position) &&
              !busy.current &&
              Boolean(draggedImagePath.current),
          );
          return;
        }
        if (event.payload.type === "leave") {
          draggedImagePath.current = null;
          setDragging(false);
          return;
        }

        const insideDropZone = isInsideDropZone(event.payload.position);
        setDragging(false);
        if (
          event.payload.type !== "drop" ||
          busy.current ||
          !insideDropZone ||
          event.payload.paths.length !== 1
        ) {
          return;
        }
        const path = event.payload.paths[0];
        draggedImagePath.current = null;
        if (IMG_RE.test(path)) void runOcr(path);
      })
      .then((stopListening) => {
        if (disposed) stopListening();
        else unlisten = stopListening;
      })
      .catch((cause) => {
        if (!disposed) setError(`初始化拖拽失败：${String(cause)}`);
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [desktop, runOcr]);

  useEffect(() => {
    if (!desktop || !activeWorkspaceId) return;
    try {
      localStorage.setItem(ACTIVE_WORKSPACE_KEY, activeWorkspaceId);
    } catch (cause) {
      setError("保存工作空间失败：" + String(cause));
    }
  }, [desktop, activeWorkspaceId]);

  function selectWorkspace(workspaceId: string) {
    if (busy.current || libraryLoading) return;
    setError(null);
    setCopiedImageId(null);
    setActiveWorkspaceId(workspaceId);
  }

  async function handleCreateWorkspace(name: string) {
    if (!name) return;

    try {
      const workspace = await createWorkspace(name);
      setWorkspaces((current) => [workspace, ...current]);
      setActiveWorkspaceId(workspace.id);
    } catch (cause) {
      throw new Error(`创建工作空间失败：${String(cause)}`);
    }
  }

  async function handleRenameWorkspace(workspace: Workspace, name: string) {
    if (!name || name === workspace.name) return;

    try {
      await renameWorkspace(workspace.id, name);
      setWorkspaces((current) =>
        current.map((item) =>
          item.id === workspace.id
            ? { ...item, name, updatedAt: Math.floor(Date.now() / 1000) }
            : item,
        ),
      );
    } catch (cause) {
      throw new Error(`重命名工作空间失败：${String(cause)}`);
    }
  }

  async function handleDeleteWorkspace(workspace: Workspace) {
    if (workspace.id === "default") return;

    try {
      await deleteWorkspace(workspace.id);
      const remaining = workspaces.filter((item) => item.id !== workspace.id);
      setWorkspaces(remaining);
      if (workspace.id === activeWorkspaceId) {
        setActiveWorkspaceId(
          remaining.find((item) => item.id === "default")?.id ??
            remaining[0]?.id ??
            null,
        );
      }
    } catch (cause) {
      throw new Error(`删除工作空间失败：${String(cause)}`);
    }
  }

  async function handleDeleteDocument(document: OcrResult) {
    if (!activeWorkspaceId) return;

    try {
      await deleteDocument(activeWorkspaceId, document.imageId);
      const remaining = documents.filter(
        (item) => item.imageId !== document.imageId,
      );
      setDocuments(remaining);
      if (selectedImageId === document.imageId) {
        setSelectedImageId(remaining[remaining.length - 1]?.imageId ?? null);
      }
      if (copiedImageId === document.imageId) setCopiedImageId(null);
      setWorkspaces(await listWorkspaces());
    } catch (cause) {
      throw new Error(`删除图片失败：${String(cause)}`);
    }
  }

  async function copyDocument(document: OcrResult) {
    try {
      await writeText(document.text);
      setSelectedImageId(document.imageId);
      setCopiedImageId(document.imageId);
    } catch (cause) {
      setError(`复制失败：${String(cause)}`);
    }
  }

  async function recognizeDocument(document: OcrResult) {
    if (busy.current || document.workspaceId !== activeWorkspaceId) return;
    busy.current = true;
    setLoading(true);
    setError(null);
    setCopiedImageId(null);
    setSelectedImageId(document.imageId);
    try {
      const result = await recognizeExistingImage(document.imageId);
      setDocuments((current) =>
        current.map((item) => item.imageId === result.imageId ? result : item),
      );
      setError(result.errorMessage);
    } catch (cause) {
      setError(`重新识别失败：${String(cause)}`);
    } finally {
      busy.current = false;
      setLoading(false);
    }
  }

  async function toggleAlwaysOnTop() {
    if (!desktop) return;
    try {
      const nextAlwaysOnTop = !alwaysOnTop;
      await getCurrentWindow().setAlwaysOnTop(nextAlwaysOnTop);
      setAlwaysOnTop(nextAlwaysOnTop);
      try {
        localStorage.setItem(ALWAYS_ON_TOP_KEY, String(nextAlwaysOnTop));
      } catch (cause) {
        setError(`保存置顶设置失败：${String(cause)}`);
      }
    } catch (cause) {
      setError(`设置置顶失败：${String(cause)}`);
    }
  }

  async function handleWindowAction(
    action: "minimize" | "toggleMaximize" | "close",
  ) {
    if (!desktop) return;
    const labels = {
      minimize: "最小化",
      toggleMaximize: "切换窗口大小",
      close: "关闭窗口",
    };
    try {
      const win = getCurrentWindow();
      await win[action]();
      if (action === "toggleMaximize") setMaximized(await win.isMaximized());
    } catch (cause) {
      setError(`${labels[action]}失败：${String(cause)}`);
    }
  }

  async function hideToTray() {
    try {
      await getCurrentWindow().close();
    } catch (cause) {
      setError(`关闭到托盘失败：${String(cause)}`);
    }
  }

  async function quitApplication() {
    try {
      await invoke<void>("request_desktop_quit");
    } catch (cause) {
      setError(`完全退出失败：${String(cause)}`);
    }
  }

  async function capture() {
    if (!activeWorkspaceId || captureBusy.current) return;
    captureBusy.current = true;
    try {
      await startCapture(activeWorkspaceId);
    } catch (e) {
      setError(`截图失败：${String(e)}`);
    } finally {
      captureBusy.current = false;
    }
  }
  async function pin() {
    if (!selectedDocument) return;
    try {
      await createPin(selectedDocument.imageId);
    } catch (e) {
      setError(`贴图失败：${String(e)}`);
    }
  }
  useEffect(() => {
    if (!desktop) return;
    let disposed = false;
    const stops: (() => void)[] = [];
    const bind = (promise: Promise<() => void>) => {
      void promise
        .then((stop) => {
          if (disposed) stop();
          else stops.push(stop);
        })
        .catch((e) => {
          if (!disposed) setError(String(e));
        });
    };
    bind(listen<string>("desktop:error", (e) => setError(e.payload)));
    bind(
      listen<string>("ocr:changed", (e) => {
        if (e.payload !== activeWorkspaceId) return;
        void Promise.all([listDocuments(e.payload), listWorkspaces()])
          .then(([items, spaces]) => {
            if (disposed) return;
            setDocuments(items);
            setWorkspaces(spaces);
            setSelectedImageId(items[items.length - 1]?.imageId ?? null);
          })
          .catch((e) => {
            if (!disposed) setError(String(e));
          });
      }),
    );
    return () => {
      disposed = true;
      stops.forEach((stop) => stop());
    };
  }, [desktop, activeWorkspaceId]);

  const commands: Command[] = [
    {
      id: "capture.start",
      title: "截图",
      scope: "app",
      global: true,
      shortcut: "F1",
      enabled: () => desktop && Boolean(activeWorkspaceId),
      run: capture,
    },
    {
      id: "pins.pasteClipboard",
      title: "贴图到桌面",
      scope: "app",
      global: true,
      shortcut: "F3",
      enabled: () => desktop && Boolean(activeWorkspaceId),
      run: async () => {
        if (activeWorkspaceId) await pasteClipboardPin(activeWorkspaceId);
      },
    },
    {
      id: "ocr.copyText",
      title: "复制识别内容",
      scope: "workspace",
      shortcut: "Mod+Shift+Y",
      allowInEditable: true,
      enabled: () => Boolean(selectedDocument?.text) && !busy.current,
      run: () => {
        if (selectedDocument) void copyDocument(selectedDocument);
      },
    },
    {
      id: "ocr.focusResult",
      title: "聚焦识别结果",
      scope: "workspace",
      shortcut: "Mod+Shift+E",
      allowInEditable: true,
      enabled: () => Boolean(selectedDocument) && !busy.current,
      run: () => resultRef.current?.focus(),
    },
    {
      id: "window.toggleAlwaysOnTop",
      global: true,
      title: "窗口置顶",
      scope: "app",
      shortcut: "Mod+Shift+P",
      enabled: () => desktop,
      run: toggleAlwaysOnTop,
    },
    {
      id: "window.show",
      title: "唤起主窗口",
      scope: "app",
      global: true,
      shortcut: "Mod+Shift+O",
      enabled: () => desktop,
      run: async () => {
        const win = getCurrentWindow();
        await win.show();
        await win.unminimize();
        await win.setFocus();
      },
    },
  ];

  const shortcuts = useShortcuts(
    commands,
    {
      workspaceId: activeWorkspaceId,
      scopes:
        shortcutsOpen || libraryAction || desktopManagerOpen
          ? ["dialog"]
          : ["app", "workspace"],
    },
    setError,
  );

  return (
    <main
      className="app-window flex h-dvh min-h-0 flex-col bg-background text-foreground"
      data-maximized={maximized}
    >
      <WindowTitleBar
        desktop={desktop}
        maximized={maximized}
        alwaysOnTop={alwaysOnTop}
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
        onOpenShortcuts={() => setShortcutsOpen(true)}
        onHideToTray={() => void hideToTray()}
        onQuit={() => void quitApplication()}
        onError={setError}
        onToggleAlwaysOnTop={() =>
          void shortcuts.execute("window.toggleAlwaysOnTop")
        }
        onWindowAction={(action) => void handleWindowAction(action)}
      />

      <DesktopTools
        desktop={desktop}
        workspaceId={activeWorkspaceId}
        imageId={selectedDocument?.imageId ?? null}
        onCapture={() => void shortcuts.execute("capture.start")}
        onPin={() => void pin()}
        onError={setError}
        onManagerChange={setDesktopManagerOpen}
      />

      <div className="flex min-h-0 flex-1">
        <WorkspaceSidebar
          workspaces={workspaces}
          activeWorkspaceId={activeWorkspaceId}
          query={searchQuery}
          disabled={loading || libraryLoading}
          onSelect={selectWorkspace}
          onCreate={() =>
            setLibraryAction({
              title: "新建工作空间",
              description: "创建一个空间来整理图片和识别结果。",
              initialName: "",
              submitLabel: "创建",
              onSubmit: handleCreateWorkspace,
            })
          }
          onRename={(workspace) =>
            setLibraryAction({
              title: "重命名工作空间",
              description: "输入新的工作空间名称。",
              initialName: workspace.name,
              submitLabel: "保存",
              onSubmit: (name) => handleRenameWorkspace(workspace, name),
            })
          }
          onDelete={(workspace) =>
            setLibraryAction({
              title: "删除工作空间？",
              description: `将删除“${workspace.name}”及其中所有图片和识别结果，此操作无法撤销。`,
              submitLabel: "删除空间",
              onSubmit: () => handleDeleteWorkspace(workspace),
            })
          }
        />

        <div className="relative flex min-w-0 flex-1">
          {error ? (
            <div
              role="alert"
              className="absolute top-3 right-4 left-4 z-30 flex items-start justify-between gap-3 rounded-lg border border-destructive/30 bg-background/95 p-3 text-sm text-destructive shadow-lg"
            >
              <p className="min-w-0 wrap-break-word">{error}</p>
              <Button
                variant="ghost"
                size="icon-xs"
                aria-label="关闭错误提示"
                onClick={() => setError(null)}
              >
                <X aria-hidden="true" />
              </Button>
            </div>
          ) : null}

          <OcrFeed
            key={activeWorkspaceId}
            workspaceId={activeWorkspaceId}
            contentReady={
              activeWorkspaceId !== null &&
              loadedWorkspaceId === activeWorkspaceId
            }
            workspaceName={activeWorkspace?.name ?? "未选择工作空间"}
            documents={loadedWorkspaceId === activeWorkspaceId ? documents : []}
            selectedImageId={selectedImageId}
            copiedImageId={copiedImageId}
            query={searchQuery}
            loading={loading || libraryLoading}
            dragging={dragging}
            dropZoneRef={dropZoneRef}
            resultRef={resultRef}
            onSelect={(imageId) => {
              setSelectedImageId(imageId);
              setError(
                documents.find((item) => item.imageId === imageId)
                  ?.errorMessage ?? null,
              );
            }}
            onCopy={(document) => void copyDocument(document)}
            onRecognize={(document) => void recognizeDocument(document)}
            onDelete={(document) =>
              setLibraryAction({
                title: "删除图片？",
                description: `将删除“${document.fileName}”及其识别结果，此操作无法撤销。`,
                submitLabel: "删除图片",
                onSubmit: () => handleDeleteDocument(document),
              })
            }
            onImageError={() =>
              setError("图片预览失败，图片可能已被移动或删除。")
            }
          />
        </div>
      </div>

      {libraryAction ? (
        <LibraryActionDialog
          action={libraryAction}
          onClose={() => setLibraryAction(null)}
        />
      ) : null}

      {shortcutsOpen ? (
        <ShortcutSettings
          commands={commands}
          config={shortcuts.config}
          platform={shortcuts.platform}
          onSave={shortcuts.saveConfig}
          onClose={() => setShortcutsOpen(false)}
        />
      ) : null}
    </main>
  );
}

export default App;
