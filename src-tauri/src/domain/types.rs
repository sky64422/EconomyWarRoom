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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    Light,
    Dark,
    System,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub theme: ThemeMode,
    pub opacity: f64,
    pub window: WindowGeometry,
    pub hotkey: String,
    pub autostart: bool,
    /// Seconds between quote refreshes per symbol (clamped on write).
    #[serde(default = "default_quote_refresh_secs")]
    pub quote_refresh_secs: u64,
}

fn default_quote_refresh_secs() -> u64 {
    10
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersistedState {
    pub watchlist: Vec<WatchlistItem>,
    pub settings: AppSettings,
}
