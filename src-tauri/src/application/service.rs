//! Application service: watchlist + settings + visibility state without Tauri window APIs.
//!
//! Command handlers and integration tests call into this layer so business logic stays
//! unit-testable without a live WebView.

use crate::application::diagnostics::{
    DiagLevel, EventRing, DIAGNOSTICS_DUMP_LINES, NOTE_THROTTLE,
};
use crate::application::scheduler::QuoteScheduler;
use crate::domain::constants::clamp_opacity;
use crate::domain::types::{
    AssetKind, PersistedState, Quote, Sparkline, ThemeMode, WatchlistItem, WindowGeometry,
};
use crate::domain::watchlist;
use crate::infrastructure::store::save_state;
use crate::domain::constants::clamp_geometry;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Mutex as AsyncMutex;

/// Shared core state used by commands and tests.
pub struct AppCore {
    pub persisted: Mutex<PersistedState>,
    pub app_data_dir: PathBuf,
    pub scheduler: Arc<AsyncMutex<QuoteScheduler>>,
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
    pub fn note_throttled(
        &self,
        level: DiagLevel,
        message: impl Into<String>,
        cooldown: Duration,
    ) {
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
        self.persisted
            .lock()
            .map(|g| g.clone())
            .map_err(|_| "state lock poisoned".into())
    }

    fn persist_locked(&self, state: &PersistedState) -> Result<(), String> {
        save_state(&self.app_data_dir, state)
    }

    pub async fn sync_scheduler_watchlist(&self) -> Result<(), String> {
        let items = {
            let persisted = self
                .persisted
                .lock()
                .map_err(|_| "state lock poisoned".to_string())?;
            watchlist::sorted_clone(&persisted.watchlist)
        };
        let mut sched = self.scheduler.lock().await;
        sched.set_watchlist(items);
        Ok(())
    }

    pub async fn add_symbol(
        &self,
        symbol: String,
        asset_kind: AssetKind,
    ) -> Result<WatchlistItem, String> {
        let item = {
            let mut persisted = self
                .persisted
                .lock()
                .map_err(|_| "state lock poisoned".to_string())?;
            let item = watchlist::add_item(&mut persisted.watchlist, &symbol, asset_kind, None)?;
            self.persist_locked(&persisted)?;
            item
        };

        {
            let mut sched = self.scheduler.lock().await;
            let items = {
                let persisted = self
                    .persisted
                    .lock()
                    .map_err(|_| "state lock poisoned".to_string())?;
                watchlist::sorted_clone(&persisted.watchlist)
            };
            sched.set_watchlist(items);
            sched.bump_priority(item.symbol.clone());
        }

        Ok(item)
    }

    pub async fn remove_symbol(&self, id: &str) -> Result<(), String> {
        {
            let mut persisted = self
                .persisted
                .lock()
                .map_err(|_| "state lock poisoned".to_string())?;
            if !watchlist::remove_item(&mut persisted.watchlist, id) {
                return Err(format!("unknown id {id}"));
            }
            self.persist_locked(&persisted)?;
        }
        self.sync_scheduler_watchlist().await
    }

    pub async fn reorder_symbols(&self, ordered_ids: &[String]) -> Result<(), String> {
        {
            let mut persisted = self
                .persisted
                .lock()
                .map_err(|_| "state lock poisoned".to_string())?;
            watchlist::reorder(&mut persisted.watchlist, ordered_ids)?;
            self.persist_locked(&persisted)?;
        }
        self.sync_scheduler_watchlist().await
    }

    pub fn set_theme(&self, theme: ThemeMode) -> Result<(), String> {
        let mut persisted = self
            .persisted
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        persisted.settings.theme = theme;
        self.persist_locked(&persisted)
    }

    /// Returns clamped opacity after persist.
    pub fn set_opacity(&self, opacity: f64) -> Result<f64, String> {
        let opacity = clamp_opacity(opacity);
        let mut persisted = self
            .persisted
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        persisted.settings.opacity = opacity;
        self.persist_locked(&persisted)?;
        Ok(opacity)
    }

    pub fn save_window_geometry(&self, geometry: WindowGeometry) -> Result<WindowGeometry, String> {
        let geometry = clamp_geometry(&geometry);
        let mut persisted = self
            .persisted
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        persisted.settings.window = geometry.clone();
        self.persist_locked(&persisted)?;
        Ok(geometry)
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
        sched.quote_cache().all()
    }

    pub async fn get_sparklines(&self) -> Vec<Sparkline> {
        let sched = self.scheduler.lock().await;
        sched.sparkline_cache().all()
    }

    pub async fn watchlist_snapshot(&self) -> Result<Vec<WatchlistItem>, String> {
        let persisted = self
            .persisted
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        Ok(watchlist::sorted_clone(&persisted.watchlist))
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
            .map_err(|_| "events lock poisoned".to_string())?
            .last_lines(DIAGNOSTICS_DUMP_LINES);

        let mut out = String::new();
        out.push_str("### EWR diagnostics\n");
        out.push_str(&format!("- captured_at: {captured_at}\n"));
        out.push_str(&format!("- app_version: {version}\n"));
        out.push_str(&format!("- os: {os}\n"));
        out.push_str(&format!("- visible: {visible}\n"));
        out.push_str(&format!("- app_data_dir: {app_data}\n"));
        out.push_str(&format!(
            "- settings: theme={:?} opacity={} autostart={} hotkey={:?} window={{x:{}, y:{}, w:{}, h:{}}}\n",
            settings.theme,
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
    use crate::domain::types::{AssetKind, Quote, Sparkline};
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
                    currency: "USD".into(),
                    change_percent: Some(0.0),
                    as_of: "t".into(),
                    source: "mock".into(),
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
                previous_close: None,
                as_of: "t".into(),
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

        core.set_theme(ThemeMode::Dark).unwrap();
        let op = core.set_opacity(0.1).unwrap();
        assert!((op - 0.35).abs() < 1e-9);
        let geo = core
            .save_window_geometry(WindowGeometry {
                x: 1.0,
                y: 2.0,
                width: 10.0,
                height: 10.0,
            })
            .unwrap();
        assert!(geo.width >= 260.0);
        assert!(geo.height >= 360.0);

        let reloaded = core.get_state().unwrap();
        assert_eq!(reloaded.settings.theme, ThemeMode::Dark);
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
}
