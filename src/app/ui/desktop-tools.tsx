import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { PanelsTopLeft, Pin, ScanLine, X } from "lucide-react";
import type { Pin as PinItem } from "@/features/pins";

type Items = { pins: PinItem[] };

function DesktopManager({
  workspaceId,
  onClose,
  onError,
}: {
  workspaceId: string;
  onClose: () => void;
  onError: (message: string) => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [items, setItems] = useState<Items | null>(null);
  const [pendingId, setPendingId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const refresh = useCallback(
    () =>
      invoke<Items>("list_desktop_items", { workspaceId })
        .then(setItems)
        .catch((error) => onError(String(error))),
    [workspaceId, onError],
  );

  useEffect(() => {
    dialog.current?.showModal();
    void refresh();
    let disposed = false;
    let stop: (() => void) | undefined;
    void listen("desktop:changed", () => void refresh())
      .then((unlisten) => {
        if (disposed) unlisten();
        else stop = unlisten;
      })
      .catch((error) => onError(String(error)));
    return () => {
      disposed = true;
      stop?.();
    };
  }, [refresh, onError]);

  async function open(id: string) {
    try {
      await invoke("open_pin", { id });
    } catch (error) {
      onError(String(error));
    }
  }

  async function remove() {
    if (!pendingId) return;
    setBusy(true);
    try {
      await invoke("delete_pin", { id: pendingId });
      setPendingId(null);
      await refresh();
    } catch (error) {
      onError(String(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <dialog
      ref={dialog}
      className="desktop-manager"
      aria-labelledby="desktop-manager-title"
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
    >
      <header className="desktop-manager-header">
        <h2 id="desktop-manager-title">桌面贴图</h2>
        <button
          className="desktop-icon-button"
          aria-label="关闭管理窗口"
          onClick={onClose}
        >
          <X size={16} />
        </button>
      </header>
      <div className="desktop-manager-content">
        {items?.pins.length ? (
          items.pins.map((pin, index) => (
            <div className="desktop-item" key={pin.id}>
              <div className="desktop-item-info">
                <p>贴图 {items.pins.length - index}</p>
                <small>
                  {pin.isOpen ? "已打开" : "已关闭"} ·{" "}
                  {Math.round(pin.zoom * 100)}%
                </small>
              </div>
              <button
                className="desktop-text-button"
                onClick={() => void open(pin.id)}
              >
                {pin.isOpen ? "定位" : "打开"}
              </button>
              <button
                className="desktop-text-button"
                onClick={() => setPendingId(pin.id)}
              >
                删除
              </button>
            </div>
          ))
        ) : items ? (
          <p className="desktop-empty">
            截图后选择「贴到桌面」，或将选中的图片贴到桌面。
          </p>
        ) : (
          <p className="desktop-empty">加载中…</p>
        )}
        {pendingId ? (
          <div className="desktop-delete-confirm">
            <span>确认删除这张贴图？</span>
            <button
              className="desktop-text-button"
              disabled={busy}
              onClick={() => void remove()}
            >
              确认删除
            </button>
            <button
              className="desktop-text-button"
              disabled={busy}
              onClick={() => setPendingId(null)}
            >
              取消
            </button>
          </div>
        ) : null}
      </div>
    </dialog>
  );
}

export function DesktopTools({
  desktop,
  workspaceId,
  imageId,
  onCapture,
  onPin,
  onError,
  onManagerChange,
}: {
  desktop: boolean;
  workspaceId: string | null;
  imageId: string | null;
  onCapture: () => void;
  onPin: () => void;
  onError: (message: string) => void;
  onManagerChange: (open: boolean) => void;
}) {
  const [open, setOpen] = useState(false);
  function manage(value: boolean) {
    setOpen(value);
    onManagerChange(value);
  }
  return (
    <>
      <nav className="desktop-actions" aria-label="桌面工具">
        <button
          className="desktop-action"
          disabled={!desktop || !workspaceId}
          onClick={onCapture}
        >
          <ScanLine size={15} />
          截图
        </button>
        <button
          className="desktop-action"
          disabled={!desktop || !imageId}
          onClick={onPin}
        >
          <Pin size={15} />
          贴图选中图片
        </button>
        <button
          className="desktop-action"
          disabled={!desktop || !workspaceId}
          onClick={() => manage(true)}
        >
          <PanelsTopLeft size={15} />
          管理贴图
        </button>
        <span className="desktop-action-hint">快捷键可在设置中调整</span>
      </nav>
      {open && workspaceId ? (
        <DesktopManager
          workspaceId={workspaceId}
          onClose={() => manage(false)}
          onError={onError}
        />
      ) : null}
    </>
  );
}
