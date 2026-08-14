import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { sparklineSvgMarkup, sparklineTone } from "../sparkline";
import type {
  AssetKind,
  CardTint,
  ColumnRatios,
  Quote,
  Sparkline,
  SymbolSuggestion,
  WatchlistItem,
} from "../types";
import { CARD_TINTS, DEFAULT_COLUMN_RATIOS } from "../types";
import { applyColumnRatiosCss, RESIZE_HIT_PX, resizeColumnRatios } from "./columns";
import { moveDragHole } from "./dnd";
import { escapeAttr, escapeHtml } from "./format";
import {
  metricsMarkup,
  patchMetricsRow,
  priceRowsForQuote,
  rowAriaLabel,
  sparkFingerprint,
  sparklineBaseline,
  sparklineChangePercent,
} from "./metrics";
import { guessAssetKind, localSuggestions, mergeSuggestions } from "./search";
import { loadAddCardTint, normalizeTint, saveAddCardTint } from "./tint";

const SPARK_W = 100;
const SPARK_H = 28;
const SPARK_TICK_MS = 1000;
const DRAG_THRESHOLD_PX = 6;
const PRICE_FLASH_COOLDOWN_MS = 1400;
const PRICE_FLASH_MS = 560;

export interface WatchlistController {
  setItems: (items: WatchlistItem[]) => void;
  setQuotes: (quotes: Quote[]) => void;
  setSparklines: (sparks: Sparkline[]) => void;
  setColumnRatios: (ratios: ColumnRatios) => void;
  destroy: () => void;
}

export interface WatchlistMountOptions {
  columnRatios?: ColumnRatios;
  onQuotesChanged?: (quotes: Quote[]) => void;
}

type TintTarget = { kind: "item"; id: string } | { kind: "add" };

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

export function mountWatchlist(
  root: HTMLElement,
  opts?: WatchlistMountOptions,
): WatchlistController {
  let items: WatchlistItem[] = [];
  const quotes = new Map<string, Quote>();
  const sparks = new Map<string, Sparkline>();
  const selected = new Set<string>();
  let anchorId: string | null = null;
  let columnRatios: ColumnRatios = {
    ...DEFAULT_COLUMN_RATIOS,
    ...(opts?.columnRatios ?? {}),
  };

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
  const lastPriceFlashAt = new Map<string, number>();

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
  applyColumnRatiosCss(root, columnRatios);

  function setColumnRatios(ratios: ColumnRatios): void {
    columnRatios = {
      symbol: ratios.symbol,
      spark: ratios.spark,
      metrics: ratios.metrics,
    };
    applyColumnRatiosCss(root, columnRatios);
  }

  function persistColumnRatios(ratios: ColumnRatios): void {
    void invoke<ColumnRatios>("set_column_ratios", { ratios }).then(
      (clamped) => setColumnRatios(clamped),
      (err) => console.error("set_column_ratios failed", err),
    );
  }

  function bindColumnResizers(): void {
    listEl.querySelectorAll<HTMLElement>(".row-col-resize").forEach((handle) => {
      handle.addEventListener("pointerdown", (e) => {
        if (e.button !== 0) return;
        e.preventDefault();
        e.stopPropagation();

        const edgeRaw = handle.dataset.edge;
        const edge: 0 | 1 = edgeRaw === "1" ? 1 : 0;
        const startX = e.clientX;
        const startRatios = { ...columnRatios };
        const sampleRow = handle.closest<HTMLElement>(".watchlist-row");
        const rowW = sampleRow?.clientWidth ?? listEl.clientWidth;
        const freePx = Math.max(1, rowW - RESIZE_HIT_PX * 2);

        listEl.classList.add("is-col-resizing");
        listEl
          .querySelectorAll<HTMLElement>(`.row-col-resize[data-edge="${edge}"]`)
          .forEach((h) => h.classList.add("is-active"));

        try {
          handle.setPointerCapture(e.pointerId);
        } catch {
          /* ignore */
        }

        const onMove = (ev: PointerEvent) => {
          const next = resizeColumnRatios(edge, ev.clientX - startX, startRatios, freePx);
          setColumnRatios(next);
        };

        const onUp = (ev: PointerEvent) => {
          try {
            handle.releasePointerCapture(ev.pointerId);
          } catch {
            /* ignore */
          }
          handle.removeEventListener("pointermove", onMove);
          handle.removeEventListener("pointerup", onUp);
          handle.removeEventListener("pointercancel", onUp);
          listEl.classList.remove("is-col-resizing");
          listEl
            .querySelectorAll<HTMLElement>(".row-col-resize.is-active")
            .forEach((h) => h.classList.remove("is-active"));
          const next = resizeColumnRatios(edge, ev.clientX - startX, startRatios, freePx);
          setColumnRatios(next);
          persistColumnRatios(next);
        };

        handle.addEventListener("pointermove", onMove);
        handle.addEventListener("pointerup", onUp);
        handle.addEventListener("pointercancel", onUp);
      });
    });
  }

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
          if (!adding) renderFooter();
          else applyAddCardTintClass();
        }
      });
    });

    menu.querySelector<HTMLButtonElement>('[data-action="remove"]')?.addEventListener("click", (e) => {
      e.stopPropagation();
      closeTintMenu();
      if (target.kind !== "item") return;
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
      listEl.innerHTML = `<div class="watchlist-empty" role="status">No symbols yet. Use + Add below.</div>`;
    } else {
      const sorted = [...items].sort((a, b) => a.sort_index - b.sort_index);
      listEl.innerHTML = sorted
        .map((item) => {
          const q = quotes.get(item.symbol);
          const sp = sparks.get(item.symbol);
          const points = sp?.points ?? [];
          const quotePending = !q || q.price == null;
          const sparkPending = points.length === 0;
          const pct = sparklineChangePercent(q);
          const tone = toneForChange(pct, sparklineTone(points));
          const stroke = strokeForTone(tone);
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
              // Points are already "so far"; wall-clock clip hid the RTH selloff.
              progress: null,
            },
            sparklineBaseline(q, sp?.previous_close),
          );
          const priceRows = priceRowsForQuote(q);
          const aria = rowAriaLabel(item.symbol, priceRows, quotePending);
          return `
            <div class="watchlist-row watchlist-card${tintClass}${selectedClass}" role="listitem" tabindex="0"
              data-id="${escapeAttr(item.id)}" data-symbol="${escapeAttr(item.symbol)}"
              data-tint="${tint}"
              aria-label="${escapeAttr(aria)}"
              title="Click to select · drag to reorder · right-click menu">
              <span class="row-symbol" title="${escapeAttr(item.symbol)}">${escapeHtml(item.symbol)}</span>
              <div class="row-col-resize" data-edge="0" role="separator" aria-orientation="vertical" aria-label="Resize symbol and sparkline" title="Drag to resize columns"></div>
              <div class="row-market">
                <div class="row-sparkline-wrap${sparkPending ? " is-pending" : ""}">
                  <svg class="row-sparkline" viewBox="0 0 ${SPARK_W} ${SPARK_H}" width="100%" height="100%" preserveAspectRatio="none" aria-hidden="true" data-spark="${escapeAttr(item.symbol)}">
                    ${sparkMarkup}
                  </svg>
                </div>
                <div class="row-col-resize" data-edge="1" role="separator" aria-orientation="vertical" aria-label="Resize sparkline and price" title="Drag to resize columns"></div>
                ${metricsMarkup(item.symbol, priceRows, { pending: quotePending })}
              </div>
            </div>
          `;
        })
        .join("");
    }
    bindRowEvents();
    bindColumnResizers();
  }

  function maybeFlashPrimaryPrice(row: HTMLElement, symbol: string): void {
    const now = Date.now();
    const prev = lastPriceFlashAt.get(symbol) ?? 0;
    if (now - prev < PRICE_FLASH_COOLDOWN_MS) return;
    lastPriceFlashAt.set(symbol, now);
    row.classList.remove("row-price-tick");
    void row.offsetWidth;
    row.classList.add("row-price-tick");
    window.setTimeout(() => {
      if (row.isConnected) row.classList.remove("row-price-tick");
    }, PRICE_FLASH_MS);
  }

  function paintSparkSvg(
    svg: SVGElement,
    item: WatchlistItem,
    q: Quote | undefined,
    sp: Sparkline,
  ): void {
    const points = sp.points ?? [];
    const pct = sparklineChangePercent(q);
    const tone = toneForChange(pct, sparklineTone(points));
    const stroke = strokeForTone(tone);
    svg.innerHTML = sparklineSvgMarkup(
      points,
      SPARK_W,
      SPARK_H,
      {
        id: `spark-${escapeAttr(item.id)}`,
        assetKind: item.asset_kind,
        stroke,
        progress: null,
      },
      sparklineBaseline(q, sp.previous_close),
    );
    svg.closest(".row-sparkline-wrap")?.classList.toggle(
      "is-pending",
      points.length === 0,
    );
  }

  function patchMarketCells(mode: "full" | "spark-tick" = "full"): void {
    const byId = new Map(items.map((item) => [item.id, item]));

    listEl.querySelectorAll<HTMLElement>(".watchlist-row").forEach((row) => {
      const symbol = row.dataset.symbol;
      if (!symbol) return;
      const item = row.dataset.id ? byId.get(row.dataset.id) : undefined;
      const q = quotes.get(symbol);
      const sp = sparks.get(symbol);

      if (mode === "full") {
        const metrics = patchMetricsRow(row, priceRowsForQuote(q));
        if (metrics.primaryPriceChanged) {
          maybeFlashPrimaryPrice(row, symbol);
        }
      }

      const svg = row.querySelector<SVGElement>("[data-spark]");
      if (svg && sp && item) {
        if (mode === "spark-tick") {
          paintSparkSvg(svg, item, q, sp);
          return;
        }
        const sfp = sparkFingerprint(sp);
        const toneKey = String(sparklineChangePercent(q) ?? "");
        const fullSparkKey = `${sfp}|${toneKey}`;
        if (row.dataset.sparkFp === fullSparkKey) {
          return;
        }
        row.dataset.sparkFp = fullSparkKey;
        paintSparkSvg(svg, item, q, sp);
      }
    });
  }

  function startSparklineTicker(): void {
    stopSparklineTicker();
    if (document.hidden) return;
    sparkTickTimer = setInterval(() => {
      if (document.hidden) return;
      if (listEl.querySelector(".watchlist-row")) {
        patchMarketCells("spark-tick");
      }
    }, SPARK_TICK_MS);
  }

  function stopSparklineTicker(): void {
    if (sparkTickTimer) {
      clearInterval(sparkTickTimer);
      sparkTickTimer = null;
    }
  }

  function ownedSymbols(): string[] {
    return items.map((i) => i.symbol);
  }

  function scheduleSearch(q: string): void {
    addQuery = q;
    suggestions = localSuggestions(q, ownedSymbols());
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
          suggestions = mergeSuggestions(remote ?? [], localSuggestions(addQuery, ownedSymbols()), ownedSymbols());
          activeSuggest = suggestions.length > 0 ? 0 : -1;
          renderFooter(true);
        } catch {
          if (seq !== searchSeq) return;
          suggestions = localSuggestions(addQuery, ownedSymbols());
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
        if ((e.target as Element | null)?.closest?.(".row-col-resize")) return;

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
          moveDragHole(listEl, row, ev.clientY);
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
    opts?.onQuotesChanged?.(
      next.filter((quote) =>
        items.some(
          (item) => item.symbol === quote.symbol && item.asset_kind === "equity",
        ),
      ),
    );
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
    setColumnRatios,
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

// Re-export for tests / callers that imported resize from the monolith.
export { resizeColumnRatios } from "./columns";
