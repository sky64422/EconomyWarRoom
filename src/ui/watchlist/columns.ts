import type { ColumnRatios } from "../types";

export const RESIZE_HIT_PX = 7;
const MIN_SYMBOL_PX = 44;
const MIN_SPARK_PX = 36;
const MIN_METRICS_PX = 64;

export function applyColumnRatiosCss(el: HTMLElement, r: ColumnRatios): void {
  el.style.setProperty("--row-fr-symbol", `${r.symbol}fr`);
  el.style.setProperty("--row-fr-spark", `${r.spark}fr`);
  el.style.setProperty("--row-fr-metrics", `${r.metrics}fr`);
}

/** Drag edge 0 = symbol|spark, 1 = spark|metrics. Keeps third column fixed in px space. */
export function resizeColumnRatios(
  edge: 0 | 1,
  dx: number,
  start: ColumnRatios,
  freePx: number,
): ColumnRatios {
  const sum = start.symbol + start.spark + start.metrics;
  if (!(sum > 0) || !(freePx > 0) || !Number.isFinite(dx)) return start;
  const toPx = (fr: number) => (fr / sum) * freePx;
  const toFr = (px: number) => (px / freePx) * sum;
  let s = toPx(start.symbol);
  let p = toPx(start.spark);
  let m = toPx(start.metrics);
  if (edge === 0) {
    const maxS = s + p - MIN_SPARK_PX;
    s = Math.min(Math.max(s + dx, MIN_SYMBOL_PX), Math.max(MIN_SYMBOL_PX, maxS));
    p = freePx - m - s;
  } else {
    const maxP = p + m - MIN_METRICS_PX;
    p = Math.min(Math.max(p + dx, MIN_SPARK_PX), Math.max(MIN_SPARK_PX, maxP));
    m = freePx - s - p;
  }
  return {
    symbol: Math.max(0.45, toFr(Math.max(0, s))),
    spark: Math.max(0.45, toFr(Math.max(0, p))),
    metrics: Math.max(0.45, toFr(Math.max(0, m))),
  };
}
