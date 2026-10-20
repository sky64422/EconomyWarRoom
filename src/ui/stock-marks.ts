import apple from "../assets/marks/apple.svg";
import bitcoin from "../assets/marks/bitcoin.svg";
import ethereum from "../assets/marks/ethereum.svg";
import nvidia from "../assets/marks/nvidia.svg";
import tesla from "../assets/marks/tesla.svg";
import google from "../assets/marks/google.svg";
import meta from "../assets/marks/meta.svg";
import netflix from "../assets/marks/netflix.svg";
import { escapeAttr, escapeHtml } from "./watchlist/format";
const MARKS: Record<string,string> = {"AAPL": apple, "BTC-USD": bitcoin, "ETH-USD": ethereum, "NVDA": nvidia, "TSLA": tesla, "GOOG": google, "GOOGL": google, "META": meta, "NFLX": netflix};
export function markMarkup(symbol: string): string {
 const source=MARKS[symbol];
 return source ? `<img src="${escapeAttr(source)}" alt="" draggable="false" data-symbol="${escapeAttr(symbol)}"/>` : `<span class="mark-fallback" aria-hidden="true">${escapeHtml(symbol.replace(/[-.].*$/,'').slice(0,3))}</span>`;
}
