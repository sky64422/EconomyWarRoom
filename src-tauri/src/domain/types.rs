use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Equity,
    Crypto,
    Commodity,
    Other,
}

/// Soft pastel card highlight (user-picked); `None` / omit = default glass row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CardTint {
    #[default]
    None,
    Rose,
    Peach,
    Mint,
    Sky,
    Lavender,
    Lemon,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WatchlistItem {
    pub id: String,
    pub symbol: String,
    pub display_name: Option<String>,
    pub asset_kind: AssetKind,
    pub sort_index: u32,
    /// Soft pastel background for attention; defaults for older saved state.
    #[serde(default)]
    pub card_tint: CardTint,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quote {
    pub symbol: String,
    pub price: f64,
    pub currency: String,
    pub change_percent: Option<f64>,
    pub as_of: String,
    pub source: String,
    /// Previous session close (yesterday's reference).
    #[serde(default)]
    pub previous_close: Option<f64>,
    /// Official regular-session price (close when market has ended).
    #[serde(default)]
    pub regular_price: Option<f64>,
    /// Regular-session % change vs [`previous_close`].
    #[serde(default)]
    pub regular_change_percent: Option<f64>,
    /// Pre/post extended-session price when applicable.
    #[serde(default)]
    pub extended_price: Option<f64>,
    /// Extended-session % change vs [`regular_price`].
    #[serde(default)]
    pub extended_change_percent: Option<f64>,
    /// Close from the prior trading day (for "yesterday's" move).
    #[serde(default)]
    pub prior_close: Option<f64>,
    /// % change on the day that ended at [`previous_close`].
    #[serde(default)]
    pub previous_day_change_percent: Option<f64>,
    /// Yahoo market state hint: `regular`, `pre`, `post`, `closed`, etc.
    #[serde(default)]
    pub market_state: Option<String>,
    /// Widget primary/secondary rows; filled at IPC time, not Yahoo parse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<PriceRows>,
    /// Regular-session move used to color the sparkline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sparkline_change_percent: Option<f64>,
}

/// One price + % cell (primary or secondary).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct PriceRow {
    pub price: Option<f64>,
    pub change: Option<f64>,
}

/// Display pair: latest session print vs last completed regular session.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct PriceRows {
    pub primary: PriceRow,
    pub secondary: PriceRow,
}

impl Default for Quote {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            price: 0.0,
            currency: "USD".into(),
            change_percent: None,
            as_of: String::new(),
            source: String::new(),
            previous_close: None,
            regular_price: None,
            regular_change_percent: None,
            extended_price: None,
            extended_change_percent: None,
            prior_close: None,
            previous_day_change_percent: None,
            market_state: None,
            display: None,
            sparkline_change_percent: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SparklinePoint {
    pub t: i64,
    pub close: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sparkline {
    pub symbol: String,
    pub points: Vec<SparklinePoint>,
    pub previous_close: Option<f64>,
    pub as_of: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowGeometry {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Symbol search hit for add-flow autocomplete.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolSuggestion {
    pub symbol: String,
    pub name: Option<String>,
    pub asset_kind: AssetKind,
    pub exchange: Option<String>,
}

/// CSS `fr` shares for watchlist columns (symbol · sparkline · metrics).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ColumnRatios {
    pub symbol: f64,
    pub spark: f64,
    pub metrics: f64,
}

impl Default for ColumnRatios {
    fn default() -> Self {
        use crate::domain::constants::ColumnRatioPolicy;
        Self {
            symbol: ColumnRatioPolicy::DEFAULT_SYMBOL,
            spark: ColumnRatioPolicy::DEFAULT_SPARK,
            metrics: ColumnRatioPolicy::DEFAULT_METRICS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub opacity: f64,
    pub window: WindowGeometry,
    pub hotkey: String,
    pub autostart: bool,
    /// Quote refresh interval per symbol.
    ///
    /// Quote refresh interval in **milliseconds**.
    ///
    /// JSON key stays `quote_refresh_secs` (legacy). Values 1..=120 on load are
    /// whole seconds; everything else is milliseconds.
    #[serde(
        default = "default_quote_refresh_ms",
        rename = "quote_refresh_secs",
        alias = "quote_refresh_ms"
    )]
    pub quote_refresh_ms: u64,
    /// Proportional column widths; omitted in older state files → defaults.
    #[serde(default)]
    pub column_ratios: ColumnRatios,
}

fn default_quote_refresh_ms() -> u64 {
    crate::domain::constants::RefreshPolicy::QUOTE_REFRESH_MS_DEFAULT
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersistedState {
    pub watchlist: Vec<WatchlistItem>,
    pub settings: AppSettings,
}
