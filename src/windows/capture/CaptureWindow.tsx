import { useEffect, useRef, useState } from "react";
import { emit, listen } from "@tauri-apps/api/event";
import {
  getCapturePreview,
  getSnapshot,
  markCaptureHostReady,
  markCaptureWindowReady,
  cancelCapture,
  finishCapture,
  type Snapshot,
} from "@/features/capture/api";
type Point = { x: number; y: number };
type Rect = Point & { width: number; height: number };
type ResizeHandle = "n" | "ne" | "e" | "se" | "s" | "sw" | "w" | "nw";
type SelectionInteraction =
  | { mode: "draw"; origin: Point }
  | { mode: "move"; origin: Point; start: Rect }
  | { mode: "resize"; origin: Point; start: Rect; handle: ResizeHandle };
type SharedSelection = {
  sessionId: string;
  ownerMonitorId: number;
  rect: Rect | null;
};

const RESIZE_HANDLES: ReadonlyArray<{
  handle: ResizeHandle;
  label: string;
}> = [
  { handle: "nw", label: "调整左上边界" },
  { handle: "n", label: "调整上边界" },
  { handle: "ne", label: "调整右上边界" },
  { handle: "e", label: "调整右边界" },
  { handle: "se", label: "调整右下边界" },
  { handle: "s", label: "调整下边界" },
  { handle: "sw", label: "调整左下边界" },
  { handle: "w", label: "调整左边界" },
];
export default function CaptureWindow({
  monitorId,
}: {
  monitorId: number;
}) {
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [previewPixels, setPreviewPixels] = useState<ArrayBuffer | null>(null);
  const [rect, setRect] = useState<Rect | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const canvas = useRef<HTMLCanvasElement | null>(null);
  const selectionLayer = useRef<HTMLDivElement | null>(null);
  const interaction = useRef<SelectionInteraction | null>(null);
  const drawing = useRef(false);
  const draggingFree = useRef(false);
  const finishing = useRef(false);
  const revealing = useRef(false);
  const selection = useRef<Rect | null>(null);
  const selectionOwner = useRef<number | null>(null);
  const hovered = useRef<Rect | null>(null);
  const [hoverRect, setHoverRect] = useState<Rect | null>(null);
  const [selectionOwnerId, setSelectionOwnerId] = useState<number | null>(null);
  function applySelection(value: Rect | null, owner: number | null) {
    selectionOwner.current = owner;
    setSelectionOwnerId(owner);
    selection.current = value;
    setRect(value);
  }
  function setSelection(value: Rect | null) {
    applySelection(value, monitorId);
    if (!sessionId) return;
    void emit<SharedSelection>("capture:selection", {
      sessionId,
      ownerMonitorId: monitorId,
      rect: value,
    });
  }
  useEffect(() => {
    let disposed = false;
    const stops: Array<() => void> = [];
    const activate = (nextSessionId: string) => {
      // 预热窗口会跨会话复用，因此交互引用和可见状态必须成组重置。
      revealing.current = false;
      finishing.current = false;
      drawing.current = false;
      draggingFree.current = false;
      interaction.current = null;
      selection.current = null;
      selectionOwner.current = null;
      hovered.current = null;
      setSnapshot(null);
      setPreviewPixels(null);
      setRect(null);
      setSelectionOwnerId(null);
      setHoverRect(null);
      setError(null);
      setBusy(false);
      setSessionId(nextSessionId);
    };
    const reset = () => {
      drawing.current = false;
      draggingFree.current = false;
      interaction.current = null;
      selection.current = null;
      selectionOwner.current = null;
      hovered.current = null;
      setSessionId(null);
      setSnapshot(null);
      setPreviewPixels(null);
      setRect(null);
      setSelectionOwnerId(null);
      setHoverRect(null);
      setError(null);
      setBusy(false);
    };
    void Promise.all([
      listen<string>("capture:start", (event) => activate(event.payload)),
      listen("capture:reset", reset),
    ])
      .then((unlisteners) => {
        if (disposed) {
          unlisteners.forEach((unlisten) => unlisten());
          return;
        }
        stops.push(...unlisteners);
        void markCaptureHostReady(monitorId)
          .then((activeSessionId) => {
            if (!disposed && activeSessionId) activate(activeSessionId);
          })
          .catch((cause) => {
            if (!disposed) setError(String(cause));
          });
      })
      .catch((cause) => {
        if (!disposed) setError(String(cause));
      });
    return () => {
      disposed = true;
      stops.forEach((stop) => stop());
    };
  }, [monitorId]);
  useEffect(() => {
    if (!sessionId) return;
    let disposed = false;
    void Promise.all([
      getSnapshot(sessionId, monitorId),
      getCapturePreview(sessionId, monitorId),
    ])
      .then(([nextSnapshot, bytes]) => {
        if (!disposed) {
          setSnapshot(nextSnapshot);
          setPreviewPixels(bytes);
        }
      })
      .catch((e) => {
        if (!disposed) setError(String(e));
      });

    return () => {
      disposed = true;
    };
  }, [sessionId, monitorId]);
  async function cancel() {
    if (!sessionId) return;
    try {
      await cancelCapture(sessionId);
    } catch (e) {
      setError(String(e));
    }
  }
  async function reveal() {
    if (!sessionId || revealing.current) return;
    revealing.current = true;
    try {
      await markCaptureWindowReady(sessionId, monitorId);
    } catch (e) {
      revealing.current = false;
      setError(`截图预览加载失败：${String(e)}`);
      await cancel();
    }
  }
  useEffect(() => {
    if (!snapshot || !previewPixels || !canvas.current) return;
    try {
      canvas.current.width = snapshot.width;
      canvas.current.height = snapshot.height;
      const context = canvas.current.getContext("2d");
      if (!context) throw new Error("无法创建截图画布");
      context.putImageData(
        new ImageData(
          new Uint8ClampedArray(previewPixels),
          snapshot.width,
          snapshot.height,
        ),
        0,
        0,
      );
      // putImageData 同步写入 Canvas；完成后才报告就绪，由 Rust 统一显示
      // 所有屏幕，避免暴露未准备好的黑色窗口。
      void reveal();
    } catch (cause) {
      setError(`截图预览绘制失败：${String(cause)}`);
      void cancel();
    }
  }, [snapshot, previewPixels, sessionId, monitorId]);
  function point(e: React.PointerEvent): Point {
    const bounds = selectionLayer.current?.getBoundingClientRect();
    if (!bounds || !snapshot) return { x: 0, y: 0 };
    const x =
      snapshot.monitorX +
      ((e.clientX - bounds.left) / bounds.width) * snapshot.width;
    const y =
      snapshot.monitorY +
      ((e.clientY - bounds.top) / bounds.height) * snapshot.height;
    return {
      // 使用虚拟桌面物理像素；Pointer Capture 让坐标可越过当前窗口边缘。
      x: Math.max(
        snapshot.desktopX,
        Math.min(snapshot.desktopX + snapshot.desktopWidth, x),
      ),
      y: Math.max(
        snapshot.desktopY,
        Math.min(snapshot.desktopY + snapshot.desktopHeight, y),
      ),
    };
  }
  function beginInteraction(
    e: React.PointerEvent,
    next: SelectionInteraction,
  ) {
    if (!selectionLayer.current) return;
    interaction.current = next;
    drawing.current = true;
    selectionLayer.current.setPointerCapture(e.pointerId);
  }
  function resizeSelection(
    start: Rect,
    handle: ResizeHandle,
    pointer: Point,
  ): Rect {
    if (!snapshot) return start;
    const minWidth = 2;
    const minHeight = 2;
    let left = start.x;
    let top = start.y;
    let right = start.x + start.width;
    let bottom = start.y + start.height;
    if (handle.includes("w")) left = Math.min(pointer.x, right - minWidth);
    if (handle.includes("e")) right = Math.max(pointer.x, left + minWidth);
    if (handle.includes("n")) top = Math.min(pointer.y, bottom - minHeight);
    if (handle.includes("s")) bottom = Math.max(pointer.y, top + minHeight);
    left = Math.max(snapshot.desktopX, left);
    top = Math.max(snapshot.desktopY, top);
    right = Math.min(snapshot.desktopX + snapshot.desktopWidth, right);
    bottom = Math.min(snapshot.desktopY + snapshot.desktopHeight, bottom);
    return { x: left, y: top, width: right - left, height: bottom - top };
  }
  function windowAt(p: Point): Rect | null {
    if (!snapshot) return null;
    const imageX = p.x - snapshot.monitorX;
    const imageY = p.y - snapshot.monitorY;
    const region = snapshot.windowRegions.find(
      (candidate) =>
        imageX >= candidate.x &&
        imageX < candidate.x + candidate.width &&
        imageY >= candidate.y &&
        imageY < candidate.y + candidate.height,
    );
    if (!region) return null;
    return {
      x: snapshot.monitorX + region.x,
      y: snapshot.monitorY + region.y,
      width: region.width,
      height: region.height,
    };
  }
  function setHovered(value: Rect | null) {
    hovered.current = value;
    setHoverRect(value);
  }
  async function finish(action: "pin" | "ocr" | "copy") {
    const rect = selection.current;
    if (
      !sessionId ||
      !snapshot ||
      !rect ||
      rect.width < 2 ||
      rect.height < 2 ||
      selectionOwner.current !== monitorId ||
      finishing.current
    )
      return;
    finishing.current = true;
    setBusy(true);
    setError(null);
    try {
      await finishCapture(sessionId, monitorId, rect, action);
    } catch (e) {
      setError(String(e));
      finishing.current = false;
      setBusy(false);
    }
  }

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.repeat || e.isComposing || finishing.current) return;
      const primary = e.ctrlKey || e.metaKey;
      if (e.key === "Escape") {
        e.preventDefault();
        void cancel();
      } else if (primary && e.key.toLowerCase() === "t") {
        e.preventDefault();
        void finish("pin");
      } else if (primary && e.key.toLowerCase() === "v") {
        e.preventDefault();
        void finish("pin");
      } else if (
        e.key === "Enter" ||
        (primary && e.key.toLowerCase() === "c")
      ) {
        e.preventDefault();
        void finish("copy");
      }
    };
    let disposed = false;
    const stops: Array<() => void> = [];
    void Promise.all([
      listen("capture:pin-selection", () => {
        if (
          !drawing.current &&
          selectionOwner.current === monitorId
        )
          void finish("pin");
      }),
      listen("capture:copy-selection", () => {
        if (
          !drawing.current &&
          selectionOwner.current === monitorId
        )
          void finish("copy");
      }),
    ])
      .then((unlisteners) => {
        if (disposed) unlisteners.forEach((unlisten) => unlisten());
        else stops.push(...unlisteners);
      })
      .catch((e) => {
        if (!disposed) setError(String(e));
      });
    window.addEventListener("keydown", key);
    return () => {
      disposed = true;
      stops.forEach((stop) => stop());
      window.removeEventListener("keydown", key);
    };
  }, [sessionId, monitorId, snapshot]);
  useEffect(() => {
    if (!sessionId) return;
    let disposed = false;
    let stop: (() => void) | undefined;
    void listen<SharedSelection>("capture:selection", (event) => {
      if (event.payload.sessionId !== sessionId) return;
      applySelection(event.payload.rect, event.payload.ownerMonitorId);
      setHovered(null);
    })
      .then((unlisten) => {
        if (disposed) unlisten();
        else stop = unlisten;
      })
      .catch((cause) => {
        if (!disposed) setError(String(cause));
      });
    return () => {
      disposed = true;
      stop?.();
    };
  }, [sessionId, monitorId]);
  const valid = Boolean(
    rect &&
      rect.width >= 2 &&
      rect.height >= 2 &&
      selectionOwnerId === monitorId,
  );
  const activeRect = rect ?? hoverRect;
  return (
    <main
      className="capture-window"
      onContextMenu={(e) => {
        e.preventDefault();
        void cancel();
      }}
    >
      {snapshot && previewPixels ? (
        <canvas
          ref={canvas}
          className="capture-background"
          aria-label="屏幕截图"
        />
      ) : null}
      <div
        ref={selectionLayer}
        className="capture-selection-layer"
        onPointerLeave={() => {
          if (!drawing.current) setHovered(null);
        }}
        onPointerDown={(e) => {
          if (e.button !== 0 || !snapshot || busy) return;
          drawing.current = true;
          draggingFree.current = false;
          const origin = point(e);
          interaction.current = { mode: "draw", origin };
          setSelection(
            hovered.current ?? { ...origin, width: 0, height: 0 },
          );
          e.currentTarget.setPointerCapture(e.pointerId);
        }}
        onPointerMove={(e) => {
          const p = point(e);
          const active = interaction.current;
          if (!drawing.current || !active) {
            setHovered(windowAt(p));
            return;
          }
          if (active.mode === "move") {
            setSelection({
              ...active.start,
              x: Math.max(
                snapshot.desktopX,
                Math.min(
                  snapshot.desktopX +
                    snapshot.desktopWidth -
                    active.start.width,
                  active.start.x + p.x - active.origin.x,
                ),
              ),
              y: Math.max(
                snapshot.desktopY,
                Math.min(
                  snapshot.desktopY +
                    snapshot.desktopHeight -
                    active.start.height,
                  active.start.y + p.y - active.origin.y,
                ),
              ),
            });
            return;
          }
          if (active.mode === "resize") {
            setSelection(resizeSelection(active.start, active.handle, p));
            return;
          }
          const o = active.origin;
          if (
            !draggingFree.current &&
            Math.hypot(p.x - o.x, p.y - o.y) < 4
          ) {
            return;
          }
          draggingFree.current = true;
          setHovered(null);
          setSelection({
            x: Math.min(p.x, o.x),
            y: Math.min(p.y, o.y),
            width: Math.abs(p.x - o.x),
            height: Math.abs(p.y - o.y),
          });
        }}
        onPointerUp={(e) => {
          drawing.current = false;
          draggingFree.current = false;
          interaction.current = null;
          e.currentTarget.releasePointerCapture(e.pointerId);
        }}
        onPointerCancel={() => {
          drawing.current = false;
          draggingFree.current = false;
          interaction.current = null;
        }}
      >
        {activeRect && snapshot ? (
          <div
            className="capture-selection"
            data-suggested={!rect || undefined}
            onPointerDown={(e) => {
              if (!rect || busy) return;
              e.stopPropagation();
              const origin = point(e);
              beginInteraction(e, { mode: "move", origin, start: rect });
            }}
            style={{
              left:
                ((activeRect.x - snapshot.monitorX) / snapshot.width) * 100 +
                "%",
              top:
                ((activeRect.y - snapshot.monitorY) / snapshot.height) * 100 +
                "%",
              width: (activeRect.width / snapshot.width) * 100 + "%",
              height: (activeRect.height / snapshot.height) * 100 + "%",
            }}
          >
            <span className="capture-size">
              {Math.round(activeRect.width)} × {Math.round(activeRect.height)}
            </span>
            {rect
              ? RESIZE_HANDLES.map(({ handle, label }) => (
                  <button
                    key={handle}
                    type="button"
                    className="capture-resize-handle"
                    data-handle={handle}
                    aria-label={label}
                    onPointerDown={(e) => {
                      e.stopPropagation();
                      const origin = point(e);
                      beginInteraction(e, {
                        mode: "resize",
                        origin,
                        start: rect,
                        handle,
                      });
                    }}
                  />
                ))
              : null}
          </div>
        ) : (
          <div className="capture-shade" />
        )}
      </div>
      {!rect || selectionOwnerId === monitorId ? (
        <aside className="capture-toolbar" aria-label="截图工具">
        <span>
          {busy
            ? "处理中…"
            : valid
              ? "拖动选区或控制点可继续调整"
              : hoverRect
                ? "单击选择窗口，或拖动自由框选"
                : "移动到窗口，或拖动框选区域"}
        </span>
        <button disabled={!valid || busy} onClick={() => void finish("pin")}>
          贴到桌面 · Ctrl+V / F3
        </button>
        <button disabled={!valid || busy} onClick={() => void finish("ocr")}>
          OCR 并记录
        </button>
        <button disabled={!valid || busy} onClick={() => void finish("copy")}>
          复制图片 · Ctrl+C
        </button>
        <button disabled={busy} onClick={() => void cancel()}>
          取消 · Esc
        </button>
        </aside>
      ) : null}
      {error ? (
        <p className="capture-error" role="alert">
          {error}
        </p>
      ) : null}
    </main>
  );
}
