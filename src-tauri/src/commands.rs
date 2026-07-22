//! Tauri command handlers ??thin adapters over [`crate::application::service::AppCore`].

use crate::application::diagnostics::DiagLevel;
use crate::domain::types::{
    AssetKind, PersistedState, Quote, Sparkline, SymbolSuggestion, ThemeMode, WatchlistItem,
    WindowGeometry,
};
use crate::infrastructure::yahoo::YahooProvider;
use crate::infrastructure::window_ctl;
use crate::state::AppHandleState;
use tauri::{AppHandle, Emitter, Manager, State};

fn note_err(state: &AppHandleState, ctx: &str, e: &str) {
    state
        .core
        .note(DiagLevel::Warn, format!("{ctx} failed: {e}"));
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_state(state: State<'_, AppHandleState>) -> Result<PersistedState, String> {
    state.core.get_state().map_err(|e| {
        note_err(&state, "get_state", &e);
        e
    })
}

#[tauri::command(rename_all = "snake_case")]
pub async fn add_symbol(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    symbol: String,
    asset_kind: AssetKind,
) -> Result<WatchlistItem, String> {
    match state.core.add_symbol(symbol, asset_kind).await {
        Ok(item) => {
            let payload = state.core.watchlist_snapshot().await.map_err(|e| {
                note_err(&state, "watchlist_snapshot", &e);
                e
            })?;
            app.emit("watchlist-updated", payload).map_err(|e| {
                let s = e.to_string();
                note_err(&state, "emit watchlist-updated", &s);
                s
            })?;
            Ok(item)
        }
        Err(e) => {
            note_err(&state, "add_symbol", &e);
            Err(e)
        }
    }
}

#[tauri::command(rename_all = "snake_case")]
pub async fn remove_symbol(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    id: String,
) -> Result<(), String> {
    if let Err(e) = state.core.remove_symbol(&id).await {
        note_err(&state, "remove_symbol", &e);
        return Err(e);
    }
    let payload = state.core.watchlist_snapshot().await.map_err(|e| {
        note_err(&state, "watchlist_snapshot", &e);
        e
    })?;
    app.emit("watchlist-updated", payload).map_err(|e| {
        let s = e.to_string();
        note_err(&state, "emit watchlist-updated", &s);
        s
    })?;
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn reorder_symbols(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    ordered_ids: Vec<String>,
) -> Result<(), String> {
    if let Err(e) = state.core.reorder_symbols(&ordered_ids).await {
        note_err(&state, "reorder_symbols", &e);
        return Err(e);
    }
    let payload = state.core.watchlist_snapshot().await.map_err(|e| {
        note_err(&state, "watchlist_snapshot", &e);
        e
    })?;
    app.emit("watchlist-updated", payload).map_err(|e| {
        let s = e.to_string();
        note_err(&state, "emit watchlist-updated", &s);
        s
    })?;
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_theme(state: State<'_, AppHandleState>, theme: ThemeMode) -> Result<(), String> {
    state.core.set_theme(theme).map_err(|e| {
        note_err(&state, "set_theme", &e);
        e
    })
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_opacity(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    opacity: f64,
) -> Result<(), String> {
    let opacity = state.core.set_opacity(opacity).map_err(|e| {
        note_err(&state, "set_opacity", &e);
        e
    })?;
    window_ctl::apply_opacity(&app, opacity).map_err(|e| {
        note_err(&state, "apply_opacity", &e);
        e
    })?;
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn hide_widget(app: AppHandle, state: State<'_, AppHandleState>) -> Result<(), String> {
    set_visibility(&app, &state, false).await.map_err(|e| {
        note_err(&state, "hide_widget", &e);
        e
    })
}

#[tauri::command(rename_all = "snake_case")]
pub async fn toggle_widget_visibility(
    app: AppHandle,
    state: State<'_, AppHandleState>,
) -> Result<bool, String> {
    let next = !state.core.is_visible();
    set_visibility(&app, &state, next).await.map_err(|e| {
        note_err(&state, "toggle_widget_visibility", &e);
        e
    })?;
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

#[tauri::command(rename_all = "snake_case")]
pub fn save_window_geometry(
    state: State<'_, AppHandleState>,
    geometry: WindowGeometry,
) -> Result<(), String> {
    state.core.save_window_geometry(geometry).map_err(|e| {
        note_err(&state, "save_window_geometry", &e);
        e
    })?;
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_quotes(state: State<'_, AppHandleState>) -> Result<Vec<Quote>, String> {
    Ok(state.core.get_quotes().await)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_sparklines(state: State<'_, AppHandleState>) -> Result<Vec<Sparkline>, String> {
    Ok(state.core.get_sparklines().await)
}

#[tauri::command(rename_all = "snake_case")]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// Build diagnostics text for clipboard (Mode B agent handoff).
#[tauri::command(rename_all = "snake_case")]
pub async fn get_diagnostics(state: State<'_, AppHandleState>) -> Result<String, String> {
    state
        .core
        .note(DiagLevel::Info, "diagnostics snapshot requested");
    state.core.format_diagnostics().await.map_err(|e| {
        note_err(&state, "format_diagnostics", &e);
        e
    })
}

/// Symbol autocomplete for the add flow (Yahoo search + substring filter).
#[tauri::command(rename_all = "snake_case")]
pub async fn search_symbols(
    state: State<'_, AppHandleState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<SymbolSuggestion>, String> {
    let limit = limit.unwrap_or(8).clamp(1, 20);
    let q = query.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let provider = YahooProvider::new().map_err(|e| {
        note_err(&state, "search_symbols provider", &e);
        e
    })?;
    match provider.search_symbols(q, limit).await {
        Ok(hits) => Ok(hits),
        Err(e) => {
            state.core.note_throttled_default(
                DiagLevel::Warn,
                format!("search_symbols failed: {e}"),
            );
            Err(e)
        }
    }
}
