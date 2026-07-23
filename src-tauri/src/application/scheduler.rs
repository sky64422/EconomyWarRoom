use crate::application::cache::{QuoteCache, SparklineCache};
use crate::domain::constants::{RefreshPolicy, SparklinePolicy};
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

/// Max sparkline fetches attempted in a single tick (avoid API burst).
const SPARKLINE_FETCHES_PER_TICK: usize = 1;

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
    /// When set, skip network work until this instant.
    backoff_until: Option<Instant>,
    /// Current backoff duration; doubles on each error up to [`RefreshPolicy::BACKOFF_MAX`].
    backoff: Duration,
    /// Last quote/sparkline provider error message (for diagnostics).
    last_error: Option<String>,
    /// Provider errors since last [`drain_diag_notes`] (for ring buffer, not spammy success logs).
    pending_diag: Vec<String>,
    /// User-configurable min interval between quote fetches (same symbol).
    min_quote_interval: Duration,
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
            backoff_until: None,
            backoff: RefreshPolicy::BACKOFF_INITIAL,
            last_error: None,
            pending_diag: Vec::new(),
            min_quote_interval: RefreshPolicy::MIN_QUOTE_INTERVAL,
        }
    }

    /// Take diagnostics lines produced by recent ticks (usually 0–1).
    pub fn drain_diag_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_diag)
    }

    /// One-line scheduler status for diagnostics dumps.
    pub fn diagnostics_summary(&self) -> String {
        let backoff_active = self
            .backoff_until
            .map(|u| Instant::now() < u)
            .unwrap_or(false);
        let err = self.last_error.as_deref().unwrap_or("(none)");
        format!(
            "visible={} watchlist_len={} quote_interval_secs={} backoff_active={} backoff_secs={} last_error={}",
            self.visible,
            self.watchlist.len(),
            self.min_quote_interval.as_secs(),
            backoff_active,
            self.backoff.as_secs_f64(),
            err
        )
    }

    pub fn set_min_quote_interval(&mut self, interval: Duration) {
        self.min_quote_interval = interval.max(Duration::from_secs(
            RefreshPolicy::QUOTE_REFRESH_SECS_MIN,
        ));
    }

    pub fn min_quote_interval(&self) -> Duration {
        self.min_quote_interval
    }

    pub fn set_visible(&mut self, visible: bool) {
        if visible {
            // Force-refresh: mark all symbols stale so the next tick fetches immediately.
            self.last_quote_fetch.clear();
            self.backoff_until = None;
        }
        self.visible = visible;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Whether network work is currently suppressed due to error backoff.
    pub fn is_backing_off(&self) -> bool {
        self.backoff_until
            .map(|until| Instant::now() < until)
            .unwrap_or(false)
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

    /// One scheduler tick: no-op when not visible or in backoff; otherwise refresh quotes
    /// and at most one stale sparkline.
    pub async fn tick_once(&mut self) {
        if !self.visible {
            return;
        }
        if self.watchlist.is_empty() {
            return;
        }

        let now = Instant::now();
        if let Some(until) = self.backoff_until {
            if now < until {
                return;
            }
        }

        let min_interval = self
            .provider
            .limits()
            .min_interval
            .max(self.min_quote_interval);
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

        if !batch.is_empty() {
            match self.provider.fetch_quotes(&batch).await {
                Ok(quotes) => {
                    self.backoff = RefreshPolicy::BACKOFF_INITIAL;
                    self.backoff_until = None;
                    self.last_error = None;

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
                Err(err) => {
                    // Keep existing cache; back off network work.
                    let msg = format!("quotes: {err}");
                    self.last_error = Some(msg.clone());
                    self.pending_diag.push(msg);
                    self.backoff_until = Some(Instant::now() + self.backoff);
                    self.backoff = (self.backoff * 2).min(RefreshPolicy::BACKOFF_MAX);
                    return;
                }
            }
        }

        self.maybe_fetch_sparkline().await;
    }

    /// Fetch up to [`SPARKLINE_FETCHES_PER_TICK`] stale sparklines.
    async fn maybe_fetch_sparkline(&mut self) {
        let now = Instant::now();
        let mut fetched = 0usize;
        let symbols: Vec<String> = self.watchlist.iter().map(|i| i.symbol.clone()).collect();

        for sym in symbols {
            if fetched >= SPARKLINE_FETCHES_PER_TICK {
                break;
            }
            let stale = self
                .last_spark_fetch
                .get(&sym)
                .map(|t| now.duration_since(*t) >= RefreshPolicy::SPARKLINE_MIN_INTERVAL)
                .unwrap_or(true);
            if !stale {
                continue;
            }

            match self
                .provider
                .fetch_sparkline(&sym, SparklinePolicy::RANGE, SparklinePolicy::INTERVAL)
                .await
            {
                Ok(spark) => {
                    let at = Instant::now();
                    self.last_spark_fetch.insert(sym, at);
                    self.sparkline_cache.put(spark);
                    self.last_error = None;
                    fetched += 1;
                }
                Err(err) => {
                    // Same backoff path as quote errors to protect the provider.
                    let msg = format!("sparkline {sym}: {err}");
                    self.last_error = Some(msg.clone());
                    self.pending_diag.push(msg);
                    self.backoff_until = Some(Instant::now() + self.backoff);
                    self.backoff = (self.backoff * 2).min(RefreshPolicy::BACKOFF_MAX);
                    return;
                }
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
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use tokio::sync::Mutex;

    fn item(sym: &str, idx: u32) -> WatchlistItem {
        WatchlistItem {
            id: sym.to_string(),
            symbol: sym.to_string(),
            display_name: None,
            asset_kind: AssetKind::Equity,
            sort_index: idx,
            card_tint: Default::default(),
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
        spark_calls: AtomicUsize,
        fail_quotes: AtomicBool,
    }

    impl MockProvider {
        fn new(quotes: Vec<Quote>) -> Self {
            Self {
                quotes: Mutex::new(quotes),
                calls: AtomicUsize::new(0),
                spark_calls: AtomicUsize::new(0),
                fail_quotes: AtomicBool::new(false),
            }
        }

        fn call_count(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }

        fn spark_call_count(&self) -> usize {
            self.spark_calls.load(Ordering::SeqCst)
        }

        fn set_fail_quotes(&self, fail: bool) {
            self.fail_quotes.store(fail, Ordering::SeqCst);
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
            if self.fail_quotes.load(Ordering::SeqCst) {
                return Err("rate_limited".into());
            }
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
            self.spark_calls.fetch_add(1, Ordering::SeqCst);
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

    #[tokio::test]
    async fn provider_error_queues_diag_note() {
        let provider = Arc::new(MockProvider::new(vec![quote("MSFT", 1.0)]));
        provider.set_fail_quotes(true);
        let mut sched = QuoteScheduler::new(provider);
        sched.set_watchlist(vec![item("MSFT", 0)]);
        sched.tick_once().await;
        let notes = sched.drain_diag_notes();
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("quotes:"));
        assert!(sched.drain_diag_notes().is_empty());
    }

    #[tokio::test]
    async fn rate_limited_applies_backoff_and_skips_next_tick() {
        let provider = Arc::new(MockProvider::new(vec![quote("A", 10.0)]));
        provider.set_fail_quotes(true);
        let mut sched = QuoteScheduler::new(provider.clone());
        sched.set_watchlist(vec![item("A", 0)]);
        sched.set_visible(true);

        sched.tick_once().await;
        assert_eq!(provider.call_count(), 1);
        assert!(sched.backoff_until.is_some());
        assert_eq!(sched.backoff, RefreshPolicy::BACKOFF_INITIAL * 2);

        // Second tick while still in backoff must not hit the provider.
        sched.tick_once().await;
        assert_eq!(provider.call_count(), 1);
    }

    #[tokio::test]
    async fn sparkline_fetched_when_cache_empty() {
        let provider = Arc::new(MockProvider::new(vec![quote("A", 10.0)]));
        let mut sched = QuoteScheduler::new(provider.clone());
        sched.set_watchlist(vec![item("A", 0)]);
        sched.set_visible(true);

        assert!(sched.sparkline_cache().get("A").is_none());
        sched.tick_once().await;

        assert_eq!(provider.spark_call_count(), 1);
        assert!(sched.sparkline_cache().get("A").is_some());
        assert_eq!(
            sched.sparkline_cache().get("A").map(|s| s.symbol.as_str()),
            Some("A")
        );
    }

    #[tokio::test]
    async fn set_visible_true_marks_quotes_stale() {
        let provider = Arc::new(MockProvider::new(vec![quote("A", 10.0)]));
        let mut sched = QuoteScheduler::new(provider.clone());
        sched.set_watchlist(vec![item("A", 0)]);
        sched.set_visible(true);
        sched.tick_once().await;
        assert_eq!(provider.call_count(), 1);

        // Fresh symbols would not re-fetch without force-stale.
        sched.tick_once().await;
        assert_eq!(provider.call_count(), 1);

        sched.set_visible(true);
        sched.tick_once().await;
        assert_eq!(provider.call_count(), 2);
    }

    #[tokio::test]
    async fn empty_watchlist_is_noop() {
        let provider = Arc::new(MockProvider::new(vec![]));
        let mut sched = QuoteScheduler::new(provider.clone());
        sched.set_visible(true);
        sched.tick_once().await;
        assert_eq!(provider.call_count(), 0);
    }

    #[tokio::test]
    async fn bump_priority_fetches_symbol_first() {
        let provider = Arc::new(MockProvider::new(vec![
            quote("A", 1.0),
            quote("B", 2.0),
            quote("C", 3.0),
        ]));
        let mut sched = QuoteScheduler::new(provider.clone());
        sched.set_watchlist(vec![item("A", 0), item("B", 1), item("C", 2)]);
        sched.bump_priority("C");
        sched.tick_once().await;
        assert!(sched.quote_cache().get("C").is_some());
    }

    #[tokio::test]
    async fn is_visible_reflects_flag() {
        let provider = Arc::new(MockProvider::new(vec![]));
        let mut sched = QuoteScheduler::new(provider);
        assert!(sched.is_visible());
        sched.set_visible(false);
        assert!(!sched.is_visible());
    }

    #[test]
    fn pick_batch_empty_or_zero_size() {
        let items = vec![item("A", 0)];
        let last = HashMap::new();
        let mut cursor = 0;
        assert!(pick_batch(
            &items,
            &last,
            Instant::now(),
            Duration::from_secs(1),
            0,
            &mut cursor,
            None
        )
        .is_empty());
        assert!(pick_batch(
            &[],
            &last,
            Instant::now(),
            Duration::from_secs(1),
            2,
            &mut cursor,
            None
        )
        .is_empty());
    }

    struct FailSparkProvider {
        inner: MockProvider,
    }

    #[async_trait]
    impl MarketDataProvider for FailSparkProvider {
        fn id(&self) -> &'static str {
            "fail-spark"
        }
        fn supports(&self, k: AssetKind) -> bool {
            self.inner.supports(k)
        }
        fn limits(&self) -> ProviderLimits {
            self.inner.limits()
        }
        async fn fetch_quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, String> {
            self.inner.fetch_quotes(symbols).await
        }
        async fn fetch_sparkline(&self, _: &str, _: &str, _: &str) -> Result<Sparkline, String> {
            Err("rate_limited".into())
        }
    }

    #[tokio::test]
    async fn sparkline_failure_applies_backoff() {
        let provider = Arc::new(FailSparkProvider {
            inner: MockProvider::new(vec![quote("A", 10.0)]),
        });
        let mut sched = QuoteScheduler::new(provider);
        sched.set_watchlist(vec![item("A", 0)]);
        sched.tick_once().await;
        assert!(sched.backoff_until.is_some());
        assert!(sched.sparkline_cache().get("A").is_none());
    }
}
