import { invoke } from "@tauri-apps/api/core";
import { createLiquidLoop } from "./liquid-loop.ts";
import { decodeLiquidFrame, liquidWash } from "./liquid-frame.ts";
import { liquidContrast } from "./liquid-contrast.ts";
import { logCaptureEvent } from "./capture-log.ts";

const NS = "http://www.w3.org/2000/svg";
const MATERIALS = {
  glass: { bezel: 20, displacement: 18, dispersion: [.985, 1, 1.015], blur: [4, 10], saturation: 1.04, brightness: 1.02 },
  liquid: { bezel: 28, displacement: 0, dispersion: [.93, 1, 1.07], blur: [2.5, 8], saturation: 1.08, brightness: 1.03 },
};

const IDLE_FRAME_MS = 1000 / 30;
const DRAG_FRAME_MS = 1000 / 60;
const REDUCED_FRAME_MS = 250;
const SETTLING_FRAME_MS = 100;
const SETTLING_AFTER_MS = 150;
const STATIC_AFTER_MS = 400;
const STATIC_HEARTBEAT_MS = 1000;

interface SurfaceLayer {
  surface: HTMLElement;
  layer: HTMLDivElement;
  canvas: HTMLCanvasElement;
  filter: SVGFilterElement;
  key: string;
  slot: number;
  generation: number;
  effect: string;
  scratch: HTMLCanvasElement | null;
}

interface CapturedFrame {
  entry: SurfaceLayer;
  pixels: Uint8ClampedArray;
  width: number;
  height: number;
  key: string;
  generation: number;
}

export function mountLiquidGlass(shell: HTMLElement, rail: HTMLElement, detail: HTMLElement) {
  let material = MATERIALS.glass;
  let materialTheme: "glass" | "liquid" | null = null;
  const defs = document.createElementNS(NS, "svg");
  defs.setAttribute("class", "liquid-filter-defs");
  defs.setAttribute("aria-hidden", "true");
  const defsHost = document.createElementNS(NS, "defs");
  defs.append(defsHost);
  shell.append(defs);

  const motion = matchMedia("(prefers-reduced-motion: reduce)");
  const entries: SurfaceLayer[] = [rail, detail].map((surface, index) => {
    const layer = document.createElement("div");
    layer.className = "liquid-layer";
    layer.setAttribute("aria-hidden", "true");
    layer.hidden = true;
    const canvas = document.createElement("canvas");
    layer.append(canvas);

    if (index === 0) surface.querySelector(".notch-finish")!.before(layer);
    else surface.prepend(layer);

    const filter = document.createElementNS(NS, "filter");
    filter.id = `liquid-lens-${index}`;
    filter.setAttribute("color-interpolation-filters", "sRGB");
    filter.setAttribute("x", "0");
    filter.setAttribute("y", "0");
    filter.setAttribute("width", "100%");
    filter.setAttribute("height", "100%");
    filter.innerHTML = `<feImage result="map" preserveAspectRatio="none"/>
      <feDisplacementMap in="SourceGraphic" in2="map" scale="${material.displacement * material.dispersion[0]}" xChannelSelector="R" yChannelSelector="G" result="r"/>
      <feColorMatrix in="r" type="matrix" values="1 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 1 0" result="red"/>
      <feDisplacementMap in="SourceGraphic" in2="map" scale="${material.displacement}" xChannelSelector="R" yChannelSelector="G" result="g"/>
      <feColorMatrix in="g" type="matrix" values="0 0 0 0 0  0 1 0 0 0  0 0 0 0 0  0 0 0 1 0" result="green"/>
      <feDisplacementMap in="SourceGraphic" in2="map" scale="${material.displacement * material.dispersion[2]}" xChannelSelector="R" yChannelSelector="G" result="b"/>
      <feColorMatrix in="b" type="matrix" values="0 0 0 0 0  0 0 0 0 0  0 0 1 0 0  0 0 0 1 0" result="blue"/>
      <feBlend in="red" in2="green" mode="screen" result="rg"/><feBlend in="rg" in2="blue" mode="screen"/>`;
    defsHost.append(filter);

    const effect = "";
    canvas.style.filter = effect;
    return { surface, layer, canvas, filter, key: "", slot: index, generation: 0, effect, scratch: null };
  });

  let enabled = false, destroyed = false, nativeVisible = true;
  let lastChangedAt = performance.now(), lastVisible = false;
  let lastDetailHidden = detail.hidden, lastDragging = false;

  const clear = (releaseBuffers = false) => entries.forEach(entry => {
    entry.layer.hidden = true;
    entry.generation++;
    delete entry.surface.dataset.contrast;
    if (!releaseBuffers) return;
    entry.key = "";
    entry.canvas.width = entry.canvas.height = 0;
    entry.canvas.style.filter = "";
    entry.filter.querySelector("feImage")?.removeAttribute("href");
    if (entry.scratch) {
      entry.scratch.width = entry.scratch.height = 0;
      entry.scratch = null;
    }
  });

  const loop = createLiquidLoop<CapturedFrame[]>({
    request: cb => requestAnimationFrame(cb),
    cancel: id => cancelAnimationFrame(id),
    interval: now => {
      if (motion.matches) return REDUCED_FRAME_MS;
      if (rail.classList.contains("is-dragging")) return DRAG_FRAME_MS;
      const unchangedFor = now - lastChangedAt;
      return unchangedFor >= STATIC_AFTER_MS
        ? STATIC_HEARTBEAT_MS
        : unchangedFor >= SETTLING_AFTER_MS
          ? SETTLING_FRAME_MS
          : IDLE_FRAME_MS;
    },
    capture,
    paint: (frames, now) => {
      let painted = false;
      frames.forEach(({ entry, pixels, width, height, key, generation }) => {
        if (entry.key !== key || entry.generation !== generation || entry.surface.hidden) return;
        const ctx = entry.canvas.getContext("2d", { alpha: false })!;
        const image = new ImageData(pixels, width, height);

        if (materialTheme === "glass") {
          entry.layer.style.setProperty("--liquid-wash", liquidWash(pixels).toFixed(3));
        } else if (materialTheme === "liquid") {
          entry.surface.dataset.contrast = liquidContrast(pixels, entry.surface.dataset.contrast);
        }

        if (width === entry.canvas.width && height === entry.canvas.height) {
          ctx.putImageData(image, 0, 0);
        } else {
          const scratch = entry.scratch ?? (entry.scratch = document.createElement("canvas"));
          if (scratch.width !== width || scratch.height !== height) {
            scratch.width = width;
            scratch.height = height;
          }
          scratch.getContext("2d", { alpha: false })!.putImageData(image, 0, 0);
          ctx.drawImage(scratch, 0, 0, entry.canvas.width, entry.canvas.height);
        }
        entry.layer.hidden = false;
        painted = true;
      });

      if (painted) {
        shell.dataset.liquidStatus = "ready";
      }
      if (!rail.classList.contains("is-dragging")) {
        void invoke("release_liquid_backdrop");
        if (!painted && now - lastChangedAt >= SETTLING_AFTER_MS) {
          lastChangedAt = 0;
        }
      }
    },
    dispose: frames => {
      frames.length = 0;
    },
    release: async () => {
      await invoke("release_liquid_backdrop");
    },
    fail: error => {
      clear(true);
      shell.dataset.liquidStatus = "unavailable";
      console.error("Liquid backdrop:", error);
    },
  });

  function sync() {
    const visible = nativeVisible && enabled && !document.hidden && (!rail.classList.contains("is-folded") || !detail.hidden);
    const dragging = rail.classList.contains("is-dragging");
    if (visible && (!lastVisible || detail.hidden !== lastDetailHidden || dragging !== lastDragging)) {
      lastChangedAt = (lastDragging && !dragging)
        ? performance.now() - STATIC_AFTER_MS
        : performance.now();
      loop.refresh();
    }
    lastVisible = visible;
    lastDetailHidden = detail.hidden;
    lastDragging = dragging;
    if (!visible) clear(!enabled || !nativeVisible || document.hidden);
    loop.setEnabled(visible);
  }

  const events = new AbortController();
  document.addEventListener("visibilitychange", sync, { signal: events.signal });
  const observer = new MutationObserver(records => {
    for (const record of records) {
      if (record.target === detail && record.attributeName === "hidden") entries[1].generation++;
    }
    sync();
  });
  observer.observe(rail, { attributes: true, attributeFilter: ["class"] });
  observer.observe(detail, { attributes: true, attributeFilter: ["hidden"] });
  window.addEventListener("pointerdown", () => {
    logCaptureEvent("POINTER", "window pointerdown");
    loop.refresh();
  }, { signal: events.signal });
  rail.addEventListener("pointerenter", () => {
    logCaptureEvent("POINTER", "rail pointerenter");
    loop.refresh();
  }, { signal: events.signal });
  window.addEventListener("blur", () => {
    logCaptureEvent("FOCUS", "window blur");
    lastChangedAt = 0;
    void invoke("release_liquid_backdrop");
    setTimeout(() => {
      if (enabled && nativeVisible && !document.hidden) {
        lastChangedAt = performance.now();
        loop.refresh();
      }
    }, 800);
  }, { signal: events.signal });
  window.addEventListener("focus", () => {
    logCaptureEvent("FOCUS", "window focus");
    lastChangedAt = performance.now();
    loop.refresh();
  }, { signal: events.signal });

  function mapFor(entry: SurfaceLayer, width: number, height: number) {
    entry.canvas.style.filter = entry.effect;
    entry.canvas.width = width;
    entry.canvas.height = height;

    if (material.displacement === 0) return;
    const contentScale = Number(getComputedStyle(entry.surface).getPropertyValue("--notch-scale")) || 1;
    const bezel = material.bezel * contentScale;
    entry.filter.querySelectorAll("feDisplacementMap").forEach((node, index) => {
      const dispersion = material.dispersion[index];
      node.setAttribute("scale", String(material.displacement * contentScale * dispersion));
    });
    const mask = document.createElement("canvas");
    mask.width = width;
    mask.height = height;
    const ctx = mask.getContext("2d", { willReadFrequently: true })!;
    ctx.fillStyle = "#fff";
    if (entry.surface === rail) {
      const path = rail.querySelector<SVGPathElement>("#notch-outline")!;
      const m = path.transform.baseVal.consolidate()?.matrix;
      if (m) ctx.setTransform(m.a, m.b, m.c, m.d, m.e, m.f);
      ctx.fill(new Path2D(path.getAttribute("d")!));
      ctx.resetTransform();
    } else {
      ctx.beginPath();
      ctx.roundRect(0, 0, width, height, 16 * contentScale);
      ctx.fill();
    }
    const pixels = ctx.getImageData(0, 0, width, height).data;
    const dist = new Float32Array(width * height);
    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        const i = y * width + x;
        dist[i] = pixels[i * 4 + 3] < 128 ? 0 : Math.min(x + 1, y + 1, width - x, height - y, bezel + 2);
      }
    }
    for (let y = 1; y < height; y++) {
      for (let x = 1; x < width; x++) {
        const i = y * width + x;
        dist[i] = Math.min(dist[i], dist[i - 1] + 1, dist[i - width] + 1, dist[i - width - 1] + Math.SQRT2);
      }
    }
    for (let y = height - 2; y >= 0; y--) {
      for (let x = width - 2; x >= 0; x--) {
        const i = y * width + x;
        dist[i] = Math.min(dist[i], dist[i + 1] + 1, dist[i + width] + 1, dist[i + width + 1] + Math.SQRT2);
      }
    }
    const map = ctx.createImageData(width, height);
    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        const i = y * width + x, d = dist[i];
        const gx = (dist[y * width + Math.min(width - 1, x + 1)] - dist[y * width + Math.max(0, x - 1)]) / 2;
        const gy = (dist[Math.min(height - 1, y + 1) * width + x] - dist[Math.max(0, y - 1) * width + x]) / 2;
        const length = Math.hypot(gx, gy) || 1;
        const bend = d > 0 && d < bezel ? (1 - d / bezel) ** 2 : 0;
        map.data[i * 4] = 128 + gx / length * bend * 116;
        map.data[i * 4 + 1] = 128 + gy / length * bend * 116;
        map.data[i * 4 + 2] = 128;
        map.data[i * 4 + 3] = 255;
      }
    }
    ctx.putImageData(map, 0, 0);
    entry.filter.querySelector("feImage")!.setAttribute("href", mask.toDataURL());
  }

  async function capture(): Promise<CapturedFrame[]> {
    const frames: CapturedFrame[] = [];
    try {
      for (const entry of entries) {
        if (entry.surface.hidden || (entry.surface === rail && rail.classList.contains("is-folded"))) continue;
        const rect = entry.surface.getBoundingClientRect();
        const width = Math.round(rect.width), height = Math.round(rect.height);
        if (!width || !height) continue;
        const ratio = devicePixelRatio || 1;
        const key = `${width}:${height}:${rail.dataset.edge}:${ratio}:${shell.style.getPropertyValue("--notch-scale")}`;
        if (key !== entry.key) {
          mapFor(entry, width, height);
          entry.key = key;
          entry.generation++;
        }
        const generation = entry.generation;
        const x = Math.max(0, Math.round(rect.x * ratio));
        const y = Math.max(0, Math.round(rect.y * ratio));
        const w = Math.min(Math.round(width * ratio), Math.round(innerWidth * ratio) - x);
        const h = Math.min(Math.round(height * ratio), Math.round(innerHeight * ratio) - y);
        if (w <= 0 || h <= 0) continue;

        const dragging = rail.classList.contains("is-dragging");
        const data = await invoke<ArrayBuffer>("capture_liquid_backdrop", {
          x, y, width: w, height: h, surface: entry.slot, generation, dragging,
        });
        if (!data.byteLength) continue;
        if (entry.key !== key || entry.generation !== generation) continue;
        const pixels = decodeLiquidFrame(data, w, h);
        frames.push({ entry, pixels, width: w, height: h, key, generation });
      }
      return frames;
    } catch (error) {
      frames.length = 0;
      throw error;
    }
  }

  return {
    relayout() {
      entries.forEach(entry => {
        entry.key = "";
        entry.generation++;
      });
      lastChangedAt = performance.now();
      loop.refresh();
    },
    setVisible(value: boolean) {
      nativeVisible = value;
      sync();
    },
    setEnabled(value: boolean) {
      const nextTheme = shell.dataset.theme === "liquid" ? "liquid" : "glass";
      if (destroyed || (enabled === value && materialTheme === nextTheme)) return;
      if (materialTheme !== nextTheme) {
        loop.setEnabled(false);
        clear(true);
        materialTheme = nextTheme;
        material = MATERIALS[nextTheme];
        for (const entry of entries) {
          entry.effect = `${material.displacement === 0 ? "" : `url(#${entry.filter.id}) `}blur(${material.blur[entry.slot]}px) saturate(${material.saturation}) brightness(${material.brightness})`;
        }
      }
      enabled = value;
      delete shell.dataset.liquidStatus;
      sync();
    },
    destroy() {
      destroyed = true;
      loop.destroy();
      observer.disconnect();
      events.abort();
      clear(true);
      entries.forEach(entry => entry.layer.remove());
      defs.remove();
      void invoke("destroy_liquid_backdrop");
    },
  };
}
