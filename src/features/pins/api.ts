import { invoke } from "@tauri-apps/api/core";

export type Pin = {
  id: string;
  workspaceId: string;
  imageId: string;
  zoom: number;
  isOpen: boolean;
  createdAt: number;
};

export const createPin = (imageId: string) =>
  invoke<Pin>("create_pin", { imageId });

export const pasteClipboardPin = (workspaceId: string) =>
  invoke<Pin | null>("paste_clipboard_pin", { workspaceId });
