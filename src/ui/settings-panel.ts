import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ThemeMode } from "./types";

export interface SettingsPanelController {
  setTheme: (theme: ThemeMode) => void;
  setOpacity: (opacity: number) => void;
  setQuoteRefreshSecs: (secs: number) => void;
  setAutostart: (enabled: boolean) => void;
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

const REFRESH_PRESETS = [5, 10, 15, 30, 60, 120] as const;

export function mountSettingsPanel(
  root: HTMLElement,
  initial: {
    theme: ThemeMode;
    opacity: number;
    quoteRefreshSecs: number;
    autostart: boolean;
  },
  options: SettingsPanelOptions = {},
): SettingsPanelController {
  let theme = initial.theme;
  let opacity = initial.opacity;
  let quoteRefreshSecs = initial.quoteRefreshSecs;
  let autostart = initial.autostart;
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
      <div class="settings-section">
        <div class="settings-label">Price refresh</div>
        <div class="segmented refresh-segmented" role="group" aria-label="Price refresh interval">
          ${REFRESH_PRESETS.map(
            (s) => `
            <button type="button" data-refresh="${s}" class="${s === quoteRefreshSecs ? "active" : ""}">${formatRefresh(s)}</button>
          `,
          ).join("")}
        </div>
      </div>
      <div class="settings-section">
        <label class="settings-toggle" for="autostart-toggle">
          <span class="settings-toggle-text">
            <span class="settings-toggle-title">Launch at login</span>
            <span class="settings-toggle-hint">Start with Windows</span>
          </span>
          <input type="checkbox" id="autostart-toggle" ${autostart ? "checked" : ""} />
          <span class="settings-switch" aria-hidden="true"></span>
        </label>
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

    root.querySelectorAll<HTMLButtonElement>("[data-refresh]").forEach((btn) => {
      btn.addEventListener("click", () => {
        const secs = Number(btn.dataset.refresh);
        if (!Number.isFinite(secs)) return;
        void applyQuoteRefresh(secs);
      });
    });

    const autostartToggle = root.querySelector(
      "#autostart-toggle",
    ) as HTMLInputElement;
    autostartToggle.addEventListener("change", () => {
      void applyAutostart(autostartToggle.checked);
    });

    root.querySelector("#btn-diag")!.addEventListener("click", () => {
      void copyDiagnostics(root.querySelector("#btn-diag") as HTMLButtonElement);
    });

    root.querySelector("#btn-quit")!.addEventListener("click", () => {
      void invoke("quit_app");
    });
  }

  async function applyQuoteRefresh(secs: number): Promise<void> {
    quoteRefreshSecs = secs;
    render();
    try {
      const applied = await invoke<number>("set_quote_refresh_secs", { secs });
      quoteRefreshSecs = applied;
      if (visible) render();
    } catch (err) {
      console.error("set_quote_refresh_secs failed", err);
    }
  }

  async function applyAutostart(enabled: boolean): Promise<void> {
    const previous = autostart;
    autostart = enabled;
    try {
      await invoke("set_autostart", { enabled });
    } catch (err) {
      console.error("set_autostart failed", err);
      autostart = previous;
      if (visible) render();
    }
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
    setQuoteRefreshSecs: (s) => {
      quoteRefreshSecs = s;
      if (visible) render();
    },
    setAutostart: (enabled) => {
      autostart = enabled;
      if (visible) render();
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

function formatRefresh(secs: number): string {
  if (secs >= 60 && secs % 60 === 0) return `${secs / 60}m`;
  return `${secs}s`;
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

/**
 * Glass opacity + matching text/chart alpha (TokenUsage-aligned).
 * Background uses --panel-opacity; fg/accent/chrome track the slider so
 * labels, prices, and sparklines don't stay fully solid while glass fades.
 */
export function applyPanelOpacity(panel: HTMLElement, opacity: number): void {
  const o = Math.min(1, Math.max(0.35, opacity));
  // Keep type slightly stronger than glass so low opacity stays readable
  const fg = Math.min(1, Math.max(0.62, o * 1.02));
  const accent = Math.min(1, Math.max(0.55, o * 1.05));
  const chrome = Math.min(1, Math.max(0.4, o));

  const root = document.documentElement;
  for (const el of [panel, root]) {
    el.style.setProperty("--panel-opacity", String(o));
    el.style.setProperty("--fg-opacity", String(fg));
    el.style.setProperty("--accent-opacity", String(accent));
    el.style.setProperty("--chrome-opacity", String(chrome));
  }
}
