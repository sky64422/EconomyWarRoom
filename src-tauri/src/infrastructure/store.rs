use crate::domain::constants::{
    clamp_column_ratios, clamp_opacity, clamp_quote_refresh_ms, HotkeyPolicy, OpacityPolicy,
    RefreshPolicy, WindowPolicy,
};
use crate::domain::types::{
    AppSettings, AssetKind, CardTint, ColumnRatios, PersistedState, WatchlistItem, WindowGeometry,
};
use std::path::{Path, PathBuf};

pub fn default_state() -> PersistedState {
    PersistedState {
        watchlist: vec![
            WatchlistItem {
                id: "seed-aapl".into(),
                symbol: "AAPL".into(),
                display_name: Some("Apple".into()),
                asset_kind: AssetKind::Equity,
                sort_index: 0,
                card_tint: CardTint::None,
            },
            WatchlistItem {
                id: "seed-btc".into(),
                symbol: "BTC-USD".into(),
                display_name: Some("Bitcoin".into()),
                asset_kind: AssetKind::Crypto,
                sort_index: 1,
                card_tint: CardTint::None,
            },
        ],
        settings: AppSettings {
            opacity: OpacityPolicy::DEFAULT,
            window: WindowGeometry {
                x: 80.0,
                y: 80.0,
                width: WindowPolicy::DEFAULT_WIDTH,
                height: WindowPolicy::DEFAULT_HEIGHT,
            },
            hotkey: HotkeyPolicy::DEFAULT.into(),
            autostart: true,
            quote_refresh_ms: RefreshPolicy::QUOTE_REFRESH_MS_DEFAULT,
            column_ratios: ColumnRatios::default(),
        },
    }
}

pub fn state_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("economy-war-room-state.json")
}

fn corrupt_backup_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "economy-war-room-state.json".into());
    path.with_file_name(format!("{name}.corrupt"))
}

fn sanitize_state(mut state: PersistedState) -> PersistedState {
    state.settings.opacity = clamp_opacity(state.settings.opacity);
    state.settings.quote_refresh_ms = clamp_quote_refresh_ms(state.settings.quote_refresh_ms);
    state.settings.column_ratios = clamp_column_ratios(state.settings.column_ratios);
    state
}

/// Load persisted state.
///
/// Missing file is a first run → `Ok(default_state())`.
/// Unreadable or invalid JSON is **not** treated as defaults: the file is moved
/// aside to `*.json.corrupt` and this returns `Err`.
pub fn load_state(app_data_dir: &Path) -> Result<PersistedState, String> {
    let path = state_path(app_data_dir);
    match std::fs::read_to_string(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(default_state()),
        Err(e) => Err(format!("read {}: {e}", path.display())),
        Ok(s) => match serde_json::from_str::<PersistedState>(&s) {
            Ok(state) => Ok(sanitize_state(state)),
            Err(e) => {
                let bak = corrupt_backup_path(&path);
                std::fs::rename(&path, &bak).map_err(|re| {
                    format!("corrupt state ({e}); also failed to move aside: {re}")
                })?;
                Err(format!(
                    "corrupt state JSON ({e}); moved to {}",
                    bak.display()
                ))
            }
        },
    }
}

pub fn save_state(app_data_dir: &Path, state: &PersistedState) -> Result<(), String> {
    std::fs::create_dir_all(app_data_dir).map_err(|e| e.to_string())?;
    let path = state_path(app_data_dir);
    let mut cloned = state.clone();
    cloned.settings.opacity = clamp_opacity(cloned.settings.opacity);
    cloned.settings.quote_refresh_ms = clamp_quote_refresh_ms(cloned.settings.quote_refresh_ms);
    cloned.settings.column_ratios = clamp_column_ratios(cloned.settings.column_ratios);
    let json = serde_json::to_string_pretty(&cloned).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn round_trip() {
        let dir = tempdir().unwrap();
        let mut state = default_state();
        state.settings.opacity = 0.77;
        save_state(dir.path(), &state).unwrap();
        let loaded = load_state(dir.path()).unwrap();
        assert!((loaded.settings.opacity - 0.77).abs() < 1e-9);
        assert_eq!(loaded.watchlist.len(), 2);
    }

    #[test]
    fn load_missing_file_returns_defaults() {
        let dir = tempdir().unwrap();
        let loaded = load_state(dir.path()).unwrap();
        assert_eq!(loaded.watchlist.len(), 2);
        assert!(loaded.settings.autostart);
    }

    #[test]
    fn load_corrupt_json_is_err_and_keeps_backup() {
        let dir = tempdir().unwrap();
        let path = state_path(dir.path());
        std::fs::write(&path, "{not-json").unwrap();
        let err = load_state(dir.path()).unwrap_err();
        assert!(err.contains("corrupt"), "{err}");
        assert!(!path.exists());
        assert!(corrupt_backup_path(&path).exists());
    }

    #[test]
    fn missing_column_ratios_defaults() {
        let dir = tempdir().unwrap();
        let path = state_path(dir.path());
        // Older files omit column_ratios entirely.
        let state = default_state();
        let json = serde_json::to_value(&state).unwrap();
        let mut obj = json.as_object().unwrap().clone();
        let mut settings = obj.get("settings").unwrap().as_object().unwrap().clone();
        settings.remove("column_ratios");
        obj.insert("settings".into(), serde_json::Value::Object(settings));
        std::fs::write(&path, serde_json::to_string(&obj).unwrap()).unwrap();
        let loaded = load_state(dir.path()).unwrap();
        assert_eq!(loaded.settings.column_ratios, ColumnRatios::default());
    }

    #[test]
    fn missing_quote_refresh_uses_default_ms() {
        let dir = tempdir().unwrap();
        let path = state_path(dir.path());
        let state = default_state();
        let json = serde_json::to_value(&state).unwrap();
        let mut obj = json.as_object().unwrap().clone();
        let mut settings = obj.get("settings").unwrap().as_object().unwrap().clone();
        settings.remove("quote_refresh_secs");
        obj.insert("settings".into(), serde_json::Value::Object(settings));
        std::fs::write(&path, serde_json::to_string(&obj).unwrap()).unwrap();
        let loaded = load_state(dir.path()).unwrap();
        assert_eq!(
            loaded.settings.quote_refresh_ms,
            RefreshPolicy::QUOTE_REFRESH_MS_DEFAULT
        );
    }

    #[test]
    fn save_clamps_out_of_range_opacity() {
        let dir = tempdir().unwrap();
        let mut state = default_state();
        state.settings.opacity = 0.01;
        save_state(dir.path(), &state).unwrap();
        let loaded = load_state(dir.path()).unwrap();
        assert!((loaded.settings.opacity - OpacityPolicy::MIN).abs() < 1e-9);
    }

    #[test]
    fn quote_refresh_json_key_remains_secs() {
        let state = default_state();
        let json = serde_json::to_value(&state).unwrap();
        let settings = json.get("settings").unwrap();
        assert!(settings.get("quote_refresh_secs").is_some());
        assert!(settings.get("quote_refresh_ms").is_none());
        assert_eq!(
            settings.get("quote_refresh_secs").unwrap().as_u64(),
            Some(RefreshPolicy::QUOTE_REFRESH_MS_DEFAULT)
        );
    }

    #[test]
    fn state_path_name() {
        let dir = tempdir().unwrap();
        assert!(state_path(dir.path())
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains("economy-war-room-state"));
    }
}
