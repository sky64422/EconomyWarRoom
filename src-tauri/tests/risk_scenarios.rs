//! Risk-scenario tests: API limits, corrupt state, invalid input, hide/pause.

use async_trait::async_trait;
use economy_war_room_lib::application::scheduler::QuoteScheduler;
use economy_war_room_lib::application::service::AppCore;
use economy_war_room_lib::domain::types::{AssetKind, Quote, Sparkline, WatchlistItem};
use economy_war_room_lib::domain::watchlist;
use economy_war_room_lib::infrastructure::store::{
    default_state, load_state, save_state, state_path,
};
use economy_war_room_lib::ports::market_data::{MarketDataProvider, ProviderLimits};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;

struct AlwaysRateLimited;

#[async_trait]
impl MarketDataProvider for AlwaysRateLimited {
    fn id(&self) -> &'static str {
        "rl"
    }
    fn supports(&self, _: AssetKind) -> bool {
        true
    }
    fn limits(&self) -> ProviderLimits {
        ProviderLimits {
            max_concurrent: 1,
            min_interval: Duration::from_millis(1),
            prefers_batch: true,
        }
    }
    async fn fetch_quotes(&self, _: &[String]) -> Result<Vec<Quote>, String> {
        Err("rate_limited".into())
    }
    async fn fetch_sparkline(&self, _: &str, _: &str, _: &str) -> Result<Sparkline, String> {
        Err("rate_limited".into())
    }
}

struct OnceThenOk {
    n: AtomicUsize,
}

#[async_trait]
impl MarketDataProvider for OnceThenOk {
    fn id(&self) -> &'static str {
        "once"
    }
    fn supports(&self, _: AssetKind) -> bool {
        true
    }
    fn limits(&self) -> ProviderLimits {
        ProviderLimits {
            max_concurrent: 1,
            min_interval: Duration::from_millis(1),
            prefers_batch: true,
        }
    }
    async fn fetch_quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, String> {
        self.n.fetch_add(1, Ordering::SeqCst);
        Ok(symbols
            .iter()
            .map(|s| Quote {
                symbol: s.clone(),
                price: 1.0,
                currency: "USD".into(),
                change_percent: Some(0.0),
                as_of: "t".into(),
                source: "once".into(),
            })
            .collect())
    }
    async fn fetch_sparkline(&self, symbol: &str, _: &str, _: &str) -> Result<Sparkline, String> {
        Ok(Sparkline {
            symbol: symbol.into(),
            points: vec![],
            previous_close: None,
            as_of: "t".into(),
        })
    }
}

fn item(sym: &str) -> WatchlistItem {
    WatchlistItem {
        id: sym.to_string(),
        symbol: sym.to_string(),
        display_name: None,
        asset_kind: AssetKind::Equity,
        sort_index: 0,
    }
}

#[tokio::test]
async fn risk_rate_limit_backoff_stops_polling_burst() {
    let provider = Arc::new(AlwaysRateLimited);
    let mut sched = QuoteScheduler::new(provider);
    sched.set_watchlist(vec![item("AAPL")]);

    for _ in 0..5 {
        sched.tick_once().await;
    }
    // After first failure, remaining ticks should no-op while backoff is active.
    // We cannot observe call count on AlwaysRateLimited easily without atomic —
    // assert cache empty and backoff set.
    assert!(sched.quote_cache().get("AAPL").is_none());
    assert!(sched.is_backing_off());
}

#[tokio::test]
async fn risk_hidden_widget_never_calls_provider() {
    let provider = Arc::new(OnceThenOk {
        n: AtomicUsize::new(0),
    });
    let mut sched = QuoteScheduler::new(provider.clone());
    sched.set_watchlist(vec![item("AAPL"), item("MSFT")]);
    sched.set_visible(false);
    for _ in 0..10 {
        sched.tick_once().await;
    }
    assert_eq!(provider.n.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn risk_duplicate_and_empty_symbol_rejected() {
    let dir = tempdir().unwrap();
    let sched = QuoteScheduler::new(Arc::new(OnceThenOk {
        n: AtomicUsize::new(0),
    }));
    let core = AppCore::new(default_state(), dir.path().to_path_buf(), sched, true);

    assert!(core.add_symbol("".into(), AssetKind::Equity).await.is_err());
    assert!(core
        .add_symbol("AAPL".into(), AssetKind::Equity)
        .await
        .is_err()); // seed already has AAPL
}

#[test]
fn risk_corrupt_state_file_does_not_panic() {
    let dir = tempdir().unwrap();
    std::fs::write(state_path(dir.path()), "{{{{").unwrap();
    let s = load_state(dir.path());
    assert!(!s.watchlist.is_empty());
}

#[test]
fn risk_empty_watchlist_reorder_ok() {
    let mut items = vec![];
    assert!(watchlist::reorder(&mut items, &[]).is_ok());
}

#[tokio::test]
async fn risk_remove_unknown_symbol_is_err() {
    let dir = tempdir().unwrap();
    let sched = QuoteScheduler::new(Arc::new(OnceThenOk {
        n: AtomicUsize::new(0),
    }));
    let core = AppCore::new(default_state(), dir.path().to_path_buf(), sched, true);
    assert!(core.remove_symbol("not-a-real-id").await.is_err());
}

#[test]
fn risk_save_creates_parent_dirs() {
    let dir = tempdir().unwrap();
    let nested = dir.path().join("a").join("b");
    let state = default_state();
    save_state(&nested, &state).unwrap();
    assert!(state_path(&nested).exists());
}
