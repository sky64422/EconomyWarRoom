//! Tauri command handlers for the widget UI.

use crate::domain::constants::clamp_opacity;
use crate::domain::types::{
    AssetKind, PersistedState, Quote, Sparkline, ThemeMode, WatchlistItem, WindowGeometry,
};
use crate::domain::watchlist;
use crate::infrastructure::store::save_state;
use crate::infrastructure::window_ctl;
use crate::state::AppHandleState;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};

fn persist(state: &AppHandleState) -> Result<(), String> {
    let persisted = state
        .persisted
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?;
    save_state(&state.app_data_dir, &persisted)
}

async fn sync_scheduler_watchlist(state: &AppHandleState) -> Result<(), String> {
    let items = {
        let persisted = state
            .persisted
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        watchlist::sorted_clone(&persisted.watchlist)
    };
    let mut sched = state.scheduler.lock().await;
    sched.set_watchlist(items);
    Ok(())
}

#[tauri::command]
pub fn get_state(state: State<'_, AppHandleState>) -> Result<PersistedState, String> {
    state
        .persisted
        .lock()
        .map(|g| g.clone())
        .map_err(|_| "state lock poisoned".into())
}

#[tauri::command]
pub async fn add_symbol(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    symbol: String,
    asset_kind: AssetKind,
) -> Result<WatchlistItem, String> {
    let item = {
        let mut persisted = state
            .persisted
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        let item = watchlist::add_item(&mut persisted.watchlist, &symbol, asset_kind, None)?;
        save_state(&state.app_data_dir, &persisted)?;
        item
    };

    {
        let mut sched = state.scheduler.lock().await;
        let items = {
            let persisted = state
                .persisted
                .lock()
                .map_err(|_| "state lock poisoned".to_string())?;
            watchlist::sorted_clone(&persisted.watchlist)
        };
        sched.set_watchlist(items);
        sched.bump_priority(item.symbol.clone());
    }

    let watchlist_payload = {
        let persisted = state
            .persisted
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        watchlist::sorted_clone(&persisted.watchlist)
    };
    app.emit("watchlist-updated", watchlist_payload)
        .map_err(|e| e.to_string())?;

    Ok(item)
}

#[tauri::command]
pub async fn remove_symbol(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    id: String,
) -> Result<(), String> {
    {
        let mut persisted = state
            .persisted
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        if !watchlist::remove_item(&mut persisted.watchlist, &id) {
            return Err(format!("unknown id {id}"));
        }
        save_state(&state.app_data_dir, &persisted)?;
    }
    sync_scheduler_watchlist(&state).await?;

    let watchlist_payload = {
        let persisted = state
            .persisted
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        watchlist::sorted_clone(&persisted.watchlist)
    };
    app.emit("watchlist-updated", watchlist_payload)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn reorder_symbols(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    ordered_ids: Vec<String>,
) -> Result<(), String> {
    {
        let mut persisted = state
            .persisted
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        watchlist::reorder(&mut persisted.watchlist, &ordered_ids)?;
        save_state(&state.app_data_dir, &persisted)?;
    }
    sync_scheduler_watchlist(&state).await?;

    let watchlist_payload = {
        let persisted = state
            .persisted
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        watchlist::sorted_clone(&persisted.watchlist)
    };
    app.emit("watchlist-updated", watchlist_payload)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn set_theme(
    state: State<'_, AppHandleState>,
    theme: ThemeMode,
) -> Result<(), String> {
    {
        let mut persisted = state
            .persisted
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        persisted.settings.theme = theme;
    }
    persist(&state)
}

#[tauri::command]
pub fn set_opacity(
    app: AppHandle,
    state: State<'_, AppHandleState>,
    opacity: f64,
) -> Result<(), String> {
    let opacity = clamp_opacity(opacity);
    {
        let mut persisted = state
            .persisted
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        persisted.settings.opacity = opacity;
    }
    persist(&state)?;
    window_ctl::apply_opacity(&app, opacity)?;
    Ok(())
}

#[tauri::command]
pub async fn hide_widget(
    app: AppHandle,
    state: State<'_, AppHandleState>,
) -> Result<(), String> {
    set_visibility(&app, &state, false).await
}

#[tauri::command]
pub async fn toggle_widget_visibility(
    app: AppHandle,
    state: State<'_, AppHandleState>,
) -> Result<bool, String> {
    let next = !state.visible.load(Ordering::SeqCst);
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
    state.visible.store(visible, Ordering::SeqCst);
    {
        let mut sched = state.scheduler.lock().await;
        sched.set_visible(visible);
    }
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
        let next = !state.visible.load(Ordering::SeqCst);
        if let Err(e) = set_visibility(&app, &state, next).await {
            eprintln!("toggle_visibility failed: {e}");
        }
    });
}

#[tauri::command]
pub fn save_window_geometry(
    state: State<'_, AppHandleState>,
    geometry: WindowGeometry,
) -> Result<(), String> {
    {
        let mut persisted = state
            .persisted
            .lock()
            .map_err(|_| "state lock poisoned".to_string())?;
        persisted.settings.window = geometry;
    }
    persist(&state)
}

#[tauri::command]
pub async fn get_quotes(state: State<'_, AppHandleState>) -> Result<Vec<Quote>, String> {
    let sched = state.scheduler.lock().await;
    Ok(sched.quote_cache().all())
}

#[tauri::command]
pub async fn get_sparklines(state: State<'_, AppHandleState>) -> Result<Vec<Sparkline>, String> {
    let sched = state.scheduler.lock().await;
    Ok(sched.sparkline_cache().all())
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}
