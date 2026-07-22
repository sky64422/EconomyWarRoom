//! Tauri command handlers — thin adapters over [`crate::application::service::AppCore`].

use crate::application::diagnostics::DiagLevel;
use crate::domain::types::{
    AssetKind, PersistedState, Quote, Sparkline, ThemeMode, WatchlistItem, WindowGeometry,
};
use crate::infrastructure::window_ctl;
use crate::state::AppHandleState;
use tauri::{AppHandle, Emitter, Manager, State};

#[tauri::command]
pub fn get_state(state: State<'_, AppHandleState>) -> Result<PersistedState, String> {
    state.core.get_state()
}

#[tauri::command]
pub async fn add_symbol(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    symbol: String,
    asset_kind: AssetKind,
) -> Result<WatchlistItem, String> {
    match state.core.add_symbol(symbol, asset_kind).await {
        Ok(item) => {
            let payload = state.core.watchlist_snapshot().await?;
            app.emit("watchlist-updated", payload)
                .map_err(|e| e.to_string())?;
            Ok(item)
        }
        Err(e) => {
            state.core.note(DiagLevel::Warn, format!("add_symbol failed: {e}"));
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn remove_symbol(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    id: String,
) -> Result<(), String> {
    state.core.remove_symbol(&id).await?;
    let payload = state.core.watchlist_snapshot().await?;
    app.emit("watchlist-updated", payload)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn reorder_symbols(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    ordered_ids: Vec<String>,
) -> Result<(), String> {
    state.core.reorder_symbols(&ordered_ids).await?;
    let payload = state.core.watchlist_snapshot().await?;
    app.emit("watchlist-updated", payload)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn set_theme(state: State<'_, AppHandleState>, theme: ThemeMode) -> Result<(), String> {
    state.core.set_theme(theme)
}

#[tauri::command]
pub fn set_opacity(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    opacity: f64,
) -> Result<(), String> {
    let opacity = state.core.set_opacity(opacity)?;
    window_ctl::apply_opacity(&app, opacity)?;
    Ok(())
}

#[tauri::command]
pub async fn hide_widget(app: AppHandle, state: State<'_, AppHandleState>) -> Result<(), String> {
    set_visibility(&app, &state, false).await
}

#[tauri::command]
pub async fn toggle_widget_visibility(
    app: AppHandle,
    state: State<'_, AppHandleState>,
) -> Result<bool, String> {
    let next = !state.core.is_visible();
    set_visibility(&app, &state, next).await?;
    Ok(next)
}

/// Shared visibility toggle used by commands and the global hotkey handler.
pub async fn set_visibility(
    app: &AppHandle,
    state: &AppHandleState,
    visible: bool,
) -> Result<(), String> {
    let window = window_ctl::main_window(app)?;
    if visible {
        window_ctl::show_window(&window)?;
    } else {
        window_ctl::hide_window(&window)?;
    }
    state.core.set_visible_state(visible).await;
    Ok(())
}

/// Sync helper for the global-shortcut handler (no State extractor).
pub fn toggle_visibility_from_handle(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(state) = app.try_state::<AppHandleState>() else {
            eprintln!("toggle_visibility: AppHandleState not ready");
            return;
        };
        let next = !state.core.is_visible();
        if let Err(e) = set_visibility(&app, &state, next).await {
            eprintln!("toggle_visibility failed: {e}");
            state
                .core
                .note(DiagLevel::Error, format!("toggle_visibility failed: {e}"));
        }
    });
}

#[tauri::command]
pub fn save_window_geometry(
    state: State<'_, AppHandleState>,
    geometry: WindowGeometry,
) -> Result<(), String> {
    state.core.save_window_geometry(geometry)?;
    Ok(())
}

#[tauri::command]
pub async fn get_quotes(state: State<'_, AppHandleState>) -> Result<Vec<Quote>, String> {
    Ok(state.core.get_quotes().await)
}

#[tauri::command]
pub async fn get_sparklines(state: State<'_, AppHandleState>) -> Result<Vec<Sparkline>, String> {
    Ok(state.core.get_sparklines().await)
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// Build diagnostics text for clipboard (Mode B agent handoff).
#[tauri::command]
pub async fn get_diagnostics(state: State<'_, AppHandleState>) -> Result<String, String> {
    state.core.note(DiagLevel::Info, "diagnostics snapshot requested");
    state.core.format_diagnostics().await
}
