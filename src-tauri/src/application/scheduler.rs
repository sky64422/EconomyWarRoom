use crate::application::cache::{QuoteCache, SparklineCache};
use crate::domain::constants::{RefreshPolicy, SparklinePolicy};
use crate::domain::types::{SymbolSuggestion, WatchlistItem};
use crate::ports::market_data::MarketDataProvider;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::task::JoinSet;

/// Pure round-robin batch picker for stale symbols.
///
/// Respects `min_interval` via `last_fetch`, prefers `priority_symbol` first when stale,
/// advances `cursor` so subsequent calls continue around the watchlist.
/// Skips symbols listed in `exclude` (e.g. already in-flight this tick).
pub fn pick_batch(
    items: &[WatchlistItem],
    last_fetch: &HashMap<String, Instant>,
    now: Instant,
    min_interval: Duration,
    batch_size: usize,
    cursor: &mut usize,
    priority_symbol: Option<&str>,
    exclude: &HashSet<String>,
) -> Vec<String> {
    if items.is_empty() || batch_size == 0 {
        return vec![];
    }
    let mut out = Vec::new();
    if let Some(sym) = priority_symbol {
        if !exclude.contains(sym) && items.iter().any(|i| i.symbol == sym) {
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
        if exclude.contains(sym) || out.iter().any(|s| s == sym) {
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

/// Result of one scheduler tick — used to emit UI events only when caches changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TickOutcome {
    pub quotes_updated: bool,
    pub sparklines_updated: bool,
}

impl TickOutcome {
    pub fn any(self) -> bool {
        self.quotes_updated || self.sparklines_updated
    }
}

/// Quote refresh scheduler: fair RR worker pipeline, min interval, pause when hidden.
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
    /// When set, skip **quote** network work until this instant.
    backoff_until: Option<Instant>,
    /// Current quote backoff duration; doubles on each error up to [`RefreshPolicy::BACKOFF_MAX`].
    backoff: Duration,
    /// When set, skip **sparkline** network work until this instant (independent of quotes).
    spark_backoff_until: Option<Instant>,
    spark_backoff: Duration,
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
            spark_backoff_until: None,
            spark_backoff: RefreshPolicy::BACKOFF_INITIAL,
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
        let spark_backoff_active = self
            .spark_backoff_until
            .map(|u| Instant::now() < u)
            .unwrap_or(false);
        format!(
            "visible={} watchlist_len={} quote_interval_ms={} backoff_active={} spark_backoff_active={} backoff_secs={} last_error={}",
            self.visible,
            self.watchlist.len(),
            self.min_quote_interval.as_millis(),
            backoff_active,
            spark_backoff_active,
            self.backoff.as_secs_f64(),
            err
        )
    }

    pub fn set_min_quote_interval(&mut self, interval: Duration) {
        self.min_quote_interval = interval.max(RefreshPolicy::MIN_QUOTE_INTERVAL);
    }

    pub async fn search_symbols(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SymbolSuggestion>, String> {
        self.provider.search_symbols(query, limit).await
    }

    pub fn min_quote_interval(&self) -> Duration {
        self.min_quote_interval
    }

    pub fn set_visible(&mut self, visible: bool) {
        if visible {
            // Force-refresh: mark all symbols stale so the next tick fetches immediately.
            self.last_quote_fetch.clear();
            self.backoff_until = None;
            self.spark_backoff_until = None;
        }
        self.visible = visible;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Whether quote network work is currently suppressed due to error backoff.
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

    /// One scheduler tick: worker-pool pipeline for quotes (complete → cache immediately),
    /// then at most one sparkline. Independent backoffs for quote vs spark.
    pub async fn tick_once(&mut self) -> TickOutcome {
        let mut outcome = TickOutcome::default();
        if !self.visible {
            return outcome;
        }
        if self.watchlist.is_empty() {
            return outcome;
        }

        outcome.quotes_updated = self.pump_quote_pipeline().await;
        outcome.sparklines_updated = self.maybe_fetch_sparkline().await;
        outcome
    }

    /// Fair RR pipeline: keep up to `max_concurrent` single-symbol fetches in flight.
    /// Each completion updates the cache immediately (streaming within the tick).
    /// Continues until no stale work remains or quote backoff trips.
    async fn pump_quote_pipeline(&mut self) -> bool {
        let now = Instant::now();
        if self
            .backoff_until
            .map(|until| now < until)
            .unwrap_or(false)
        {
            return false;
        }

        let min_interval = self
            .provider
            .limits()
            .min_interval
            .max(self.min_quote_interval);
        let max_workers = self
            .provider
            .limits()
            .max_concurrent
            .max(1)
            .min(RefreshPolicy::MAX_CONCURRENT);

        let mut in_flight: HashSet<String> = HashSet::new();
        let mut join_set: JoinSet<(String, Result<Vec<crate::domain::types::Quote>, String>)> =
            JoinSet::new();
        let mut stop_dispatch = false;
        let mut any_updated = false;
        let mut priority = self.priority.take();

        loop {
            // Fill free worker slots with next stale symbols (RR + optional priority).
            while !stop_dispatch && in_flight.len() < max_workers {
                let now = Instant::now();
                let batch = pick_batch(
                    &self.watchlist,
                    &self.last_quote_fetch,
                    now,
                    min_interval,
                    1,
                    &mut self.cursor,
                    priority.as_deref(),
                    &in_flight,
                );
                priority = None; // only first fill uses priority
                let Some(sym) = batch.into_iter().next() else {
                    break;
                };
                in_flight.insert(sym.clone());
                let provider = self.provider.clone();
                join_set.spawn(async move {
                    let result = provider.fetch_quotes(std::slice::from_ref(&sym)).await;
                    (sym, result)
                });
            }

            if join_set.is_empty() {
                break;
            }

            let Some(joined) = join_set.join_next().await else {
                break;
            };

            match joined {
                Ok((sym, Ok(quotes))) => {
                    in_flight.remove(&sym);
                    self.backoff = RefreshPolicy::BACKOFF_INITIAL;
                    let fetched_at = Instant::now();
                    if quotes.is_empty() {
                        // Provider returned ok but omitted symbol — mark to avoid tight retry.
                        self.last_quote_fetch.insert(sym, fetched_at);
                    } else {
                        // Successful quote work clears quote backoff (spark backoff is separate).
                        if !stop_dispatch {
                            self.backoff_until = None;
                        }
                        self.last_error = None;
                        for q in quotes {
                            self.last_quote_fetch.insert(q.symbol.clone(), fetched_at);
                            self.quote_cache.put(q);
                            any_updated = true;
                        }
                    }
                }
                Ok((sym, Err(err))) => {
                    in_flight.remove(&sym);
                    let msg = format!("quotes {sym}: {err}");
                    self.last_error = Some(msg.clone());
                    self.pending_diag.push(msg);
                    self.backoff_until = Some(Instant::now() + self.backoff);
                    self.backoff = (self.backoff * 2).min(RefreshPolicy::BACKOFF_MAX);
                    stop_dispatch = true;
                    // Do not mark last_quote_fetch — retry after backoff.
                }
                Err(e) => {
                    let msg = format!("quotes join: {e}");
                    self.last_error = Some(msg.clone());
                    self.pending_diag.push(msg);
                    stop_dispatch = true;
                }
            }
        }

        any_updated
    }

    /// Fetch up to [`SPARKLINE_FETCHES_PER_TICK`] stale sparklines.
    /// Returns true if the sparkline cache changed.
    async fn maybe_fetch_sparkline(&mut self) -> bool {
        let now = Instant::now();
        if let Some(until) = self.spark_backoff_until {
            if now < until {
                return false;
            }
        }

        let mut fetched = 0usize;
        let mut updated = false;
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
                    self.spark_backoff = RefreshPolicy::BACKOFF_INITIAL;
                    self.spark_backoff_until = None;
                    self.last_error = None;
                    fetched += 1;
                    updated = true;
                }
                Err(err) => {
                    // Spark-only backoff — do not pause quote polling.
                    let msg = format!("sparkline {sym}: {err}");
                    self.last_error = Some(msg.clone());
                    self.pending_diag.push(msg);
                    self.spark_backoff_until = Some(Instant::now() + self.spark_backoff);
                    self.spark_backoff =
                        (self.spark_backoff * 2).min(RefreshPolicy::BACKOFF_MAX);
                    return updated;
                }
            }
        }
        updated
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

    fn empty_exclude() -> HashSet<String> {
        HashSet::new()
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
            &empty_exclude(),
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
            &empty_exclude(),
        );
        assert_eq!(batch[0], "C");
    }

    #[test]
    fn pick_batch_skips_excluded() {
        let items = vec![item("A", 0), item("B", 1), item("C", 2)];
        let now = Instant::now();
        let last = HashMap::new();
        let mut cursor = 0;
        let mut exclude = HashSet::new();
        exclude.insert("A".into());
        let batch = pick_batch(
            &items,
            &last,
            now,
            Duration::from_secs(1),
            2,
            &mut cursor,
            None,
            &exclude,
        );
        assert!(!batch.contains(&"A".to_string()));
        assert_eq!(batch.len(), 2);
    }

    struct MockProvider {
        quotes: Mutex<Vec<Quote>>,
        calls: AtomicUsize,
        spark_calls: AtomicUsize,
        fail_quotes: AtomicBool,
        /// Artificial latency so pipeline concurrency is observable.
        delay_ms: u64,
    }

    impl MockProvider {
        fn new(quotes: Vec<Quote>) -> Self {
            Self {
                quotes: Mutex::new(quotes),
                calls: AtomicUsize::new(0),
                spark_calls: AtomicUsize::new(0),
                fail_quotes: AtomicBool::new(false),
                delay_ms: 0,
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
                min_interval: Duration::from_millis(1),
                prefers_batch: true,
            }
        }

        async fn fetch_quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
            }
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
            as_of: "2026-01-01T00:00:00Z".into(),
            source: "mock".into(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn hidden_scheduler_does_not_call_provider() {
        let provider = Arc::new(MockProvider::new(vec![quote("A", 1.0)]));
        let mut sched = QuoteScheduler::new(provider.clone());
        sched.set_watchlist(vec![item("A", 0)]);
        sched.set_visible(false);
        let out = sched.tick_once().await;
        assert_eq!(provider.call_count(), 0);
        assert!(!out.any());
    }

    #[tokio::test]
    async fn visible_tick_fetches_and_caches() {
        let provider = Arc::new(MockProvider::new(vec![quote("A", 10.0), quote("B", 20.0)]));
        let mut sched = QuoteScheduler::new(provider.clone());
        sched.set_watchlist(vec![item("A", 0), item("B", 1)]);
        sched.set_visible(true);
        let out = sched.tick_once().await;
        // Phase 2: one provider call per symbol.
        assert_eq!(provider.call_count(), 2);
        assert!(out.quotes_updated);
        assert_eq!(sched.quote_cache().get("A").map(|q| q.price), Some(10.0));
        assert_eq!(sched.quote_cache().get("B").map(|q| q.price), Some(20.0));
    }

    #[tokio::test]
    async fn provider_error_queues_diag_note() {
        let provider = Arc::new(MockProvider::new(vec![quote("MSFT", 1.0)]));
        provider.set_fail_quotes(true);
        let mut sched = QuoteScheduler::new(provider);
        sched.set_watchlist(vec![item("MSFT", 0)]);
        let out = sched.tick_once().await;
        assert!(!out.quotes_updated);
        let notes = sched.drain_diag_notes();
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("quotes"));
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
        let out = sched.tick_once().await;

        assert_eq!(provider.spark_call_count(), 1);
        assert!(out.sparklines_updated);
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
        let out = sched.tick_once().await;
        assert_eq!(provider.call_count(), 0);
        assert!(!out.any());
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
            None,
            &empty_exclude()
        )
        .is_empty());
        assert!(pick_batch(
            &[],
            &last,
            Instant::now(),
            Duration::from_secs(1),
            2,
            &mut cursor,
            None,
            &empty_exclude()
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
    async fn sparkline_failure_applies_spark_only_backoff() {
        let provider = Arc::new(FailSparkProvider {
            inner: MockProvider::new(vec![quote("A", 10.0)]),
        });
        let mut sched = QuoteScheduler::new(provider);
        sched.set_watchlist(vec![item("A", 0)]);
        let out = sched.tick_once().await;
        // Quotes still succeed; spark failure must not freeze quote polling.
        assert!(out.quotes_updated);
        assert!(!out.sparklines_updated);
        assert!(sched.backoff_until.is_none());
        assert!(sched.spark_backoff_until.is_some());
        assert!(sched.quote_cache().get("A").is_some());
        assert!(sched.sparkline_cache().get("A").is_none());
    }

    #[tokio::test]
    async fn pipeline_fetches_all_stale_with_worker_cap() {
        // 4 symbols, max_concurrent=2 → still all cached after one tick (pipeline refill).
        let provider = Arc::new(MockProvider::new(vec![
            quote("A", 1.0),
            quote("B", 2.0),
            quote("C", 3.0),
            quote("D", 4.0),
        ]));
        let mut sched = QuoteScheduler::new(provider.clone());
        sched.set_watchlist(vec![
            item("A", 0),
            item("B", 1),
            item("C", 2),
            item("D", 3),
        ]);
        let out = sched.tick_once().await;
        assert!(out.quotes_updated);
        assert_eq!(provider.call_count(), 4);
        for s in ["A", "B", "C", "D"] {
            assert!(sched.quote_cache().get(s).is_some(), "missing {s}");
        }
    }

    #[tokio::test]
    async fn second_tick_without_stale_skips_quotes() {
        let provider = Arc::new(MockProvider::new(vec![quote("A", 1.0)]));
        let mut sched = QuoteScheduler::new(provider.clone());
        sched.set_watchlist(vec![item("A", 0)]);
        let out1 = sched.tick_once().await;
        assert!(out1.quotes_updated);
        let calls = provider.call_count();
        let out2 = sched.tick_once().await;
        assert!(!out2.quotes_updated);
        assert_eq!(provider.call_count(), calls);
    }
}
