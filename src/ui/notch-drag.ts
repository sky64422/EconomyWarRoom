import type { NotchLayout, NotchPlacement } from "./types.ts";
import { logCaptureEvent } from "./capture-log.ts";
import { invoke } from "@tauri-apps/api/core";

export interface DragNotice {
  id: number;
  active: boolean;
  finished: boolean;
  placement: NotchPlacement;
  error: string | null;
}
type PointerCapture = Pick<Element, "setPointerCapture" | "hasPointerCapture" | "releasePointerCapture">;
interface DragHandlers {
  begin: (id: number) => Promise<void>;
  finish: (id: number, cancel: boolean) => Promise<void>;
  onStart: () => void;
  onNotice: (event: DragNotice) => void;
  onState: (state: { armed: boolean; dragging: boolean }) => void;
  fail: (error: unknown) => void;
}

/** Owns the gesture session; native physical coordinates still own placement. */
export function createNotchDrag(handlers: DragHandlers) {
  let sessionId: number | null = null;
  let nextId = Date.now();
  let pressed: number | null = null;
  let captured: { element: PointerCapture; id: number } | null = null;
  let start = { x: 0, y: 0 };
  let suppressClick = false;
  let armed = false;
  let dragging = false;
  let disposed = false;
  let chain: Promise<void> = Promise.resolve();
  const paint = () => handlers.onState({ armed, dragging });
  function releaseCapture() {
    const previous = captured;
    captured = null;
    try {
      if (previous?.element.hasPointerCapture(previous.id)) {
        logCaptureEvent("POINTER", `releasePointerCapture id=${previous.id}`);
        previous.element.releasePointerCapture(previous.id);
      }
    } catch (e) {
      logCaptureEvent("POINTER", `releasePointerCapture error: ${e}`);
    }
  }
  function finish(cancel: boolean) {
    const id = sessionId;
    if (id === null) return;
    chain = chain.then(() => handlers.finish(id, cancel)).catch(handlers.fail);
  }
  return {
    get sessionId() { return sessionId; },
    get dragging() { return dragging; },
    cancel() {
      if (sessionId === null && pressed === null) return;
      logCaptureEvent("DRAG", `cancel() called (sessionId=${sessionId}, pressed=${pressed})`);
      armed = dragging = false;
      pressed = null;
      releaseCapture();
      paint();
      finish(true);
      sessionId = null;
      void invoke("release_liquid_backdrop");
      void invoke("yield_notch_focus", { forwardEscape: false });
    },
    start(pointer: { id: number; x: number; y: number; capture: PointerCapture }): boolean {
      if (disposed || sessionId !== null) return false;
      const id = ++nextId;
      sessionId = id;
      pressed = pointer.id;
      suppressClick = false;
      start = { x: pointer.x, y: pointer.y };
      handlers.onStart();
      captured = { element: pointer.capture, id: pointer.id };
      logCaptureEvent("DRAG", `start() sessionId=${id} pointerId=${pointer.id} acquired setPointerCapture`);
      captured.element.setPointerCapture(pointer.id);
      chain = chain.then(() => handlers.begin(id)).catch(error => {
        if (sessionId === id) {
          sessionId = null;
          pressed = null;
          releaseCapture();
          armed = false;
          paint();
        }
        handlers.fail(error);
      });
      return true;
    },
    move(pointerId: number, x: number, y: number, threshold: number) {
      if (pointerId !== pressed) return;
      if (Math.hypot(x - start.x, y - start.y) >= threshold) {
        suppressClick = true;
        armed = true;
        paint();
      }
    },
    end(pointerId: number, cancel: boolean, released = false) {
      if (pointerId !== pressed) return;
      if (released) pressed = null;
      armed = false;
      paint();
      finish(cancel);
      if (released) {
        void invoke("yield_notch_focus", { forwardEscape: false });
      }
    },
    finish,
    notice(event: DragNotice) {
      if (disposed || event.id !== sessionId) return;
      if (event.active) suppressClick = true;
      dragging = event.active && !event.finished;
      if (event.finished) {
        armed = false;
        sessionId = null;
        pressed = null;
        releaseCapture();
      }
      paint();
      handlers.onNotice(event);
    },
    consumeClick(detail: number) {
      const suppress = suppressClick && detail !== 0;
      suppressClick = false;
      return suppress;
    },
    settled: () => chain,
    destroy() {
      if (disposed) return;
      finish(true);
      disposed = true;
      sessionId = null;
      pressed = null;
      releaseCapture();
      armed = dragging = false;
      paint();
    },
  };
}

/** Bind browser input and release every registration with the gesture owner. */
export function mountNotchDrag(
  rail: HTMLElement,
  getLayout: () => NotchLayout | undefined,
  handlers: Omit<DragHandlers, "onState">,
) {
  const events = new AbortController();
  let snapTimer: ReturnType<typeof setTimeout> | null = null;
  let wasDragging = false;
  const triggerSnapSlosh = () => {
    rail.classList.remove("is-snapped");
    void rail.offsetWidth;
    rail.classList.add("is-snapped");
    if (snapTimer) clearTimeout(snapTimer);
    snapTimer = setTimeout(() => rail.classList.remove("is-snapped"), 450);
  };
  const drag = createNotchDrag({
    ...handlers,
    onNotice: event => {
      if (event.finished) {
        void invoke("release_liquid_backdrop");
        void invoke("yield_notch_focus", { forwardEscape: false });
      }
      if (wasDragging && event.finished) {
        triggerSnapSlosh();
        wasDragging = false;
      }
      handlers.onNotice(event);
    },
    onState: ({ armed, dragging }) => {
      rail.classList.toggle("is-drag-armed", armed);
      rail.classList.toggle("is-dragging", dragging);
      if (dragging) wasDragging = true;
    },
  });
  const options = { signal: events.signal };
  rail.addEventListener("pointerdown", e => {
    if (e.button !== 0 || !getLayout()) return;
    logCaptureEvent("POINTER", `pointerdown id=${e.pointerId} button=${e.button}`);
    drag.start({ id: e.pointerId, x: e.screenX, y: e.screenY, capture: e.target as Element });
  }, options);
  rail.addEventListener("pointermove", e => {
    const layout = getLayout();
    if (layout) drag.move(e.pointerId, e.screenX, e.screenY, layout.metrics.drag_threshold);
  }, options);
  rail.addEventListener("pointerup", e => {
    logCaptureEvent("POINTER", `pointerup id=${e.pointerId}`);
    drag.end(e.pointerId, false, true);
  }, options);
  rail.addEventListener("pointercancel", e => {
    logCaptureEvent("POINTER", `pointercancel id=${e.pointerId} -> cancel drag`);
    drag.cancel();
  }, options);
  rail.addEventListener("lostpointercapture", e => {
    logCaptureEvent("POINTER", `lostpointercapture id=${e.pointerId} buttons=${e.buttons} -> cancel drag`);
    drag.cancel();
  }, options);
  window.addEventListener("blur", () => {
    logCaptureEvent("FOCUS", "window blur -> drag.cancel()");
    drag.cancel();
  }, options);
  window.addEventListener("keydown", e => {
    if (e.key === "Escape" || e.key === "PrintScreen") {
      logCaptureEvent("KEY", `${e.key} pressed -> drag.cancel() & yield focus`);
      drag.cancel();
      (typeof document !== "undefined" ? (document.activeElement as HTMLElement | null) : null)?.blur();
      void invoke("yield_notch_focus", { forwardEscape: e.key === "Escape" });
    } else if (e.key === "Alt" || (e.altKey && e.key === "`")) {
      logCaptureEvent("KEY", `${e.key} (Alt combo) pressed -> yield focus`);
      void invoke("yield_notch_focus", { forwardEscape: false });
    } else if (e.altKey || e.ctrlKey || e.metaKey) {
      logCaptureEvent("KEY", `modifier keydown key=${e.key} alt=${e.altKey} ctrl=${e.ctrlKey}`);
    }
  }, { ...options, capture: true });
  rail.addEventListener("click", e => {
    if (drag.consumeClick(e.detail)) {
      e.preventDefault();
      e.stopImmediatePropagation();
    }
  }, { ...options, capture: true });
  return {
    get sessionId() { return drag.sessionId; },
    get dragging() { return drag.dragging; },
    cancel: drag.cancel,
    finish: drag.finish,
    notice: drag.notice,
    destroy() {
      events.abort();
      drag.destroy();
    },
  };
}
