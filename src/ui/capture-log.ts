import { invoke } from "@tauri-apps/api/core";

export function logCaptureEvent(tag: string, message: string): void {
  if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
    return;
  }
  invoke("log_capture_event", { tag, message }).catch(() => {});
}
