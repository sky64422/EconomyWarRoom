import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { sparklinePaths, sparklineTone } from "./sparkline";
import type {
  AssetKind,
  Quote,
  Sparkline,
  SymbolSuggestion,
  WatchlistItem,
} from "./types";

const SPARK_W = 72;
const SPARK_H = 32;

/** Local fallback catalog (substring filter) when network is slow/offline. */
const LOCAL_SYMBOLS: SymbolSuggestion[] = [
  { symbol: "AAPL", name: "Apple Inc.", asset_kind: "equity", exchange: "NASDAQ" },
  { symbol: "MSFT", name: "Microsoft Corporation", asset_kind: "equity", exchange: "NASDAQ" },
  { symbol: "GOOGL", name: "Alphabet Inc.", asset_kind: "equity", exchange: "NASDAQ" },
  { symbol: "AMZN", name: "Amazon.com Inc.", asset_kind: "equity", exchange: "NASDAQ" },
  { symbol: "NVDA", name: "NVIDIA Corporation", asset_kind: "equity", exchange: "NASDAQ" },
  { symbol: "META", name: "Meta Platforms Inc.", asset_kind: "equity", exchange: "NASDAQ" },
  { symbol: "TSLA", name: "Tesla Inc.", asset_kind: "equity", exchange: "NASDAQ" },
  { symbol: "SPY", name: "SPDR S&P 500 ETF", asset_kind: "equity", exchange: "NYSE" },
  { symbol: "QQQ", name: "Invesco QQQ Trust", asset_kind: "equity", exchange: "NASDAQ" },
  { symbol: "IWM", name: "iShares Russell 2000 ETF", asset_kind: "equity", exchange: "NYSE" },
  { symbol: "BTC-USD", name: "Bitcoin USD", asset_kind: "crypto", exchange: "CCC" },
  { symbol: "ETH-USD", name: "Ethereum USD", asset_kind: "crypto", exchange: "CCC" },
  { symbol: "SOL-USD", name: "Solana USD", asset_kind: "crypto", exchange: "CCC" },
];

export interface WatchlistController {
  setItems: (items: WatchlistItem[]) => void;
  setQuotes: (quotes: Quote[]) => void;
  setSparklines: (sparks: Sparkline[]) => void;
  destroy: () => void;
}

function guessAssetKind(symbol: string): AssetKind {
  const s = symbol.trim().toUpperCase();
  if (s.includes("-") || s.endsWith("USD")) return "crypto";
  return "equity";
}

function formatPrice(price: number): string {
  if (!Number.isFinite(price)) return "—";
  if (Math.abs(price) >= 1000) return price.toFixed(2);
  if (Math.abs(price) >= 1) return price.toFixed(2);
  if (Math.abs(price) >= 0.01) return price.toFixed(4);
  return price.toPrecision(4);
}

function formatChange(pct: number | null | undefined): string {
  if (pct == null || !Number.isFinite(pct)) return "—";
  const sign = pct > 0 ? "+" : "";
  return `${sign}${pct.toFixed(2)}%`;
}

function changeClass(pct: number | null | undefined): string {
  if (pct == null || !Number.isFinite(pct) || pct === 0) return "";
  return pct > 0 ? "up" : "down";
}

function strokeForTone(tone: "up" | "down" | "flat"): string {
  if (tone === "up") return "var(--sparkline-up)";
  if (tone === "down") return "var(--sparkline-down)";
  return "var(--sparkline-neutral)";
}

export function mountWatchlist(root: HTMLElement): WatchlistController {
  let items: WatchlistItem[] = [];
  const quotes = new Map<string, Quote>();
  const sparks = new Map<string, Sparkline>();

  let dragId: string | null = null;
  /** Full re-render deferred while a row drag is active (tick updates must not kill DnD). */
  let pendingFullRender = false;
  let adding = false;
  let addError: string | null = null;
  let addQuery = "";
  let suggestions: SymbolSuggestion[] = [];
  let activeSuggest = -1;
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  let searchSeq = 0;

  // Scroll region contains rows + add control so "+ Add" sits under the last
  // symbol (not pinned to the panel bottom on tall windows).
  root.innerHTML = `
    <div class="watchlist-view">
      <div class="watchlist" id="watchlist-scroll">
        <div class="watchlist-rows" id="watchlist-list" role="list"></div>
        <div class="watchlist-footer" id="watchlist-footer"></div>
      </div>
    </div>
  `;

  const listEl = root.querySelector("#watchlist-list") as HTMLElement;
  const footerEl = root.querySelector("#watchlist-footer") as HTMLElement;

  function orderedIdsFromDom(): string[] {
    return Array.from(listEl.querySelectorAll<HTMLElement>(".watchlist-row"))
      .map((el) => el.dataset.id)
      .filter((id): id is string => Boolean(id));
  }

  function renderRows(): void {
    if (items.length === 0) {
      listEl.innerHTML = `<div class="watchlist-empty">No symbols yet. Add one below.</div>`;
      // footer (add) remains a sibling under the empty state
    } else {
      const sorted = [...items].sort((a, b) => a.sort_index - b.sort_index);
      listEl.innerHTML = sorted
        .map((item) => {
          const q = quotes.get(item.symbol);
          const sp = sparks.get(item.symbol);
          const points = sp?.points ?? [];
          const { line, area, height: gh } = sparklinePaths(points, SPARK_W, SPARK_H);
          const tone = sparklineTone(points);
          const stroke = strokeForTone(tone);
          const pct = q?.change_percent ?? null;
          const gradId = `spark-fill-${escapeAttr(item.id)}`;
          return `
            <div class="watchlist-row" role="listitem" data-id="${escapeAttr(item.id)}" data-symbol="${escapeAttr(item.symbol)}" title="Drag to reorder">
              <span class="row-symbol" title="${escapeAttr(item.symbol)}">${escapeHtml(item.symbol)}</span>
              <svg class="row-sparkline" viewBox="0 0 ${SPARK_W} ${SPARK_H}" width="${SPARK_W}" height="${SPARK_H}" aria-hidden="true" data-spark="${escapeAttr(item.symbol)}">
                ${sparkSvgInner(line, area, gh, stroke, gradId)}
              </svg>
              <span class="row-price" data-price="${escapeAttr(item.symbol)}">${q ? escapeHtml(formatPrice(q.price)) : "—"}</span>
              <span class="row-change ${changeClass(pct)}" data-change="${escapeAttr(item.symbol)}">${escapeHtml(formatChange(pct))}</span>
              <button type="button" class="row-remove" data-remove="${escapeAttr(item.id)}" aria-label="Remove ${escapeAttr(item.symbol)}" title="Remove">×</button>
            </div>
          `;
        })
        .join("");
    }
    bindRowEvents();
  }

  function sparkSvgInner(
    line: string,
    area: string,
    gh: number,
    stroke: string,
    gradId: string,
  ): string {
    if (!line) return "";
    return `<defs>
      <linearGradient id="${gradId}" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="0" y2="${gh}">
        <stop offset="0%" stop-color="${stroke}" stop-opacity="0.62"/>
        <stop offset="45%" stop-color="${stroke}" stop-opacity="0.34"/>
        <stop offset="100%" stop-color="${stroke}" stop-opacity="0.08"/>
      </linearGradient>
    </defs>
    <path d="${area}" fill="url(#${gradId})" stroke="none" />
    <path d="${line}" fill="none" stroke="${stroke}" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" />`;
  }

  /** Update price / change / sparkline without rebuilding rows (preserves DnD). */
  function patchMarketCells(): void {
    listEl.querySelectorAll<HTMLElement>(".watchlist-row").forEach((row) => {
      const symbol = row.dataset.symbol;
      if (!symbol) return;
      const q = quotes.get(symbol);
      const priceEl = row.querySelector<HTMLElement>("[data-price]");
      const changeEl = row.querySelector<HTMLElement>("[data-change]");
      if (priceEl) {
        priceEl.textContent = q ? formatPrice(q.price) : "—";
      }
      if (changeEl) {
        const pct = q?.change_percent ?? null;
        changeEl.textContent = formatChange(pct);
        changeEl.classList.remove("up", "down");
        const cls = changeClass(pct);
        if (cls) changeEl.classList.add(cls);
      }
      const sp = sparks.get(symbol);
      const svg = row.querySelector<SVGElement>("[data-spark]");
      if (svg && sp) {
        const points = sp.points ?? [];
        const { line, area, height: gh } = sparklinePaths(points, SPARK_W, SPARK_H);
        const tone = sparklineTone(points);
        const stroke = strokeForTone(tone);
        const id = row.dataset.id ?? symbol;
        svg.innerHTML = sparkSvgInner(
          line,
          area,
          gh,
          stroke,
          `spark-fill-${escapeAttr(id)}`,
        );
      }
    });
  }

  function localSuggestions(q: string): SymbolSuggestion[] {
    const u = q.trim().toUpperCase();
    if (!u) return [];
    const owned = new Set(items.map((i) => i.symbol.toUpperCase()));
    return LOCAL_SYMBOLS.filter(
      (s) =>
        !owned.has(s.symbol) &&
        (s.symbol.includes(u) ||
          (s.name ?? "").toUpperCase().includes(u)),
    ).slice(0, 8);
  }

  function mergeSuggestions(
    remote: SymbolSuggestion[],
    local: SymbolSuggestion[],
  ): SymbolSuggestion[] {
    const owned = new Set(items.map((i) => i.symbol.toUpperCase()));
    const out: SymbolSuggestion[] = [];
    const seen = new Set<string>();
    for (const s of [...local, ...remote]) {
      const sym = s.symbol.toUpperCase();
      if (owned.has(sym) || seen.has(sym)) continue;
      seen.add(sym);
      out.push({ ...s, symbol: sym });
      if (out.length >= 8) break;
    }
    return out;
  }

  function scheduleSearch(q: string): void {
    addQuery = q;
    suggestions = localSuggestions(q);
    activeSuggest = suggestions.length > 0 ? 0 : -1;
    renderFooter(true);
    if (searchTimer) clearTimeout(searchTimer);
    const trimmed = q.trim();
    if (!trimmed) {
      suggestions = [];
      activeSuggest = -1;
      renderFooter(true);
      return;
    }
    const seq = ++searchSeq;
    searchTimer = setTimeout(() => {
      void (async () => {
        try {
          const remote = await invoke<SymbolSuggestion[]>("search_symbols", {
            query: trimmed,
            limit: 8,
          });
          if (seq !== searchSeq) return;
          suggestions = mergeSuggestions(remote ?? [], localSuggestions(addQuery));
          activeSuggest = suggestions.length > 0 ? 0 : -1;
          renderFooter(true);
        } catch {
          if (seq !== searchSeq) return;
          // Keep local substring results on network failure.
          suggestions = localSuggestions(addQuery);
          activeSuggest = suggestions.length > 0 ? 0 : -1;
          renderFooter(true);
        }
      })();
    }, 180);
  }

  function renderFooter(keepFocus = false): void {
    if (adding) {
      const caret = keepFocus
        ? (footerEl.querySelector("#add-symbol-input") as HTMLInputElement | null)
            ?.selectionStart ?? addQuery.length
        : addQuery.length;
      footerEl.innerHTML = `
        <div class="add-wrap">
          <form class="add-form" id="add-form" autocomplete="off">
            <input type="text" id="add-symbol-input" placeholder="Search symbol…" maxlength="32" spellcheck="false" value="${escapeAttr(addQuery)}" aria-autocomplete="list" aria-controls="add-suggest" />
            <button type="submit">Add</button>
            <button type="button" class="secondary" id="add-cancel">Cancel</button>
          </form>
          ${
            suggestions.length > 0
              ? `<ul class="add-suggest" id="add-suggest" role="listbox">
            ${suggestions
              .map(
                (s, i) => `
              <li role="option" class="add-suggest-item ${i === activeSuggest ? "active" : ""}" data-suggest-idx="${i}" data-symbol="${escapeAttr(s.symbol)}" data-kind="${escapeAttr(s.asset_kind)}">
                <span class="suggest-symbol">${escapeHtml(s.symbol)}</span>
                <span class="suggest-meta">${escapeHtml(s.name ?? s.exchange ?? s.asset_kind)}</span>
              </li>`,
              )
              .join("")}
          </ul>`
              : addQuery.trim()
                ? `<div class="add-suggest-empty">No matches</div>`
                : ""
          }
          ${addError ? `<div class="add-error">${escapeHtml(addError)}</div>` : ""}
        </div>
      `;
      const form = footerEl.querySelector("#add-form") as HTMLFormElement;
      const input = footerEl.querySelector("#add-symbol-input") as HTMLInputElement;
      const cancel = footerEl.querySelector("#add-cancel") as HTMLButtonElement;
      input.focus();
      try {
        input.setSelectionRange(caret, caret);
      } catch {
        /* ignore */
      }
      form.addEventListener("submit", (e) => {
        e.preventDefault();
        if (activeSuggest >= 0 && suggestions[activeSuggest]) {
          void onAdd(
            suggestions[activeSuggest].symbol,
            suggestions[activeSuggest].asset_kind,
          );
        } else {
          void onAdd(input.value);
        }
      });
      input.addEventListener("input", () => {
        addError = null;
        scheduleSearch(input.value);
      });
      input.addEventListener("keydown", (e) => {
        if (e.key === "ArrowDown" && suggestions.length > 0) {
          e.preventDefault();
          activeSuggest = (activeSuggest + 1) % suggestions.length;
          renderFooter(true);
        } else if (e.key === "ArrowUp" && suggestions.length > 0) {
          e.preventDefault();
          activeSuggest =
            activeSuggest <= 0 ? suggestions.length - 1 : activeSuggest - 1;
          renderFooter(true);
        } else if (e.key === "Escape") {
          e.preventDefault();
          adding = false;
          addError = null;
          addQuery = "";
          suggestions = [];
          activeSuggest = -1;
          renderFooter();
        }
      });
      footerEl.querySelectorAll<HTMLElement>("[data-suggest-idx]").forEach((el) => {
        el.addEventListener("mousedown", (e) => {
          e.preventDefault();
          const idx = Number(el.dataset.suggestIdx);
          const s = suggestions[idx];
          if (s) void onAdd(s.symbol, s.asset_kind);
        });
      });
      cancel.addEventListener("click", () => {
        adding = false;
        addError = null;
        addQuery = "";
        suggestions = [];
        activeSuggest = -1;
        if (searchTimer) clearTimeout(searchTimer);
        renderFooter();
      });
    } else {
      footerEl.innerHTML = `
        <button type="button" class="btn-add" id="btn-add" aria-label="Add symbol">+ Add</button>
      `;
      footerEl.querySelector("#btn-add")!.addEventListener("click", () => {
        adding = true;
        addError = null;
        addQuery = "";
        suggestions = [];
        activeSuggest = -1;
        renderFooter();
      });
    }
  }

  async function onAdd(raw: string, kind?: AssetKind): Promise<void> {
    const symbol = raw.trim().toUpperCase();
    if (!symbol) {
      addError = "Enter a symbol";
      renderFooter(true);
      return;
    }
    const asset_kind = kind ?? guessAssetKind(symbol);
    try {
      await invoke("add_symbol", { symbol, asset_kind });
      adding = false;
      addError = null;
      addQuery = "";
      suggestions = [];
      activeSuggest = -1;
      renderFooter();
    } catch (err) {
      addError = String(err);
      renderFooter(true);
    }
  }

  function syncItemsFromDom(): string[] {
    const ids = orderedIdsFromDom();
    const byId = new Map(items.map((it) => [it.id, it]));
    items = ids
      .map((id, i) => {
        const it = byId.get(id);
        return it ? { ...it, sort_index: i } : null;
      })
      .filter((x): x is WatchlistItem => x != null);
    return ids;
  }

  function persistOrder(ids: string[]): void {
    void invoke("reorder_symbols", { ordered_ids: ids }).catch((err) => {
      console.error("reorder_symbols failed", err);
    });
  }

  /** FLIP: animate siblings when the dragged hole moves in the list. */
  function flipRows(mutate: () => void): void {
    const rows = Array.from(
      listEl.querySelectorAll<HTMLElement>(".watchlist-row"),
    );
    const first = new Map<HTMLElement, DOMRect>();
    for (const r of rows) first.set(r, r.getBoundingClientRect());
    mutate();
    for (const r of rows) {
      if (!r.isConnected || r.classList.contains("is-dragging")) continue;
      const a = first.get(r);
      if (!a) continue;
      const b = r.getBoundingClientRect();
      const dy = a.top - b.top;
      if (Math.abs(dy) < 0.5) continue;
      r.style.transition = "none";
      r.style.transform = `translateY(${dy}px)`;
      // Force reflow then ease to rest.
      void r.offsetHeight;
      r.style.transition = "transform 0.22s cubic-bezier(0.2, 0.8, 0.2, 1)";
      r.style.transform = "";
      const clear = () => {
        r.style.transition = "";
        r.removeEventListener("transitionend", clear);
      };
      r.addEventListener("transitionend", clear);
    }
  }

  function moveDragHole(source: HTMLElement, clientY: number): void {
    const others = Array.from(
      listEl.querySelectorAll<HTMLElement>(".watchlist-row:not(.is-dragging)"),
    );
    if (others.length === 0) return;

    let insertBefore: HTMLElement | null = null;
    for (const other of others) {
      const rect = other.getBoundingClientRect();
      const mid = rect.top + rect.height / 2;
      if (clientY < mid) {
        insertBefore = other;
        break;
      }
    }

    const next =
      insertBefore === null
        ? source.nextSibling === null && source.parentElement?.lastElementChild === source
          ? null // already last
          : "end"
        : insertBefore;

    if (next === "end") {
      if (listEl.lastElementChild === source) return;
      flipRows(() => listEl.appendChild(source));
    } else if (next instanceof HTMLElement) {
      if (source.nextElementSibling === next) return;
      flipRows(() => listEl.insertBefore(source, next));
    }
  }

  /**
   * Pointer reorder with floating ghost + FLIP list animation.
   * (HTML5 DnD is unreliable on WebView2 transparent windows.)
   */
  function bindRowEvents(): void {
    listEl.querySelectorAll<HTMLElement>(".watchlist-row").forEach((row) => {
      row.addEventListener("pointerdown", (e) => {
        if (e.button !== 0) return;
        const t = e.target as HTMLElement | null;
        if (t?.closest?.(".row-remove")) return;

        const sourceId = row.dataset.id;
        if (!sourceId) return;

        e.preventDefault();
        dragId = sourceId;
        pendingFullRender = false;

        const rect = row.getBoundingClientRect();
        const offsetX = e.clientX - rect.left;
        const offsetY = e.clientY - rect.top;

        // Floating clone that tracks the pointer.
        const ghost = row.cloneNode(true) as HTMLElement;
        ghost.classList.add("drag-ghost");
        ghost.classList.remove("dragging", "is-dragging", "drag-over");
        ghost.style.width = `${rect.width}px`;
        ghost.style.height = `${rect.height}px`;
        ghost.style.left = "0";
        ghost.style.top = "0";
        const placeGhost = (cx: number, cy: number) => {
          const x = cx - offsetX;
          const y = cy - offsetY;
          ghost.style.transform = `translate3d(${x}px, ${y}px, 0) scale(1.03)`;
        };
        placeGhost(e.clientX, e.clientY);
        document.body.appendChild(ghost);

        row.classList.add("is-dragging");
        listEl.classList.add("is-reordering");
        row.setPointerCapture(e.pointerId);

        const onMove = (ev: PointerEvent) => {
          if (!dragId) return;
          placeGhost(ev.clientX, ev.clientY);
          // Hit-test under ghost (ghost has pointer-events: none).
          moveDragHole(row, ev.clientY);
        };

        const finish = (ev: PointerEvent) => {
          try {
            row.releasePointerCapture(ev.pointerId);
          } catch {
            /* already released */
          }
          row.removeEventListener("pointermove", onMove);
          row.removeEventListener("pointerup", finish);
          row.removeEventListener("pointercancel", finish);

          ghost.remove();
          row.classList.remove("is-dragging");
          listEl.classList.remove("is-reordering");
          listEl
            .querySelectorAll(".drag-over")
            .forEach((n) => n.classList.remove("drag-over"));

          const src = dragId;
          dragId = null;
          if (!src) return;

          const ids = syncItemsFromDom();
          persistOrder(ids);

          if (pendingFullRender) {
            pendingFullRender = false;
            renderRows();
          } else {
            // Clear any leftover inline FLIP styles.
            listEl.querySelectorAll<HTMLElement>(".watchlist-row").forEach((r) => {
              r.style.transform = "";
              r.style.transition = "";
            });
          }
        };

        row.addEventListener("pointermove", onMove);
        row.addEventListener("pointerup", finish);
        row.addEventListener("pointercancel", finish);
      });
    });

    listEl.querySelectorAll<HTMLButtonElement>("[data-remove]").forEach((btn) => {
      btn.addEventListener("click", (e) => {
        e.stopPropagation();
        const id = btn.dataset.remove;
        if (!id) return;
        void invoke("remove_symbol", { id }).catch((err) => {
          console.error("remove_symbol failed", err);
        });
      });
    });
  }

  function setItems(next: WatchlistItem[]): void {
    items = next;
    if (dragId) {
      pendingFullRender = true;
      return;
    }
    renderRows();
  }

  function setQuotes(next: Quote[]): void {
    for (const q of next) quotes.set(q.symbol, q);
    if (dragId) {
      // Still patch numbers if DOM is intact; never rebuild rows mid-drag.
      patchMarketCells();
      return;
    }
    if (listEl.querySelector(".watchlist-row")) {
      patchMarketCells();
    } else {
      renderRows();
    }
  }

  function setSparklines(next: Sparkline[]): void {
    for (const s of next) sparks.set(s.symbol, s);
    if (dragId) {
      patchMarketCells();
      return;
    }
    if (listEl.querySelector(".watchlist-row")) {
      patchMarketCells();
    } else {
      renderRows();
    }
  }

  renderRows();
  renderFooter();

  const unlisteners: Array<() => void> = [];

  void listen<Quote[]>("quotes-updated", (e) => {
    setQuotes(e.payload ?? []);
  }).then((u) => unlisteners.push(u));

  void listen<Sparkline[]>("sparklines-updated", (e) => {
    setSparklines(e.payload ?? []);
  }).then((u) => unlisteners.push(u));

  void listen<WatchlistItem[]>("watchlist-updated", (e) => {
    setItems(e.payload ?? []);
  }).then((u) => unlisteners.push(u));

  return {
    setItems,
    setQuotes,
    setSparklines,
    destroy: () => {
      for (const u of unlisteners) u();
    },
  };
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function escapeAttr(s: string): string {
  return escapeHtml(s).replace(/'/g, "&#39;");
}
