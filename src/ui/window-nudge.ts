/** Logical pixels per arrow key when focus is on the widget body. */
export const WINDOW_NUDGE_PX = 4;
/** Shift+arrow for a coarser hop. */
export const WINDOW_NUDGE_SHIFT_PX = 16;

export function arrowNudgeDelta(
  key: string,
  shift = false,
): { dx: number; dy: number } | null {
  const step = shift ? WINDOW_NUDGE_SHIFT_PX : WINDOW_NUDGE_PX;
  switch (key) {
    case "ArrowLeft":
      return { dx: -step, dy: 0 };
    case "ArrowRight":
      return { dx: step, dy: 0 };
    case "ArrowUp":
      return { dx: 0, dy: -step };
    case "ArrowDown":
      return { dx: 0, dy: step };
    default:
      return null;
  }
}

/** True when arrows should move the window, not another control. */
export function shouldNudgeWindow(
  e: KeyboardEvent,
  opts: { settingsOpen: boolean },
): boolean {
  if (opts.settingsOpen) return false;
  if (e.ctrlKey || e.altKey || e.metaKey) return false;
  if (!arrowNudgeDelta(e.key, e.shiftKey)) return false;
  const el = e.target;
  if (!(el instanceof Element)) return true;
  if (el.closest("input, textarea, select, [contenteditable=true], [role=slider]")) {
    return false;
  }
  return true;
}
