use crate::domain::types::{AssetKind, Quote, Sparkline, SymbolSuggestion};
use async_trait::async_trait;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ProviderLimits {
    pub max_concurrent: usize,
    pub min_interval: Duration,
    pub prefers_batch: bool,
}

#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn supports(&self, kind: AssetKind) -> bool;
    fn limits(&self) -> ProviderLimits;
    async fn fetch_quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, String>;
    async fn fetch_sparkline(
        &self,
        symbol: &str,
        range: &str,
        interval: &str,
    ) -> Result<Sparkline, String>;

    /// Symbol autocomplete. Default is empty (providers without search).
    async fn search_symbols(
        &self,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<SymbolSuggestion>, String> {
        Ok(vec![])
    }
}
