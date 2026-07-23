//! Managed application state shared by commands and setup.

use crate::application::scheduler::QuoteScheduler;
use crate::application::service::AppCore;
use crate::domain::types::PersistedState;
use std::path::PathBuf;
use std::sync::Arc;

pub struct AppHandleState {
    pub core: Arc<AppCore>,
}

impl AppHandleState {
    pub fn new(
        persisted: PersistedState,
        app_data_dir: PathBuf,
        scheduler: QuoteScheduler,
        visible: bool,
    ) -> Self {
        Self {
            core: Arc::new(AppCore::new(persisted, app_data_dir, scheduler, visible)),
        }
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
                max_concurrent: 1,
                min_interval: Duration::from_secs(1),
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
            })
        }
    }

    #[test]
    fn app_handle_state_wraps_core() {
        let dir = tempdir().unwrap();
        let sched = QuoteScheduler::new(Arc::new(MockProvider));
        let state = AppHandleState::new(
            crate::infrastructure::store::default_state(),
            dir.path().to_path_buf(),
            sched,
            true,
        );
        assert!(state.core.is_visible());
        let s = state.core.get_state().unwrap();
        assert_eq!(s.watchlist.len(), 2);
    }
}
