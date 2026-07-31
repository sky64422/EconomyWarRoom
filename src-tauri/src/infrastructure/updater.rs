use crate::application::diagnostics::DiagLevel;
use crate::state::AppHandleState;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

const UPDATE_CHECK_DELAY: Duration = Duration::from_secs(30);

pub fn spawn_update_check(app: AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(UPDATE_CHECK_DELAY).await;
        if let Err(err) = check_and_install_update(&app).await {
            note(
                &app,
                DiagLevel::Warn,
                format!("updater check failed: {err}"),
            );
        }
    });
}

/// Check GitHub `latest.json`, install if newer, then **restart** so the new binary loads.
///
/// Returns:
/// - `Ok(true)` only if an update was applied (normally does not return — process restarts)
/// - `Ok(false)` when already on the latest published version
pub async fn check_and_install_update(app: &AppHandle) -> Result<bool, String> {
    note(app, DiagLevel::Info, "updater check started");

    let updater = app
        .updater()
        .map_err(|e| format!("updater init failed: {e}"))?;
    match updater
        .check()
        .await
        .map_err(|e| format!("updater check failed: {e}"))?
    {
        Some(update) => {
            note(
                app,
                DiagLevel::Info,
                format!(
                    "update available: {} -> {}",
                    update.current_version, update.version
                ),
            );
            update
                .download_and_install(|_, _| {}, || {})
                .await
                .map_err(|e| format!("updater install failed: {e}"))?;
            note(app, DiagLevel::Info, "update installed; restarting");
            // Without restart the old process keeps running and the UI looks unchanged.
            app.restart();
        }
        None => {
            note(app, DiagLevel::Info, "no update available");
            Ok(false)
        }
    }
}

fn note(app: &AppHandle, level: DiagLevel, message: impl Into<String>) {
    let message = message.into();
    if let Some(state) = app.try_state::<AppHandleState>() {
        state.core.note(level, message);
    } else {
        eprintln!("{message}");
    }
}
