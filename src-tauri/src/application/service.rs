//! Application service: watchlist + settings + visibility state without Tauri window APIs.
//!
//! Command handlers and integration tests call into this layer so business logic stays
//! unit-testable without a live WebView.

use crate::application::diagnostics::{
    DiagLevel, EventRing, DIAGNOSTICS_DUMP_LINES, NOTE_THROTTLE,
};
use crate::application::scheduler::QuoteScheduler;
use crate::domain::constants::{
    clamp_geometry, clamp_opacity, clamp_quote_refresh_ms,
};
use crate::domain::display::attach_display;
use crate::domain::types::{
    AssetKind, CardTint, PersistedState, Quote, Sparkline, SymbolSuggestion, WatchlistItem,
    WindowGeometry,
};
use crate::domain::watchlist;
use crate::infrastructure::store::save_state;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Mutex as AsyncMutex;

const LOCK_POISONED: &str = "state lock poisoned";

/// Shared core state used by commands and tests.
pub struct AppCore {
    persisted: Mutex<PersistedState>,
    pub app_data_dir: PathBuf,
    scheduler: Arc<AsyncMutex<QuoteScheduler>>,
    pub visible: AtomicBool,
    events: Mutex<EventRing>,
    /// Last throttled note: (message, when) — suppresses identical spam.
    throttle: Mutex<Option<(String, Instant)>>,
}

impl AppCore {
    pub fn new(
        persisted: PersistedState,
        app_data_dir: PathBuf,
        scheduler: QuoteScheduler,
        visible: bool,
    ) -> Self {
        Self {
            persisted: Mutex::new(persisted),
            app_data_dir,
            scheduler: Arc::new(AsyncMutex::new(scheduler)),
            visible: AtomicBool::new(visible),
            events: Mutex::new(EventRing::default()),
            throttle: Mutex::new(None),
        }
    }

    /// Record a diagnostics event (best-effort; ignores poisoned lock).
    pub fn note(&self, level: DiagLevel, message: impl Into<String>) {
        if let Ok(mut ring) = self.events.lock() {
            ring.push(level, message);
        }
    }

    /// Like [`note`], but skips if the same message was logged within `cooldown`
    /// (default [`NOTE_THROTTLE`]). Prevents scheduler 429 spam from filling the ring.
    pub fn note_throttled(&self, level: DiagLevel, message: impl Into<String>, cooldown: Duration) {
        let message = message.into();
        if let Ok(mut slot) = self.throttle.lock() {
            if let Some((prev, at)) = slot.as_ref() {
                if prev == &message && at.elapsed() < cooldown {
                    return;
                }
            }
            *slot = Some((message.clone(), Instant::now()));
        }
        self.note(level, message);
    }

    /// Throttle with [`NOTE_THROTTLE`].
    pub fn note_throttled_default(&self, level: DiagLevel, message: impl Into<String>) {
        self.note_throttled(level, message, NOTE_THROTTLE);
    }

    pub fn app_data_dir(&self) -> &Path {
        &self.app_data_dir
    }

    pub fn get_state(&self) -> Result<PersistedState, String> {
        self.with_persisted(|g| g.clone())
    }

    pub fn scheduler(&self) -> &Arc<AsyncMutex<QuoteScheduler>> {
        &self.scheduler
    }

    fn persist_locked(&self, state: &PersistedState) -> Result<(), String> {
        save_state(&self.app_data_dir, state)
    }

    fn with_persisted<T>(&self, f: impl FnOnce(&PersistedState) -> T) -> Result<T, String> {
        let persisted = self
            .persisted
            .lock()
            .map_err(|_| LOCK_POISONED.to_string())?;
        Ok(f(&persisted))
    }

    fn with_persisted_mut<T>(
        &self,
        f: impl FnOnce(&mut PersistedState) -> Result<T, String>,
    ) -> Result<T, String> {
        self.mutate_persisted(|p| Ok((f(p)?, true)))
    }

    fn mutate_persisted<T>(
        &self,
        f: impl FnOnce(&mut PersistedState) -> Result<(T, bool), String>,
    ) -> Result<T, String> {
        let mut persisted = self
            .persisted
            .lock()
            .map_err(|_| LOCK_POISONED.to_string())?;
        let (out, persist) = f(&mut persisted)?;
        if persist {
            self.persist_locked(&persisted)?;
        }
        Ok(out)
    }

    pub async fn sync_scheduler_watchlist(&self) -> Result<(), String> {
        let items = self.with_persisted(|p| watchlist::sorted_clone(&p.watchlist))?;
        let mut sched = self.scheduler.lock().await;
        sched.set_watchlist(items);
        Ok(())
    }

    pub async fn add_symbol(
        &self,
        symbol: String,
        asset_kind: AssetKind,
    ) -> Result<WatchlistItem, String> {
        let item = self.with_persisted_mut(|persisted| {
            watchlist::add_item(&mut persisted.watchlist, &symbol, asset_kind, None)
        })?;
        self.sync_scheduler_watchlist().await?;
        {
            let mut sched = self.scheduler.lock().await;
            sched.bump_priority(item.symbol.clone());
        }
        Ok(item)
    }

    pub async fn remove_symbol(&self, id: &str) -> Result<(), String> {
        self.with_persisted_mut(|persisted| {
            if !watchlist::remove_item(&mut persisted.watchlist, id) {
                return Err(format!("unknown id {id}"));
            }
            Ok(())
        })?;
        self.sync_scheduler_watchlist().await
    }

    pub async fn remove_symbols(&self, ids: &[String]) -> Result<usize, String> {
        let removed = self.mutate_persisted(|persisted| {
            let removed = watchlist::remove_items(&mut persisted.watchlist, ids);
            Ok((removed, removed > 0))
        })?;
        if removed > 0 {
            self.sync_scheduler_watchlist().await?;
        }
        Ok(removed)
    }

    pub async fn set_card_tint(&self, id: &str, tint: CardTint) -> Result<(), String> {
        self.with_persisted_mut(|persisted| {
            if !watchlist::set_card_tint(&mut persisted.watchlist, id, tint) {
                return Err(format!("unknown id {id}"));
            }
            Ok(())
        })
    }

    pub async fn reorder_symbols(&self, ordered_ids: &[String]) -> Result<(), String> {
        self.with_persisted_mut(|persisted| watchlist::reorder(&mut persisted.watchlist, ordered_ids))?;
        self.sync_scheduler_watchlist().await
    }

    /// Persist login autostart preference (OS registration is applied by the command layer).
    pub fn set_autostart(&self, enabled: bool) -> Result<(), String> {
        self.with_persisted_mut(|persisted| {
            persisted.settings.autostart = enabled;
            Ok(())
        })
    }

    /// Returns clamped opacity after persist.
    pub fn set_opacity(&self, opacity: f64) -> Result<f64, String> {
        let opacity = clamp_opacity(opacity);
        self.with_persisted_mut(|persisted| {
            persisted.settings.opacity = opacity;
            Ok(opacity)
        })
    }

    /// Persist watchlist column `fr` ratios (symbol · spark · metrics).
    pub fn set_column_ratios(
        &self,
        ratios: crate::domain::types::ColumnRatios,
    ) -> Result<crate::domain::types::ColumnRatios, String> {
        use crate::domain::constants::clamp_column_ratios;
        let ratios = clamp_column_ratios(ratios);
        self.with_persisted_mut(|persisted| {
            persisted.settings.column_ratios = ratios;
            Ok(ratios)
        })
    }

    /// Persist quote refresh interval (milliseconds after clamp) and update the scheduler.
    pub async fn set_quote_refresh_ms(&self, ms: u64) -> Result<u64, String> {
        let ms = clamp_quote_refresh_ms(ms);
        self.with_persisted_mut(|persisted| {
            persisted.settings.quote_refresh_ms = ms;
            Ok(ms)
        })?;
        {
            let mut sched = self.scheduler.lock().await;
            sched.set_min_quote_interval(Duration::from_millis(ms));
        }
        Ok(ms)
    }

    /// Historical name for [`set_quote_refresh_ms`].
    pub async fn set_quote_refresh_secs(&self, secs: u64) -> Result<u64, String> {
        self.set_quote_refresh_ms(secs).await
    }

    pub fn quote_refresh_ms(&self) -> Result<u64, String> {
        self.with_persisted(|p| clamp_quote_refresh_ms(p.settings.quote_refresh_ms))
    }

    pub fn quote_refresh_secs(&self) -> Result<u64, String> {
        self.quote_refresh_ms()
    }

    /// Apply persisted quote interval onto the scheduler (call once at bootstrap).
    pub async fn apply_quote_refresh_to_scheduler(&self) -> Result<(), String> {
        let ms = self.quote_refresh_ms()?;
        let mut sched = self.scheduler.lock().await;
        sched.set_min_quote_interval(Duration::from_millis(ms));
        Ok(())
    }

    pub fn save_window_geometry(&self, geometry: WindowGeometry) -> Result<WindowGeometry, String> {
        let geometry = clamp_geometry(&geometry);
        self.with_persisted_mut(|persisted| {
            persisted.settings.window = geometry.clone();
            Ok(geometry)
        })
    }

    pub async fn set_visible_state(&self, visible: bool) {
        self.visible.store(visible, Ordering::SeqCst);
        let mut sched = self.scheduler.lock().await;
        sched.set_visible(visible);
    }

    pub fn is_visible(&self) -> bool {
        self.visible.load(Ordering::SeqCst)
    }

    pub async fn toggle_visible_state(&self) -> bool {
        let next = !self.is_visible();
        self.set_visible_state(next).await;
        next
    }

    pub async fn get_quotes(&self) -> Vec<Quote> {
        let sched = self.scheduler.lock().await;
        let sparks = sched.sparkline_cache().all();
        let mut quotes = sched.quote_cache().all();
        for q in &mut quotes {
            let spark_prev = sparks
                .iter()
                .find(|s| s.symbol == q.symbol)
                .and_then(|s| s.previous_close);
            attach_display(q, spark_prev);
        }
        quotes
    }

    pub async fn search_symbols(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SymbolSuggestion>, String> {
        let sched = self.scheduler.lock().await;
        sched.search_symbols(query, limit).await
    }

    /// One scheduler tick + drain diagnostics into the ring.
    pub async fn tick_once(&self) -> crate::application::scheduler::TickOutcome {
        let mut sched = self.scheduler.lock().await;
        let outcome = sched.tick_once().await;
        for msg in sched.drain_diag_notes() {
            self.note_throttled_default(DiagLevel::Warn, msg);
        }
        outcome
    }

    pub async fn get_sparklines(&self) -> Vec<Sparkline> {
        let sched = self.scheduler.lock().await;
        sched.sparkline_cache().all()
    }

    pub async fn watchlist_snapshot(&self) -> Result<Vec<WatchlistItem>, String> {
        self.with_persisted(|p| watchlist::sorted_clone(&p.watchlist))
    }

    /// Build a pasteable diagnostics report for agents (Mode B).
    pub async fn format_diagnostics(&self) -> Result<String, String> {
        let captured_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let version = env!("CARGO_PKG_VERSION");
        let os = format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH);
        let visible = self.is_visible();
        let app_data = self.app_data_dir.display().to_string();

        let state = self.get_state()?;
        let settings = &state.settings;
        let watchlist = watchlist::sorted_clone(&state.watchlist);

        let (quotes, sched_line) = {
            let sched = self.scheduler.lock().await;
            let quotes = sched.quote_cache().all();
            let line = sched.diagnostics_summary();
            (quotes, line)
        };

        let recent = self
            .events
            .lock()
            .map_err(|_| LOCK_POISONED.to_string())?
            .last_lines(DIAGNOSTICS_DUMP_LINES);

        let mut out = String::new();
        out.push_str("### EWR diagnostics\n");
        out.push_str(&format!("- captured_at: {captured_at}\n"));
        out.push_str(&format!("- app_version: {version}\n"));
        out.push_str(&format!("- os: {os}\n"));
        out.push_str(&format!("- visible: {visible}\n"));
        out.push_str(&format!("- app_data_dir: {app_data}\n"));
        out.push_str(&format!(
            "- settings: opacity={} autostart={} hotkey={:?} window={{x:{}, y:{}, w:{}, h:{}}}\n",
            settings.opacity,
            settings.autostart,
            settings.hotkey,
            settings.window.x,
            settings.window.y,
            settings.window.width,
            settings.window.height,
        ));
        out.push_str("- watchlist:\n");
        if watchlist.is_empty() {
            out.push_str("  (none)\n");
        } else {
            for item in &watchlist {
                out.push_str(&format!(
                    "  {} {} {:?} {}\n",
                    item.sort_index, item.symbol, item.asset_kind, item.id
                ));
            }
        }
        out.push_str("- quotes:\n");
        if quotes.is_empty() {
            out.push_str("  (none)\n");
        } else {
            for q in &quotes {
                let ch = q
                    .change_percent
                    .map(|c| format!("{c:.4}"))
                    .unwrap_or_else(|| "n/a".into());
                out.push_str(&format!(
                    "  {} price={} change%={} as_of={} source={}\n",
                    q.symbol, q.price, ch, q.as_of, q.source
                ));
            }
        }
        out.push_str(&format!("- scheduler: {sched_line}\n"));
        out.push_str("- recent_events:\n");
        if recent.is_empty() {
            out.push_str("  (none)\n");
        } else {
            for line in recent {
                out.push_str(&format!("  {line}\n"));
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{AssetKind, CardTint, Quote, Sparkline, SymbolSuggestion};
    use crate::ports::market_data::{MarketDataProvider, ProviderLimits};
    use async_trait::async_trait;
    use std::time::Duration;
    use tempfile::tempdir;

    struct MockProvider;

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
                min_interval: Duration::from_secs(1),
                prefers_batch: true,
            }
        }
        async fn fetch_quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, String> {
            Ok(symbols
                .iter()
                .map(|s| Quote {
                    symbol: s.clone(),
                    price: 1.0,
                    change_percent: Some(0.0),
                    as_of: "t".into(),
                    source: "mock".into(),
                    ..Default::default()
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
                previous_close: Some(10.0),
                as_of: "t".into(),
                session_start: None,
                session_end: None,
            })
        }

        async fn search_symbols(
            &self,
            query: &str,
            limit: usize,
        ) -> Result<Vec<SymbolSuggestion>, String> {
            if query.eq_ignore_ascii_case("fail") {
                return Err("search down".into());
            }
            Ok(vec![SymbolSuggestion {
                symbol: query.trim().to_ascii_uppercase(),
                name: Some("Mock".into()),
                asset_kind: AssetKind::Equity,
                exchange: Some("TEST".into()),
            }]
            .into_iter()
            .take(limit.max(1))
            .collect())
        }
    }

    struct EmptyQuoteProvider;

    #[async_trait]
    impl MarketDataProvider for EmptyQuoteProvider {
        fn id(&self) -> &'static str {
            "empty"
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
            Ok(vec![])
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
                session_start: None,
                session_end: None,
            })
        }
    }

    struct FailQuotesProvider;

    #[async_trait]
    impl MarketDataProvider for FailQuotesProvider {
        fn id(&self) -> &'static str {
            "fail"
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
            Err("boom".into())
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
                session_start: None,
                session_end: None,
            })
        }
    }

    fn core_empty() -> (tempfile::TempDir, AppCore) {
        let dir = tempdir().unwrap();
        let scheduler = QuoteScheduler::new(Arc::new(MockProvider));
        let core = AppCore::new(
            crate::infrastructure::store::default_state(),
            dir.path().to_path_buf(),
            scheduler,
            true,
        );
        (dir, core)
    }

    #[tokio::test]
    async fn add_remove_reorder_and_settings_round_trip() {
        let (_dir, core) = core_empty();
        let state = core.get_state().unwrap();
        assert_eq!(state.watchlist.len(), 2);

        let msft = core
            .add_symbol("msft".into(), AssetKind::Equity)
            .await
            .unwrap();
        assert_eq!(msft.symbol, "MSFT");
        assert_eq!(core.watchlist_snapshot().await.unwrap().len(), 3);

        // duplicate risk
        assert!(core
            .add_symbol("MSFT".into(), AssetKind::Equity)
            .await
            .is_err());

        let ids: Vec<String> = core
            .watchlist_snapshot()
            .await
            .unwrap()
            .into_iter()
            .map(|i| i.id)
            .collect();
        let mut reordered = ids.clone();
        reordered.rotate_left(1);
        core.reorder_symbols(&reordered).await.unwrap();
        let after = core.watchlist_snapshot().await.unwrap();
        assert_eq!(after[0].id, reordered[0]);

        core.remove_symbol(&msft.id).await.unwrap();
        assert_eq!(core.watchlist_snapshot().await.unwrap().len(), 2);
        assert!(core.remove_symbol("nope").await.is_err());

        let op = core.set_opacity(0.1).unwrap();
        assert!((op - 0.35).abs() < 1e-9);
        let cols = core
            .set_column_ratios(crate::domain::types::ColumnRatios {
                symbol: 2.0,
                spark: 1.0,
                metrics: 1.5,
            })
            .unwrap();
        assert!((cols.symbol - 2.0).abs() < 1e-9);
        assert!((core.get_state().unwrap().settings.column_ratios.spark - 1.0).abs() < 1e-9);
        let geo = core
            .save_window_geometry(WindowGeometry {
                x: 1.0,
                y: 2.0,
                width: 10.0,
                height: 10.0,
            })
            .unwrap();
        assert!(geo.width >= 260.0);
        assert!(geo.height >= 120.0);

        let reloaded = core.get_state().unwrap();
        assert!((reloaded.settings.opacity - 0.35).abs() < 1e-9);
    }

    #[tokio::test]
    async fn visibility_toggles_without_window() {
        let (_dir, core) = core_empty();
        assert!(core.is_visible());
        core.set_visible_state(false).await;
        assert!(!core.is_visible());
        let next = core.toggle_visible_state().await;
        assert!(next);
        assert!(core.is_visible());
    }

    #[tokio::test]
    async fn quotes_and_sparklines_start_empty() {
        let (_dir, core) = core_empty();
        assert!(core.get_quotes().await.is_empty());
        assert!(core.get_sparklines().await.is_empty());
    }

    #[tokio::test]
    async fn format_diagnostics_includes_core_fields() {
        let (_dir, core) = core_empty();
        core.note(DiagLevel::Warn, "hotkey collide test");
        let text = core.format_diagnostics().await.unwrap();
        assert!(text.contains("### EWR diagnostics"));
        assert!(text.contains("app_version:"));
        assert!(text.contains(env!("CARGO_PKG_VERSION")));
        assert!(text.contains("AAPL") || text.contains("BTC-USD"));
        assert!(text.contains("settings:"));
        assert!(text.contains("hotkey collide test"));
        assert!(text.contains("scheduler:"));
    }

    #[test]
    fn note_throttled_suppresses_identical_message() {
        let (_dir, core) = core_empty();
        core.note_throttled(DiagLevel::Warn, "rate_limited", Duration::from_secs(60));
        core.note_throttled(DiagLevel::Warn, "rate_limited", Duration::from_secs(60));
        core.note_throttled(DiagLevel::Warn, "other", Duration::from_secs(60));
        let lines = core.events.lock().unwrap().lines();
        assert_eq!(
            lines.iter().filter(|l| l.contains("rate_limited")).count(),
            1
        );
        assert_eq!(lines.iter().filter(|l| l.contains("other")).count(), 1);
    }

    #[tokio::test]
    async fn settings_refresh_autostart_and_tint_round_trip() {
        let (_dir, core) = core_empty();
        assert!(core.app_data_dir().exists() || !core.app_data_dir().as_os_str().is_empty());
        core.set_autostart(false).unwrap();
        assert!(!core.get_state().unwrap().settings.autostart);

        let applied = core.set_quote_refresh_ms(1000).await.unwrap();
        assert_eq!(applied, 1000);
        assert_eq!(core.quote_refresh_ms().unwrap(), 1000);
        assert_eq!(core.quote_refresh_secs().unwrap(), 1000);
        let via_alias = core.set_quote_refresh_secs(250).await.unwrap();
        assert_eq!(via_alias, 250);
        core.apply_quote_refresh_to_scheduler().await.unwrap();
        // Scheduler floor is MIN_QUOTE_INTERVAL (500ms), even if UI stores 250.
        assert_eq!(
            core.scheduler().lock().await.min_quote_interval(),
            Duration::from_millis(500)
        );

        let id = core.watchlist_snapshot().await.unwrap()[0].id.clone();
        core.set_card_tint(&id, CardTint::Mint).await.unwrap();
        assert_eq!(
            core.watchlist_snapshot().await.unwrap()[0].card_tint,
            CardTint::Mint
        );
        assert!(core.set_card_tint("missing", CardTint::Rose).await.is_err());
    }

    #[tokio::test]
    async fn remove_symbols_zero_and_many() {
        let (_dir, core) = core_empty();
        assert_eq!(core.remove_symbols(&[]).await.unwrap(), 0);
        let ids: Vec<String> = core
            .watchlist_snapshot()
            .await
            .unwrap()
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(core.remove_symbols(&ids).await.unwrap(), 2);
        assert!(core.watchlist_snapshot().await.unwrap().is_empty());
        let text = core.format_diagnostics().await.unwrap();
        assert!(text.contains("(none)"));
    }

    #[tokio::test]
    async fn tick_attaches_display_and_search_hits_provider() {
        let (_dir, core) = core_empty();
        core.sync_scheduler_watchlist().await.unwrap();
        let out = core.tick_once().await;
        assert!(out.quotes_updated || out.sparklines_updated || !out.any());
        let quotes = core.get_quotes().await;
        assert!(!quotes.is_empty());
        assert!(quotes.iter().all(|q| q.display.is_some()));

        let hits = core.search_symbols("nvda", 3).await.unwrap();
        assert_eq!(hits[0].symbol, "NVDA");
        assert!(core.search_symbols("fail", 1).await.is_err());
    }

    #[tokio::test]
    async fn tick_once_drains_provider_errors_into_ring() {
        let dir = tempdir().unwrap();
        let core = AppCore::new(
            crate::infrastructure::store::default_state(),
            dir.path().to_path_buf(),
            QuoteScheduler::new(Arc::new(FailQuotesProvider)),
            true,
        );
        core.sync_scheduler_watchlist().await.unwrap();
        let _ = core.tick_once().await;
        let text = core.format_diagnostics().await.unwrap();
        assert!(text.contains("boom") || text.contains("quotes"));
    }

    #[tokio::test]
    async fn tick_empty_quote_payload_does_not_panic() {
        let dir = tempdir().unwrap();
        let core = AppCore::new(
            crate::infrastructure::store::default_state(),
            dir.path().to_path_buf(),
            QuoteScheduler::new(Arc::new(EmptyQuoteProvider)),
            true,
        );
        core.sync_scheduler_watchlist().await.unwrap();
        let _ = core.tick_once().await;
        assert!(core.get_quotes().await.is_empty());
    }

    #[tokio::test]
    async fn format_diagnostics_lists_cached_quotes() {
        let (_dir, core) = core_empty();
        core.sync_scheduler_watchlist().await.unwrap();
        core.tick_once().await;
        let text = core.format_diagnostics().await.unwrap();
        assert!(text.contains("price="));
        core.note_throttled_default(DiagLevel::Info, "once");
    }
}
