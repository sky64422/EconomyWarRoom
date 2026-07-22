use crate::application::cache::{QuoteCache, SparklineCache};
use crate::domain::constants::RefreshPolicy;
use crate::domain::types::WatchlistItem;
use crate::ports::market_data::MarketDataProvider;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Pure round-robin batch picker for stale symbols.
///
/// Respects `min_interval` via `last_fetch`, prefers `priority_symbol` first when stale,
/// advances `cursor` so subsequent calls continue around the watchlist.
pub fn pick_batch(
    items: &[WatchlistItem],
    last_fetch: &HashMap<String, Instant>,
    now: Instant,
    min_interval: Duration,
    batch_size: usize,
    cursor: &mut usize,
    priority_symbol: Option<&str>,
) -> Vec<String> {
    if items.is_empty() || batch_size == 0 {
        return vec![];
    }
    let mut out = Vec::new();
    if let Some(sym) = priority_symbol {
        if items.iter().any(|i| i.symbol == sym) {
            let stale = last_fetch
                .get(sym)
                .map(|t| now.duration_since(*t) >= min_interval)
                .unwrap_or(true);
            if stale {
                out.push(sym.to_string());
            }
        }
    }
    let n = items.len();
    let start = *cursor % n;
    for offset in 0..n {
        if out.len() >= batch_size {
            break;
        }
        let idx = (start + offset) % n;
        let sym = &items[idx].symbol;
        if out.iter().any(|s| s == sym) {
            continue;
        }
        let stale = last_fetch
            .get(sym)
            .map(|t| now.duration_since(*t) >= min_interval)
            .unwrap_or(true);
        if stale {
            out.push(sym.clone());
        }
    }
    *cursor = start.wrapping_add(out.len().max(1));
    out
}

/// Quote refresh scheduler: round-robin batches, min interval, pause when hidden.
pub struct QuoteScheduler {
    visible: bool,
    watchlist: Vec<WatchlistItem>,
    quote_cache: QuoteCache,
    sparkline_cache: SparklineCache,
    last_quote_fetch: HashMap<String, Instant>,
    last_spark_fetch: HashMap<String, Instant>,
    cursor: usize,
    priority: Option<String>,
    provider: Arc<dyn MarketDataProvider>,
}

impl QuoteScheduler {
    pub fn new(provider: Arc<dyn MarketDataProvider>) -> Self {
        Self {
            visible: true,
            watchlist: Vec::new(),
            quote_cache: QuoteCache::default(),
            sparkline_cache: SparklineCache::default(),
            last_quote_fetch: HashMap::new(),
            last_spark_fetch: HashMap::new(),
            cursor: 0,
            priority: None,
            provider,
        }
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn set_watchlist(&mut self, items: Vec<WatchlistItem>) {
        self.watchlist = items;
    }

    pub fn bump_priority(&mut self, symbol: impl Into<String>) {
        self.priority = Some(symbol.into());
    }

    pub fn quote_cache(&self) -> &QuoteCache {
        &self.quote_cache
    }

    pub fn sparkline_cache(&self) -> &SparklineCache {
        &self.sparkline_cache
    }

    /// One scheduler tick: no-op when not visible; otherwise pick a batch and fetch quotes.
    pub async fn tick_once(&mut self) {
        if !self.visible {
            return;
        }
        if self.watchlist.is_empty() {
            return;
        }

        let now = Instant::now();
        let min_interval = self
            .provider
            .limits()
            .min_interval
            .max(RefreshPolicy::MIN_QUOTE_INTERVAL);
        let priority = self.priority.take();
        let batch = pick_batch(
            &self.watchlist,
            &self.last_quote_fetch,
            now,
            min_interval,
            RefreshPolicy::BATCH_SIZE,
            &mut self.cursor,
            priority.as_deref(),
        );
        if batch.is_empty() {
            return;
        }

        match self.provider.fetch_quotes(&batch).await {
            Ok(quotes) => {
                let fetched_at = Instant::now();
                for q in quotes {
                    self.last_quote_fetch.insert(q.symbol.clone(), fetched_at);
                    self.quote_cache.put(q);
                }
                // Mark requested symbols as fetched even if provider omitted them,
                // so we do not hammer missing symbols every tick.
                for sym in &batch {
                    self.last_quote_fetch
                        .entry(sym.clone())
                        .or_insert(fetched_at);
                }
            }
            Err(_err) => {
                // Backoff wiring lands with the Yahoo provider task.
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{AssetKind, Quote, Sparkline};
    use crate::ports::market_data::{MarketDataProvider, ProviderLimits};
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::Mutex;

    fn item(sym: &str, idx: u32) -> WatchlistItem {
        WatchlistItem {
            id: sym.to_string(),
            symbol: sym.to_string(),
            display_name: None,
            asset_kind: AssetKind::Equity,
            sort_index: idx,
        }
    }

    #[test]
    fn round_robin_respects_batch_and_staleness() {
        let items = vec![item("A", 0), item("B", 1), item("C", 2), item("D", 3)];
        let now = Instant::now();
        let mut last = HashMap::new();
        last.insert("A".into(), now); // fresh
        let mut cursor = 0;
        let batch = pick_batch(
            &items,
            &last,
            now,
            Duration::from_secs(10),
            2,
            &mut cursor,
            None,
        );
        assert!(!batch.contains(&"A".to_string()));
        assert_eq!(batch.len(), 2);
    }

    #[test]
    fn priority_symbol_first() {
        let items = vec![item("A", 0), item("B", 1), item("C", 2)];
        let now = Instant::now();
        let last = HashMap::new();
        let mut cursor = 0;
        let batch = pick_batch(
            &items,
            &last,
            now,
            Duration::from_secs(10),
            2,
            &mut cursor,
            Some("C"),
        );
        assert_eq!(batch[0], "C");
    }

    struct MockProvider {
        quotes: Mutex<Vec<Quote>>,
        calls: AtomicUsize,
    }

    impl MockProvider {
        fn new(quotes: Vec<Quote>) -> Self {
            Self {
                quotes: Mutex::new(quotes),
                calls: AtomicUsize::new(0),
            }
        }

        fn call_count(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl MarketDataProvider for MockProvider {
        fn id(&self) -> &'static str {
            "mock"
        }

        fn supports(&self, _: AssetKind) -> bool {
            true
        }

        fn limits(&self) -> ProviderLimits {
            ProviderLimits {
                max_concurrent: 2,
                // Below RefreshPolicy::MIN_QUOTE_INTERVAL so policy floor still applies.
                min_interval: Duration::from_secs(1),
                prefers_batch: true,
            }
        }

        async fn fetch_quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let all = self.quotes.lock().await;
            Ok(all
                .iter()
                .filter(|q| symbols.contains(&q.symbol))
                .cloned()
                .collect())
        }

        async fn fetch_sparkline(
            &self,
            symbol: &str,
            _: &str,
            _: &str,
        ) -> Result<Sparkline, String> {
            Ok(Sparkline {
                symbol: symbol.into(),
                points: vec![],
                previous_close: None,
                as_of: "2026-01-01T00:00:00Z".into(),
            })
        }
    }

    fn quote(sym: &str, price: f64) -> Quote {
        Quote {
            symbol: sym.into(),
            price,
            currency: "USD".into(),
            change_percent: None,
            as_of: "2026-01-01T00:00:00Z".into(),
            source: "mock".into(),
        }
    }

    #[tokio::test]
    async fn hidden_scheduler_does_not_call_provider() {
        let provider = Arc::new(MockProvider::new(vec![quote("A", 1.0)]));
        let mut sched = QuoteScheduler::new(provider.clone());
        sched.set_watchlist(vec![item("A", 0)]);
        sched.set_visible(false);
        sched.tick_once().await;
        assert_eq!(provider.call_count(), 0);
    }

    #[tokio::test]
    async fn visible_tick_fetches_and_caches() {
        let provider = Arc::new(MockProvider::new(vec![quote("A", 10.0), quote("B", 20.0)]));
        let mut sched = QuoteScheduler::new(provider.clone());
        sched.set_watchlist(vec![item("A", 0), item("B", 1)]);
        sched.set_visible(true);
        sched.tick_once().await;
        assert_eq!(provider.call_count(), 1);
        assert_eq!(sched.quote_cache().get("A").map(|q| q.price), Some(10.0));
        assert_eq!(sched.quote_cache().get("B").map(|q| q.price), Some(20.0));
    }
}
