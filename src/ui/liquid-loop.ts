/** Serialize capture, discard old-theme images, and release native exclusion on stop. */
export function createLiquidLoop<T>(io: {
  request: (tick: (time: number) => void) => number;
  cancel: (id: number) => void;
  interval: (time: number) => number;
  capture: () => Promise<T>;
  paint: (frame: T, time: number) => void;
  dispose: (frame: T) => void;
  release: () => Promise<void>;
  fail: (error: unknown) => void;
}) {
  let requested = false, enabled = false, destroyed = false, busy = false;
  let releasePending = false, epoch = 0, frame = 0, due = -Infinity, previousInterval = -1;
  function schedule() {
    if (enabled && !destroyed && !frame && previousInterval !== Infinity) {
      frame = io.request(tick);
    }
  }
  async function release() {
    if (!releasePending) return;
    releasePending = false;
    try { await io.release(); } catch (error) { io.fail(error); }
  }
  function tick(now: number) {
    frame = 0;
    if (!enabled || destroyed) return;
    const interval = io.interval(now);
    if (interval !== previousInterval) { due = now; previousInterval = interval; }
    schedule();
    if (busy || now < due) return;
    // Preserve fractional frame time; missed refreshes must not accumulate queued reads.
    due = Math.max(due, now - interval) + interval;
    busy = true;
    const generation = epoch;
    void (async () => {
      try {
        const result = await io.capture();
        try { if (enabled && !destroyed && epoch === generation) io.paint(result, now); }
        finally { io.dispose(result); }
      } catch (error) {
        if (generation === epoch) {
          enabled = false; epoch++;
          io.cancel(frame); frame = 0;
          io.fail(error);
        }
        releasePending = true;
      } finally {
        await release();
        busy = false;
        schedule();
      }
    })();
  }
  function setEnabled(value: boolean) {
    if (destroyed || requested === value) return;
    requested = enabled = value; epoch++;
    if (value) { due = -Infinity; previousInterval = -1; schedule(); return; }
    io.cancel(frame); frame = 0; releasePending = true;
    if (!busy) {
      busy = true;
      void release().finally(() => { busy = false; schedule(); });
    }
  }
  return {
    setEnabled,
    refresh() {
      due = -Infinity;
      previousInterval = -1;
      if (enabled && !destroyed && !frame) frame = io.request(tick);
    },
    destroy() { setEnabled(false); destroyed = true; io.cancel(frame); frame = 0; },
  };
}
