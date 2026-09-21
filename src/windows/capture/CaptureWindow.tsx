import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { convertFileSrc } from "@tauri-apps/api/core";
import {
  getSnapshot,
  cancelCapture,
  finishCapture,
  type Snapshot,
} from "@/features/capture/api";
type Point = { x: number; y: number };
type Rect = Point & { width: number; height: number };
export default function CaptureWindow({
  sessionId,
  monitorId,
}: {
  sessionId: string;
  monitorId: number;
}) {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [rect, setRect] = useState<Rect | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const origin = useRef<Point | null>(null);
  const drawing = useRef(false);
  const finishing = useRef(false);
  const selection = useRef<Rect | null>(null);
  function setSelection(value: Rect | null) {
    selection.current = value;
    setRect(value);
  }
  useEffect(() => {
    let disposed = false;
    void getSnapshot(sessionId, monitorId)
      .then((s) => {
        if (!disposed) setSnapshot(s);
      })
      .catch((e) => {
        if (!disposed) setError(String(e));
      });

    return () => {
      disposed = true;
    };
  }, [sessionId, monitorId]);
  async function cancel() {
    try {
      await cancelCapture(sessionId);
    } catch (e) {
      setError(String(e));
    }
  }
  function point(e: React.PointerEvent): Point {
    return {
      x: Math.max(0, Math.min(innerWidth, e.clientX)),
      y: Math.max(0, Math.min(innerHeight, e.clientY)),
    };
  }
  async function finish(action: "pin" | "ocr" | "copy") {
    const rect = selection.current;
    if (!rect || rect.width < 2 || rect.height < 2 || finishing.current) return;
    finishing.current = true;
    setBusy(true);
    setError(null);
    try {
      await finishCapture(
        sessionId,
        monitorId,
        {
          x: rect.x / innerWidth,
          y: rect.y / innerHeight,
          width: rect.width / innerWidth,
          height: rect.height / innerHeight,
        },
        action,
      );
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
      } else if (
        e.key === "Enter" ||
        (primary && e.key.toLowerCase() === "c")
      ) {
        e.preventDefault();
        void finish("copy");
      }
    };
    let disposed = false;
    let stop: (() => void) | undefined;
    void listen("capture:pin-selection", () => {
      if (!drawing.current) void finish("pin");
    })
      .then((s) => {
        if (disposed) s();
        else stop = s;
      })
      .catch((e) => {
        if (!disposed) setError(String(e));
      });
    window.addEventListener("keydown", key);
    return () => {
      disposed = true;
      stop?.();
      window.removeEventListener("keydown", key);
    };
  }, [sessionId, monitorId]);
  const valid = rect && rect.width >= 2 && rect.height >= 2;
  return (
    <main
      className="capture-window"
      onContextMenu={(e) => {
        e.preventDefault();
        void cancel();
      }}
    >
      {snapshot ? (
        <img
          className="capture-background"
          src={convertFileSrc(snapshot.imagePath)}
          alt="屏幕截图"
          draggable={false}
        />
      ) : null}
      <div
        className="capture-selection-layer"
        onPointerDown={(e) => {
          if (e.button !== 0 || !snapshot || busy) return;
          drawing.current = true;
          origin.current = point(e);
          setSelection({ ...origin.current, width: 0, height: 0 });
          e.currentTarget.setPointerCapture(e.pointerId);
        }}
        onPointerMove={(e) => {
          if (!drawing.current || !origin.current) return;
          const p = point(e);
          const o = origin.current;
          setSelection({
            x: Math.min(p.x, o.x),
            y: Math.min(p.y, o.y),
            width: Math.abs(p.x - o.x),
            height: Math.abs(p.y - o.y),
          });
        }}
        onPointerUp={(e) => {
          drawing.current = false;
          origin.current = null;
          e.currentTarget.releasePointerCapture(e.pointerId);
        }}
        onPointerCancel={() => {
          drawing.current = false;
          origin.current = null;
          setSelection(null);
        }}
      >
        {rect ? (
          <div
            className="capture-selection"
            style={{
              left: rect.x,
              top: rect.y,
              width: rect.width,
              height: rect.height,
            }}
          >
            {valid ? (
              <span className="capture-size">
                {Math.round(
                  (rect.width * (snapshot?.width ?? innerWidth)) / innerWidth,
                )}{" "}
                ×{" "}
                {Math.round(
                  (rect.height * (snapshot?.height ?? innerHeight)) /
                    innerHeight,
                )}
              </span>
            ) : null}
          </div>
        ) : (
          <div className="capture-shade" />
        )}
      </div>
      <aside className="capture-toolbar" aria-label="截图工具">
        <span>{busy ? "处理中…" : valid ? "选区已就绪" : "拖动框选区域"}</span>
        <button disabled={!valid || busy} onClick={() => void finish("pin")}>
          贴到桌面 · F3
        </button>
        <button disabled={!valid || busy} onClick={() => void finish("ocr")}>
          OCR 识别
        </button>
        <button disabled={!valid || busy} onClick={() => void finish("copy")}>
          复制图片 · Enter
        </button>
        <button disabled={busy} onClick={() => void cancel()}>
          取消 · Esc
        </button>
      </aside>
      {error ? (
        <p className="capture-error" role="alert">
          {error}
        </p>
      ) : null}
    </main>
  );
}
