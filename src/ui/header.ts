import { invoke } from "@tauri-apps/api/core";

export interface HeaderHandlers {
  onSettings: () => void;
}

export function renderHeader(root: HTMLElement, handlers: HeaderHandlers): void {
  // Markup/chrome aligned with TokenUsage header (flat Apple-like bar).
  root.innerHTML = `
    <div class="header" data-tauri-drag-region>
      <div class="title">WarRoom</div>
      <div class="header-actions">
        <button type="button" class="icon-btn" id="btn-update" aria-label="Check for updates" title="Check for updates">↻</button>
        <button type="button" class="icon-btn" id="btn-settings" aria-label="Settings" title="Settings">⚙</button>
        <button type="button" class="icon-btn" id="btn-hide" aria-label="Hide" title="Hide">–</button>
      </div>
    </div>
  `;

  root.querySelector("#btn-hide")!.addEventListener("click", (e) => {
    e.stopPropagation();
    void invoke("hide_widget");
  });

  root.querySelector("#btn-settings")!.addEventListener("click", (e) => {
    e.stopPropagation();
    handlers.onSettings();
  });

  const updateBtn = root.querySelector("#btn-update") as HTMLButtonElement;
  updateBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    void checkForUpdates(updateBtn);
  });
}

export function setSettingsButtonActive(root: HTMLElement, active: boolean): void {
  const btn = root.querySelector("#btn-settings");
  if (btn) btn.classList.toggle("active", active);
}

async function checkForUpdates(btn: HTMLButtonElement): Promise<void> {
  const originalTitle = btn.getAttribute("title") ?? "Check for updates";
  btn.disabled = true;
  btn.classList.add("busy");
  btn.setAttribute("title", "Checking...");
  try {
    const hasUpdate = await invoke<boolean>("check_for_updates");
    btn.setAttribute("title", hasUpdate ? "Updating..." : "Up to date");
    window.setTimeout(() => {
      if (btn.isConnected) {
        btn.setAttribute("title", originalTitle);
        btn.disabled = false;
        btn.classList.remove("busy");
      }
    }, 2000);
  } catch (err) {
    console.error("check_for_updates failed", err);
    btn.setAttribute("title", "Check failed");
    window.setTimeout(() => {
      if (btn.isConnected) {
        btn.setAttribute("title", originalTitle);
        btn.disabled = false;
        btn.classList.remove("busy");
      }
    }, 2000);
  }
}
