//! Tauri command handlers — thin adapters over [`crate::application::service::AppCore`].

use crate::application::diagnostics::DiagLevel;
use crate::domain::types::{
    AssetKind, CardTint, PersistedState, Quote, Sparkline, SymbolSuggestion,
    WatchlistItem, WindowGeometry,
};
use crate::infrastructure::window_ctl;
use crate::state::AppHandleState;
use tauri::{AppHandle, Emitter, Manager, State};

fn note_err(state: &AppHandleState, ctx: &str, e: &str) {
    state
        .core
        .note(DiagLevel::Warn, format!("{ctx} failed: {e}"));
}

fn map_note<T>(state: &AppHandleState, ctx: &str, result: Result<T, String>) -> Result<T, String> {
    result.map_err(|e| {
        note_err(state, ctx, &e);
        e
    })
}

async fn emit_watchlist(app: &AppHandle, state: &AppHandleState) -> Result<(), String> {
    let payload = map_note(state, "watchlist_snapshot", state.core.watchlist_snapshot().await)?;
    app.emit("watchlist-updated", payload).map_err(|e| {
        let s = e.to_string();
        note_err(state, "emit watchlist-updated", &s);
        s
    })
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_state(state: State<'_, AppHandleState>) -> Result<PersistedState, String> {
    map_note(&state, "get_state", state.core.get_state())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn add_symbol(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    symbol: String,
    asset_kind: AssetKind,
) -> Result<WatchlistItem, String> {
    let item = map_note(
        &state,
        "add_symbol",
        state.core.add_symbol(symbol, asset_kind).await,
    )?;
    emit_watchlist(&app, &state).await?;
    Ok(item)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn remove_symbol(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    id: String,
) -> Result<(), String> {
    map_note(&state, "remove_symbol", state.core.remove_symbol(&id).await)?;
    emit_watchlist(&app, &state).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn remove_symbols(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    ids: Vec<String>,
) -> Result<(), String> {
    map_note(
        &state,
        "remove_symbols",
        state.core.remove_symbols(&ids).await,
    )?;
    emit_watchlist(&app, &state).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn set_card_tint(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    id: String,
    tint: CardTint,
) -> Result<(), String> {
    map_note(
        &state,
        "set_card_tint",
        state.core.set_card_tint(&id, tint).await,
    )?;
    emit_watchlist(&app, &state).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn reorder_symbols(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    ordered_ids: Vec<String>,
) -> Result<(), String> {
    map_note(
        &state,
        "reorder_symbols",
        state.core.reorder_symbols(&ordered_ids).await,
    )?;
    emit_watchlist(&app, &state).await
}

/// Persist and apply OS login autostart (Windows / macOS LaunchAgent / etc.).
#[tauri::command(rename_all = "snake_case")]
pub fn set_autostart(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    enabled: bool,
) -> Result<(), String> {
    map_note(&state, "set_autostart", state.core.set_autostart(enabled))?;
    use tauri_plugin_autostart::ManagerExt;
    let autolaunch = app.autolaunch();
    let result = if enabled {
        autolaunch.enable()
    } else {
        autolaunch.disable()
    };
    if let Err(e) = result {
        let s = e.to_string();
        note_err(&state, "autolaunch", &s);
        return Err(s);
    }
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_opacity(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    opacity: f64,
) -> Result<(), String> {
    let opacity = map_note(&state, "set_opacity", state.core.set_opacity(opacity))?;
    map_note(
        &state,
        "apply_opacity",
        window_ctl::apply_opacity(&app, opacity),
    )?;
    Ok(())
}

/// Persist quote interval. Parameter is **milliseconds** (historical name `secs`).
#[tauri::command(rename_all = "snake_case")]
pub async fn set_quote_refresh_secs(
    state: State<'_, AppHandleState>,
    secs: u64,
) -> Result<u64, String> {
    map_note(
        &state,
        "set_quote_refresh_secs",
        state.core.set_quote_refresh_ms(secs).await,
    )
}

/// Same as [`set_quote_refresh_secs`]; name matches stored unit (ms).
#[tauri::command(rename_all = "snake_case")]
pub async fn set_quote_refresh_ms(
    state: State<'_, AppHandleState>,
    ms: u64,
) -> Result<u64, String> {
    map_note(
        &state,
        "set_quote_refresh_ms",
        state.core.set_quote_refresh_ms(ms).await,
    )
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_column_ratios(
    state: State<'_, AppHandleState>,
    ratios: crate::domain::types::ColumnRatios,
) -> Result<crate::domain::types::ColumnRatios, String> {
    map_note(&state, "set_column_ratios", state.core.set_column_ratios(ratios))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn hide_widget(app: AppHandle, state: State<'_, AppHandleState>) -> Result<(), String> {
    map_note(&state, "hide_widget", set_visibility(&app, &state, false).await)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn toggle_widget_visibility(
    app: AppHandle,
    state: State<'_, AppHandleState>,
) -> Result<bool, String> {
    let next = !state.core.is_visible();
    map_note(
        &state,
        "toggle_widget_visibility",
        set_visibility(&app, &state, next).await,
    )?;
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
    map_note(
        &state,
        "save_window_geometry",
        state.core.save_window_geometry(geometry),
    )?;
    Ok(())
}

/// Update OS min-size from measured content height (logical px).
/// `grow_if_needed`: snap height to content (grow or shrink). False = min only.
#[tauri::command(rename_all = "snake_case")]
pub fn set_content_min_size(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    width: f64,
    height: f64,
    grow_if_needed: bool,
) -> Result<(), String> {
    state.set_content_min_logical(width, height);
    let (w, h) = state.content_min_logical();
    let window = map_note(&state, "set_content_min_size", window_ctl::main_window(&app))?;
    map_note(
        &state,
        "apply_content_min_size",
        window_ctl::apply_content_min_size(&window, w, h),
    )?;
    if grow_if_needed {
        // Full content-hug: settings open/close must shrink as well as grow.
        map_note(
            &state,
            "snap_height_to_content",
            window_ctl::snap_height_to_content(&window, w, h),
        )?;
        let _ = window_ctl::apply_clean_glass_edge(&window);
    }
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
    map_note(
        &state,
        "format_diagnostics",
        state.core.format_diagnostics().await,
    )
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
    match state.core.search_symbols(q, limit).await {
        Ok(hits) => Ok(hits),
        Err(e) => {
            state
                .core
                .note_throttled_default(DiagLevel::Warn, format!("search_symbols failed: {e}"));
            Err(e)
        }
    }
}

/// Trigger in-app update check manually.
#[tauri::command(rename_all = "snake_case")]
pub async fn check_for_updates(app: AppHandle) -> Result<bool, String> {
    crate::infrastructure::updater::check_and_install_update(&app).await
}

