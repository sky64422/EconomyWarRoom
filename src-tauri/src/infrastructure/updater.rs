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

pub async fn check_and_install_update(app: &AppHandle) -> Result<bool, String> {
    note(app, DiagLevel::Info, "updater check started");

    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await.map_err(|e| e.to_string())? {
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
                .map_err(|e| e.to_string())?;
            note(app, DiagLevel::Info, "update installed");
            Ok(true)
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
