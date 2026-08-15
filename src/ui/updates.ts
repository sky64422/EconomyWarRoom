import { invoke } from "@tauri-apps/api/core";

export type UpdatePhase = "idle" | "downloading" | "ready";

export interface UpdateInfo {
  current_version: string;
  version: string;
}

export interface DownloadProgress {
  version: string;
  chunk_len: number;
  content_length: number | null;
  received: number;
}

export async function checkForUpdates(): Promise<boolean> {
  return invoke<boolean>("check_for_updates");
}

export function formatUpdateError(err: unknown): string {
  if (typeof err === "string") return err;
  if (err && typeof err === "object" && "message" in err) {
    return String((err as { message: unknown }).message);
  }
  return "Update check failed";
}
