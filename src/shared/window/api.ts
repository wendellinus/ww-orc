import { invoke } from "@tauri-apps/api/core";
export type ObjectKind = "note" | "pin";
export const openObject = (kind: ObjectKind, id: string) =>
  invoke<void>("open_desktop_object", { kind, id });
export const closeObject = (kind: ObjectKind, id: string) =>
  invoke<void>("close_desktop_object", { kind, id });
export const deleteObject = (kind: ObjectKind, id: string) =>
  invoke<void>("delete_desktop_object", { kind, id });
export const setTopmost = (kind: ObjectKind, id: string, enabled: boolean) =>
  invoke<void>("set_object_topmost", { kind, id, enabled });
