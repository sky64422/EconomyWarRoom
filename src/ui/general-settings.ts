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
  embedded?: boolean;
}

function focusableIn(container: HTMLElement): HTMLElement[] {
  const sel =
    'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';
  return Array.from(container.querySelectorAll<HTMLElement>(sel)).filter(
    (el) => !el.hasAttribute("disabled") && el.offsetParent !== null,
  );
}

/** Settings choices in milliseconds; legacy stored intervals stay readable. */
const REFRESH_PRESETS = [1000, 10_000, 60_000] as const;

const ICON_DOWNLOAD = `<svg class="icon-svg" width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true" focusable="false"><path d="M8 2.5v7.5M5 7.25 8 10.25 11 7.25" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/><path d="M3 12.5h10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>`;
const ICON_RELOAD = '<svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M14 8a6 6 0 1 1-1.76-4.24L14 5.5M14 2v3.5h-3.5"/></svg>';
const ICON_CHECK = '<svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m3.5 8.5 3 3 6-7"/></svg>';
const ICON_COPY = '<svg width="13" height="13" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" aria-hidden="true"><rect x="5" y="5" width="8" height="8" rx="1.5"/><path d="M3.5 10.5H3A1.5 1.5 0 0 1 1.5 9V3A1.5 1.5 0 0 1 3 1.5h6A1.5 1.5 0 0 1 10.5 3v.5"/></svg>';
const ICON_QUIT = '<svg width="13" height="13" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" aria-hidden="true"><path d="M8 2v5.5m3.8-3.2a5.2 5.2 0 1 1-7.6 0"/></svg>';
const UPDATE_CURRENT = '최신 버전입니다';
const FEEDBACK_RESET_MS = 2500;

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
  let quoteRefreshMs = initial.quoteRefreshMs;
  let autostart = initial.autostart;
  const appVersion = initial.appVersion.trim() || "unknown";
  let visible = false;
  let saving = false;
  let updatePhase: UpdatePhase = "idle";
  let updateVersion: string | null = null;
  let updateHint = "";
  let updateBusy = false;
  let copyTimer: ReturnType<typeof setTimeout> | undefined;

  root.classList.add("settings-panel", "hidden");
  root.setAttribute("role", "dialog");
  root.setAttribute("aria-modal", "true");
  root.setAttribute("aria-label", "Settings");

  function render(): void {
    root.innerHTML = `
      <div class="settings-section">
        <div class="settings-label">시세 갱신</div>
        <div class="segmented refresh-segmented" role="group" aria-label="갱신 간격">
          ${REFRESH_PRESETS.map(
            (s) => `
            <button type="button" data-refresh="${s}" class="${s === quoteRefreshMs ? "active" : ""}" aria-pressed="${s === quoteRefreshMs ? "true" : "false"}">${formatRefresh(s)}</button>
          `,
          ).join("")}
        </div>
      </div>
      <div class="settings-section">
        <div class="settings-label">시스템</div>
        <div class="settings-group"><label class="settings-toggle" for="autostart-toggle">
          <span class="settings-toggle-title">Windows 시작 시 실행</span>
          <input type="checkbox" id="autostart-toggle" ${autostart ? "checked" : ""} />
          <span class="settings-switch" aria-hidden="true"></span>
        </label></div>
      </div>
      <div class="settings-section">
        <div class="settings-label">앱 정보</div>
        <div class="settings-group"><div class="settings-update">
        <span class="settings-update-title">버전</span>
        <div class="settings-update-copy">
          <span class="settings-meta">v${escapeHtml(appVersion)}</span>
          <button type="button" class="icon-btn settings-update-btn" id="btn-check-update" aria-label="업데이트 확인"><span class="feedback-icon-stack" aria-hidden="true"><span class="update-icon-reload">${ICON_RELOAD}</span><span class="update-icon-check">${ICON_CHECK}</span><span class="update-icon-restart">${ICON_DOWNLOAD}</span></span></button>
        </div>
        </div></div>
        <span class="settings-update-status" id="update-status" role="status"></span>
      </div>
      <div class="settings-action-row">
        <button type="button" class="settings-pill-action settings-debug" id="btn-diag"><span class="feedback-icon-stack" aria-hidden="true"><span class="diag-copy-icon">${ICON_COPY}</span><span class="diag-check-icon">${ICON_CHECK}</span></span><span class="diag-label">로그 복사</span></button>
        <button type="button" class="settings-pill-action settings-quit is-quit" id="btn-quit">${ICON_QUIT}<span>종료</span></button>
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
    if (saving) root.querySelectorAll<HTMLInputElement | HTMLButtonElement>('button,input').forEach(el=>el.disabled=true);
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

    let title = "업데이트 확인";
    if (updatePhase === "ready" && updateVersion) {
      title = `${updateVersion} 설치 후 다시 시작`;
      status.textContent = updateHint || `${updateVersion} 설치 준비 완료`;
    } else if (updatePhase === "downloading" && updateVersion) {
      title = updateHint || `${updateVersion} 다운로드 중…`;
      status.textContent = title;
    } else if (updateBusy) {
      title = "업데이트 확인 중…";
      status.textContent = updateHint || title;
    } else {
      status.textContent = updateHint;
    }
    btn.setAttribute("aria-label", title);
    btn.dataset.feedback=updateBusy || updatePhase==='downloading' ? 'loading' : updateHint===UPDATE_CURRENT ? 'done' : 'idle';
  }

  async function runUpdateAction(btn: HTMLButtonElement): Promise<void> {
    const phaseAtClick = updatePhase;
    const version = updateVersion;

    if (phaseAtClick === "downloading") {
      updateHint = "다운로드 중…";
      paintUpdateUi();
      return;
    }

    updateBusy = true;
    if (phaseAtClick === "ready") {
      updateHint = version ? `${version} 설치 후 다시 시작 중…` : "다시 시작 중…";
    } else {
      updateHint = "업데이트 확인 중…";
    }
    paintUpdateUi();

    try {
      const hasUpdate = await checkForUpdates();
      if (hasUpdate) {
        updateBusy = false;
        if (updatePhase === "idle") updatePhase = "downloading";
        updateHint = "새 버전 다운로드 중…";
        paintUpdateUi();
        return;
      }
      updatePhase = "idle";
      updateVersion = null;
      updateBusy = false;
      updateHint = UPDATE_CURRENT;
      paintUpdateUi();
      window.setTimeout(() => {
        if (updatePhase !== "idle") return;
        updateHint = "";
        paintUpdateUi();
      }, FEEDBACK_RESET_MS);
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
    if (saving) return;
    const previous=quoteRefreshMs;
    saving=true;
    quoteRefreshMs=nearestRefreshPreset(ms);
    render();
    try { quoteRefreshMs=nearestRefreshPreset(await invoke<number>('set_quote_refresh_ms',{ms:quoteRefreshMs})); }
    catch(error) { quoteRefreshMs=previous;console.error('set_quote_refresh_ms failed',error); }
    finally { saving=false;if(visible)render(); }
  }

  async function applyAutostart(enabled: boolean): Promise<void> {
    if(saving)return;
    const previous=autostart;saving=true;autostart=enabled;render();
    try { await invoke('set_autostart',{enabled}); }
    catch(error) { autostart=previous;console.error('set_autostart failed',error); }
    finally { saving=false;if(visible)render(); }
  }

  async function copyDiagnostics(
    btn: HTMLButtonElement,
    liveRegion: HTMLElement | null,
  ): Promise<void> {
    if(btn.disabled)return;
    clearTimeout(copyTimer);
    const label=btn.querySelector<HTMLElement>('.diag-label')!;
    btn.disabled=true;btn.dataset.feedback='loading';
    label.textContent='복사 중…';
    try {
      const text = await invoke<string>("get_diagnostics");
      await writeClipboard(text);
      label.textContent='복사됨';
      btn.dataset.feedback='done';
      btn.classList.add("is-done");
      if (liveRegion) liveRegion.textContent = "진단 로그를 복사했습니다.";
      copyTimer=window.setTimeout(() => {
        if (!btn.isConnected) return;
        label.textContent='로그 복사';
        btn.dataset.feedback='idle';
        btn.classList.remove("is-done");
        if (liveRegion) liveRegion.textContent = "";
      }, 1600);
    } catch (err) {
      console.error("copy diagnostics failed", err);
      label.textContent='재시도';
      btn.dataset.feedback='error';
      btn.classList.remove("is-done");
      if (liveRegion) liveRegion.textContent = "복사 실패";
      copyTimer=window.setTimeout(() => {
        if (btn.isConnected){label.textContent='로그 복사';btn.dataset.feedback='idle';}
        if (liveRegion) liveRegion.textContent = "";
      }, 2000);
    } finally { btn.disabled=false; }
  }

  render();

  const unlisteners: Array<() => void> = [];
  void listen<UpdateInfo>("update-available", (ev) => {
    if (!ev.payload?.version) return;
    updatePhase = "downloading";
    updateVersion = ev.payload.version;
    updateBusy = false;
    updateHint = `${ev.payload.version} 다운로드 중…`;
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
      updateHint = `${p.version} 다운로드 중… ${pct}%`;
    } else {
      updateHint = `${p.version} 다운로드 중…`;
    }
    paintUpdateUi();
  }).then((u) => unlisteners.push(u));

  void listen<UpdateInfo>("update-ready", (ev) => {
    if (!ev.payload?.version) return;
    updatePhase = "ready";
    updateVersion = ev.payload.version;
    updateBusy = false;
    updateHint = `${ev.payload.version} 설치 준비 완료`;
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
    const msg = typeof ev.payload === "string" ? ev.payload : "업데이트 실패";
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
      quoteRefreshMs = s;
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
        if (!options.embedded) focusableIn(root)[0]?.focus();
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
      clearTimeout(copyTimer);
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
  return `${ms / 1000}s`;
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
