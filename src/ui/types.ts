/** Types matching Rust domain serde (snake_case fields; enum rename_all = "snake_case"). */

export type AssetKind = "equity" | "crypto" | "commodity" | "other";

export type ThemeMode = "light" | "dark" | "system";

export interface WatchlistItem {
  id: string;
  symbol: string;
  display_name: string | null;
  asset_kind: AssetKind;
  sort_index: number;
}

export interface Quote {
  symbol: string;
  price: number;
  currency: string;
  change_percent: number | null;
  as_of: string;
  source: string;
}

export interface SparklinePoint {
  t: number;
  close: number;
}

export interface Sparkline {
  symbol: string;
  points: SparklinePoint[];
  previous_close: number | null;
  as_of: string;
}

export interface WindowGeometry {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface AppSettings {
  theme: ThemeMode;
  opacity: number;
  window: WindowGeometry;
  hotkey: string;
  autostart: boolean;
}

export interface PersistedState {
  watchlist: WatchlistItem[];
  settings: AppSettings;
}

/** Result of `search_symbols` (Yahoo autocomplete). */
export interface SymbolSuggestion {
  symbol: string;
  name: string | null;
  asset_kind: AssetKind;
  exchange: string | null;
}
