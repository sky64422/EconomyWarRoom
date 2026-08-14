/** Compact prices for narrow widget columns. */
export function formatPrice(price: number): string {
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
  if (a >= 1) return price.toFixed(2);
  if (a >= 0.01) return price.toFixed(3);
  return price.toPrecision(2);
}

export function formatChange(pct: number | null | undefined, compact = false): string {
  if (pct == null || !Number.isFinite(pct)) return "";
  const sign = pct > 0 ? "+" : "";
  const digits = compact ? 1 : 2;
  return `${sign}${pct.toFixed(digits)}%`;
}

/** Inline suffix: `(+0.42%)` — empty when no change. Gap vs price is CSS. */
export function formatChangeParen(pct: number | null | undefined, compact = false): string {
  const inner = formatChange(pct, compact);
  return inner ? `(${inner})` : "";
}

export function changeClass(pct: number | null | undefined): string {
  if (pct == null || !Number.isFinite(pct) || pct === 0) return "";
  return pct > 0 ? "up" : "down";
}

export function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

export function escapeAttr(s: string): string {
  return escapeHtml(s).replace(/'/g, "&#39;");
}
