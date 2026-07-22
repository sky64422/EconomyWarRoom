import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { sparklinePath, sparklineTone } from "./sparkline";
import type {
  AssetKind,
  Quote,
  Sparkline,
  WatchlistItem,
} from "./types";

const SPARK_W = 72;
const SPARK_H = 28;

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
  let adding = false;
  let addError: string | null = null;

  root.innerHTML = `
    <div class="watchlist-view">
      <div class="watchlist" id="watchlist-list" role="list"></div>
      <div class="watchlist-footer" id="watchlist-footer"></div>
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
    } else {
      const sorted = [...items].sort((a, b) => a.sort_index - b.sort_index);
      listEl.innerHTML = sorted
        .map((item) => {
          const q = quotes.get(item.symbol);
          const sp = sparks.get(item.symbol);
          const points = sp?.points ?? [];
          const path = sparklinePath(points, SPARK_W, SPARK_H);
          const tone = sparklineTone(points);
          const stroke = strokeForTone(tone);
          const pct = q?.change_percent ?? null;
          return `
            <div class="watchlist-row" role="listitem" draggable="true" data-id="${escapeAttr(item.id)}" data-symbol="${escapeAttr(item.symbol)}">
              <span class="row-symbol" title="${escapeAttr(item.symbol)}">${escapeHtml(item.symbol)}</span>
              <svg class="row-sparkline" viewBox="0 0 ${SPARK_W} ${SPARK_H}" width="${SPARK_W}" height="${SPARK_H}" aria-hidden="true">
                ${path ? `<path d="${path}" fill="none" stroke="${stroke}" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />` : ""}
              </svg>
              <span class="row-price">${q ? escapeHtml(formatPrice(q.price)) : "—"}</span>
              <span class="row-change ${changeClass(pct)}">${escapeHtml(formatChange(pct))}</span>
              <button type="button" class="row-remove" data-remove="${escapeAttr(item.id)}" aria-label="Remove ${escapeAttr(item.symbol)}" title="Remove">×</button>
            </div>
          `;
        })
        .join("");
    }
    bindRowEvents();
  }

  function renderFooter(): void {
    if (adding) {
      footerEl.innerHTML = `
        <form class="add-form" id="add-form" autocomplete="off">
          <input type="text" id="add-symbol-input" placeholder="Symbol (e.g. AAPL, BTC-USD)" maxlength="32" spellcheck="false" />
          <button type="submit">Add</button>
          <button type="button" class="secondary" id="add-cancel">Cancel</button>
        </form>
        ${addError ? `<div class="add-error">${escapeHtml(addError)}</div>` : ""}
      `;
      const form = footerEl.querySelector("#add-form") as HTMLFormElement;
      const input = footerEl.querySelector("#add-symbol-input") as HTMLInputElement;
      const cancel = footerEl.querySelector("#add-cancel") as HTMLButtonElement;
      input.focus();
      form.addEventListener("submit", (e) => {
        e.preventDefault();
        void onAdd(input.value);
      });
      cancel.addEventListener("click", () => {
        adding = false;
        addError = null;
        renderFooter();
      });
    } else {
      footerEl.innerHTML = `
        <button type="button" class="btn-add" id="btn-add" aria-label="Add symbol">+ Add</button>
      `;
      footerEl.querySelector("#btn-add")!.addEventListener("click", () => {
        adding = true;
        addError = null;
        renderFooter();
      });
    }
  }

  async function onAdd(raw: string): Promise<void> {
    const symbol = raw.trim().toUpperCase();
    if (!symbol) {
      addError = "Enter a symbol";
      renderFooter();
      return;
    }
    const asset_kind = guessAssetKind(symbol);
    try {
      await invoke("add_symbol", { symbol, asset_kind });
      adding = false;
      addError = null;
      renderFooter();
    } catch (err) {
      addError = String(err);
      renderFooter();
    }
  }

  function bindRowEvents(): void {
    listEl.querySelectorAll<HTMLElement>(".watchlist-row").forEach((row) => {
      row.addEventListener("dragstart", (e) => {
        dragId = row.dataset.id ?? null;
        row.classList.add("dragging");
        e.dataTransfer?.setData("text/plain", dragId ?? "");
        if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
      });
      row.addEventListener("dragend", () => {
        dragId = null;
        row.classList.remove("dragging");
        listEl.querySelectorAll(".drag-over").forEach((el) => el.classList.remove("drag-over"));
      });
      row.addEventListener("dragover", (e) => {
        e.preventDefault();
        if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
        listEl.querySelectorAll(".drag-over").forEach((el) => el.classList.remove("drag-over"));
        if (row.dataset.id !== dragId) row.classList.add("drag-over");
      });
      row.addEventListener("dragleave", () => {
        row.classList.remove("drag-over");
      });
      row.addEventListener("drop", (e) => {
        e.preventDefault();
        row.classList.remove("drag-over");
        const targetId = row.dataset.id;
        if (!dragId || !targetId || dragId === targetId) return;
        const ids = orderedIdsFromDom();
        const from = ids.indexOf(dragId);
        const to = ids.indexOf(targetId);
        if (from < 0 || to < 0) return;
        ids.splice(from, 1);
        ids.splice(to, 0, dragId);
        // Optimistic local reorder
        const byId = new Map(items.map((it) => [it.id, it]));
        items = ids
          .map((id, i) => {
            const it = byId.get(id);
            return it ? { ...it, sort_index: i } : null;
          })
          .filter((x): x is WatchlistItem => x != null);
        renderRows();
        void invoke("reorder_symbols", { ordered_ids: ids }).catch((err) => {
          console.error("reorder_symbols failed", err);
        });
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
    renderRows();
  }

  function setQuotes(next: Quote[]): void {
    for (const q of next) quotes.set(q.symbol, q);
    renderRows();
  }

  function setSparklines(next: Sparkline[]): void {
    for (const s of next) sparks.set(s.symbol, s);
    renderRows();
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
