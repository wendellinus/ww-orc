import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ScanLine, StickyNote, Pin, PanelsTopLeft, X } from "lucide-react";
import type { Note } from "@/features/notes";
import type { Pin as PinItem } from "@/features/pins";
import { openObject, deleteObject, type ObjectKind } from "@/shared/window/api";
type Items = { notes: Note[]; pins: PinItem[] };
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
  const [pending, setPending] = useState<{
    kind: ObjectKind;
    id: string;
  } | null>(null);
  const [busy, setBusy] = useState(false);
  const refresh = useCallback(
    () =>
      invoke<Items>("list_desktop_items", { workspaceId })
        .then(setItems)
        .catch((e) => onError(String(e))),
    [workspaceId, onError],
  );
  useEffect(() => {
    dialog.current?.showModal();
    void refresh();
    let disposed = false;
    let stop: (() => void) | undefined;
    void listen("desktop:changed", () => void refresh())
      .then((s) => {
        if (disposed) s();
        else stop = s;
      })
      .catch((e) => onError(String(e)));
    return () => {
      disposed = true;
      stop?.();
    };
  }, [refresh, onError]);
  async function open(kind: ObjectKind, id: string) {
    try {
      await openObject(kind, id);
    } catch (e) {
      onError(String(e));
    }
  }
  async function remove() {
    if (!pending) return;
    setBusy(true);
    try {
      await deleteObject(pending.kind, pending.id);
      setPending(null);
      await refresh();
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <dialog
      ref={dialog}
      className="desktop-manager"
      aria-labelledby="desktop-manager-title"
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
    >
      <header className="desktop-manager-header">
        <h2 id="desktop-manager-title">桌面贴图与便签</h2>
        <button
          className="desktop-icon-button"
          aria-label="关闭管理窗口"
          onClick={onClose}
        >
          <X size={16} />
        </button>
      </header>
      <div className="desktop-manager-content">
        <h3>便签</h3>
        {items ? (
          items.notes.length ? (
            items.notes.map((note) => (
              <div className="desktop-item" key={note.id}>
                <div className="desktop-item-info">
                  <p>{note.text.trim().split("\n")[0] || "空白便签"}</p>
                  <small>{note.isOpen ? "已打开" : "已关闭"}</small>
                </div>
                <button
                  className="desktop-text-button"
                  onClick={() => void open("note", note.id)}
                >
                  {note.isOpen ? "定位" : "打开"}
                </button>
                <button
                  className="desktop-text-button"
                  onClick={() => setPending({ kind: "note", id: note.id })}
                >
                  删除
                </button>
              </div>
            ))
          ) : (
            <p className="desktop-empty">点击「新建便签」记录要记住的事。</p>
          )
        ) : (
          <p className="desktop-empty">加载中…</p>
        )}
        <h3>贴图</h3>
        {items?.pins.length ? (
          items.pins.map((pin, i) => (
            <div className="desktop-item" key={pin.id}>
              <div className="desktop-item-info">
                <p>贴图 {items.pins.length - i}</p>
                <small>
                  {pin.isOpen ? "已打开" : "已关闭"} ·{" "}
                  {Math.round(pin.zoom * 100)}%
                </small>
              </div>
              <button
                className="desktop-text-button"
                onClick={() => void open("pin", pin.id)}
              >
                {pin.isOpen ? "定位" : "打开"}
              </button>
              <button
                className="desktop-text-button"
                onClick={() => setPending({ kind: "pin", id: pin.id })}
              >
                删除
              </button>
            </div>
          ))
        ) : (
          <p className="desktop-empty">
            截图后选择「贴到桌面」，或将选中的图片贴到桌面。
          </p>
        )}
        {pending ? (
          <div className="desktop-delete-confirm">
            <span>
              确认删除这张{pending.kind === "note" ? "便签及其内容" : "贴图"}？
            </span>
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
              onClick={() => setPending(null)}
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
  onNote,
  onPin,
  onError,
  onManagerChange,
}: {
  desktop: boolean;
  workspaceId: string | null;
  imageId: string | null;
  onCapture: () => void;
  onNote: () => void;
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
          disabled={!desktop || !workspaceId}
          onClick={onNote}
        >
          <StickyNote size={15} />
          新建便签
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
          管理贴图与便签
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
