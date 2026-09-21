import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, PhysicalSize } from "@tauri-apps/api/window";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  getPin,
  copyPinImage,
  recognizePin,
  updatePinZoom,
  type PinView,
} from "@/features/pins/api";
import { closeObject } from "@/shared/window/api";

export default function PinWindow({ id }: { id: string }) {
  const [view, setView] = useState<PinView | null>(null);
  const [zoom, setZoom] = useState(1);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const writeQueue = useRef(Promise.resolve());
  const sizeQueue = useRef(Promise.resolve());
  const currentZoom = useRef(1);
  const ready = useRef(false);
  const closing = useRef(false);

  function saveZoom(value: number) {
    const operation = writeQueue.current.then(() => updatePinZoom(id, value));
    writeQueue.current = operation.catch(() => {});
    return operation;
  }
  function scale(value: number) {
    const next = Math.max(0.1, Math.min(5, Math.round(value * 100) / 100));
    currentZoom.current = next;
    setZoom(next);
    clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      void saveZoom(next).catch((e) => setError(String(e)));
    }, 300);
  }
  async function close() {
    if (closing.current) return;
    closing.current = true;
    try {
      clearTimeout(timer.current);
      if (ready.current) await saveZoom(currentZoom.current);
      await closeObject("pin", id);
    } catch (e) {
      setError(String(e));
      closing.current = false;
    }
  }
  async function ocr() {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      const text = await recognizePin(id);
      await writeText(text);
      setNotice(text ? "识别文字已复制" : "未识别到文字");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
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
    void getPin(id)
      .then((v) => {
        if (disposed) return;
        setView(v);
        setZoom(v.pin.zoom);
        currentZoom.current = v.pin.zoom;
        ready.current = true;
      })
      .catch((e) => {
        if (!disposed) setError(String(e));
      });
    bind(
      getCurrentWindow().onCloseRequested((e) => {
        e.preventDefault();
        void close();
      }),
    );
    bind(
      listen("desktop:flush-object", () => {
        void (async () => {
          try {
            clearTimeout(timer.current);
            if (ready.current) await saveZoom(currentZoom.current);
            await sizeQueue.current;
            await invoke("window_ready_to_quit");
          } catch (e) {
            setError(String(e));
            await invoke("cancel_desktop_quit", {
              message: `贴图保存失败：${String(e)}`,
            });
          }
        })();
      }),
    );
    return () => {
      disposed = true;
      clearTimeout(timer.current);
      stops.forEach((stop) => stop());
    };
  }, [id]);

  useEffect(() => {
    if (!view) return;
    const size = new PhysicalSize(
      Math.max(1, Math.round(view.width * zoom)),
      Math.max(1, Math.round(view.height * zoom)),
    );
    const operation = sizeQueue.current.then(() =>
      getCurrentWindow().setSize(size),
    );
    sizeQueue.current = operation.catch((e) => setError(String(e)));
  }, [view, zoom]);
  useEffect(() => {
    if (!notice) return;
    const timeout = setTimeout(() => setNotice(null), 1800);
    return () => clearTimeout(timeout);
  }, [notice]);
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.repeat || e.isComposing) return;
      if (e.key === "Escape") {
        e.preventDefault();
        if (menu) setMenu(null);
        else void close();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "c") {
        e.preventDefault();
        void copyPinImage(id).catch((e) => setError(String(e)));
      } else if (e.key === "0") {
        e.preventDefault();
        scale(1);
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [id, menu]);

  return (
    <main
      className="desktop-pin"
      aria-label="桌面贴图"
      onWheel={(e) => {
        e.preventDefault();
        setMenu(null);
        scale(currentZoom.current * (e.deltaY < 0 ? 1.1 : 1 / 1.1));
      }}
      onPointerDown={(e) => {
        if (
          e.button !== 0 ||
          (e.target as HTMLElement).closest(".pin-menu, .pin-message")
        )
          return;
        setMenu(null);
        void getCurrentWindow()
          .startDragging()
          .catch((e) => setError(String(e)));
      }}
      onDoubleClick={() => void close()}
      onContextMenu={(e) => {
        e.preventDefault();
        setMenu({ x: e.clientX, y: e.clientY });
      }}
    >
      {view ? (
        <img
          className="pin-image"
          src={convertFileSrc(view.imagePath)}
          alt="桌面贴图"
          draggable={false}
          onError={() => setError("贴图读取失败，请检查图片是否存在")}
        />
      ) : null}
      {menu ? (
        <div
          className="pin-menu"
          role="menu"
          aria-label="贴图操作"
          style={{
            left: Math.max(0, Math.min(menu.x, innerWidth - 180)),
            top: Math.max(0, Math.min(menu.y, innerHeight - 160)),
          }}
          onDoubleClick={(e) => e.stopPropagation()}
        >
          <button
            role="menuitem"
            onClick={() => {
              setMenu(null);
              void copyPinImage(id).catch((e) => setError(String(e)));
            }}
          >
            复制图片 <span>Ctrl+C</span>
          </button>
          <button
            role="menuitem"
            disabled={busy}
            onClick={() => {
              setMenu(null);
              void ocr();
            }}
          >
            OCR 并复制文字
          </button>
          <button
            role="menuitem"
            onClick={() => {
              setMenu(null);
              scale(1);
            }}
          >
            恢复原始比例 <span>0</span>
          </button>
          <button role="menuitem" onClick={() => void close()}>
            关闭贴图 <span>Esc</span>
          </button>
        </div>
      ) : null}
      {error || busy || notice ? (
        <div
          className="pin-message"
          role={error ? "alert" : "status"}
          onDoubleClick={(e) => e.stopPropagation()}
        >
          {error || (busy ? "正在识别…" : notice)}
          {error ? (
            <button aria-label="关闭提示" onClick={() => setError(null)}>
              ×
            </button>
          ) : null}
        </div>
      ) : null}
    </main>
  );
}
