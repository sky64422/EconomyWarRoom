import { invoke } from "@tauri-apps/api/core";
import { mountLiquidContrast } from "./liquid-contrast.ts";

const MATERIAL_BLUR = { glass: [4, 10], liquid: [3, 10] } as const;
const GEOMETRY_PROPERTY = /^(transform|translate|scale|width|height|left|right|top|bottom|inset|borderRadius|clipPath)/;
interface NativeSurface {
  slot: number; x: number; y: number; width: number; height: number;
  edge: "left" | "right" | "top" | "bottom" | null; radius: number; blur: number;
}

/** The compositor owns live pixels; the web layer only synchronizes geometry and scrims. */
export function mountLiquidGlass(shell: HTMLElement, rail: HTMLElement, detail: HTMLElement) {
  const notchSurface = rail.querySelector<HTMLElement>(".notch-surface") ?? rail;
  const contrast = mountLiquidContrast(rail, notchSurface, detail);
  const entries = [rail, detail].map((surface, slot) => {
    const layer = document.createElement("div");
    layer.className = "liquid-layer";
    layer.setAttribute("aria-hidden", "true");
    layer.hidden = true;
    if (slot === 0) surface.querySelector(".notch-finish")!.before(layer);
    else surface.prepend(layer);
    return { surface, layer, slot };
  });
  let enabled = false, nativeVisible = true, destroyed = false, failed = false;
  let session: number | null = null, revision = 0, busy = false, clearRequired = false;
  let desired: NativeSurface[] = [], desiredKey = "[]", appliedKey = "[]", frame = 0;

  function hide() {
    contrast.setActive(false);
    for (const entry of entries) entry.layer.hidden = true;
    delete shell.dataset.liquidStatus;
  }

  async function flush() {
    if (busy || failed) return;
    busy = true;
    try {
      while (clearRequired || desiredKey !== appliedKey) {
        if (session === null) {
          if (!desired.length && !clearRequired) break;
          session = await invoke<number>("begin_liquid_backdrop");
        }
        if (clearRequired) {
          clearRequired = false;
          await invoke("release_liquid_backdrop", { session, revision: ++revision });
          appliedKey = "[]";
          continue;
        }
        const surfaces = desired, key = desiredKey;
        if (!surfaces.length) {
          await invoke("release_liquid_backdrop", { session, revision: ++revision });
        } else {
          await invoke("sync_liquid_backdrop", { session, revision: ++revision, surfaces });
        }
        appliedKey = key;
        // Closing or changing geometry while IPC is pending invalidates its acknowledgement.
        if (!destroyed && key === desiredKey && !clearRequired && surfaces.length) {
          for (const entry of entries) entry.layer.hidden = !surfaces.some(s => s.slot === entry.slot);
          shell.dataset.liquidStatus = "ready";
          contrast.setActive(shell.dataset.theme === "liquid");
        }
      }
    } catch (error) {
      failed = true;
      hide();
      shell.dataset.liquidStatus = "unavailable";
      console.error("Liquid backdrop:", error);
      if (session !== null) {
        try {
          await invoke("release_liquid_backdrop", { session, revision: ++revision });
          appliedKey = "[]";
        } catch (cleanupError) { console.error("Liquid backdrop cleanup:", cleanupError); }
      }
      clearRequired = false;
    } finally {
      busy = false;
      if (!failed && (clearRequired || desiredKey !== appliedKey)) void flush();
    }
  }

  function request(surfaces: NativeSurface[]) {
    const key = JSON.stringify(surfaces);
    if (key === desiredKey) return;
    desired = surfaces; desiredKey = key;
    if (!surfaces.length) {
      hide();
      // Keep this barrier even if an immediate reopen coalesces to the previous geometry.
      clearRequired ||= session !== null || busy;
    } else {
      for (const entry of entries) {
        if (!surfaces.some(s => s.slot === entry.slot)) entry.layer.hidden = true;
      }
    }
    void flush();
  }

  function sync() {
    const theme = shell.dataset.theme;
    if (destroyed || !enabled || !nativeVisible || document.hidden || shell.hidden
      || (theme !== "glass" && theme !== "liquid") || getComputedStyle(shell).display === "none") {
      request([]); return;
    }
    const ratio = devicePixelRatio || 1;
    const surfaces: NativeSurface[] = [];
    for (const { surface, slot } of entries) {
      if (surface.hidden || (slot === 0 && rail.classList.contains("is-folded"))) continue;
      const style = getComputedStyle(surface);
      if (style.display === "none" || style.visibility === "hidden") continue;
      const paintedSurface = slot === 0 ? notchSurface : surface;
      const rect = paintedSurface.getBoundingClientRect();
      const width = Math.round(rect.width * ratio), height = Math.round(rect.height * ratio);
      if (width <= 0 || height <= 0) continue;
      const edge = rail.dataset.edge;
      const vertical = edge === "left" || edge === "right";
      const depth = parseFloat(getComputedStyle(rail).getPropertyValue("--notch-depth"))
        || (vertical ? paintedSurface.offsetWidth : paintedSurface.offsetHeight) || 0;
      const length = (vertical ? paintedSurface.offsetHeight : paintedSurface.offsetWidth) || 0;
      const notchRadius = length > 0 ? depth / 2 * (vertical ? rect.height : rect.width) / length * ratio : 0;
      surfaces.push({ slot, x: Math.round(rect.x * ratio), y: Math.round(rect.y * ratio), width, height,
        edge: slot === 0 && (edge === "left" || edge === "right" || edge === "top" || edge === "bottom") ? edge : null,
        radius: slot === 0 ? notchRadius : (parseFloat(style.borderRadius) || 0) * ratio,
        blur: MATERIAL_BLUR[theme][slot] * ratio });
    }
    request(surfaces);
    contrast.setActive(theme === "liquid" && shell.dataset.liquidStatus === "ready");
  }

  function animatingGeometry() {
    return shell.getAnimations({ subtree: true }).some(animation => {
      if (animation.playState !== "running" || !(animation.effect instanceof KeyframeEffect)
        || animation.effect.getTiming().iterations === Infinity) return false;
      return animation.effect.getKeyframes().some(key => Object.keys(key).some(name => GEOMETRY_PROPERTY.test(name)));
    });
  }
  function schedule() {
    if (destroyed || frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0; sync();
      if (animatingGeometry()) schedule();
    });
  }

  const events = new AbortController();
  document.addEventListener("visibilitychange", () => { sync(); schedule(); }, { signal: events.signal });
  window.addEventListener("resize", schedule, { signal: events.signal });
  for (const event of ["transitionrun", "transitionstart", "transitionend", "transitioncancel", "animationstart", "animationend", "animationcancel"]) {
    shell.addEventListener(event, schedule, { signal: events.signal });
  }
  const observer = new MutationObserver(() => { sync(); schedule(); });
  observer.observe(shell, { attributes: true, attributeFilter: ["data-theme", "style", "class", "hidden"] });
  observer.observe(rail, { attributes: true, attributeFilter: ["class", "style", "data-edge"] });
  observer.observe(detail, { attributes: true, attributeFilter: ["hidden", "class", "style"] });
  const resize = new ResizeObserver(schedule);
  resize.observe(rail); resize.observe(detail);
  if (notchSurface !== rail) resize.observe(notchSurface);

  return {
    setVisible(value: boolean) { nativeVisible = value; sync(); schedule(); },
    setEnabled(value: boolean) {
      if (destroyed) return;
      enabled = value;
      if (failed && value) { failed = false; desiredKey = ""; }
      sync(); schedule();
    },
    destroy() {
      if (destroyed) return;
      destroyed = true;
      observer.disconnect(); resize.disconnect(); events.abort();
      cancelAnimationFrame(frame); frame = 0;
      request([]);
      entries.forEach(entry => entry.layer.remove());
    },
  };
}
