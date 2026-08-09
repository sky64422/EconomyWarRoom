import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface HeaderHandlers {
  onSettings: () => void;
}

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

type UpdatePhase = "idle" | "downloading" | "ready";

/** Stroke SVG icons — avoid platform glyph variance (P2). */
const ICON_REFRESH = `<svg class="icon-svg" width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true" focusable="false"><path d="M13.5 8A5.5 5.5 0 1 1 11.2 3.6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/><path d="M13.5 3v3.2H10.3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
const ICON_SETTINGS = `<svg class="icon-svg" width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true" focusable="false"><circle cx="8" cy="8" r="2.25" stroke="currentColor" stroke-width="1.5"/><path d="M8 1.75v1.5M8 12.75v1.5M1.75 8h1.5M12.75 8h1.5M3.4 3.4l1.06 1.06M11.54 11.54l1.06 1.06M12.6 3.4l-1.06 1.06M4.46 11.54l-1.06 1.06" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>`;
const ICON_HIDE = `<svg class="icon-svg" width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true" focusable="false"><path d="M3.5 8h9" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>`;

function setBtnChrome(btn: HTMLButtonElement, title: string): void {
  btn.setAttribute("title", title);
  btn.setAttribute("aria-label", title);
}

export function renderHeader(root: HTMLElement, handlers: HeaderHandlers): void {
  root.innerHTML = `
    <div class="header" data-tauri-drag-region>
      <div class="title">WarRoom</div>
      <div class="header-actions">
        <button type="button" class="icon-btn" id="btn-update" aria-label="Check for updates" title="Check for updates">${ICON_REFRESH}</button>
        <button type="button" class="icon-btn" id="btn-settings" aria-label="Settings" title="Settings">${ICON_SETTINGS}</button>
        <button type="button" class="icon-btn" id="btn-hide" aria-label="Hide" title="Hide">${ICON_HIDE}</button>
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
  let phase: UpdatePhase = "idle";
  let pendingVersion: string | null = null;

  const setPhase = (next: UpdatePhase, version?: string) => {
    phase = next;
    if (version) pendingVersion = version;
    updateBtn.classList.toggle("update-available", next !== "idle");
    updateBtn.classList.toggle("update-ready", next === "ready");
    updateBtn.classList.toggle("update-downloading", next === "downloading");

    if (next === "ready" && pendingVersion) {
      setBtnChrome(updateBtn, `Update ${pendingVersion} ready — click to restart`);
      updateBtn.dataset.updateVersion = pendingVersion;
    } else if (next === "downloading" && pendingVersion) {
      setBtnChrome(updateBtn, `Downloading ${pendingVersion}…`);
      updateBtn.dataset.updateVersion = pendingVersion;
    } else {
      setBtnChrome(updateBtn, "Check for updates");
      delete updateBtn.dataset.updateVersion;
      pendingVersion = null;
    }
  };

  updateBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    void runUpdateAction(updateBtn, () => phase, setPhase);
  });

  void listen<UpdateInfo>("update-available", (ev) => {
    const info = ev.payload;
    if (!info?.version) return;
    setPhase("downloading", info.version);
  });

  void listen<DownloadProgress>("update-download-progress", (ev) => {
    const p = ev.payload;
    if (!p?.version || phase === "ready") return;
    pendingVersion = p.version;
    updateBtn.classList.add("update-available", "update-downloading");
    // S4: keep title and aria-label in sync during download
    if (p.content_length && p.content_length > 0) {
      const pct = Math.min(99, Math.round((p.received / p.content_length) * 100));
      setBtnChrome(updateBtn, `Downloading ${p.version}… ${pct}%`);
    } else {
      setBtnChrome(updateBtn, `Downloading ${p.version}…`);
    }
  });

  void listen<UpdateInfo>("update-ready", (ev) => {
    const info = ev.payload;
    if (!info?.version) return;
    setPhase("ready", info.version);
  });

  void listen("update-not-available", () => {
    if (phase !== "idle") setPhase("idle");
  });

  void listen<string>("update-failed", (ev) => {
    const msg = typeof ev.payload === "string" ? ev.payload : "Update failed";
    setBtnChrome(updateBtn, msg.slice(0, 120));
    updateBtn.classList.remove("update-downloading");
    window.setTimeout(() => {
      if (!updateBtn.isConnected) return;
      if (phase === "ready" && pendingVersion) {
        setPhase("ready", pendingVersion);
      } else {
        setPhase("idle");
      }
    }, 4000);
  });
}

export function setSettingsButtonActive(root: HTMLElement, active: boolean): void {
  const btn = root.querySelector("#btn-settings");
  if (btn) btn.classList.toggle("active", active);
}

export function focusSettingsButton(root: HTMLElement): void {
  root.querySelector<HTMLButtonElement>("#btn-settings")?.focus();
}

/** Drop update/hide from Tab/AT while settings is open; settings toggle stays usable. */
export function setHeaderBackdropInert(root: HTMLElement, inert: boolean): void {
  for (const id of ["btn-update", "btn-hide"] as const) {
    const el = root.querySelector<HTMLElement>(`#${id}`);
    if (el) el.inert = inert;
  }
}

async function runUpdateAction(
  btn: HTMLButtonElement,
  getPhase: () => UpdatePhase,
  setPhase: (p: UpdatePhase, version?: string) => void,
): Promise<void> {
  const phaseAtClick = getPhase();
  const version = btn.dataset.updateVersion;

  btn.disabled = true;
  btn.classList.add("busy");

  if (phaseAtClick === "downloading") {
    setBtnChrome(btn, "Still downloading…");
    window.setTimeout(() => {
      if (!btn.isConnected) return;
      btn.disabled = false;
      btn.classList.remove("busy");
      if (version) setBtnChrome(btn, `Downloading ${version}…`);
    }, 1500);
    return;
  }

  if (phaseAtClick === "ready") {
    setBtnChrome(btn, version ? `Restarting to install ${version}…` : "Restarting…");
  } else {
    setBtnChrome(btn, "Checking for updates…");
  }

  try {
    const hasUpdate = await invoke<boolean>("check_for_updates");
    if (hasUpdate) {
      setBtnChrome(btn, "Update installed — restarting…");
      return;
    }
    setPhase("idle");
    setBtnChrome(btn, "Already up to date");
    window.setTimeout(() => {
      if (btn.isConnected) {
        setBtnChrome(btn, "Check for updates");
        btn.disabled = false;
        btn.classList.remove("busy");
      }
    }, 2500);
  } catch (err) {
    console.error("check_for_updates failed", err);
    const msg =
      typeof err === "string"
        ? err
        : err && typeof err === "object" && "message" in err
          ? String((err as { message: unknown }).message)
          : "Update check failed";
    setBtnChrome(btn, msg.slice(0, 120));
    window.setTimeout(() => {
      if (!btn.isConnected) return;
      btn.disabled = false;
      btn.classList.remove("busy");
      if (phaseAtClick === "ready" && version) {
        setPhase("ready", version);
      } else {
        setBtnChrome(btn, "Check for updates");
      }
    }, 4000);
  }
}
