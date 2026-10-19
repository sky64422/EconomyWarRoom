import { invoke } from "@tauri-apps/api/core";
import { decodeLiquidFrame } from "./liquid-frame.ts";

const SAMPLE_INTERVAL_MS = 250;
const FIRST_SAMPLE_DELAY_MS = 120;
const STRIP_WIDTH = 3;
const DETAIL_INSET = 8;
const RAIL_INSET = 7;
const END_INSET = 24;
const MAX_SAMPLES = 64;
const LIGHT_ENTER = .68;
const LIGHT_LEAVE = .56;

/** Separate thresholds avoid flickering near the bright/dark boundary. */
export function liquidContrast(pixels: Uint8ClampedArray, previous = "dark"): string {
  const values: number[] = [];
  const stride = Math.max(1, Math.floor(pixels.length / 4 / MAX_SAMPLES)) * 4;
  for (let index = 0; index < pixels.length; index += stride) {
    values.push((pixels[index] * .2126 + pixels[index + 1] * .7152 + pixels[index + 2] * .0722) / 255);
  }
  if (!values.length) return previous;
  values.sort((a, b) => a - b);
  const median = values[Math.floor(values.length / 2)];
  return median >= (previous === "light" ? LIGHT_LEAVE : LIGHT_ENTER) ? "light" : "dark";
}

/** Samples painted empty margins, never feeds captured pixels into the material. */
export function mountLiquidContrast(rail: HTMLElement, paintedRail: HTMLElement, detail: HTMLElement) {
  let active = false, generation = 0, timer: ReturnType<typeof setTimeout> | undefined;
  let warned = false;
  async function sample(epoch: number) {
    try {
      for (const [slot, element, measured] of [[0, rail, paintedRail], [1, detail, detail]] as const) {
        if (!active || epoch !== generation || document.hidden) return;
        if (element.hidden || (slot === 0 && rail.classList.contains("is-folded"))) continue;
        const rect = measured.getBoundingClientRect();
        const horizontal = slot === 0 && ["top", "bottom"].includes(rail.dataset.edge ?? "");
        const ratio = devicePixelRatio || 1;
        const inset = slot === 0 ? RAIL_INSET : DETAIL_INSET;
        const x = Math.round((rect.x + (horizontal ? END_INSET : inset)) * ratio);
        const y = Math.round((rect.y + (horizontal ? inset : END_INSET)) * ratio);
        const width = horizontal ? Math.round((rect.width - END_INSET * 2) * ratio) : STRIP_WIDTH;
        const height = horizontal ? STRIP_WIDTH : Math.round((rect.height - END_INSET * 2) * ratio);
        if (x < 0 || y < 0 || width <= 0 || height <= 0) continue;
        const bytes = await invoke<ArrayBuffer>("capture_liquid_backdrop", { x, y, width, height, surface: slot, generation: epoch });
        if (!active || epoch !== generation) return;
        if (bytes.byteLength) {
          element.dataset.contrast = liquidContrast(decodeLiquidFrame(bytes, width, height), element.dataset.contrast);
        }
      }
    } catch (error) {
      if (active && epoch === generation && !warned) {
        console.warn("Liquid foreground contrast unavailable:", error);
        warned = true;
      }
    } finally {
      if (active && epoch === generation) timer = setTimeout(() => void sample(epoch), SAMPLE_INTERVAL_MS);
    }
  }
  return {
    setActive(value: boolean) {
      if (active === value) return;
      active = value;
      generation++;
      clearTimeout(timer);
      if (active) {
        warned = false;
        timer = setTimeout(() => void sample(generation), FIRST_SAMPLE_DELAY_MS);
      } else {
        delete rail.dataset.contrast;
        delete detail.dataset.contrast;
      }
    },
  };
}
