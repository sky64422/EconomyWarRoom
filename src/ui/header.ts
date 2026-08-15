import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  meterFillPct,
  opacityToPct,
  OPACITY_MAX_PCT,
  OPACITY_MIN_PCT,
  OPACITY_STEP_PCT,
  pctToOpacity,
  snapOpacityPct,
} from "./opacity";
import type { DownloadProgress, UpdateInfo, UpdatePhase } from "./updates";

export interface HeaderHandlers {
  onSettings: () => void;
  opacity: number;
  onOpacityChange: (opacity: number) => void;
}

type UsMarketStatus = "live" | "pre" | "post" | "closed" | "unknown";

function resolveUsMarketStatus(marketStates: Array<string | null | undefined>): UsMarketStatus {
  const states = marketStates
    .map((state) => state?.toLowerCase().trim())
    .filter((state): state is string => Boolean(state));
  if (states.some((state) => state === "regular")) return "live";
  if (states.some((state) => state === "pre" || state === "prepre")) return "pre";
  if (states.some((state) => state === "post" || state === "postpost")) return "post";
  return states.length > 0 ? "closed" : "unknown";
}

export function setUsMarketStatus(
  root: HTMLElement,
  marketStates: Array<string | null | undefined>,
): void {
  const statusEl = root.querySelector<HTMLElement>("#us-market-status");
  const labelEl = root.querySelector<HTMLElement>("#us-market-status-label");
  if (!statusEl || !labelEl) return;

  const status = resolveUsMarketStatus(marketStates);
  const labels: Record<UsMarketStatus, string> = {
    live: "LIVE",
    pre: "PRE",
    post: "POST",
    closed: "CLOSED",
    unknown: "--",
  };
  const descriptions: Record<UsMarketStatus, string> = {
    live: "US market is live",
    pre: "US pre-market is open",
    post: "US after-hours market is open",
    closed: "US market is closed",
    unknown: "US market status is unavailable",
  };
  statusEl.className = "market-status is-" + status;
  statusEl.setAttribute("aria-label", descriptions[status]);
  labelEl.textContent = labels[status];
}

/** Stroke SVG icons — avoid platform glyph variance (P2). */
const ICON_SETTINGS = `<svg class="icon-svg" width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden="true" focusable="false"><circle cx="12" cy="12" r="3" stroke="currentColor" stroke-width="2"/><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" stroke="currentColor" stroke-width="2" stroke-linejoin="round"/></svg>`;
const ICON_HIDE = `<svg class="icon-svg" width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true" focusable="false"><path d="M3.5 8h9" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>`;

export function renderHeader(root: HTMLElement, handlers: HeaderHandlers): void {
  const initialPct = opacityToPct(handlers.opacity);
  root.innerHTML = `
    <div class="header" data-tauri-drag-region>
      <div class="header-leading">
        <div class="title">WarRoom</div>
        <div class="market-status is-unknown" id="us-market-status" role="status" aria-live="polite" aria-label="US market status is unavailable">
          <span class="market-status-dot" aria-hidden="true"></span>
          <span id="us-market-status-label">--</span>
        </div>
      </div>
      <div class="header-opacity">
        <div class="opacity-slider" id="opacity-slider" role="slider"
          aria-label="Opacity" aria-valuemin="${OPACITY_MIN_PCT}"
          aria-valuemax="${OPACITY_MAX_PCT}" aria-valuenow="${initialPct}"
          aria-valuetext="${initialPct}%" tabindex="0"
          title="Opacity ${initialPct}%"
          style="--opacity-fill: ${meterFillPct(initialPct)}%">
          <span class="opacity-slider-rail" aria-hidden="true">
            <span class="opacity-slider-track">
              <span class="opacity-slider-fill"></span>
            </span>
            <span class="opacity-slider-thumb"></span>
          </span>
        </div>
      </div>
      <div class="header-actions">
        <button type="button" class="icon-btn" id="btn-settings" aria-label="Settings" title="Settings">${ICON_SETTINGS}</button>
        <button type="button" class="icon-btn" id="btn-hide" aria-label="Hide" title="Hide">${ICON_HIDE}</button>
      </div>
    </div>
  `;

  bindOpacitySlider(root, initialPct, handlers.onOpacityChange);

  root.querySelector("#btn-hide")!.addEventListener("click", (e) => {
    e.stopPropagation();
    void invoke("hide_widget");
  });

  root.querySelector("#btn-settings")!.addEventListener("click", (e) => {
    e.stopPropagation();
    handlers.onSettings();
  });

  bindSettingsUpdateBadge(root);
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
  for (const id of ["btn-hide", "opacity-slider"] as const) {
    const el = root.querySelector<HTMLElement>(`#${id}`);
    if (el) el.inert = inert;
  }
}

export function setHeaderOpacity(root: HTMLElement, opacity: number): void {
  const slider = root.querySelector<HTMLElement>("#opacity-slider");
  if (!slider) return;
  const snapped = snapOpacityPct(opacityToPct(opacity));
  slider.style.setProperty("--opacity-fill", `${meterFillPct(snapped)}%`);
  slider.setAttribute("aria-valuenow", String(snapped));
  slider.setAttribute("aria-valuetext", `${snapped}%`);
  slider.title = `Opacity ${snapped}%`;
  slider.dataset.pct = String(snapped);
}

function bindOpacitySlider(
  root: HTMLElement,
  initialPct: number,
  onChange: (o: number) => void,
): void {
  const slider = root.querySelector("#opacity-slider") as HTMLElement | null;
  if (!slider) return;

  slider.dataset.pct = String(snapOpacityPct(initialPct));
  let dragging = false;

  const currentPct = (): number => {
    const raw = Number(slider.dataset.pct);
    return Number.isFinite(raw) ? raw : snapOpacityPct(initialPct);
  };

  const paint = (next: number, emit: boolean) => {
    const snapped = snapOpacityPct(next);
    slider.style.setProperty("--opacity-fill", `${meterFillPct(snapped)}%`);
    slider.setAttribute("aria-valuenow", String(snapped));
    slider.setAttribute("aria-valuetext", `${snapped}%`);
    slider.title = `Opacity ${snapped}%`;
    if (snapped === currentPct()) return;
    slider.dataset.pct = String(snapped);
    if (emit) onChange(pctToOpacity(snapped));
  };

  const pctFromClientX = (clientX: number): number => {
    const rail = slider.querySelector(".opacity-slider-rail") as HTMLElement | null;
    const rect = (rail ?? slider).getBoundingClientRect();
    if (rect.width <= 0) return currentPct();
    const t = Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
    return OPACITY_MIN_PCT + t * (OPACITY_MAX_PCT - OPACITY_MIN_PCT);
  };

  slider.addEventListener("pointerdown", (e) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    dragging = true;
    slider.classList.add("is-dragging");
    slider.setPointerCapture(e.pointerId);
    paint(pctFromClientX(e.clientX), true);
  });

  slider.addEventListener("pointermove", (e) => {
    if (!dragging) return;
    paint(pctFromClientX(e.clientX), true);
  });

  const endDrag = (e: PointerEvent) => {
    if (!dragging) return;
    dragging = false;
    slider.classList.remove("is-dragging");
    if (slider.hasPointerCapture(e.pointerId)) {
      slider.releasePointerCapture(e.pointerId);
    }
  };
  slider.addEventListener("pointerup", endDrag);
  slider.addEventListener("pointercancel", endDrag);

  slider.addEventListener("keydown", (e) => {
    if (e.key === "ArrowLeft" || e.key === "ArrowDown") {
      e.preventDefault();
      paint(currentPct() - OPACITY_STEP_PCT, true);
    } else if (e.key === "ArrowRight" || e.key === "ArrowUp") {
      e.preventDefault();
      paint(currentPct() + OPACITY_STEP_PCT, true);
    } else if (e.key === "Home") {
      e.preventDefault();
      paint(OPACITY_MIN_PCT, true);
    } else if (e.key === "End") {
      e.preventDefault();
      paint(OPACITY_MAX_PCT, true);
    }
  });

  slider.addEventListener(
    "wheel",
    (e) => {
      e.preventDefault();
      const axis = Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY;
      if (axis === 0) return;
      paint(currentPct() - Math.sign(axis) * OPACITY_STEP_PCT, true);
    },
    { passive: false },
  );
}

function setSettingsUpdateBadge(btn: HTMLElement, phase: UpdatePhase, version: string | null): void {
  btn.classList.toggle("update-available", phase !== "idle");
  btn.classList.toggle("update-downloading", phase === "downloading");
  btn.classList.toggle("update-ready", phase === "ready");
  if (phase === "ready" && version) {
    btn.setAttribute("title", `Update ${version} ready — open Settings`);
    btn.setAttribute("aria-label", `Settings, update ${version} ready`);
  } else if (phase === "downloading" && version) {
    btn.setAttribute("title", `Downloading ${version}…`);
    btn.setAttribute("aria-label", `Settings, downloading ${version}`);
  } else {
    btn.setAttribute("title", "Settings");
    btn.setAttribute("aria-label", "Settings");
  }
}

function bindSettingsUpdateBadge(root: HTMLElement): void {
  const settingsBtn = root.querySelector("#btn-settings") as HTMLButtonElement | null;
  if (!settingsBtn) return;

  let phase: UpdatePhase = "idle";
  let pendingVersion: string | null = null;

  const setPhase = (next: UpdatePhase, version?: string) => {
    phase = next;
    if (version) pendingVersion = version;
    if (next === "idle") pendingVersion = null;
    setSettingsUpdateBadge(settingsBtn, phase, pendingVersion);
  };

  void listen<UpdateInfo>("update-available", (ev) => {
    if (!ev.payload?.version) return;
    setPhase("downloading", ev.payload.version);
  });

  void listen<DownloadProgress>("update-download-progress", (ev) => {
    const p = ev.payload;
    if (!p?.version || phase === "ready") return;
    setPhase("downloading", p.version);
  });

  void listen<UpdateInfo>("update-ready", (ev) => {
    if (!ev.payload?.version) return;
    setPhase("ready", ev.payload.version);
  });

  void listen("update-not-available", () => {
    if (phase !== "idle") setPhase("idle");
  });

  void listen<string>("update-failed", () => {
    if (phase === "ready") {
      setPhase("ready", pendingVersion ?? undefined);
      return;
    }
    setPhase("idle");
  });
}
