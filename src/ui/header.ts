import { invoke } from "@tauri-apps/api/core";

export interface HeaderHandlers {
  onSettings: () => void;
}

export function renderHeader(root: HTMLElement, handlers: HeaderHandlers): void {
  root.innerHTML = `
    <header class="header" data-tauri-drag-region>
      <span class="title">War Room</span>
      <div class="header-actions">
        <button type="button" class="icon-btn" id="btn-settings" aria-label="Settings" title="Settings">⚙</button>
        <button type="button" class="icon-btn" id="btn-hide" aria-label="Hide" title="Hide">−</button>
      </div>
    </header>
  `;

  root.querySelector("#btn-hide")!.addEventListener("click", (e) => {
    e.stopPropagation();
    void invoke("hide_widget");
  });

  root.querySelector("#btn-settings")!.addEventListener("click", (e) => {
    e.stopPropagation();
    handlers.onSettings();
  });
}

export function setSettingsButtonActive(root: HTMLElement, active: boolean): void {
  const btn = root.querySelector("#btn-settings");
  if (btn) btn.classList.toggle("active", active);
}
