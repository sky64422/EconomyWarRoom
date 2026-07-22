use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Equity,
    Crypto,
    Commodity,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WatchlistItem {
    pub id: String,
    pub symbol: String,
    pub display_name: Option<String>,
    pub asset_kind: AssetKind,
    pub sort_index: u32,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub theme: ThemeMode,
    pub opacity: f64,
    pub window: WindowGeometry,
    pub hotkey: String,
    pub autostart: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersistedState {
    pub watchlist: Vec<WatchlistItem>,
    pub settings: AppSettings,
}
