import { invoke } from "@tauri-apps/api/core";
export type Note = {
  id: string;
  workspaceId: string;
  text: string;
  color: string;
  revision: number;
  isOpen: boolean;
  createdAt: number;
  updatedAt: number;
};
export const createNote = (workspaceId: string, text = "") =>
  invoke<Note>("create_note", { workspaceId, text });
export const getNote = (id: string) => invoke<Note>("get_note", { id });
export const updateNote = (
  id: string,
  text: string,
  color: string,
  revision: number,
) => invoke<Note>("update_note", { id, text, color, revision });
