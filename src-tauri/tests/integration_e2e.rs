//! Integration / E2E-style tests without a live WebView.
//!
//! These exercise store + service + scheduler + (mock/HTTP) provider together —
//! the same paths production uses for watchlist and market data.

use economy_war_room_lib::application::scheduler::QuoteScheduler;
use economy_war_room_lib::application::service::AppCore;
use economy_war_room_lib::domain::types::{AssetKind, Quote, Sparkline, ThemeMode};
use economy_war_room_lib::infrastructure::store::{default_state, load_state, save_state};
use economy_war_room_lib::infrastructure::yahoo::YahooProvider;
use economy_war_room_lib::ports::market_data::{MarketDataProvider, ProviderLimits};
use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct CountingProvider {
    calls: AtomicUsize,
}

#[async_trait]
impl MarketDataProvider for CountingProvider {
    fn id(&self) -> &'static str {
        "counting"
    }
    fn supports(&self, _: AssetKind) -> bool {
        true
    }
    fn limits(&self) -> ProviderLimits {
        ProviderLimits {
            max_concurrent: 2,
            min_interval: Duration::from_secs(1),
            prefers_batch: true,
        }
    }
    async fn fetch_quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(symbols
            .iter()
            .map(|s| Quote {
                symbol: s.clone(),
                price: 42.0,
                currency: "USD".into(),
                change_percent: Some(1.5),
                as_of: "t".into(),
                source: "counting".into(),
            })
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
            previous_close: Some(40.0),
            as_of: "t".into(),
        })
    }
}

fn core_with(provider: Arc<dyn MarketDataProvider>) -> (tempfile::TempDir, AppCore) {
    let dir = tempdir().unwrap();
    let mut sched = QuoteScheduler::new(provider);
    let state = default_state();
    sched.set_watchlist(state.watchlist.clone());
    let core = AppCore::new(state, dir.path().to_path_buf(), sched, true);
    (dir, core)
}

#[tokio::test]
async fn e2e_watchlist_persist_and_scheduler_refresh() {
    let provider = Arc::new(CountingProvider {
        calls: AtomicUsize::new(0),
    });
    let (dir, core) = core_with(provider.clone());

    // Seed present
    assert_eq!(core.watchlist_snapshot().await.unwrap().len(), 2);

    // User adds NVDA, reorders, removes BTC, changes theme
    let nvda = core
        .add_symbol("nvda".into(), AssetKind::Equity)
        .await
        .unwrap();
    assert_eq!(nvda.symbol, "NVDA");

    let mut ids: Vec<String> = core
        .watchlist_snapshot()
        .await
        .unwrap()
        .into_iter()
        .map(|i| i.id)
        .collect();
    ids.rotate_right(1);
    core.reorder_symbols(&ids).await.unwrap();

    let btc_id = core
        .watchlist_snapshot()
        .await
        .unwrap()
        .into_iter()
        .find(|i| i.symbol == "BTC-USD")
        .unwrap()
        .id;
    core.remove_symbol(&btc_id).await.unwrap();
    core.set_theme(ThemeMode::Light).unwrap();
    core.set_opacity(0.8).unwrap();

    // Reload from disk (simulate process restart)
    let reloaded = load_state(dir.path());
    assert_eq!(reloaded.watchlist.len(), 2);
    assert!(reloaded.watchlist.iter().any(|i| i.symbol == "NVDA"));
    assert!(!reloaded.watchlist.iter().any(|i| i.symbol == "BTC-USD"));
    assert_eq!(reloaded.settings.theme, ThemeMode::Light);

    // Scheduler tick fills caches
    {
        let mut sched = core.scheduler.lock().await;
        sched.set_watchlist(reloaded.watchlist.clone());
        sched.tick_once().await;
    }
    let quotes = core.get_quotes().await;
    assert!(!quotes.is_empty());
    assert!(provider.calls.load(Ordering::SeqCst) >= 1);

    // Hide pauses network
    core.set_visible_state(false).await;
    let before = provider.calls.load(Ordering::SeqCst);
    {
        let mut sched = core.scheduler.lock().await;
        sched.tick_once().await;
    }
    assert_eq!(provider.calls.load(Ordering::SeqCst), before);
}

#[tokio::test]
async fn e2e_yahoo_http_mock_provider_pipeline() {
    let server = MockServer::start().await;
    let body = r#"{
      "chart": {
        "result": [{
          "meta": {
            "currency": "USD",
            "symbol": "AAPL",
            "regularMarketPrice": 200.0,
            "previousClose": 190.0
          },
          "timestamp": [1, 2],
          "indicators": { "quote": [{ "close": [190.0, 200.0] }] }
        }],
        "error": null
      }
    }"#;
    Mock::given(method("GET"))
        .and(path("/v8/finance/chart/AAPL"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;

    let provider = Arc::new(YahooProvider::with_base_url(server.uri()).unwrap());
    let mut sched = QuoteScheduler::new(provider);
    sched.set_watchlist(vec![economy_war_room_lib::domain::types::WatchlistItem {
        id: "1".into(),
        symbol: "AAPL".into(),
        display_name: None,
        asset_kind: AssetKind::Equity,
        sort_index: 0,
    }]);
    sched.tick_once().await;
    let q = sched.quote_cache().get("AAPL").unwrap();
    assert!((q.price - 200.0).abs() < 1e-9);
    assert!(sched.sparkline_cache().get("AAPL").is_some());
}

#[tokio::test]
async fn e2e_rate_limit_does_not_wipe_cache() {
    struct Flaky {
        phase: AtomicUsize,
        last_good: MutexQuotes,
    }
    struct MutexQuotes(std::sync::Mutex<Vec<Quote>>);

    #[async_trait]
    impl MarketDataProvider for Flaky {
        fn id(&self) -> &'static str {
            "flaky"
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
            let n = self.phase.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                let quotes: Vec<_> = symbols
                    .iter()
                    .map(|s| Quote {
                        symbol: s.clone(),
                        price: 99.0,
                        currency: "USD".into(),
                        change_percent: None,
                        as_of: "t".into(),
                        source: "flaky".into(),
                    })
                    .collect();
                *self.last_good.0.lock().unwrap() = quotes.clone();
                Ok(quotes)
            } else {
                Err("rate_limited".into())
            }
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
                as_of: "t".into(),
            })
        }
    }

    let provider = Arc::new(Flaky {
        phase: AtomicUsize::new(0),
        last_good: MutexQuotes(std::sync::Mutex::new(vec![])),
    });
    let mut sched = QuoteScheduler::new(provider);
    sched.set_watchlist(vec![economy_war_room_lib::domain::types::WatchlistItem {
        id: "1".into(),
        symbol: "AAPL".into(),
        display_name: None,
        asset_kind: AssetKind::Equity,
        sort_index: 0,
    }]);
    sched.tick_once().await;
    assert_eq!(sched.quote_cache().get("AAPL").unwrap().price, 99.0);

    // Force stale + second tick fails → cache retained
    sched.set_visible(true);
    sched.tick_once().await;
    assert_eq!(sched.quote_cache().get("AAPL").unwrap().price, 99.0);
}

#[test]
fn e2e_default_state_save_load_seed() {
    let dir = tempdir().unwrap();
    let state = default_state();
    save_state(dir.path(), &state).unwrap();
    let loaded = load_state(dir.path());
    assert_eq!(loaded.watchlist.len(), 2);
    assert_eq!(loaded.watchlist[0].symbol, "AAPL");
    assert_eq!(loaded.watchlist[1].symbol, "BTC-USD");
}
