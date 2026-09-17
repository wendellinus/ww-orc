import { invoke } from "@tauri-apps/api/core";
import type { OcrResult } from "./ocr.types";

export function recognizeImage(
  workspaceId: string,
  path: string,
): Promise<OcrResult> {
  return invoke<OcrResult>("ocr_image", { workspaceId, path });
}

export function recognizeImageBytes(
  workspaceId: string,
  bytes: Uint8Array,
): Promise<OcrResult> {
  return invoke<OcrResult>("ocr_image_bytes", bytes, {
    headers: {
      "x-workspace-id": workspaceId,
    },
  });
}

export function listDocuments(workspaceId: string): Promise<OcrResult[]> {
  return invoke<OcrResult[]>("list_documents", { workspaceId });
}

export function deleteDocument(
  workspaceId: string,
  imageId: string,
): Promise<void> {
  return invoke("delete_document", { workspaceId, imageId });
}
