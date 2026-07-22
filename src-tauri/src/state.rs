//! Managed application state shared by commands and setup.

use crate::application::scheduler::QuoteScheduler;
use crate::domain::types::PersistedState;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;

pub struct AppHandleState {
    pub persisted: Mutex<PersistedState>,
    pub app_data_dir: PathBuf,
    pub scheduler: Arc<AsyncMutex<QuoteScheduler>>,
    pub visible: AtomicBool,
}

impl AppHandleState {
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
        }
    }
}
