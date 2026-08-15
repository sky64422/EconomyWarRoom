import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  checkForUpdates,
  formatUpdateError,
  type DownloadProgress,
  type UpdateInfo,
  type UpdatePhase,
} from "./updates";

export { applyPanelOpacity } from "./opacity";

export interface SettingsPanelController {
  setQuoteRefreshMs: (ms: number) => void;
  setAutostart: (enabled: boolean) => void;
  show: () => void;
  hide: () => void;
  isVisible: () => boolean;
  destroy: () => void;
}

export interface SettingsPanelOptions {
  /** Esc / close request from inside the dialog (S3). */
  onCloseRequest?: () => void;
}

function focusableIn(container: HTMLElement): HTMLElement[] {
  const sel =
    'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';
  return Array.from(container.querySelectorAll<HTMLElement>(sel)).filter(
    (el) => !el.hasAttribute("disabled") && el.offsetParent !== null,
  );
}

/** Price refresh presets in milliseconds (matches Rust clamp / storage). */
const REFRESH_PRESETS = [250, 1000, 10_000, 60_000] as const;

const ICON_DOWNLOAD = `<svg class="icon-svg" width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true" focusable="false"><path d="M8 2.5v7.5M5 7.25 8 10.25 11 7.25" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/><path d="M3 12.5h10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>`;

function nearestRefreshPreset(ms: number): number {
  return REFRESH_PRESETS.reduce((best, p) =>
    Math.abs(p - ms) < Math.abs(best - ms) ? p : best,
  );
}

export function mountSettingsPanel(
  root: HTMLElement,
  initial: {
    quoteRefreshMs: number;
    autostart: boolean;
    appVersion: string;
  },
  options: SettingsPanelOptions = {},
): SettingsPanelController {
  // Snap legacy/custom intervals onto the compact preset row for chip UI.
  let quoteRefreshMs = nearestRefreshPreset(initial.quoteRefreshMs);
  let autostart = initial.autostart;
  const appVersion = initial.appVersion.trim() || "unknown";
  let visible = false;
  let updatePhase: UpdatePhase = "idle";
  let updateVersion: string | null = null;
  let updateHint = "";
  let updateBusy = false;

  root.classList.add("settings-panel", "hidden");
  root.setAttribute("role", "dialog");
  root.setAttribute("aria-modal", "true");
  root.setAttribute("aria-label", "Settings");

  function render(): void {
    root.innerHTML = `
      <div class="settings-section">
        <div class="settings-label">Refresh</div>
        <div class="segmented refresh-segmented" role="group" aria-label="Refresh interval">
          ${REFRESH_PRESETS.map(
            (s) => `
            <button type="button" data-refresh="${s}" class="${s === quoteRefreshMs ? "active" : ""}" aria-pressed="${s === quoteRefreshMs ? "true" : "false"}">${formatRefresh(s)}</button>
          `,
          ).join("")}
        </div>
      </div>
      <div class="settings-section">
        <label class="settings-toggle" for="autostart-toggle">
          <span class="settings-toggle-title">Launch at login</span>
          <input type="checkbox" id="autostart-toggle" ${autostart ? "checked" : ""} />
          <span class="settings-switch" aria-hidden="true"></span>
        </label>
      </div>
      <div class="settings-update">
        <span class="settings-update-title">Version</span>
        <div class="settings-update-copy">
          <span class="settings-meta">v${escapeHtml(appVersion)}</span>
          <span class="settings-update-status" id="update-status"></span>
          <button type="button" class="icon-btn settings-update-btn" id="btn-check-update" aria-label="Check for updates" title="Check for updates">${ICON_DOWNLOAD}</button>
        </div>
      </div>
      <div class="settings-action-row">
        <button type="button" class="settings-debug" id="btn-diag" title="Copy diagnostic log for troubleshooting">Copy Log</button>
        <button type="button" class="settings-quit" id="btn-quit">Quit</button>
      </div>
      <div class="settings-live" id="settings-live" role="status" aria-live="polite" aria-atomic="true"></div>
    `;

    const liveRegion = root.querySelector("#settings-live") as HTMLElement | null;

    root.querySelectorAll<HTMLButtonElement>("[data-refresh]").forEach((btn) => {
      btn.addEventListener("click", () => {
        const ms = Number(btn.dataset.refresh);
        if (!Number.isFinite(ms)) return;
        void applyQuoteRefresh(ms);
      });
    });

    const autostartToggle = root.querySelector(
      "#autostart-toggle",
    ) as HTMLInputElement;
    autostartToggle.addEventListener("change", () => {
      void applyAutostart(autostartToggle.checked);
    });

    root.querySelector("#btn-diag")!.addEventListener("click", () => {
      void copyDiagnostics(
        root.querySelector("#btn-diag") as HTMLButtonElement,
        liveRegion,
      );
    });

    root.querySelector("#btn-quit")!.addEventListener("click", () => {
      void invoke("quit_app");
    });

    const updateBtn = root.querySelector("#btn-check-update") as HTMLButtonElement;
    updateBtn.addEventListener("click", () => {
      void runUpdateAction(updateBtn);
    });
    paintUpdateUi();
  }

  function paintUpdateUi(): void {
    const btn = root.querySelector<HTMLButtonElement>("#btn-check-update");
    const status = root.querySelector<HTMLElement>("#update-status");
    if (!btn || !status) return;

    btn.disabled = updateBusy;
    btn.classList.toggle("busy", updateBusy);
    btn.classList.toggle("update-available", updatePhase !== "idle");
    btn.classList.toggle("update-downloading", updatePhase === "downloading");
    btn.classList.toggle("update-ready", updatePhase === "ready");

    let title = "Check for updates";
    if (updatePhase === "ready" && updateVersion) {
      title = `Restart to install ${updateVersion}`;
      status.textContent = updateHint || `Update ${updateVersion} is ready`;
    } else if (updatePhase === "downloading" && updateVersion) {
      title = updateHint || `Downloading ${updateVersion}…`;
      status.textContent = title;
    } else if (updateBusy) {
      title = "Checking for updates…";
      status.textContent = updateHint || title;
    } else {
      status.textContent = updateHint;
    }
    btn.setAttribute("title", title);
    btn.setAttribute("aria-label", title);
  }

  async function runUpdateAction(btn: HTMLButtonElement): Promise<void> {
    const phaseAtClick = updatePhase;
    const version = updateVersion;

    if (phaseAtClick === "downloading") {
      updateHint = "Still downloading…";
      paintUpdateUi();
      return;
    }

    updateBusy = true;
    if (phaseAtClick === "ready") {
      updateHint = version ? `Restarting to install ${version}…` : "Restarting…";
    } else {
      updateHint = "Checking for updates…";
    }
    paintUpdateUi();

    try {
      const hasUpdate = await checkForUpdates();
      if (hasUpdate) {
        updateBusy = false;
        if (updatePhase === "idle") updatePhase = "downloading";
        updateHint = "Update found — downloading…";
        paintUpdateUi();
        return;
      }
      updatePhase = "idle";
      updateVersion = null;
      updateBusy = false;
      updateHint = "Already up to date";
      paintUpdateUi();
      window.setTimeout(() => {
        if (updatePhase !== "idle") return;
        updateHint = "";
        paintUpdateUi();
      }, 2500);
    } catch (err) {
      console.error("check_for_updates failed", err);
      updateBusy = false;
      updateHint = formatUpdateError(err).slice(0, 120);
      if (phaseAtClick === "ready" && version) {
        updatePhase = "ready";
        updateVersion = version;
      }
      paintUpdateUi();
      window.setTimeout(() => {
        if (!btn.isConnected) return;
        if (updatePhase === "ready") {
          updateHint = "";
        } else {
          updateHint = "";
        }
        paintUpdateUi();
      }, 4000);
    }
  }

  async function applyQuoteRefresh(ms: number): Promise<void> {
    quoteRefreshMs = nearestRefreshPreset(ms);
    render();
    try {
      const applied = await invoke<number>("set_quote_refresh_ms", {
        ms: quoteRefreshMs,
      });
      quoteRefreshMs = nearestRefreshPreset(applied);
      if (visible) render();
    } catch (err) {
      console.error("set_quote_refresh_ms failed", err);
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

  async function copyDiagnostics(
    btn: HTMLButtonElement,
    liveRegion: HTMLElement | null,
  ): Promise<void> {
    const original = "Copy Log";
    try {
      const text = await invoke<string>("get_diagnostics");
      await writeClipboard(text);
      btn.textContent = "Copied";
      btn.classList.add("is-done");
      if (liveRegion) liveRegion.textContent = "Diagnostic log copied";
      window.setTimeout(() => {
        if (!btn.isConnected) return;
        btn.textContent = original;
        btn.classList.remove("is-done");
        if (liveRegion) liveRegion.textContent = "";
      }, 1600);
    } catch (err) {
      console.error("copy diagnostics failed", err);
      btn.textContent = "Failed";
      btn.classList.remove("is-done");
      if (liveRegion) liveRegion.textContent = "Copy failed";
      window.setTimeout(() => {
        if (btn.isConnected) btn.textContent = original;
        if (liveRegion) liveRegion.textContent = "";
      }, 2000);
    }
  }

  // S3: Esc closes; Tab cycles within dialog
  root.addEventListener("keydown", (e) => {
    if (!visible) return;
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      options.onCloseRequest?.();
      return;
    }
    if (e.key !== "Tab") return;
    const list = focusableIn(root);
    if (list.length === 0) return;
    const first = list[0];
    const last = list[list.length - 1];
    const active = document.activeElement as HTMLElement | null;
    if (e.shiftKey) {
      if (active === first || !root.contains(active)) {
        e.preventDefault();
        last.focus();
      }
    } else if (active === last) {
      e.preventDefault();
      first.focus();
    }
  });

  render();

  const unlisteners: Array<() => void> = [];
  void listen<UpdateInfo>("update-available", (ev) => {
    if (!ev.payload?.version) return;
    updatePhase = "downloading";
    updateVersion = ev.payload.version;
    updateBusy = false;
    updateHint = `Downloading ${ev.payload.version}…`;
    paintUpdateUi();
  }).then((u) => unlisteners.push(u));

  void listen<DownloadProgress>("update-download-progress", (ev) => {
    const p = ev.payload;
    if (!p?.version || updatePhase === "ready") return;
    updatePhase = "downloading";
    updateVersion = p.version;
    updateBusy = false;
    if (p.content_length && p.content_length > 0) {
      const pct = Math.min(99, Math.round((p.received / p.content_length) * 100));
      updateHint = `Downloading ${p.version}… ${pct}%`;
    } else {
      updateHint = `Downloading ${p.version}…`;
    }
    paintUpdateUi();
  }).then((u) => unlisteners.push(u));

  void listen<UpdateInfo>("update-ready", (ev) => {
    if (!ev.payload?.version) return;
    updatePhase = "ready";
    updateVersion = ev.payload.version;
    updateBusy = false;
    updateHint = `Update ${ev.payload.version} is ready`;
    paintUpdateUi();
  }).then((u) => unlisteners.push(u));

  void listen("update-not-available", () => {
    if (updatePhase === "idle") return;
    updatePhase = "idle";
    updateVersion = null;
    updateBusy = false;
    paintUpdateUi();
  }).then((u) => unlisteners.push(u));

  void listen<string>("update-failed", (ev) => {
    const msg = typeof ev.payload === "string" ? ev.payload : "Update failed";
    updateBusy = false;
    if (updatePhase === "ready") {
      paintUpdateUi();
      return;
    }
    updatePhase = "idle";
    updateHint = msg.slice(0, 120);
    paintUpdateUi();
    window.setTimeout(() => {
      if (updatePhase !== "idle") return;
      updateHint = "";
      paintUpdateUi();
    }, 4000);
  }).then((u) => unlisteners.push(u));

  return {
    setQuoteRefreshMs: (s) => {
      quoteRefreshMs = nearestRefreshPreset(s);
      if (visible) render();
    },
    setAutostart: (enabled) => {
      autostart = enabled;
      if (visible) render();
    },
    show: () => {
      visible = true;
      root.classList.remove("hidden");
      root.hidden = false;
      render();
      requestAnimationFrame(() => {
        focusableIn(root)[0]?.focus();
      });
    },
    hide: () => {
      visible = false;
      root.classList.add("hidden");
      root.hidden = true;
      const live = root.querySelector("#settings-live");
      if (live) live.textContent = "";
    },
    isVisible: () => visible,
    destroy: () => {
      for (const u of unlisteners) u();
    },
  };
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** Format refresh interval stored as milliseconds. */
function formatRefresh(ms: number): string {
  const secs = ms / 1000;
  if (secs >= 60 && secs % 60 === 0) return `${secs / 60}m`;
  if (Number.isInteger(secs)) return `${secs}s`;
  // Sub-second presets (e.g. 250 → 0.25s)
  const trimmed = Number(secs.toFixed(2)).toString();
  return `${trimmed}s`;
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


