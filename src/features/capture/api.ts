import { invoke } from "@tauri-apps/api/core";
export type Snapshot = {
  sessionId: string;
  monitorId: number;
  imagePath: string;
  width: number;
  height: number;
};
export type Selection = { x: number; y: number; width: number; height: number };
export const startCapture = (workspaceId: string) =>
  invoke<string>("start_capture", { workspaceId });
export const getSnapshot = (sessionId: string, monitorId: number) =>
  invoke<Snapshot>("get_capture_snapshot", { sessionId, monitorId });
export const cancelCapture = (sessionId: string) =>
  invoke<void>("cancel_capture", { sessionId });
export const finishCapture = (
  sessionId: string,
  monitorId: number,
  selection: Selection,
  action: "pin" | "ocr" | "copy",
) =>
  invoke<string>("finish_capture", { sessionId, monitorId, selection, action });
