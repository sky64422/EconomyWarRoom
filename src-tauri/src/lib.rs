//! Economy War Room — floating market watchlist widget.

pub mod application;
mod commands;
pub mod domain;
pub mod infrastructure;
pub mod ports;
pub mod state;

use application::diagnostics::DiagLevel;
use application::scheduler::QuoteScheduler;
use domain::constants::{HotkeyPolicy, RefreshPolicy};
use domain::watchlist;
use infrastructure::store::{load_state, save_state};
use infrastructure::updater;
use infrastructure::window_ctl;
use infrastructure::yahoo::YahooProvider;
use state::AppHandleState;
use std::sync::Arc;
use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, _sc, event| {
                    if event.state() == ShortcutState::Pressed {
                        commands::toggle_visibility_from_handle(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            // --- app data dir + persisted state ---
            let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
            std::fs::create_dir_all(&app_data_dir).map_err(|e| e.to_string())?;
            let persisted = load_state(&app_data_dir);
            // Ensure a valid file exists on first run.
            let initial_save_err = save_state(&app_data_dir, &persisted).err();

            // --- market data + scheduler ---
            let provider = YahooProvider::new().map_err(|e| e.to_string())?;
            let mut scheduler = QuoteScheduler::new(Arc::new(provider));
            scheduler.set_watchlist(watchlist::sorted_clone(&persisted.watchlist));
            scheduler.set_visible(true);

            let handle_state =
                AppHandleState::new(persisted.clone(), app_data_dir, scheduler, true);
            handle_state
                .core
                .note(DiagLevel::Info, "app setup starting");
            if let Some(e) = initial_save_err {
                eprintln!("initial save_state: {e}");
                handle_state
                    .core
                    .note(DiagLevel::Error, format!("initial save_state: {e}"));
            }

            // --- window policy from settings ---
            if let Some(window) = app.get_webview_window("main") {
                let _ = window_ctl::apply_always_on_top(&window, true);
                if let Err(e) = window_ctl::apply_geometry(&window, &persisted.settings.window) {
                    eprintln!("apply_geometry: {e}");
                    handle_state
                        .core
                        .note(DiagLevel::Warn, format!("apply_geometry: {e}"));
                }
                if let Err(e) = window_ctl::apply_opacity(app.handle(), persisted.settings.opacity)
                {
                    eprintln!("apply_opacity: {e}");
                    handle_state
                        .core
                        .note(DiagLevel::Warn, format!("apply_opacity: {e}"));
                }
                let _ = window_ctl::show_window(&window);
            } else {
                eprintln!("main window not found at setup");
                handle_state
                    .core
                    .note(DiagLevel::Error, "main window not found at setup");
            }

            // --- autostart (best-effort) ---
            {
                use tauri_plugin_autostart::ManagerExt;
                let autostart = app.autolaunch();
                if persisted.settings.autostart {
                    if let Err(e) = autostart.enable() {
                        eprintln!("autostart enable failed: {e}");
                        handle_state
                            .core
                            .note(DiagLevel::Warn, format!("autostart enable failed: {e}"));
                    }
                } else if let Err(e) = autostart.disable() {
                    eprintln!("autostart disable failed: {e}");
                    handle_state
                        .core
                        .note(DiagLevel::Warn, format!("autostart disable failed: {e}"));
                }
            }

            app.manage(handle_state);

            if let Some(state) = app.try_state::<AppHandleState>() {
                let core = state.core.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = core.apply_quote_refresh_to_scheduler().await {
                        eprintln!("apply_quote_refresh_to_scheduler: {e}");
                    }
                });
            }

            updater::spawn_update_check(app.handle().clone());

            // --- global shortcut (best-effort) ---
            let hotkey_str = if persisted.settings.hotkey.is_empty() {
                HotkeyPolicy::DEFAULT.to_string()
            } else {
                persisted.settings.hotkey.clone()
            };

            if let Ok(shortcut) = hotkey_str.parse::<Shortcut>() {
                if let Err(e) = app.global_shortcut().register(shortcut) {
                    eprintln!("global-shortcut register failed: {e}");
                    if let Some(state) = app.handle().try_state::<AppHandleState>() {
                        state.core.note(
                            DiagLevel::Warn,
                            format!("global-shortcut register failed: {e}"),
                        );
                    }
                } else if let Some(state) = app.handle().try_state::<AppHandleState>() {
                    state
                        .core
                        .note(DiagLevel::Info, format!("hotkey registered: {hotkey_str}"));
                }
            }

            // --- quote tick loop ---
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(RefreshPolicy::TICK);
                loop {
                    interval.tick().await;
                    let Some(state) = app_handle.try_state::<AppHandleState>() else {
                        continue;
                    };
                    if !state.core.is_visible() {
                        continue;
                    }
                    {
                        let mut sched = state.core.scheduler.lock().await;
                        sched.tick_once().await;
                        for msg in sched.drain_diag_notes() {
                            state.core.note_throttled_default(DiagLevel::Warn, msg);
                        }
                    }
                    let quotes = state.core.get_quotes().await;
                    let sparks = state.core.get_sparklines().await;
                    let _ = app_handle.emit("quotes-updated", quotes);
                    let _ = app_handle.emit("sparklines-updated", sparks);
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            // Hard clamp while resizing: if drag goes under content min, snap to floor
            // and re-assert OS min so further drag hits a wall (not a rubber-band loop
            // driven by frontend setSize after the fact).
            if let WindowEvent::Resized(size) = event {
                let Some(state) = window.try_state::<AppHandleState>() else {
                    return;
                };
                let (min_w, min_h) = state.content_min_logical();
                if let Err(e) =
                    window_ctl::clamp_physical_size_to_content_min(window, *size, min_w, min_h)
                {
                    eprintln!("clamp resize: {e}");
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::add_symbol,
            commands::remove_symbol,
            commands::remove_symbols,
            commands::set_card_tint,
            commands::reorder_symbols,
            commands::set_theme,
            commands::set_opacity,
            commands::set_autostart,
            commands::set_quote_refresh_secs,
            commands::hide_widget,
            commands::toggle_widget_visibility,
            commands::save_window_geometry,
            commands::set_content_min_size,
            commands::get_quotes,
            commands::get_sparklines,
            commands::quit_app,
            commands::get_diagnostics,
            commands::search_symbols,
            commands::check_for_updates,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
