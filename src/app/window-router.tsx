import { lazy, Suspense } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./App.css";
import "@/shared/styles/desktop.css";

const MainWindow = lazy(() => import("./App"));
const CaptureWindow = lazy(() => import("@/windows/capture/CaptureWindow"));

export function WindowRouter() {
  const label = isTauri() ? getCurrentWindow().label : "main";
  const capture = label.match(/^capture_(\d+)$/);
  const body = capture ? (
    <CaptureWindow monitorId={Number(capture[1])} />
  ) : (
    <MainWindow />
  );
  return (
    <Suspense fallback={capture ? null : <p className="desktop-loading">加载中…</p>}>
      {body}
    </Suspense>
  );
}
