import { lazy, Suspense } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./App.css";
import "@/shared/styles/desktop.css";
const MainWindow = lazy(() => import("./App"));
const NoteWindow = lazy(() => import("@/windows/note/NoteWindow"));
const PinWindow = lazy(() => import("@/windows/pin/PinWindow"));
const CaptureWindow = lazy(() => import("@/windows/capture/CaptureWindow"));
export function WindowRouter() {
  const label = isTauri() ? getCurrentWindow().label : "main";
  document.documentElement.dataset.window = label.startsWith("pin_")
    ? "pin"
    : "other";
  const capture = label.match(/^capture_(\d+)$/);
  const quietFallback = label.startsWith("pin_") || Boolean(capture);
  const body = label.startsWith("note_") ? (
    <NoteWindow id={label.slice(5)} />
  ) : label.startsWith("pin_") ? (
    <PinWindow id={label.slice(4)} />
  ) : capture ? (
    <CaptureWindow monitorId={Number(capture[1])} />
  ) : (
    <MainWindow />
  );
  return (
    <Suspense
      fallback={
        quietFallback ? null : <p className="desktop-loading">加载中…</p>
      }
    >
      {body}
    </Suspense>
  );
}
