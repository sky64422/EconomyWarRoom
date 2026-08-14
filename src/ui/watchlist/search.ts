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
  { symbol: "SPY", name: "SPDR S&P 500 ETF", asset_kind: "equity", exchange: "NYSE" },
  { symbol: "QQQ", name: "Invesco QQQ Trust", asset_kind: "equity", exchange: "NASDAQ" },
  { symbol: "IWM", name: "iShares Russell 2000 ETF", asset_kind: "equity", exchange: "NYSE" },
  { symbol: "BTC-USD", name: "Bitcoin USD", asset_kind: "crypto", exchange: "CCC" },
  { symbol: "ETH-USD", name: "Ethereum USD", asset_kind: "crypto", exchange: "CCC" },
  { symbol: "SOL-USD", name: "Solana USD", asset_kind: "crypto", exchange: "CCC" },
];

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
  return LOCAL_SYMBOLS.filter(
    (s) =>
      !owned.has(s.symbol) &&
      (s.symbol.includes(u) || (s.name ?? "").toUpperCase().includes(u)),
  ).slice(0, 8);
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
