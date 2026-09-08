import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { convertFileSrc, invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  Check,
  Copy,
  FileImage,
  LoaderCircle,
  Keyboard,
  Minus,
  Pin,
  ScanText,
  Square,
  X,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import { ShortcutSettings } from "@/components/shortcut-settings";
import { useShortcuts } from "@/hooks/use-shortcuts";
import { type Command } from "@/lib/shortcuts/registry";
import "./App.css";

type OcrResult = { text: string };
const IMG_RE = /\.(png|jpe?g)$/i;
const MAX_CLIPBOARD_IMAGE_BYTES = 20 * 1024 * 1024;

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
  const resultRef = useRef<HTMLTextAreaElement>(null);
  const pastedPreviewUrl = useRef<string | null>(null);
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const dropZoneRef = useRef<HTMLDivElement>(null);
  const draggedImagePath = useRef<string | null>(null);
  const [alwaysOnTop, setAlwaysOnTop] = useState(true);
  const [maximized, setMaximized] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [loading, setLoading] = useState(false);
  const [preview, setPreview] = useState<string | null>(null);
  const [fileName, setFileName] = useState("");
  const [text, setText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

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

  const releasePastedPreview = useCallback(() => {
    if (!pastedPreviewUrl.current) return;
    URL.revokeObjectURL(pastedPreviewUrl.current);
    pastedPreviewUrl.current = null;
  }, []);

  const runOcr = useCallback(
    async (path: string) => {
      if (busy.current) return;
      busy.current = true;
      setLoading(true);
      setError(null);
      setText("");
      setCopied(false);
      releasePastedPreview();
      setPreview(null);
      setFileName(path.split(/[\\/]/).pop() || path);
      try {
        const result = await invoke<OcrResult>("ocr_image", { path });
        setPreview(convertFileSrc(path));
        setText(result.text);
      } catch (e) {
        setError(String(e));
      } finally {
        busy.current = false;
        setLoading(false);
      }
    },
    [releasePastedPreview],
  );

  const runPastedOcr = useCallback(
    async (file: File) => {
      if (busy.current) return;
      if (file.size > MAX_CLIPBOARD_IMAGE_BYTES) {
        setError("剪贴板图片不能超过 20 MB。");
        return;
      }

      busy.current = true;
      setLoading(true);
      setError(null);
      setText("");
      setCopied(false);
      releasePastedPreview();
      setPreview(null);
      setFileName(file.name || "剪贴板图片");
      try {
        const bytes = new Uint8Array(await file.arrayBuffer());
        const result = await invoke<OcrResult>("ocr_image_bytes", bytes);
        const previewUrl = URL.createObjectURL(file);
        pastedPreviewUrl.current = previewUrl;
        setPreview(previewUrl);
        setText(result.text);
      } catch (e) {
        setError(String(e));
      } finally {
        busy.current = false;
        setLoading(false);
      }
    },
    [releasePastedPreview],
  );

  useEffect(() => () => releasePastedPreview(), [releasePastedPreview]);

  useEffect(() => {
    if (!desktop || shortcutsOpen) return;

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
  }, [desktop, runPastedOcr, shortcutsOpen]);

  useEffect(() => {
    if (!desktop) return;
    let disposed = false;
    let unlistenResize: (() => void) | undefined;
    const win = getCurrentWindow();
    void Promise.all([
      win.isAlwaysOnTop(),
      win.isMaximized(),
      win.setResizable(true),
    ])
      .then(([top, isMaximized]) => {
        if (!disposed) {
          setAlwaysOnTop(top);
          setMaximized(isMaximized);
        }
      })
      .catch((e) => {
        if (!disposed) setError(`读取窗口状态失败：${String(e)}`);
      });
    void win
      .onResized(() => {
        void win
          .isMaximized()
          .then((value) => {
            if (!disposed) setMaximized(value);
          })
          .catch((e) => {
            if (!disposed) setError(`读取窗口状态失败：${String(e)}`);
          });
      })
      .then((unlisten) => {
        if (disposed) unlisten();
        else unlistenResize = unlisten;
      })
      .catch((e) => {
        if (!disposed) setError(`监听窗口状态失败：${String(e)}`);
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
        )
          return;
        const path = event.payload.paths[0];
        draggedImagePath.current = null;
        if (IMG_RE.test(path)) void runOcr(path);
      })
      .then((stopListening) => {
        if (disposed) stopListening();
        else unlisten = stopListening;
      })
      .catch((e) => {
        if (!disposed) setError(`初始化拖拽失败：${String(e)}`);
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [desktop, runOcr]);

  async function toggleAlwaysOnTop() {
    try {
      await getCurrentWindow().setAlwaysOnTop(!alwaysOnTop);
      setAlwaysOnTop(!alwaysOnTop);
    } catch (e) {
      setError(`设置置顶失败：${String(e)}`);
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
    } catch (e) {
      setError(`${labels[action]}失败：${String(e)}`);
    }
  }

  async function copyText() {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
    } catch {
      setError("复制失败，请在识别结果中选中文字后手动复制。");
    }
  }

  const commands: Command[] = [
    {
      id: "app.shortcuts",
      title: "快捷键设置",
      scope: "app",
      shortcut: "Mod+Slash",
      allowInEditable: true,
      run: () => setShortcutsOpen(true),
    },
    {
      id: "ocr.copyText",
      title: "复制识别文字",
      scope: "workspace",
      shortcut: "Mod+Shift+Y",
      allowInEditable: true,
      enabled: () => Boolean(text) && !busy.current,
      run: copyText,
    },
    {
      id: "ocr.focusResult",
      title: "聚焦识别结果",
      scope: "workspace",
      shortcut: "Mod+Shift+E",
      allowInEditable: true,
      enabled: () => Boolean(preview) && !busy.current,
      run: () => {
        resultRef.current?.focus();
      },
    },
    {
      id: "window.toggleAlwaysOnTop",
      title: "切换窗口置顶",
      scope: "app",
      shortcut: "Mod+Shift+P",
      enabled: () => desktop,
      run: toggleAlwaysOnTop,
    },
    {
      id: "window.minimize",
      title: "最小化窗口",
      scope: "app",
      enabled: () => desktop,
      run: () => handleWindowAction("minimize"),
    },
    {
      id: "window.toggleMaximize",
      title: "最大化 / 还原窗口",
      scope: "app",
      enabled: () => desktop,
      run: () => handleWindowAction("toggleMaximize"),
    },
    {
      id: "window.close",
      title: "关闭窗口",
      scope: "app",
      enabled: () => desktop,
      run: () => handleWindowAction("close"),
    },
  ];
  const shortcuts = useShortcuts(
    commands,
    {
      // Until workspace management is introduced, this shell owns one explicit workspace.
      workspaceId: "default",
      scopes: shortcutsOpen ? ["dialog"] : ["app", "workspace"],
    },
    setError,
  );

  return (
    <main
      className="app-window flex h-dvh min-h-0 flex-col bg-background text-foreground"
      data-maximized={maximized}
    >
      <header className="flex h-12 shrink-0 items-center gap-3 border-b px-4">
        <div
          className="flex min-w-0 flex-1 items-center self-stretch select-none"
          data-tauri-drag-region
        >
          <p className="pointer-events-none font-heading text-base font-semibold tracking-tight">
            ww-ocr
          </p>
        </div>
        <div
          className="flex shrink-0 items-center gap-1"
          role="group"
          aria-label="窗口操作"
        >
          <Button
            size="icon-sm"
            variant="ghost"
            onClick={() => void shortcuts.execute("app.shortcuts")}
            aria-label="快捷键设置"
            title={`快捷键设置 (${shortcuts.label("app.shortcuts")})`}
          >
            <Keyboard aria-hidden="true" />
          </Button>
          <Button
            size="icon-sm"
            variant="ghost"
            className="border-0 text-muted-foreground hover:bg-primary/10 hover:text-primary hover:shadow-sm aria-pressed:bg-primary/15 aria-pressed:text-primary"
            aria-pressed={alwaysOnTop}
            disabled={!desktop}
            onClick={() => void shortcuts.execute("window.toggleAlwaysOnTop")}
            aria-label="窗口置顶"
            title={`${alwaysOnTop ? "取消置顶" : "窗口置顶"} (${shortcuts.label("window.toggleAlwaysOnTop")})`}
          >
            <Pin
              className={cn(
                "transition-transform duration-200",
                alwaysOnTop && "fill-current",
              )}
              aria-hidden="true"
            />
          </Button>
          <Button
            size="icon-sm"
            variant="ghost"
            className="border-0 text-muted-foreground hover:bg-muted hover:text-foreground"
            disabled={!desktop}
            onClick={() => void shortcuts.execute("window.minimize")}
            aria-label="最小化"
            title="最小化"
          >
            <Minus aria-hidden="true" />
          </Button>
          <Button
            size="icon-sm"
            variant="ghost"
            className="border-0 text-muted-foreground hover:bg-muted hover:text-foreground"
            disabled={!desktop}
            onClick={() => void shortcuts.execute("window.toggleMaximize")}
            aria-label={maximized ? "还原窗口" : "最大化"}
            title={maximized ? "还原窗口" : "最大化"}
          >
            {maximized ? (
              <Copy aria-hidden="true" />
            ) : (
              <Square aria-hidden="true" />
            )}
          </Button>
          <Button
            size="icon-sm"
            variant="ghost"
            className="window-close-button border-0 text-muted-foreground"
            disabled={!desktop}
            onClick={() => void shortcuts.execute("window.close")}
            aria-label="关闭窗口"
            title="关闭窗口"
          >
            <X aria-hidden="true" />
          </Button>
        </div>
      </header>

      <section
        className="flex min-h-0 flex-1 flex-col gap-4 overflow-auto p-4 sm:p-6"
        aria-label="图片文字识别工作区"
        aria-busy={loading}
      >
        {error ? (
          <div
            role="alert"
            className="flex shrink-0 items-start justify-between gap-3 rounded-lg border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive"
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

        <div
          className={cn(
            "relative flex min-h-64 flex-1 flex-col rounded-xl border transition-colors",
            dragging
              ? "border-primary bg-primary/5 ring-2 ring-primary/20"
              : "border-border",
            !preview && "border-dashed",
          )}
          ref={dropZoneRef}
        >
          {dragging ? (
            <div className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center rounded-xl bg-background/95">
              <p className="flex items-center gap-3 font-medium text-primary">
                <FileImage aria-hidden="true" />
                松开图片，开始识别
              </p>
            </div>
          ) : null}

          {loading ? (
            <div
              role="status"
              className="flex flex-1 flex-col items-center justify-center gap-4 p-8 text-center"
            >
              <LoaderCircle
                className="size-8 text-primary motion-safe:animate-spin"
                aria-hidden="true"
              />
              <div>
                <p className="font-medium">正在识别图片…</p>
                <p className="mt-2 text-sm text-muted-foreground">
                  首次加载模型需要一些时间，请稍候。
                </p>
              </div>
            </div>
          ) : preview ? (
            <div className="grid min-h-0 flex-1 grid-cols-1 overflow-auto sm:grid-cols-2">
              <section
                className="flex min-h-56 flex-col border-b sm:min-h-0 sm:border-r sm:border-b-0"
                aria-label="原始图片"
              >
                <div className="flex h-12 shrink-0 items-center gap-2 border-b px-4 text-xs font-medium text-muted-foreground">
                  <FileImage className="size-4" aria-hidden="true" />
                  原始图片
                </div>
                <div className="flex min-h-0 flex-1 items-center justify-center overflow-auto bg-muted/20 p-4">
                  <img
                    src={preview}
                    alt={fileName || "待识别图片"}
                    draggable={false}
                    className="max-h-full max-w-full rounded-md object-contain"
                    onDragStart={(event) => event.preventDefault()}
                    onError={() =>
                      setError(
                        "图片预览加载失败，识别文字仍可使用。请重新拖入图片或检查文件是否被移动。",
                      )
                    }
                  />
                </div>
              </section>
              <section
                className="flex min-h-64 flex-col sm:min-h-0"
                aria-label="提取的文字"
              >
                <div className="flex h-12 shrink-0 items-center justify-between gap-2 border-b px-4">
                  <label
                    htmlFor="ocr-text"
                    className="text-xs font-medium text-muted-foreground"
                  >
                    识别文字
                  </label>
                  <Button
                    size="xs"
                    variant={copied ? "secondary" : "default"}
                    disabled={!text}
                    onClick={() => void shortcuts.execute("ocr.copyText")}
                    title={shortcuts.label("ocr.copyText")}
                  >
                    {copied ? (
                      <Check aria-hidden="true" />
                    ) : (
                      <Copy aria-hidden="true" />
                    )}
                    {copied ? "已复制" : "复制文字"}
                  </Button>
                </div>
                <div className="flex min-h-0 flex-1 p-3">
                  <Textarea
                    id="ocr-text"
                    ref={resultRef}
                    readOnly
                    value={text}
                    placeholder="未识别到文字。试试更清晰、文字方向正确的图片。"
                    draggable={false}
                    onDragStart={(event) => event.preventDefault()}
                    className="h-full min-h-40 flex-1 resize-none border-0 bg-transparent p-2 text-sm leading-7 shadow-none field-sizing-fixed dark:bg-transparent sm:min-h-0"
                  />
                </div>
              </section>
            </div>
          ) : (
            <div className="flex flex-1 flex-col items-center justify-center gap-6 p-6 text-center">
              <div
                className="relative flex size-20 items-center justify-center text-primary"
                aria-hidden="true"
              >
                <span className="absolute top-0 left-0 size-5 rounded-tl-lg border-t-2 border-l-2" />
                <span className="absolute top-0 right-0 size-5 rounded-tr-lg border-t-2 border-r-2" />
                <ScanText className="size-9" strokeWidth={1.5} />
                <span className="absolute bottom-0 left-0 size-5 rounded-bl-lg border-b-2 border-l-2" />
                <span className="absolute bottom-0 right-0 size-5 rounded-br-lg border-b-2 border-r-2" />
              </div>
              <div>
                <h2 className="font-heading text-xl font-semibold tracking-tight">
                  拖入或粘贴图片
                </h2>
                <p className="mt-3 max-w-sm text-sm leading-6 text-muted-foreground">
                  从文件资源管理器拖入图片，或复制图片后按 Ctrl+V，
                  <br />
                  识别完成后即可查看和复制文字。
                </p>
              </div>
              <div className="flex items-center gap-2">
                {["PNG", "JPG", "JPEG"].map((format) => (
                  <Badge key={format} variant="outline">
                    {format}
                  </Badge>
                ))}
              </div>
            </div>
          )}
        </div>
      </section>

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
