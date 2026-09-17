import { invoke } from "@tauri-apps/api/core";
import type { Workspace } from "./workspace.types";

export function listWorkspaces(): Promise<Workspace[]> {
  return invoke<Workspace[]>("list_workspaces");
}

export function createWorkspace(name: string): Promise<Workspace> {
  return invoke<Workspace>("create_workspace", { name });
}

export function renameWorkspace(workspaceId: string, name: string): Promise<void> {
  return invoke("rename_workspace", { workspaceId, name });
}

export function deleteWorkspace(workspaceId: string): Promise<void> {
  return invoke("delete_workspace", { workspaceId });
}
