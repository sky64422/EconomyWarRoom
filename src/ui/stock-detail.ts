import type { WatchlistItem, Quote, Sparkline } from "./types";
import { priceRowsForQuote, metricsMarkup, patchMetricsRow, sparklineChangePercent, sparklineBaseline } from "./watchlist/metrics";
import { escapeHtml, escapeAttr } from "./watchlist/format";
import { sparklineSvgMarkup } from "./sparkline";
import { markMarkup } from "./stock-marks";

const SPARK_WIDTH = 224;
const SPARK_HEIGHT = 52;
export function mountStockDetail(root: HTMLElement) {
  let currentId = "";
  let lastSpark = "";
  return {
    update(item: WatchlistItem, quote: Quote | undefined, spark: Sparkline | undefined) {
      const rows = priceRowsForQuote(quote);
      if (currentId !== item.id) {
        currentId=item.id;
        lastSpark="";
        root.innerHTML = `<div class="watchlist-row stock-quote" data-id="${escapeAttr(item.id)}" data-symbol="${escapeAttr(item.symbol)}" aria-label="${escapeAttr(item.display_name || item.symbol)}, ${escapeAttr(item.symbol)}"><span class="row-symbol stock-identity"><span class="stock-symbol-mark">${markMarkup(item.symbol)}</span><strong>${escapeHtml(item.symbol)}</strong></span><div class="row-market">${metricsMarkup(item.symbol, rows)}<div class="row-sparkline-wrap"><svg class="row-sparkline" viewBox="0 0 ${SPARK_WIDTH} ${SPARK_HEIGHT}" preserveAspectRatio="none" aria-label="가격 스파크라인"></svg></div></div></div>`;
      }
      const row=root.querySelector<HTMLElement>(".stock-quote")!;
      const className=`watchlist-row stock-quote${item.card_tint && item.card_tint!=='none' ? ' tint-'+item.card_tint : ''}`;
      if(row.className!==className)row.className=className;
      patchMetricsRow(row,rows);
      const pct=sparklineChangePercent(quote);
      const color=pct === null || pct===0 ? "var(--sparkline-neutral)" : pct>0 ? "var(--sparkline-up)" : "var(--sparkline-down)";
      const baseline=sparklineBaseline(quote,spark?.previous_close);
      // Include every point: an earlier sample can be corrected without changing the tail.
      const sparkKey=JSON.stringify([spark?.points,spark?.session_start,spark?.session_end,item.asset_kind,color,baseline]);
      if(lastSpark!==sparkKey){
        row.querySelector(".row-sparkline")!.innerHTML=sparklineSvgMarkup(spark?.points ?? [],SPARK_WIDTH,SPARK_HEIGHT,{id:`detail-${item.id}`,assetKind:item.asset_kind,stroke:color,sessionStart:spark?.session_start,sessionEnd:spark?.session_end,variant:'detail'},baseline);
        lastSpark=sparkKey;
      }
      row.querySelector(".row-sparkline-wrap")!.classList.toggle("is-pending",!spark?.points.length);
    },
  };
}
