import type { NotchLayout, NotchPlacement } from "./types";

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
    if (previous?.element.hasPointerCapture(previous.id)) previous.element.releasePointerCapture(previous.id);
  }
  function finish(cancel: boolean) {
    const id = sessionId;
    if (id === null) return;
    chain = chain.then(() => handlers.finish(id, cancel)).catch(handlers.fail);
  }
  return {
    get sessionId() { return sessionId; },
    get dragging() { return dragging; },
    start(pointer: { id: number; x: number; y: number; capture: PointerCapture }): boolean {
      if (disposed || sessionId !== null) return false;
      const id = ++nextId;
      sessionId = id;
      pressed = pointer.id;
      suppressClick = false;
      start = { x: pointer.x, y: pointer.y };
      handlers.onStart();
      captured = { element: pointer.capture, id: pointer.id };
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
    drag.start({ id: e.pointerId, x: e.screenX, y: e.screenY, capture: e.target as Element });
  }, options);
  rail.addEventListener("pointermove", e => {
    const layout = getLayout();
    if (layout) drag.move(e.pointerId, e.screenX, e.screenY, layout.metrics.drag_threshold);
  }, options);
  rail.addEventListener("pointerup", e => drag.end(e.pointerId, false, true), options);
  rail.addEventListener("pointercancel", e => drag.end(e.pointerId, true), options);
  rail.addEventListener("lostpointercapture", e => drag.end(e.pointerId, e.buttons !== 0), options);
  rail.addEventListener("click", e => {
    if (drag.consumeClick(e.detail)) {
      e.preventDefault();
      e.stopImmediatePropagation();
    }
  }, { ...options, capture: true });
  return {
    get sessionId() { return drag.sessionId; },
    get dragging() { return drag.dragging; },
    finish: drag.finish,
    notice: drag.notice,
    destroy() {
      events.abort();
      if (snapTimer) clearTimeout(snapTimer);
      rail.classList.remove("is-snapped");
      drag.destroy();
    },
  };
}
