import { invoke } from "@tauri-apps/api/core";
export type Snapshot = {
  sessionId: string;
  monitorId: number;
  monitorX: number;
  monitorY: number;
  width: number;
  height: number;
  desktopX: number;
  desktopY: number;
  desktopWidth: number;
  desktopHeight: number;
  windowRegions: Selection[];
};
export type Selection = { x: number; y: number; width: number; height: number };
export const startCapture = (workspaceId: string) =>
  invoke<string>("start_capture", { workspaceId });
export const getSnapshot = (sessionId: string, monitorId: number) =>
  invoke<Snapshot>("get_capture_snapshot", { sessionId, monitorId });
export const getCapturePreview = (sessionId: string, monitorId: number) =>
  invoke<ArrayBuffer>("get_capture_preview", { sessionId, monitorId });
export const markCaptureHostReady = (monitorId: number) =>
  invoke<string | null>("capture_host_ready", { monitorId });
export const markCaptureWindowReady = (sessionId: string, monitorId: number) =>
  invoke<void>("capture_window_ready", { sessionId, monitorId });
export const cancelCapture = (sessionId: string) =>
  invoke<void>("cancel_capture", { sessionId });
export const finishCapture = (
  sessionId: string,
  monitorId: number,
  selection: Selection,
  action: "pin" | "ocr" | "copy",
) =>
  invoke<string>("finish_capture", { sessionId, monitorId, selection, action });
