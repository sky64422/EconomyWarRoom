import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { renderHeader, setSettingsButtonActive } from "./header";
import {
  applyPanelOpacity,
  applyThemeToDocument,
  mountSettingsPanel,
} from "./settings-panel";
import { mountWatchlist } from "./watchlist";
import type { PersistedState, Quote, Sparkline, ThemeMode } from "./types";

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
  const theme: ThemeMode = state.settings.theme ?? "system";
  const opacity = state.settings.opacity ?? 0.92;

  applyThemeToDocument(theme);
  applyPanelOpacity(panel, opacity);

  const watchlist = mountWatchlist(watchlistRoot);
  watchlist.setItems(state.watchlist ?? []);

  const settings = mountSettingsPanel(
    settingsRoot,
    { theme, opacity },
    {
      onThemeChange: (t) => applyThemeToDocument(t),
      onOpacityChange: (o) => applyPanelOpacity(panel, o),
    },
  );

  function toggleSettings(): void {
    settingsOpen = !settingsOpen;
    // Keep watchlist visible; settings is a compact sheet above the list so tall
    // windows still grow the quote area (not empty settings chrome).
    if (settingsOpen) {
      settings.show();
      panel.classList.add("settings-open");
    } else {
      settings.hide();
      panel.classList.remove("settings-open");
    }
    setSettingsButtonActive(headerRoot, settingsOpen);
  }

  renderHeader(headerRoot, { onSettings: toggleSettings });

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

  await setupGeometryPersistence(panel);
}

async function setupGeometryPersistence(panel: HTMLElement): Promise<void> {
  try {
    const win = getCurrentWindow();
    let saveTimer: ReturnType<typeof setTimeout> | null = null;
    let minSizeTimer: ReturnType<typeof setTimeout> | null = null;

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

    const updateMinSize = async () => {
      try {
        const rect = panel.getBoundingClientRect();
        const minWidth = Math.max(260, Math.ceil(rect.width));
        const minHeight = Math.max(360, Math.ceil(rect.height));
        await win.setMinSize(new LogicalSize(minWidth, minHeight));
      } catch (err) {
        console.error("setMinSize failed", err);
      }
    };

    const schedule = () => {
      if (saveTimer) clearTimeout(saveTimer);
      saveTimer = setTimeout(() => {
        void persist();
      }, 250);
      if (minSizeTimer) clearTimeout(minSizeTimer);
      minSizeTimer = setTimeout(() => {
        void updateMinSize();
      }, 100);
    };

    const observer = new ResizeObserver(() => schedule());
    observer.observe(panel);

    await win.onMoved(() => schedule());
    await win.onResized(() => schedule());
    schedule();
  } catch (err) {
    console.error("geometry persistence unavailable", err);
  }
}
