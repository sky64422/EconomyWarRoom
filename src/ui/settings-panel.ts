import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ThemeMode } from "./types";

export interface SettingsPanelController {
  setTheme: (theme: ThemeMode) => void;
  setOpacity: (opacity: number) => void;
  show: () => void;
  hide: () => void;
  isVisible: () => boolean;
  destroy: () => void;
}

export interface SettingsPanelOptions {
  onThemeChange?: (theme: ThemeMode) => void;
  onOpacityChange?: (opacity: number) => void;
}

const THEMES: { value: ThemeMode; label: string }[] = [
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
  { value: "system", label: "System" },
];

export function mountSettingsPanel(
  root: HTMLElement,
  initial: { theme: ThemeMode; opacity: number },
  options: SettingsPanelOptions = {},
): SettingsPanelController {
  let theme = initial.theme;
  let opacity = initial.opacity;
  let visible = false;

  root.classList.add("settings-panel", "hidden");

  function render(): void {
    root.innerHTML = `
      <div class="settings-section">
        <div class="settings-label">Theme</div>
        <div class="segmented" role="group" aria-label="Theme">
          ${THEMES.map(
            (t) => `
            <button type="button" data-theme="${t.value}" class="${t.value === theme ? "active" : ""}">${t.label}</button>
          `,
          ).join("")}
        </div>
      </div>
      <div class="settings-section">
        <div class="settings-label">Opacity</div>
        <div class="opacity-row">
          <input type="range" id="opacity-range" min="0.35" max="1" step="0.01" value="${opacity}" />
          <span class="opacity-value" id="opacity-value">${Math.round(opacity * 100)}%</span>
        </div>
      </div>
      <div class="settings-actions">
        <button type="button" class="btn-diag" id="btn-diag">Copy diagnostics</button>
        <button type="button" class="btn-quit" id="btn-quit">Quit</button>
      </div>
    `;

    root.querySelectorAll<HTMLButtonElement>("[data-theme]").forEach((btn) => {
      btn.addEventListener("click", () => {
        const next = btn.dataset.theme as ThemeMode;
        void applyTheme(next);
      });
    });

    const range = root.querySelector("#opacity-range") as HTMLInputElement;
    const valueEl = root.querySelector("#opacity-value") as HTMLElement;
    range.addEventListener("input", () => {
      const v = Number(range.value);
      opacity = v;
      valueEl.textContent = `${Math.round(v * 100)}%`;
      options.onOpacityChange?.(v);
    });
    range.addEventListener("change", () => {
      const v = Number(range.value);
      void invoke("set_opacity", { opacity: v }).catch((err) => {
        console.error("set_opacity failed", err);
      });
    });

    root.querySelector("#btn-diag")!.addEventListener("click", () => {
      void copyDiagnostics(root.querySelector("#btn-diag") as HTMLButtonElement);
    });

    root.querySelector("#btn-quit")!.addEventListener("click", () => {
      void invoke("quit_app");
    });
  }

  async function copyDiagnostics(btn: HTMLButtonElement): Promise<void> {
    const original = "Copy diagnostics";
    try {
      const text = await invoke<string>("get_diagnostics");
      await writeClipboard(text);
      btn.textContent = "Copied";
      window.setTimeout(() => {
        if (btn.isConnected) btn.textContent = original;
      }, 1600);
    } catch (err) {
      console.error("copy diagnostics failed", err);
      btn.textContent = "Failed";
      window.setTimeout(() => {
        if (btn.isConnected) btn.textContent = original;
      }, 2000);
    }
  }

  async function applyTheme(next: ThemeMode): Promise<void> {
    theme = next;
    options.onThemeChange?.(next);
    render();
    try {
      await invoke("set_theme", { theme: next });
    } catch (err) {
      console.error("set_theme failed", err);
    }
  }

  render();

  const unlisteners: Array<() => void> = [];
  void listen<number>("opacity-updated", (e) => {
    const v = e.payload;
    if (typeof v !== "number" || !Number.isFinite(v)) return;
    opacity = v;
    options.onOpacityChange?.(v);
    if (visible) {
      const range = root.querySelector("#opacity-range") as HTMLInputElement | null;
      const valueEl = root.querySelector("#opacity-value") as HTMLElement | null;
      if (range) range.value = String(v);
      if (valueEl) valueEl.textContent = `${Math.round(v * 100)}%`;
    }
  }).then((u) => unlisteners.push(u));

  return {
    setTheme: (t) => {
      theme = t;
      if (visible) render();
    },
    setOpacity: (o) => {
      opacity = o;
      if (visible) {
        const range = root.querySelector("#opacity-range") as HTMLInputElement | null;
        const valueEl = root.querySelector("#opacity-value") as HTMLElement | null;
        if (range) range.value = String(o);
        if (valueEl) valueEl.textContent = `${Math.round(o * 100)}%`;
      }
    },
    show: () => {
      visible = true;
      root.classList.remove("hidden");
      render();
    },
    hide: () => {
      visible = false;
      root.classList.add("hidden");
    },
    isVisible: () => visible,
    destroy: () => {
      for (const u of unlisteners) u();
    },
  };
}

/** Copy text to the system clipboard (clipboard API with textarea fallback). */
async function writeClipboard(text: string): Promise<void> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return;
    }
  } catch {
    // fall through
  }
  const ta = document.createElement("textarea");
  ta.value = text;
  ta.setAttribute("readonly", "");
  ta.style.position = "fixed";
  ta.style.left = "-9999px";
  document.body.appendChild(ta);
  ta.select();
  const ok = document.execCommand("copy");
  document.body.removeChild(ta);
  if (!ok) {
    throw new Error("clipboard copy failed");
  }
}

/** Apply theme to documentElement.dataset.theme */
export function applyThemeToDocument(theme: ThemeMode): void {
  document.documentElement.dataset.theme = theme;
}

/** Apply opacity CSS variable on the glass panel. */
export function applyPanelOpacity(panel: HTMLElement, opacity: number): void {
  const clamped = Math.min(1, Math.max(0.35, opacity));
  panel.style.setProperty("--panel-opacity", String(clamped));
}
