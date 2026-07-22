use crate::domain::constants::{
    clamp_opacity, HotkeyPolicy, OpacityPolicy, WindowPolicy,
};
use crate::domain::types::{
    AppSettings, AssetKind, PersistedState, ThemeMode, WatchlistItem, WindowGeometry,
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
            },
            WatchlistItem {
                id: "seed-btc".into(),
                symbol: "BTC-USD".into(),
                display_name: Some("Bitcoin".into()),
                asset_kind: AssetKind::Crypto,
                sort_index: 1,
            },
        ],
        settings: AppSettings {
            theme: ThemeMode::System,
            opacity: OpacityPolicy::DEFAULT,
            window: WindowGeometry {
                x: 80.0,
                y: 80.0,
                width: WindowPolicy::DEFAULT_WIDTH,
                height: WindowPolicy::DEFAULT_HEIGHT,
            },
            hotkey: HotkeyPolicy::DEFAULT.into(),
            autostart: true,
        },
    }
}

pub fn state_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("economy-war-room-state.json")
}

pub fn load_state(app_data_dir: &Path) -> PersistedState {
    let path = state_path(app_data_dir);
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|_| default_state()),
        Err(_) => default_state(),
    }
}

pub fn save_state(app_data_dir: &Path, state: &PersistedState) -> Result<(), String> {
    std::fs::create_dir_all(app_data_dir).map_err(|e| e.to_string())?;
    let path = state_path(app_data_dir);
    let mut cloned = state.clone();
    cloned.settings.opacity = clamp_opacity(cloned.settings.opacity);
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
        let loaded = load_state(dir.path());
        assert!((loaded.settings.opacity - 0.77).abs() < 1e-9);
        assert_eq!(loaded.watchlist.len(), 2);
    }

    #[test]
    fn load_missing_file_returns_defaults() {
        let dir = tempdir().unwrap();
        let loaded = load_state(dir.path());
        assert_eq!(loaded.watchlist.len(), 2);
        assert!(loaded.settings.autostart);
    }

    #[test]
    fn load_corrupt_json_falls_back_to_defaults() {
        let dir = tempdir().unwrap();
        let path = state_path(dir.path());
        std::fs::write(&path, "{not-json").unwrap();
        let loaded = load_state(dir.path());
        assert_eq!(loaded.watchlist[0].symbol, "AAPL");
    }

    #[test]
    fn save_clamps_out_of_range_opacity() {
        let dir = tempdir().unwrap();
        let mut state = default_state();
        state.settings.opacity = 0.01;
        save_state(dir.path(), &state).unwrap();
        let loaded = load_state(dir.path());
        assert!((loaded.settings.opacity - OpacityPolicy::MIN).abs() < 1e-9);
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
