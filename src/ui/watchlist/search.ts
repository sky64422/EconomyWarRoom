import type { AssetKind, SymbolSuggestion } from "../types";

/** Local fallback catalog (substring filter) when network is slow/offline. */
export const LOCAL_SYMBOLS: SymbolSuggestion[] = [
  { symbol: "AAPL", name: "Apple Inc.", asset_kind: "equity", exchange: "NASDAQ" },
  { symbol: "MSFT", name: "Microsoft Corporation", asset_kind: "equity", exchange: "NASDAQ" },
  { symbol: "GOOGL", name: "Alphabet Inc.", asset_kind: "equity", exchange: "NASDAQ" },
  { symbol: "AMZN", name: "Amazon.com Inc.", asset_kind: "equity", exchange: "NASDAQ" },
  { symbol: "NVDA", name: "NVIDIA Corporation", asset_kind: "equity", exchange: "NASDAQ" },
  { symbol: "META", name: "Meta Platforms Inc.", asset_kind: "equity", exchange: "NASDAQ" },
  { symbol: "TSLA", name: "Tesla Inc.", asset_kind: "equity", exchange: "NASDAQ" },
  { symbol: "PLTR", name: "Palantir Technologies Inc.", asset_kind: "equity", exchange: "NASDAQ" },
  { symbol: "AVGO", name: "Broadcom Inc.", asset_kind: "equity", exchange: "NASDAQ" },
  { symbol: "NFLX", name: "Netflix Inc.", asset_kind: "equity", exchange: "NASDAQ" },
  { symbol: "SPY", name: "SPDR S&P 500 ETF", asset_kind: "equity", exchange: "NYSE" },
  { symbol: "QQQ", name: "Invesco QQQ Trust", asset_kind: "equity", exchange: "NASDAQ" },
  { symbol: "IWM", name: "iShares Russell 2000 ETF", asset_kind: "equity", exchange: "NYSE" },
  { symbol: "005930.KS", name: "삼성전자", asset_kind: "equity", exchange: "KOSPI" },
  { symbol: "000660.KS", name: "SK하이닉스", asset_kind: "equity", exchange: "KOSPI" },
  { symbol: "005380.KS", name: "현대차", asset_kind: "equity", exchange: "KOSPI" },
  { symbol: "035420.KS", name: "NAVER", asset_kind: "equity", exchange: "KOSPI" },
  { symbol: "035720.KS", name: "카카오", asset_kind: "equity", exchange: "KOSPI" },
  { symbol: "086520.KQ", name: "에코프로", asset_kind: "equity", exchange: "KOSDAQ" },
  { symbol: "247540.KQ", name: "에코프로비엠", asset_kind: "equity", exchange: "KOSDAQ" },
  { symbol: "196170.KQ", name: "알테오젠", asset_kind: "equity", exchange: "KOSDAQ" },
  { symbol: "BTC-USD", name: "Bitcoin USD", asset_kind: "crypto", exchange: "CCC" },
  { symbol: "ETH-USD", name: "Ethereum USD", asset_kind: "crypto", exchange: "CCC" },
  { symbol: "SOL-USD", name: "Solana USD", asset_kind: "crypto", exchange: "CCC" },
];

const SEARCH_ALIASES: Record<string, string[]> = {
  AAPL: ["애플"],
  MSFT: ["마이크로소프트", "마소"],
  GOOGL: ["구글", "알파벳"],
  AMZN: ["아마존"],
  NVDA: ["엔비디아"],
  META: ["메타", "페이스북"],
  TSLA: ["테슬라"],
  PLTR: ["팔란티어"],
  AVGO: ["브로드컴"],
  NFLX: ["넷플릭스"],
  SPY: ["에스앤피", "S&P500"],
  QQQ: ["나스닥100", "큐큐큐"],
  IWM: ["러셀2000"],
  "005930.KS": ["삼성전자", "삼전", "SAMSUNG"],
  "000660.KS": ["SK하이닉스", "하이닉스", "SK HYNIX"],
  "005380.KS": ["현대차", "현대자동차", "HYUNDAI"],
  "035420.KS": ["네이버", "NAVER"],
  "035720.KS": ["카카오", "KAKAO"],
  "086520.KQ": ["에코프로", "ECOPRO"],
  "247540.KQ": ["에코프로비엠", "ECOPROBM"],
  "196170.KQ": ["알테오젠", "ALTEOGEN"],
  "BTC-USD": ["비트코인", "비트"],
  "ETH-USD": ["이더리움", "이더"],
  "SOL-USD": ["솔라나"],
};

export function guessAssetKind(symbol: string): AssetKind {
  const s = symbol.trim().toUpperCase();
  if (s.includes("-") || s.endsWith("USD")) return "crypto";
  return "equity";
}

export function localSuggestions(
  q: string,
  ownedSymbols: Iterable<string>,
): SymbolSuggestion[] {
  const u = q.trim().toUpperCase();
  if (!u) return [];
  const owned = new Set([...ownedSymbols].map((s) => s.toUpperCase()));
  return LOCAL_SYMBOLS.filter((s) => {
    if (owned.has(s.symbol)) return false;
    if (s.symbol.includes(u) || (s.name ?? "").toUpperCase().includes(u)) {
      return true;
    }
    const aliases = SEARCH_ALIASES[s.symbol] ?? [];
    return aliases.some((alias) => alias.toUpperCase().includes(u));
  }).slice(0, 8);
}

export function mergeSuggestions(
  remote: SymbolSuggestion[],
  local: SymbolSuggestion[],
  ownedSymbols: Iterable<string>,
): SymbolSuggestion[] {
  const owned = new Set([...ownedSymbols].map((s) => s.toUpperCase()));
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
