import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { LogicalPosition } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { arrowNudgeDelta, shouldNudgeWindow } from "./window-nudge";
import {
  focusSettingsButton,
  renderHeader,
  setHeaderBackdropInert,
  setSettingsButtonActive,
  setUsMarketStatus,
} from "./header";
import { applyPanelOpacity, mountSettingsPanel } from "./settings-panel";
import { mountWatchlist } from "./watchlist";
import type { PersistedState, Quote, Sparkline } from "./types";
import { DEFAULT_COLUMN_RATIOS } from "./types";

export async function mountApp(root: HTMLElement): Promise<void> {
  root.innerHTML = `
    <div class="panel" id="glass-panel">
      <div id="header-root"></div>
      <div class="content" id="content-root">
        <div id="settings-root"></div>
        <div id="watchlist-root"></div>
      </div>
    </div>
  `;

  const panel = root.querySelector("#glass-panel") as HTMLElement;
  const headerRoot = root.querySelector("#header-root") as HTMLElement;
  const watchlistRoot = root.querySelector("#watchlist-root") as HTMLElement;
  const settingsRoot = root.querySelector("#settings-root") as HTMLElement;

  let settingsOpen = false;

  const state = await invoke<PersistedState>("get_state");
  let appVersion = "unknown";
  try {
    appVersion = await getVersion();
  } catch (err) {
    console.warn("getVersion failed", err);
  }
  const opacity = state.settings.opacity ?? 0.92;
  // Stored as ms (legacy 1..=120 = whole seconds, normalized on Rust load).
  const quoteRefreshMs =
    state.settings.quote_refresh_ms ?? state.settings.quote_refresh_secs ?? 500;
  const autostart = state.settings.autostart ?? true;

  applyPanelOpacity(panel, opacity);

  const columnRatios = {
    ...DEFAULT_COLUMN_RATIOS,
    ...(state.settings.column_ratios ?? {}),
  };

  const watchlist = mountWatchlist(watchlistRoot, {
    columnRatios,
    onQuotesChanged: (quotes) =>
      setUsMarketStatus(headerRoot, quotes.map((quote) => quote.market_state)),
  });
  watchlist.setItems(state.watchlist ?? []);

  function setSettingsBackdropInert(on: boolean): void {
    watchlistRoot.inert = on;
    setHeaderBackdropInert(headerRoot, on);
  }

  function openSettings(): void {
    if (settingsOpen) return;
    settingsOpen = true;
    setSettingsBackdropInert(true);
    settings.show();
    panel.classList.add("settings-open");
    setSettingsButtonActive(headerRoot, true);
  }

  function closeSettings(): void {
    if (!settingsOpen) return;
    settingsOpen = false;
    settings.hide();
    panel.classList.remove("settings-open");
    setSettingsButtonActive(headerRoot, false);
    setSettingsBackdropInert(false);
    focusSettingsButton(headerRoot);
  }

  function toggleSettings(): void {
    // Overlay only: list fades under an opaque settings sheet. Window size stays put.
    if (settingsOpen) closeSettings();
    else openSettings();
  }

  const settings = mountSettingsPanel(
    settingsRoot,
    {
      quoteRefreshMs,
      autostart,
      appVersion,
    },
    {
      onCloseRequest: closeSettings,
    },
  );

  // Geometry hug tracks watchlist only; settings is an overlay (no size snap).
  await setupGeometryPersistence(panel);

  renderHeader(headerRoot, {
    onSettings: toggleSettings,
    opacity,
    onOpacityChange: (o) => {
      applyPanelOpacity(panel, o);
      void invoke("set_opacity", { opacity: o }).catch((err) => {
        console.error("set_opacity failed", err);
      });
    },
  });

  // Esc also closes when focus is on header chrome
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && settingsOpen) {
      e.preventDefault();
      closeSettings();
      return;
    }
    if (!shouldNudgeWindow(e, { settingsOpen })) return;
    const delta = arrowNudgeDelta(e.key, { shift: e.shiftKey, ctrl: e.ctrlKey });
    if (!delta) return;
    e.preventDefault();
    void nudgeWindow(delta.dx, delta.dy);
  });

  // Initial market data (best-effort; empty until scheduler fills)
  try {
    const quotes = await invoke<Quote[]>("get_quotes");
    watchlist.setQuotes(quotes);
  } catch {
    /* not ready */
  }
  try {
    const sparks = await invoke<Sparkline[]>("get_sparklines");
    watchlist.setSparklines(sparks);
  } catch {
    /* not ready */
  }
}

/**
 * True glass height (header + rows + +Add), ignoring window clamp.
 * Settings is an absolute overlay and must not inflate this measure.
 *
 * getBoundingClientRect under max-height:100% / watchlist max-height:100vh shrinks
 * with the window, so setMinSize was ratcheting down and hid +Add.
 */
function measureContentHugHeight(panel: HTMLElement): number {
  const liftSelectors = [
    panel,
    panel.querySelector<HTMLElement>(".content"),
    panel.querySelector<HTMLElement>("#watchlist-root"),
    panel.querySelector<HTMLElement>(".watchlist"),
    panel.querySelector<HTMLElement>(".watchlist-view"),
  ].filter((el): el is HTMLElement => Boolean(el));

  const saved = liftSelectors.map((el) => ({
    el,
    maxHeight: el.style.maxHeight,
    height: el.style.height,
    overflow: el.style.overflow,
  }));

  try {
    for (const { el } of saved) {
      el.style.maxHeight = "none";
      el.style.height = "max-content";
      el.style.overflow = "visible";
    }
    void panel.offsetHeight;
    // ceil + 1px slack: avoids subpixel overflow that shows a scrollbar at min size
    const hug = Math.ceil(panel.getBoundingClientRect().height) + 1;
    if (hug >= 80) return hug;

    // Structural fallback: header + list rows/empty + footer (settings is overlay).
    const header = panel.querySelector<HTMLElement>("#header-root");
    const rows = panel.querySelector<HTMLElement>(".watchlist-rows");
    const empty = panel.querySelector<HTMLElement>(".watchlist-empty");
    const footer = panel.querySelector<HTMLElement>(".watchlist-footer");
    const watchlist = panel.querySelector<HTMLElement>(".watchlist");
    let padGap = 20;
    if (watchlist) {
      const cs = getComputedStyle(watchlist);
      padGap =
        (parseFloat(cs.paddingTop) || 0) +
        (parseFloat(cs.paddingBottom) || 0) +
        (parseFloat(cs.gap) || 0);
    }
    return Math.ceil(
      (header?.offsetHeight ?? 38) +
        (rows?.scrollHeight ?? empty?.scrollHeight ?? 0) +
        (footer?.scrollHeight ?? 52) +
        padGap +
        2,
    );
  } finally {
    for (const s of saved) {
      s.el.style.maxHeight = s.maxHeight;
      s.el.style.height = s.height;
      s.el.style.overflow = s.overflow;
    }
  }
}

let nudgeChain: Promise<void> = Promise.resolve();

function nudgeWindow(dx: number, dy: number): Promise<void> {
  nudgeChain = nudgeChain.then(async () => {
    try {
      const win = getCurrentWindow();
      const factor = await win.scaleFactor();
      const pos = await win.outerPosition();
      await win.setPosition(
        new LogicalPosition(pos.x / factor + dx, pos.y / factor + dy),
      );
    } catch (err) {
      console.error("nudge window failed", err);
    }
  });
  return nudgeChain;
}

async function setupGeometryPersistence(
  panel: HTMLElement,
): Promise<(growIfNeeded: boolean) => void> {
  const noop = (_growIfNeeded: boolean) => {};
  try {
    const win = getCurrentWindow();
    let saveTimer: ReturnType<typeof setTimeout> | null = null;
    let contentMinTimer: ReturnType<typeof setTimeout> | null = null;
    /** Last content floor (logical height). Only changes when content changes. */
    let lastContentMinH = 0;
    const POLICY_MIN_W = 260;
    const CHROME_MIN_H = 120;

    const persist = async () => {
      try {
        const factor = await win.scaleFactor();
        const pos = await win.outerPosition();
        const size = await win.innerSize();
        const logical = {
          x: pos.x / factor,
          y: pos.y / factor,
          width: size.width / factor,
          height: size.height / factor,
        };
        await invoke("save_window_geometry", { geometry: logical });
      } catch (err) {
        console.error("save_window_geometry failed", err);
      }
    };

    /**
     * Publish content-hug min to OS. Does **not** run on every drag frame.
     * Snap height (grow or shrink) when content changes / boot / settings toggle.
     */
    const syncContentMinSize = async (opts: { growIfNeeded: boolean }) => {
      try {
        const contentH = measureContentHugHeight(panel);
        const minHeight = Math.max(CHROME_MIN_H, contentH);
        const grew = minHeight > lastContentMinH + 0.5;
        const shrank = minHeight < lastContentMinH - 0.5;
        const changed = Math.abs(minHeight - lastContentMinH) >= 1;
        // Snap when content grew/shrank (rows/footer), or explicit fit (boot).
        const fit = opts.growIfNeeded || grew || shrank;
        if (!changed && !opts.growIfNeeded) return;
        lastContentMinH = minHeight;
        await invoke("set_content_min_size", {
          width: POLICY_MIN_W,
          height: minHeight,
          grow_if_needed: fit,
        });
      } catch (err) {
        console.error("set_content_min_size failed", err);
      }
    };

    const schedulePersist = () => {
      if (saveTimer) clearTimeout(saveTimer);
      saveTimer = setTimeout(() => {
        void persist();
      }, 250);
    };

    const scheduleContentMin = (growIfNeeded: boolean) => {
      if (contentMinTimer) clearTimeout(contentMinTimer);
      contentMinTimer = setTimeout(() => {
        void syncContentMinSize({ growIfNeeded });
      }, 40);
    };

    // Content changes only — never remeasure min from window resize (that caused
    // min to track the drag and then bounce with setSize).
    const mutObs = new MutationObserver(() => {
      scheduleContentMin(false);
    });
    mutObs.observe(panel, {
      childList: true,
      subtree: true,
      characterData: true,
      attributes: true,
      attributeFilter: ["class", "style", "hidden"],
    });

    await win.onMoved(() => schedulePersist());
    // Persist only; OS min + Rust Resized clamp handle the hard wall.
    await win.onResized(() => schedulePersist());

    // Boot: measure after layout, install hard min, grow if restored size too small.
    requestAnimationFrame(() => {
      void syncContentMinSize({ growIfNeeded: true });
      window.setTimeout(() => {
        void syncContentMinSize({ growIfNeeded: true });
      }, 200);
    });

    return scheduleContentMin;
  } catch (err) {
    console.error("geometry persistence unavailable", err);
    return noop;
  }
}
