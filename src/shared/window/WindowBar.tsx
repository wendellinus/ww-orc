import { getCurrentWindow } from "@tauri-apps/api/window";
import { Pin, PinOff, X } from "lucide-react";
export function WindowBar({
  title,
  topmost,
  onToggle,
  onClose,
  children,
  onError,
}: {
  title: string;
  topmost: boolean;
  onToggle: () => void;
  onClose: () => void;
  children?: React.ReactNode;
  onError: (message: string) => void;
}) {
  return (
    <header className="desktop-window-bar">
      <span
        className="desktop-drag-title"
        onPointerDown={(e) => {
          if (e.button === 0)
            void getCurrentWindow()
              .startDragging()
              .catch((e) => onError(`移动窗口失败：${String(e)}`));
        }}
      >
        {title}
      </span>
      {children}
      <button
        type="button"
        className="desktop-icon-button"
        aria-label={topmost ? "取消置顶" : "置顶"}
        title={topmost ? "取消置顶" : "置顶"}
        aria-pressed={topmost}
        onClick={onToggle}
      >
        {topmost ? <Pin size={15} /> : <PinOff size={15} />}
      </button>
      <button
        type="button"
        className="desktop-icon-button desktop-close"
        aria-label="关闭"
        title="关闭"
        onClick={onClose}
      >
        <X size={16} />
      </button>
    </header>
  );
}
