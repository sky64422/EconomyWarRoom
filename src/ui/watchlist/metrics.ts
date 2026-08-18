import type { PriceRow, PriceRows, Quote, Sparkline } from "../types";
import {
  changeClass,
  escapeAttr,
  escapeHtml,
  formatChange,
  formatChangeParen,
  formatPrice,
} from "./format";

const EMPTY: PriceRow = { price: null, change: null };

/** Prefer Rust-computed display rows; empty when quote not yet attached. */
export function priceRowsForQuote(q: Quote | undefined): PriceRows {
  if (q?.display) return q.display;
  return { primary: EMPTY, secondary: EMPTY };
}

export function sparklineChangePercent(q: Quote | undefined): number | null {
  const pct = q?.sparkline_change_percent;
  if (pct == null || !Number.isFinite(pct)) return null;
  return pct;
}

/** Same T-1 close the quote % uses — do not fall back to chart bottom. */
export function sparklineBaseline(
  q: Quote | undefined,
  sparkPrev: number | null | undefined,
): number | null {
  const fromQuote = q?.previous_close;
  if (fromQuote != null && Number.isFinite(fromQuote)) return fromQuote;
  if (sparkPrev != null && Number.isFinite(sparkPrev)) return sparkPrev;
  return null;
}

export function metricsMarkup(
  symbol: string,
  rows: PriceRows,
  opts?: { pending?: boolean },
): string {
  const pending = opts?.pending === true;
  const primaryPrice =
    rows.primary.price != null ? formatPrice(rows.primary.price) : "—";
  const primaryChange = formatChangeParen(rows.primary.change);
  const primaryCls = [changeClass(rows.primary.change), pending ? "is-pending" : ""]
    .filter(Boolean)
    .join(" ");
  const secondaryPrice =
    rows.secondary.price != null ? formatPrice(rows.secondary.price) : "—";
  const secondaryChange = formatChangeParen(rows.secondary.change);
  const secondaryCls = [
    changeClass(rows.secondary.change),
    pending ? "is-pending" : "",
  ]
    .filter(Boolean)
    .join(" ");
  const pricePending = pending || rows.primary.price == null;

  return `
    <div class="row-metrics">
      <div class="row-quote row-quote--primary">
        <span class="row-price${pricePending ? " is-pending" : ""}" data-price-primary="${escapeAttr(symbol)}">${escapeHtml(primaryPrice)}</span><span class="row-change ${primaryCls}" data-change-primary="${escapeAttr(symbol)}"${primaryChange ? "" : " hidden"}>${escapeHtml(primaryChange)}</span>
      </div>
      <div class="row-quote row-quote--secondary">
        <span class="row-price${rows.secondary.price == null ? " is-pending" : ""}" data-price-secondary="${escapeAttr(symbol)}">${escapeHtml(secondaryPrice)}</span><span class="row-change ${secondaryCls}" data-change-secondary="${escapeAttr(symbol)}"${secondaryChange ? "" : " hidden"}>${escapeHtml(secondaryChange)}</span>
      </div>
    </div>
  `;
}

export function rowAriaLabel(
  symbol: string,
  rows: PriceRows,
  pending: boolean,
): string {
  if (pending || rows.primary.price == null) {
    return `${symbol}, waiting for quote`;
  }
  const price = formatPrice(rows.primary.price);
  const ch = formatChange(rows.primary.change);
  const dir =
    rows.primary.change == null || !Number.isFinite(rows.primary.change)
      ? ""
      : rows.primary.change > 0
        ? " up"
        : rows.primary.change < 0
          ? " down"
          : " flat";
  return ch ? `${symbol}, ${price}, ${ch}${dir}` : `${symbol}, ${price}`;
}

export function metricsFingerprint(rows: PriceRows): string {
  const p = (r: PriceRow) => `${r.price ?? ""}:${r.change ?? ""}`;
  return `${p(rows.primary)}|${p(rows.secondary)}`;
}

export function sparkFingerprint(sp: Sparkline | undefined): string {
  if (!sp) return "";
  const last = sp.points.length ? sp.points[sp.points.length - 1] : null;
  return `${sp.as_of}|${sp.points.length}|${last?.t ?? ""}|${last?.close ?? ""}|${sp.previous_close ?? ""}|${sp.session_start ?? ""}|${sp.session_end ?? ""}`;
}

export interface PatchMetricsResult {
  changed: boolean;
  primaryPriceChanged: boolean;
}

export function patchMetricsRow(
  row: HTMLElement,
  rows: PriceRows,
): PatchMetricsResult {
  const fp = metricsFingerprint(rows);
  const prevFp = row.dataset.metricsFp ?? "";
  if (fp === prevFp) {
    return { changed: false, primaryPriceChanged: false };
  }

  const prevPrimaryPrice = row.dataset.primaryPrice ?? "";
  const nextPrimaryPrice =
    rows.primary.price != null ? formatPrice(rows.primary.price) : "—";
  const primaryPriceChanged =
    prevFp !== "" && prevPrimaryPrice !== "" && prevPrimaryPrice !== nextPrimaryPrice;

  const primaryPriceEl = row.querySelector<HTMLElement>("[data-price-primary]");
  const primaryChangeEl = row.querySelector<HTMLElement>("[data-change-primary]");
  const secondaryPriceEl = row.querySelector<HTMLElement>("[data-price-secondary]");
  const secondaryChangeEl = row.querySelector<HTMLElement>("[data-change-secondary]");
  const pending = rows.primary.price == null;
  const symbol = row.dataset.symbol ?? "";

  if (primaryPriceEl) {
    primaryPriceEl.textContent = nextPrimaryPrice;
    primaryPriceEl.classList.toggle("is-pending", pending);
  }
  if (primaryChangeEl) {
    const txt = formatChangeParen(rows.primary.change);
    primaryChangeEl.textContent = txt;
    primaryChangeEl.hidden = !txt;
    primaryChangeEl.classList.remove("up", "down", "is-pending");
    const cls = changeClass(rows.primary.change);
    if (cls) primaryChangeEl.classList.add(cls);
    if (pending) primaryChangeEl.classList.add("is-pending");
  }
  if (secondaryPriceEl) {
    const sec =
      rows.secondary.price != null ? formatPrice(rows.secondary.price) : "—";
    secondaryPriceEl.textContent = sec;
    secondaryPriceEl.classList.toggle("is-pending", rows.secondary.price == null);
  }
  if (secondaryChangeEl) {
    const txt = formatChangeParen(rows.secondary.change);
    secondaryChangeEl.textContent = txt;
    secondaryChangeEl.hidden = !txt;
    secondaryChangeEl.classList.remove("up", "down", "is-pending");
    const cls = changeClass(rows.secondary.change);
    if (cls) secondaryChangeEl.classList.add(cls);
  }

  if (symbol) {
    row.setAttribute("aria-label", rowAriaLabel(symbol, rows, pending));
  }

  row.dataset.metricsFp = fp;
  row.dataset.primaryPrice = nextPrimaryPrice;
  return { changed: true, primaryPriceChanged };
}
