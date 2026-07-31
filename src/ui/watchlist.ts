import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { sparklineProgress, sparklineSvgMarkup, sparklineTone } from "./sparkline";
import type {
  AssetKind,
  CardTint,
  Quote,
  Sparkline,
  SymbolSuggestion,
  WatchlistItem,
} from "./types";
import { CARD_TINTS } from "./types";

/**
 * SVG coordinate system only — CSS width is the proportional spark column.
 * none: fill the share edge-to-edge as the card resizes (ratio model).
 */
const SPARK_W = 100;
const SPARK_H = 28;
const SPARK_TICK_MS = 1000;
const DRAG_THRESHOLD_PX = 6;

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

/** Compact prices for narrow widget columns. */
function formatPrice(price: number): string {
  if (!Number.isFinite(price)) return "--";
  const a = Math.abs(price);
  if (a >= 10_000) {
    return Math.round(price).toLocaleString("en-US");
  }
  if (a >= 1000) {
    return price.toLocaleString("en-US", {
      minimumFractionDigits: 0,
      maximumFractionDigits: 0,
    });
  }
  if (a >= 100) return price.toFixed(1);
  if (a >= 1) return price.toFixed(2);
  if (a >= 0.01) return price.toFixed(3);
  return price.toPrecision(2);
}

function formatChange(pct: number | null | undefined, compact = false): string {
  if (pct == null || !Number.isFinite(pct)) return "";
  const sign = pct > 0 ? "+" : "";
  const digits = compact ? 1 : 2;
  return `${sign}${pct.toFixed(digits)}%`;
}

/** Inline suffix: `(+0.42%)` — empty when no change. Gap vs price is CSS. */
function formatChangeParen(pct: number | null | undefined, compact = false): string {
  const inner = formatChange(pct, compact);
  return inner ? `(${inner})` : "";
}

function changeClass(pct: number | null | undefined): string {
  if (pct == null || !Number.isFinite(pct) || pct === 0) return "";
  return pct > 0 ? "up" : "down";
}

interface PriceRow {
  price: number | null;
  change: number | null;
}

interface PriceRows {
  primary: PriceRow;
  secondary: PriceRow;
}

function isExtendedSession(state: string | null | undefined): boolean {
  if (!state) return false;
  const s = state.toLowerCase();
  return s === "pre" || s === "prepre" || s === "post" || s === "postpost" || s === "closed";
}

function extendedChangePercent(q: Quote): number | null {
  if (q.extended_change_percent != null && Number.isFinite(q.extended_change_percent)) {
    return q.extended_change_percent;
  }
  const ext = q.extended_price;
  const reg = q.regular_price ?? q.price;
  if (ext == null || !Number.isFinite(ext) || !Number.isFinite(reg) || reg === 0) {
    return null;
  }
  return ((ext - reg) / reg) * 100;
}

function isExtendedQuote(q: Quote): boolean {
  if (q.extended_price == null) return false;
  if (isExtendedSession(q.market_state)) return true;
  const reg = q.regular_price ?? q.price;
  return Math.abs(q.extended_price - reg) > 0.0001;
}

function pctChange(from: number | null | undefined, to: number | null | undefined): number | null {
  if (from == null || to == null || !Number.isFinite(from) || !Number.isFinite(to) || from === 0) {
    return null;
  }
  return ((to - from) / from) * 100;
}

/** Primary = latest (live/extended); secondary = reference (regular close or prior). */
function resolvePriceRows(q: Quote | undefined, sparkPrevClose: number | null): PriceRows {
  const empty: PriceRow = { price: null, change: null };
  if (!q) {
    return { primary: empty, secondary: empty };
  }

  const previousClose = q.previous_close ?? sparkPrevClose ?? null;
  const regularPrice = q.regular_price ?? q.price;
  // Regular-session move vs prior close (always about the official session).
  const regularChange =
    q.regular_change_percent ??
    pctChange(previousClose, regularPrice) ??
    (!isExtendedQuote(q) ? (q.change_percent ?? null) : null);
  const priorChange =
    q.previous_day_change_percent ?? pctChange(q.prior_close ?? null, previousClose);

  if (isExtendedQuote(q) && q.extended_price != null) {
    return {
      primary: {
        price: q.extended_price,
        change: extendedChangePercent(q),
      },
      secondary: {
        price: regularPrice ?? null,
        change: regularChange,
      },
    };
  }

  return {
    primary: {
      price: q.price,
      change: q.change_percent ?? regularChange,
    },
    secondary: {
      price: previousClose,
      change: priorChange,
    },
  };
}

function metricsMarkup(
  symbol: string,
  rows: PriceRows,
): string {
  const primaryPrice = rows.primary.price != null ? formatPrice(rows.primary.price) : "--";
  const primaryChange = formatChangeParen(rows.primary.change);
  const primaryCls = changeClass(rows.primary.change);
  const secondaryPrice =
    rows.secondary.price != null ? formatPrice(rows.secondary.price) : "--";
  const secondaryChange = formatChangeParen(rows.secondary.change, true);
  const secondaryCls = changeClass(rows.secondary.change);

  return `
    <div class="row-metrics">
      <div class="row-quote row-quote--primary">
        <span class="row-price" data-price-primary="${escapeAttr(symbol)}">${escapeHtml(primaryPrice)}</span><span class="row-change ${primaryCls}" data-change-primary="${escapeAttr(symbol)}"${primaryChange ? "" : " hidden"}>${escapeHtml(primaryChange)}</span>
      </div>
      <div class="row-quote row-quote--secondary">
        <span class="row-price" data-price-secondary="${escapeAttr(symbol)}">${escapeHtml(secondaryPrice)}</span><span class="row-change ${secondaryCls}" data-change-secondary="${escapeAttr(symbol)}"${secondaryChange ? "" : " hidden"}>${escapeHtml(secondaryChange)}</span>
      </div>
    </div>
  `;
}

function patchMetricsRow(
  row: HTMLElement,
  rows: PriceRows,
): void {
  const primaryPriceEl = row.querySelector<HTMLElement>("[data-price-primary]");
  const primaryChangeEl = row.querySelector<HTMLElement>("[data-change-primary]");
  const secondaryPriceEl = row.querySelector<HTMLElement>("[data-price-secondary]");
  const secondaryChangeEl = row.querySelector<HTMLElement>("[data-change-secondary]");

  if (primaryPriceEl) {
    primaryPriceEl.textContent =
      rows.primary.price != null ? formatPrice(rows.primary.price) : "--";
  }
  if (primaryChangeEl) {
    const txt = formatChangeParen(rows.primary.change);
    primaryChangeEl.textContent = txt;
    primaryChangeEl.hidden = !txt;
    primaryChangeEl.classList.remove("up", "down");
    const cls = changeClass(rows.primary.change);
    if (cls) primaryChangeEl.classList.add(cls);
  }
  if (secondaryPriceEl) {
    secondaryPriceEl.textContent =
      rows.secondary.price != null ? formatPrice(rows.secondary.price) : "--";
  }
  if (secondaryChangeEl) {
    const txt = formatChangeParen(rows.secondary.change, true);
    secondaryChangeEl.textContent = txt;
    secondaryChangeEl.hidden = !txt;
    secondaryChangeEl.classList.remove("up", "down");
    const cls = changeClass(rows.secondary.change);
    if (cls) secondaryChangeEl.classList.add(cls);
  }
}

function toneForChange(
  pct: number | null | undefined,
  fallback: ReturnType<typeof sparklineTone>,
): ReturnType<typeof sparklineTone> {
  if (pct == null || !Number.isFinite(pct)) return fallback;
  if (pct > 0) return "up";
  if (pct < 0) return "down";
  return "flat";
}

function strokeForTone(tone: "up" | "down" | "flat"): string {
  if (tone === "up") return "var(--sparkline-up)";
  if (tone === "down") return "var(--sparkline-down)";
  return "var(--sparkline-neutral)";
}

function normalizeTint(raw: CardTint | undefined | null): CardTint {
  if (!raw || raw === "none") return "none";
  const ok = CARD_TINTS.some((t) => t.value === raw);
  return ok ? raw : "none";
}

const ADD_TINT_STORAGE_KEY = "ewr.add_card_tint";

function loadAddCardTint(): CardTint {
  try {
    return normalizeTint(localStorage.getItem(ADD_TINT_STORAGE_KEY) as CardTint | null);
  } catch {
    return "none";
  }
}

function saveAddCardTint(tint: CardTint): void {
  try {
    if (tint === "none") localStorage.removeItem(ADD_TINT_STORAGE_KEY);
    else localStorage.setItem(ADD_TINT_STORAGE_KEY, tint);
  } catch {
    /* ignore quota / private mode */
  }
}

type TintTarget = { kind: "item"; id: string } | { kind: "add" };

export function mountWatchlist(root: HTMLElement): WatchlistController {
  let items: WatchlistItem[] = [];
  const quotes = new Map<string, Quote>();
  const sparks = new Map<string, Sparkline>();
  const selected = new Set<string>();
  let anchorId: string | null = null;

  let dragId: string | null = null;
  let pendingFullRender = false;
  let adding = false;
  let addError: string | null = null;
  let addQuery = "";
  let suggestions: SymbolSuggestion[] = [];
  let activeSuggest = -1;
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  let searchSeq = 0;
  let sparkTickTimer: ReturnType<typeof setInterval> | null = null;
  let tintMenuEl: HTMLElement | null = null;
  let addCardTint: CardTint = loadAddCardTint();

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

  function orderedItemIds(): string[] {
    return [...items]
      .sort((a, b) => a.sort_index - b.sort_index)
      .map((i) => i.id);
  }

  function closeTintMenu(): void {
    if (tintMenuEl) {
      tintMenuEl.remove();
      tintMenuEl = null;
    }
  }

  function openTintMenu(target: TintTarget, clientX: number, clientY: number): void {
    closeTintMenu();
    let current: CardTint = "none";
    if (target.kind === "item") {
      const item = items.find((i) => i.id === target.id);
      if (!item) return;
      current = normalizeTint(item.card_tint);
    } else {
      current = normalizeTint(addCardTint);
    }
    const menu = document.createElement("div");
    menu.className = "tint-menu";
    menu.setAttribute("role", "menu");
    const removeLabel =
      target.kind === "item" && selected.size > 1 && selected.has(target.id)
        ? `Remove ${selected.size}`
        : "Remove";
    menu.innerHTML = `
      <div class="tint-menu-label">Card color</div>
      <div class="tint-swatches">
        ${CARD_TINTS.map(
          (t) => `
          <button type="button" class="tint-swatch tint-${t.value} ${t.value === current ? "active" : ""}"
            data-tint="${t.value}" title="${t.label}" aria-label="${t.label}"></button>
        `,
        ).join("")}
      </div>
      ${
        target.kind === "item"
          ? `<div class="tint-menu-divider" role="separator"></div>
      <button type="button" class="tint-menu-action tint-menu-action--danger" data-action="remove" role="menuitem">${escapeHtml(removeLabel)}</button>`
          : ""
      }
    `;
    document.body.appendChild(menu);
    const pad = 8;
    const rect = menu.getBoundingClientRect();
    let left = clientX;
    let top = clientY;
    if (left + rect.width > window.innerWidth - pad) {
      left = window.innerWidth - rect.width - pad;
    }
    if (top + rect.height > window.innerHeight - pad) {
      top = window.innerHeight - rect.height - pad;
    }
    menu.style.left = `${Math.max(pad, left)}px`;
    menu.style.top = `${Math.max(pad, top)}px`;
    tintMenuEl = menu;

    menu.querySelectorAll<HTMLButtonElement>("[data-tint]").forEach((btn) => {
      btn.addEventListener("click", (e) => {
        e.stopPropagation();
        const tint = normalizeTint(btn.dataset.tint as CardTint);
        closeTintMenu();
        if (target.kind === "item") {
          void invoke("set_card_tint", { id: target.id, tint }).catch((err) => {
            console.error("set_card_tint failed", err);
          });
        } else {
          addCardTint = tint;
          saveAddCardTint(tint);
          // Idle +Add re-render so tint class applies immediately
          if (!adding) renderFooter();
          else applyAddCardTintClass();
        }
      });
    });

    menu.querySelector<HTMLButtonElement>('[data-action="remove"]')?.addEventListener("click", (e) => {
      e.stopPropagation();
      closeTintMenu();
      if (target.kind !== "item") return;
      // Multi-select: remove all selected when the target is part of the selection
      if (selected.size > 1 && selected.has(target.id)) {
        void deleteSelected();
        return;
      }
      selected.delete(target.id);
      void invoke("remove_symbol", { id: target.id }).catch((err) => {
        console.error("remove_symbol failed", err);
      });
    });
  }

  function addCardTintClass(): string {
    const tint = normalizeTint(addCardTint);
    return tint !== "none" ? ` tint-${tint}` : "";
  }

  function applyAddCardTintClass(): void {
    const el = footerEl.querySelector<HTMLElement>(".add-card");
    if (!el) return;
    for (const t of CARD_TINTS) {
      if (t.value === "none") continue;
      el.classList.toggle(`tint-${t.value}`, t.value === addCardTint);
    }
  }

  function applySelectionClasses(): void {
    listEl.querySelectorAll<HTMLElement>(".watchlist-row").forEach((row) => {
      const id = row.dataset.id;
      row.classList.toggle("is-selected", Boolean(id && selected.has(id)));
    });
  }

  function selectSingle(id: string): void {
    selected.clear();
    selected.add(id);
    anchorId = id;
    applySelectionClasses();
  }

  function toggleSelect(id: string): void {
    if (selected.has(id)) {
      selected.delete(id);
    } else {
      selected.add(id);
    }
    anchorId = id;
    applySelectionClasses();
  }

  function selectRange(toId: string): void {
    const order = orderedItemIds();
    const from = anchorId && order.includes(anchorId) ? anchorId : toId;
    const a = order.indexOf(from);
    const b = order.indexOf(toId);
    if (a < 0 || b < 0) {
      selectSingle(toId);
      return;
    }
    const lo = Math.min(a, b);
    const hi = Math.max(a, b);
    selected.clear();
    for (let i = lo; i <= hi; i++) selected.add(order[i]);
    applySelectionClasses();
  }

  function pruneSelection(): void {
    const alive = new Set(items.map((i) => i.id));
    for (const id of [...selected]) {
      if (!alive.has(id)) selected.delete(id);
    }
    if (anchorId && !alive.has(anchorId)) {
      anchorId = selected.values().next().value ?? null;
    }
  }

  async function deleteSelected(): Promise<void> {
    if (selected.size === 0) return;
    const ids = [...selected];
    selected.clear();
    anchorId = null;
    try {
      if (ids.length === 1) {
        await invoke("remove_symbol", { id: ids[0] });
      } else {
        await invoke("remove_symbols", { ids });
      }
    } catch (err) {
      console.error("remove failed", err);
    }
  }

  function renderRows(): void {
    pruneSelection();
    if (items.length === 0) {
      listEl.innerHTML = `<div class="watchlist-empty">No symbols yet. Add one below.</div>`;
    } else {
      const sorted = [...items].sort((a, b) => a.sort_index - b.sort_index);
      listEl.innerHTML = sorted
        .map((item) => {
          const q = quotes.get(item.symbol);
          const sp = sparks.get(item.symbol);
          const points = sp?.points ?? [];
          const pct = q?.change_percent ?? null;
          const tone = toneForChange(pct, sparklineTone(points));
          const stroke = strokeForTone(tone);
          const progress = sparklineProgress(points, item.asset_kind);
          const tint = normalizeTint(item.card_tint);
          const tintClass = tint !== "none" ? ` tint-${tint}` : "";
          const selectedClass = selected.has(item.id) ? " is-selected" : "";
          const sparkMarkup = sparklineSvgMarkup(
            points,
            SPARK_W,
            SPARK_H,
            {
              id: `spark-${escapeAttr(item.id)}`,
              assetKind: item.asset_kind,
              stroke,
              progress,
            },
            sp?.previous_close ?? null,
          );
          const priceRows = resolvePriceRows(q, sp?.previous_close ?? null);
          return `
            <div class="watchlist-row watchlist-card${tintClass}${selectedClass}" role="listitem" tabindex="0"
              data-id="${escapeAttr(item.id)}" data-symbol="${escapeAttr(item.symbol)}"
              data-tint="${tint}" title="Click to select · drag to reorder · right-click menu">
              <span class="row-symbol" title="${escapeAttr(item.symbol)}">${escapeHtml(item.symbol)}</span>
              <div class="row-market">
                <div class="row-sparkline-wrap">
                  <svg class="row-sparkline" viewBox="0 0 ${SPARK_W} ${SPARK_H}" width="100%" height="100%" preserveAspectRatio="none" aria-hidden="true" data-spark="${escapeAttr(item.symbol)}">
                    ${sparkMarkup}
                  </svg>
                </div>
                ${metricsMarkup(item.symbol, priceRows)}
              </div>
            </div>
          `;
        })
        .join("");
    }
    bindRowEvents();
  }

  /** Update price / change / sparkline without rebuilding rows (preserves DnD). */
  function patchMarketCells(): void {
    const byId = new Map(items.map((item) => [item.id, item]));
    listEl.querySelectorAll<HTMLElement>(".watchlist-row").forEach((row) => {
      const symbol = row.dataset.symbol;
      if (!symbol) return;
      const item = row.dataset.id ? byId.get(row.dataset.id) : undefined;
      const q = quotes.get(symbol);
      const sp = sparks.get(symbol);
      patchMetricsRow(row, resolvePriceRows(q, sp?.previous_close ?? null));

      const svg = row.querySelector<SVGElement>("[data-spark]");
      if (svg && sp && item) {
        const points = sp.points ?? [];
        const pct = q?.change_percent ?? null;
        const tone = toneForChange(pct, sparklineTone(points));
        const stroke = strokeForTone(tone);
        const progress = sparklineProgress(points, item.asset_kind);
        svg.innerHTML = sparklineSvgMarkup(
          points,
          SPARK_W,
          SPARK_H,
          {
            id: `spark-${escapeAttr(item.id)}`,
            assetKind: item.asset_kind,
            stroke,
            progress,
          },
          sp.previous_close ?? null,
        );
      }
    });
  }

  function startSparklineTicker(): void {
    stopSparklineTicker();
    if (document.hidden) return;
    sparkTickTimer = setInterval(() => {
      if (document.hidden) return;
      if (listEl.querySelector(".watchlist-row")) {
        patchMarketCells();
      }
    }, SPARK_TICK_MS);
  }

  function stopSparklineTicker(): void {
    if (sparkTickTimer) {
      clearInterval(sparkTickTimer);
      sparkTickTimer = null;
    }
  }

  function localSuggestions(q: string): SymbolSuggestion[] {
    const u = q.trim().toUpperCase();
    if (!u) return [];
    const owned = new Set(items.map((i) => i.symbol.toUpperCase()));
    return LOCAL_SYMBOLS.filter(
      (s) =>
        !owned.has(s.symbol) &&
        (s.symbol.includes(u) || (s.name ?? "").toUpperCase().includes(u)),
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
          <form class="add-card add-card--active watchlist-card" id="add-form" autocomplete="off">
            <input type="text" id="add-symbol-input" class="add-card-input" placeholder="Symbol..." maxlength="32" spellcheck="false" value="${escapeAttr(addQuery)}" aria-autocomplete="list" aria-controls="add-suggest" />
            <button type="submit" class="add-card-btn primary">Add</button>
            <button type="button" class="add-card-btn" id="add-cancel">Cancel</button>
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
        <button type="button" class="add-card watchlist-card${addCardTintClass()}" id="btn-add"
          aria-label="Add symbol"
          title="Click to add · right-click color">+ Add</button>
      `;
      const btn = footerEl.querySelector("#btn-add") as HTMLButtonElement;
      btn.addEventListener("click", () => {
        adding = true;
        addError = null;
        addQuery = "";
        suggestions = [];
        activeSuggest = -1;
        renderFooter();
      });
      btn.addEventListener("contextmenu", (e) => {
        e.preventDefault();
        e.stopPropagation();
        openTintMenu({ kind: "add" }, e.clientX, e.clientY);
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

  function flipRows(mutate: () => void): void {
    const rows = Array.from(listEl.querySelectorAll<HTMLElement>(".watchlist-row"));
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

    let targetIndex = others.length;
    for (let i = 0; i < others.length; i++) {
      const rect = others[i].getBoundingClientRect();
      const mid = rect.top + rect.height / 2;
      if (clientY < mid) {
        targetIndex = i;
        break;
      }
    }

    const allRows = Array.from(
      listEl.querySelectorAll<HTMLElement>(".watchlist-row"),
    );
    const currentIndex = allRows.indexOf(source);
    if (currentIndex < 0 || currentIndex === targetIndex) return;

    if (targetIndex >= others.length) {
      flipRows(() => listEl.appendChild(source));
    } else {
      const ref = others[targetIndex];
      flipRows(() => listEl.insertBefore(source, ref));
    }
  }

  function bindRowEvents(): void {
    listEl.querySelectorAll<HTMLElement>(".watchlist-row").forEach((row) => {
      row.addEventListener("contextmenu", (e) => {
        e.preventDefault();
        const id = row.dataset.id;
        if (!id) return;
        if (!selected.has(id)) selectSingle(id);
        openTintMenu({ kind: "item", id }, e.clientX, e.clientY);
      });

      row.addEventListener("pointerdown", (e) => {
        if (e.button !== 0) return;

        const sourceId = row.dataset.id;
        if (!sourceId) return;

        closeTintMenu();
        e.preventDefault();

        const startX = e.clientX;
        const startY = e.clientY;
        const multi = e.ctrlKey || e.metaKey;
        const range = e.shiftKey;
        let dragging = false;
        let ghost: HTMLElement | null = null;
        let offsetX = 0;
        let offsetY = 0;

        const beginDrag = (ev: PointerEvent) => {
          if (dragging) return;
          dragging = true;
          dragId = sourceId;
          pendingFullRender = false;
          if (!selected.has(sourceId)) selectSingle(sourceId);

          const rect = row.getBoundingClientRect();
          offsetX = startX - rect.left;
          offsetY = startY - rect.top;

          ghost = row.cloneNode(true) as HTMLElement;
          ghost.classList.add("drag-ghost");
          ghost.classList.remove("dragging", "is-dragging", "drag-over", "is-selected");
          ghost.style.width = `${rect.width}px`;
          ghost.style.height = `${rect.height}px`;
          ghost.style.left = "0";
          ghost.style.top = "0";
          const placeGhost = (cx: number, cy: number) => {
            const x = cx - offsetX;
            const y = cy - offsetY;
            ghost!.style.transform = `translate3d(${x}px, ${y}px, 0) scale(1.03)`;
          };
          placeGhost(ev.clientX, ev.clientY);
          document.body.appendChild(ghost);

          row.classList.add("is-dragging");
          listEl.classList.add("is-reordering");
        };

        try {
          row.setPointerCapture(e.pointerId);
        } catch {
          /* ignore */
        }

        const onMove = (ev: PointerEvent) => {
          const dx = ev.clientX - startX;
          const dy = ev.clientY - startY;
          if (!dragging && Math.hypot(dx, dy) >= DRAG_THRESHOLD_PX) {
            beginDrag(ev);
          }
          if (!dragging || !dragId || !ghost) return;
          ghost.style.transform = `translate3d(${ev.clientX - offsetX}px, ${ev.clientY - offsetY}px, 0) scale(1.03)`;
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

          if (!dragging) {
            // Click selection — re-click sole selection toggles off
            if (range) {
              selectRange(sourceId);
            } else if (multi) {
              toggleSelect(sourceId);
            } else if (selected.has(sourceId) && selected.size === 1) {
              selected.delete(sourceId);
              anchorId = null;
              applySelectionClasses();
            } else {
              selectSingle(sourceId);
            }
            row.focus({ preventScroll: true });
            return;
          }

          ghost?.remove();
          ghost = null;
          row.classList.remove("is-dragging");
          listEl.classList.remove("is-reordering");
          listEl.querySelectorAll(".drag-over").forEach((n) => n.classList.remove("drag-over"));

          const src = dragId;
          dragId = null;
          if (!src) return;

          const ids = syncItemsFromDom();
          persistOrder(ids);

          if (pendingFullRender) {
            pendingFullRender = false;
            renderRows();
          } else {
            listEl.querySelectorAll<HTMLElement>(".watchlist-row").forEach((r) => {
              r.style.transform = "";
              r.style.transition = "";
            });
            applySelectionClasses();
          }
        };

        row.addEventListener("pointermove", onMove);
        row.addEventListener("pointerup", finish);
        row.addEventListener("pointercancel", finish);
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
  startSparklineTicker();

  const onKeyDown = (e: KeyboardEvent) => {
    const target = e.target as HTMLElement | null;
    if (target?.closest?.("input, textarea, [contenteditable=true]")) return;
    if (e.key === "Delete" || e.key === "Backspace") {
      if (selected.size === 0) return;
      e.preventDefault();
      void deleteSelected();
    } else if (e.key === "Escape") {
      closeTintMenu();
      if (selected.size > 0) {
        selected.clear();
        applySelectionClasses();
      }
    }
  };
  document.addEventListener("keydown", onKeyDown);

  const onDocPointer = (e: PointerEvent) => {
    if (tintMenuEl && !tintMenuEl.contains(e.target as Node)) {
      closeTintMenu();
    }
  };
  document.addEventListener("pointerdown", onDocPointer, true);

  const onVis = () => {
    if (document.hidden) stopSparklineTicker();
    else startSparklineTicker();
  };
  document.addEventListener("visibilitychange", onVis);

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
      if (searchTimer) clearTimeout(searchTimer);
      stopSparklineTicker();
      closeTintMenu();
      document.removeEventListener("keydown", onKeyDown);
      document.removeEventListener("pointerdown", onDocPointer, true);
      document.removeEventListener("visibilitychange", onVis);
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
