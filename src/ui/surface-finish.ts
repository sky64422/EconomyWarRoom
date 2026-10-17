// Material interaction inspired by Lucian UX/UI 001 and 052 (MIT, Seokwoooo).
// https://github.com/Seokwoooo/lucian-uxui-prompt

const FOIL_SWING = 24;

/** Move light across the surface without transforming its layout or hit region. */
export function mountSurfaceFinish(surfaces: HTMLElement[]) {
  const events = new AbortController();
  const options = { signal: events.signal };
  const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
  let frame = 0;
  let enabled = false;
  let pending: { surface: HTMLElement; x: number; y: number } | undefined;

  function rest(surface: HTMLElement) {
    surface.style.removeProperty("--finish-x");
    surface.style.removeProperty("--finish-y");
    surface.style.removeProperty("--finish-rotation");
    surface.style.removeProperty("--finish-shift");
    surface.removeAttribute("data-finish-active");
  }

  function reset() {
    cancelAnimationFrame(frame);
    frame = 0;
    pending = undefined;
    surfaces.forEach(rest);
  }

  for (const surface of surfaces) {
    surface.addEventListener("pointermove", event => {
      if (!enabled || reducedMotion.matches || event.pointerType === "touch" || event.buttons) return;
      const rect = surface.getBoundingClientRect();
      if (!rect.width || !rect.height) return;
      pending = {
        surface,
        x: Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width)),
        y: Math.min(1, Math.max(0, (event.clientY - rect.top) / rect.height)),
      };
      if (frame) return;
      frame = requestAnimationFrame(() => {
        frame = 0;
        if (!pending) return;
        const { surface: target, x, y } = pending;
        pending = undefined;
        target.dataset.finishActive = "true";
        target.style.setProperty("--finish-x", `${x * 100}%`);
        target.style.setProperty("--finish-y", `${y * 100}%`);
        target.style.setProperty("--finish-rotation", `${(x - y) * FOIL_SWING}deg`);
        target.style.setProperty("--finish-shift", `${(x - 0.5) * 100}%`);
      });
    }, options);
    surface.addEventListener("pointerleave", () => {
      if (pending?.surface === surface) pending = undefined;
      rest(surface);
    }, options);
  }
  reducedMotion.addEventListener("change", () => {
    if (!reducedMotion.matches) return;
    reset();
  }, options);

  return {
    setEnabled(value: boolean) {
      enabled = value;
      if (!value) reset();
    },
    destroy() {
      events.abort();
      reset();
    },
  };
}
