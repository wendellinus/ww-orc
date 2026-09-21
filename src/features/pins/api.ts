import { invoke } from "@tauri-apps/api/core";
export type Pin = {
  id: string;
  workspaceId: string;
  imageId: string;
  zoom: number;
  isOpen: boolean;
  createdAt: number;
};
export type PinView = {
  pin: Pin;
  imagePath: string;
  width: number;
  height: number;
};
export const createPin = (imageId: string) =>
  invoke<Pin>("create_pin", { imageId });
export const getPin = (id: string) => invoke<PinView>("get_pin", { id });
export const updatePinZoom = (id: string, zoom: number) =>
  invoke<void>("update_pin_zoom", { id, zoom });
export const recognizePin = (id: string) =>
  invoke<string>("recognize_pin", { id });

export const pasteClipboardPin = (workspaceId: string) =>
  invoke<Pin | null>("paste_clipboard_pin", { workspaceId });
export const copyPinImage = (id: string) =>
  invoke<void>("copy_pin_image", { id });
