//! Economy War Room — floating market watchlist widget.

mod application;
mod commands;
mod domain;
mod infrastructure;
mod ports;
mod state;

use application::scheduler::QuoteScheduler;
use domain::constants::{HotkeyPolicy, RefreshPolicy};
use domain::watchlist;
use infrastructure::store::{load_state, save_state};
use infrastructure::window_ctl;
use infrastructure::yahoo::YahooProvider;
use state::AppHandleState;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{Emitter, Manager};
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
        .setup(|app| {
            // --- app data dir + persisted state ---
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| e.to_string())?;
            std::fs::create_dir_all(&app_data_dir).map_err(|e| e.to_string())?;
            let persisted = load_state(&app_data_dir);
            // Ensure a valid file exists on first run.
            if let Err(e) = save_state(&app_data_dir, &persisted) {
                eprintln!("initial save_state: {e}");
            }

            // --- market data + scheduler ---
            let provider = YahooProvider::new().map_err(|e| e.to_string())?;
            let mut scheduler = QuoteScheduler::new(Arc::new(provider));
            scheduler.set_watchlist(watchlist::sorted_clone(&persisted.watchlist));
            scheduler.set_visible(true);

            let handle_state =
                AppHandleState::new(persisted.clone(), app_data_dir, scheduler, true);

            // --- window policy from settings ---
            if let Some(window) = app.get_webview_window("main") {
                let _ = window_ctl::apply_always_on_top(&window, true);
                if let Err(e) = window_ctl::apply_geometry(&window, &persisted.settings.window) {
                    eprintln!("apply_geometry: {e}");
                }
                if let Err(e) = window_ctl::apply_opacity(app.handle(), persisted.settings.opacity)
                {
                    eprintln!("apply_opacity: {e}");
                }
            } else {
                eprintln!("main window not found at setup");
            }

            // --- autostart (best-effort) ---
            {
                use tauri_plugin_autostart::ManagerExt;
                let autostart = app.autolaunch();
                if persisted.settings.autostart {
                    if let Err(e) = autostart.enable() {
                        eprintln!("autostart enable failed: {e}");
                    }
                } else if let Err(e) = autostart.disable() {
                    eprintln!("autostart disable failed: {e}");
                }
            }

            app.manage(handle_state);

            // --- global shortcut (best-effort) ---
            let hotkey_str = if persisted.settings.hotkey.is_empty() {
                HotkeyPolicy::DEFAULT.to_string()
            } else {
                persisted.settings.hotkey.clone()
            };

            match hotkey_str.parse::<Shortcut>() {
                Ok(shortcut) => {
                    let plugin = tauri_plugin_global_shortcut::Builder::new()
                        .with_handler(move |app, sc, event| {
                            if event.state() == ShortcutState::Pressed && *sc == shortcut {
                                commands::toggle_visibility_from_handle(app);
                            }
                        })
                        .build();
                    if let Err(e) = app.handle().plugin(plugin) {
                        eprintln!("global-shortcut plugin failed: {e}");
                    } else if let Err(e) = app.global_shortcut().register(shortcut) {
                        eprintln!("global-shortcut register failed: {e}");
                    }
                }
                Err(e) => {
                    eprintln!("invalid hotkey {hotkey_str:?}: {e}");
                    let plugin = tauri_plugin_global_shortcut::Builder::new().build();
                    if let Err(e) = app.handle().plugin(plugin) {
                        eprintln!("global-shortcut plugin failed: {e}");
                    }
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
                    if !state.visible.load(Ordering::SeqCst) {
                        continue;
                    }
                    let mut sched = state.scheduler.lock().await;
                    sched.tick_once().await;
                    let quotes = sched.quote_cache().all();
                    let sparks = sched.sparkline_cache().all();
                    drop(sched);
                    let _ = app_handle.emit("quotes-updated", quotes);
                    let _ = app_handle.emit("sparklines-updated", sparks);
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::add_symbol,
            commands::remove_symbol,
            commands::reorder_symbols,
            commands::set_theme,
            commands::set_opacity,
            commands::hide_widget,
            commands::toggle_widget_visibility,
            commands::save_window_geometry,
            commands::get_quotes,
            commands::get_sparklines,
            commands::quit_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
