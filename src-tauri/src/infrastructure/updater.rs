//! In-app updates: background download on check, install on user action (Electron-like).

use crate::application::diagnostics::DiagLevel;
use crate::state::AppHandleState;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

const UPDATE_CHECK_DELAY: Duration = Duration::from_secs(30);

/// Payload for frontend badge / tooltip.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct UpdateInfo {
    pub current_version: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DownloadProgress {
    pub version: String,
    pub chunk_len: usize,
    pub content_length: Option<u64>,
    pub received: u64,
}

/// Holds a verified package ready to install (download finished).
pub struct PendingUpdateState {
    ready: Mutex<Option<ReadyPackage>>,
    downloading: AtomicBool,
}

struct ReadyPackage {
    update: Update,
    bytes: Vec<u8>,
    info: UpdateInfo,
}

impl Default for PendingUpdateState {
    fn default() -> Self {
        Self {
            ready: Mutex::new(None),
            downloading: AtomicBool::new(false),
        }
    }
}

impl PendingUpdateState {
    pub fn is_ready(&self) -> bool {
        self.ready
            .lock()
            .map(|g| g.is_some())
            .unwrap_or(false)
    }

    pub fn is_downloading(&self) -> bool {
        self.downloading.load(Ordering::SeqCst)
    }
}

/// Release builds only: check, notify UI, download in background, then mark ready.
pub fn spawn_update_check(app: AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(UPDATE_CHECK_DELAY).await;
        if let Err(err) = prepare_update(&app).await {
            note(
                &app,
                DiagLevel::Warn,
                format!("updater prepare failed: {err}"),
            );
            let _ = app.emit("update-failed", err);
        }
    });
}

/// Check + download into pending store (no install). Emits progress / ready events.
pub async fn prepare_update(app: &AppHandle) -> Result<Option<UpdateInfo>, String> {
    let pending = app
        .try_state::<PendingUpdateState>()
        .ok_or_else(|| "pending update state missing".to_string())?;

    if pending.is_ready() {
        if let Ok(guard) = pending.ready.lock() {
            if let Some(pkg) = guard.as_ref() {
                let info = pkg.info.clone();
                let _ = app.emit("update-ready", &info);
                return Ok(Some(info));
            }
        }
    }

    if pending
        .downloading
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("update download already in progress".into());
    }

    let result = prepare_update_inner(app, &pending).await;
    pending.downloading.store(false, Ordering::SeqCst);
    result
}

async fn prepare_update_inner(
    app: &AppHandle,
    pending: &PendingUpdateState,
) -> Result<Option<UpdateInfo>, String> {
    note(app, DiagLevel::Info, "updater check started");

    let updater = app
        .updater()
        .map_err(|e| format!("updater init failed: {e}"))?;
    let Some(update) = updater
        .check()
        .await
        .map_err(|e| format!("updater check failed: {e}"))?
    else {
        note(app, DiagLevel::Info, "no update available");
        let _ = app.emit("update-not-available", ());
        return Ok(None);
    };

    let info = UpdateInfo {
        current_version: update.current_version.clone(),
        version: update.version.clone(),
    };
    note(
        app,
        DiagLevel::Info,
        format!(
            "update available: {} -> {} (downloading)",
            info.current_version, info.version
        ),
    );
    let _ = app.emit("update-available", &info);

    let version = info.version.clone();
    let app_progress = app.clone();
    let mut received: u64 = 0;
    let bytes = update
        .download(
            move |chunk_len, content_length| {
                received = received.saturating_add(chunk_len as u64);
                let _ = app_progress.emit(
                    "update-download-progress",
                    DownloadProgress {
                        version: version.clone(),
                        chunk_len,
                        content_length,
                        received,
                    },
                );
            },
            || {},
        )
        .await
        .map_err(|e| format!("updater download failed: {e}"))?;

    {
        let mut guard = pending
            .ready
            .lock()
            .map_err(|_| "pending update lock poisoned".to_string())?;
        *guard = Some(ReadyPackage {
            update,
            bytes,
            info: info.clone(),
        });
    }

    note(
        app,
        DiagLevel::Info,
        format!("update {} downloaded — ready to install", info.version),
    );
    let _ = app.emit("update-ready", &info);
    Ok(Some(info))
}

/// Install a previously downloaded package and restart.
pub fn install_pending_update(app: &AppHandle) -> Result<bool, String> {
    let pending = app
        .try_state::<PendingUpdateState>()
        .ok_or_else(|| "pending update state missing".to_string())?;

    let pkg = {
        let mut guard = pending
            .ready
            .lock()
            .map_err(|_| "pending update lock poisoned".to_string())?;
        guard.take()
    };

    let Some(pkg) = pkg else {
        return Ok(false);
    };

    note(
        app,
        DiagLevel::Info,
        format!("installing update {}", pkg.info.version),
    );
    pkg.update
        .install(&pkg.bytes)
        .map_err(|e| format!("updater install failed: {e}"))?;
    note(app, DiagLevel::Info, "update installed; restarting");
    app.restart();
}

/// Manual path: install pending if ready; otherwise download+install in one shot.
pub async fn check_and_install_update(app: &AppHandle) -> Result<bool, String> {
    if let Ok(true) = try_install_if_ready(app) {
        // restart() does not return on success
        return Ok(true);
    }

    if let Some(state) = app.try_state::<PendingUpdateState>() {
        if state.is_downloading() {
            return Err("update is still downloading".into());
        }
    }

    // No pending package — prepare then install immediately (user clicked ↻).
    match prepare_update(app).await? {
        Some(_) => {
            install_pending_update(app)?;
            Ok(true)
        }
        None => Ok(false),
    }
}

fn try_install_if_ready(app: &AppHandle) -> Result<bool, String> {
    let pending = match app.try_state::<PendingUpdateState>() {
        Some(p) => p,
        None => return Ok(false),
    };
    if !pending.is_ready() {
        return Ok(false);
    }
    install_pending_update(app)?;
    Ok(true)
}

fn note(app: &AppHandle, level: DiagLevel, message: impl Into<String>) {
    let message = message.into();
    if let Some(state) = app.try_state::<AppHandleState>() {
        state.core.note(level, message);
    } else {
        eprintln!("{message}");
    }
}
