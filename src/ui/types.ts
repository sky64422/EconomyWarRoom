/** Types matching Rust domain serde (snake_case fields; enum rename_all = "snake_case"). */

export type AssetKind = "equity" | "crypto" | "commodity" | "other";


export type CardTint =
  | "none"
  | "rose"
  | "peach"
  | "mint"
  | "sky"
  | "lavender"
  | "lemon";

export const CARD_TINTS: { value: CardTint; label: string }[] = [
  { value: "none", label: "Default" },
  { value: "rose", label: "Rose" },
  { value: "peach", label: "Peach" },
  { value: "mint", label: "Mint" },
  { value: "sky", label: "Sky" },
  { value: "lavender", label: "Lavender" },
  { value: "lemon", label: "Lemon" },
];

export interface WatchlistItem {
  id: string;
  symbol: string;
  display_name: string | null;
  asset_kind: AssetKind;
  sort_index: number;
  card_tint?: CardTint;
}

export interface Quote {
  symbol: string;
  price: number;
  currency: string;
  change_percent: number | null;
  as_of: string;
  source: string;
  previous_close?: number | null;
  regular_price?: number | null;
  regular_change_percent?: number | null;
  extended_price?: number | null;
  extended_change_percent?: number | null;
  prior_close?: number | null;
  previous_day_change_percent?: number | null;
  market_state?: string | null;
  /** Filled by Rust at IPC time (primary / secondary rows). */
  display?: PriceRows | null;
  sparkline_change_percent?: number | null;
}

export interface PriceRow {
  price: number | null;
  change: number | null;
}

export interface PriceRows {
  primary: PriceRow;
  secondary: PriceRow;
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
  /** Regular-session unix bounds; when set, x is time-of-session not index. */
  session_start?: number | null;
  session_end?: number | null;
}

export interface WindowGeometry {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** CSS fr shares for symbol · spark · metrics columns. */
export interface ColumnRatios {
  symbol: number;
  spark: number;
  metrics: number;
}

export const DEFAULT_COLUMN_RATIOS: ColumnRatios = {
  symbol: 1.15,
  spark: 1.25,
  metrics: 2.0,
};

export interface AppSettings {
  opacity: number;
  window: WindowGeometry;
  hotkey: string;
  autostart: boolean;
  /** Wire name is historical; value is milliseconds. Alias: quote_refresh_ms. */
  quote_refresh_secs?: number;
  quote_refresh_ms?: number;
  column_ratios?: ColumnRatios;
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
