use std::time::Duration;

/// Quote / refresh scheduler policy.
pub struct RefreshPolicy;

impl RefreshPolicy {
    /// Scheduler loop cadence (shorter than network RTT so we pick work promptly).
    pub const TICK: Duration = Duration::from_millis(500);
    pub const BATCH_SIZE: usize = 4;
    /// Default min interval between quote fetches for the same symbol.
    pub const MIN_QUOTE_INTERVAL: Duration = Duration::from_millis(500);
    /// User-configurable quote interval bounds (**milliseconds** in persisted field
    /// `quote_refresh_secs`; legacy values 1..=120 are treated as whole seconds).
    pub const QUOTE_REFRESH_MS_MIN: u64 = 250;
    pub const QUOTE_REFRESH_MS_MAX: u64 = 120_000;
    pub const QUOTE_REFRESH_MS_DEFAULT: u64 = 500;
    /// @deprecated name — use QUOTE_REFRESH_MS_*; kept for call-site clarity.
    pub const QUOTE_REFRESH_SECS_MIN: u64 = 250;
    pub const QUOTE_REFRESH_SECS_MAX: u64 = 120_000;
    pub const QUOTE_REFRESH_SECS_DEFAULT: u64 = 500;
    pub const MAX_CONCURRENT: usize = 3;
    pub const SPARKLINE_MIN_INTERVAL: Duration = Duration::from_secs(300);
    pub const BACKOFF_INITIAL: Duration = Duration::from_secs(5);
    pub const BACKOFF_MAX: Duration = Duration::from_secs(120);
}

/// Normalize + clamp quote refresh.
///
/// Persisted field is historically named `quote_refresh_secs`. Values in **1..=120**
/// are treated as **legacy whole seconds** (multiplied by 1000). Larger values are
/// milliseconds (e.g. 500 → 500ms, 1000 → 1s).
pub fn clamp_quote_refresh_secs(stored: u64) -> u64 {
    let ms = if (1..=120).contains(&stored) {
        stored.saturating_mul(1000)
    } else if stored == 0 {
        RefreshPolicy::QUOTE_REFRESH_MS_DEFAULT
    } else {
        stored
    };
    ms.clamp(
        RefreshPolicy::QUOTE_REFRESH_MS_MIN,
        RefreshPolicy::QUOTE_REFRESH_MS_MAX,
    )
}

/// Sparkline fetch policy.
pub struct SparklinePolicy;

impl SparklinePolicy {
    pub const RANGE: &'static str = "1d";
    pub const INTERVAL: &'static str = "5m";
    pub const TARGET_POINTS: usize = 32;
}

/// Default and minimum window geometry (logical pixels).
pub struct WindowPolicy;

impl WindowPolicy {
    pub const DEFAULT_WIDTH: f64 = 320.0;
    pub const DEFAULT_HEIGHT: f64 = 640.0;
    pub const MIN_WIDTH: f64 = 260.0;
    /// Absolute floor: header + padding + Add card (content-hug chrome).
    /// Runtime also sets min size from live panel height so rows cannot be clipped.
    pub const MIN_HEIGHT: f64 = 120.0;
}

/// Global hotkey defaults.
pub struct HotkeyPolicy;

impl HotkeyPolicy {
    pub const DEFAULT: &'static str = "Ctrl+Shift+Space";
}

/// Window opacity bounds.
pub struct OpacityPolicy;

impl OpacityPolicy {
    pub const MIN: f64 = 0.35;
    pub const MAX: f64 = 1.0;
    pub const DEFAULT: f64 = 0.92;
}

/// Clamp opacity into [`OpacityPolicy::MIN`]..=[`OpacityPolicy::MAX`].
pub fn clamp_opacity(value: f64) -> f64 {
    value.clamp(OpacityPolicy::MIN, OpacityPolicy::MAX)
}

/// Watchlist triptych column shares (CSS `fr` units): symbol · spark · metrics.
pub struct ColumnRatioPolicy;

impl ColumnRatioPolicy {
    pub const DEFAULT_SYMBOL: f64 = 1.15;
    pub const DEFAULT_SPARK: f64 = 1.25;
    pub const DEFAULT_METRICS: f64 = 2.0;
    pub const MIN: f64 = 0.45;
    pub const MAX: f64 = 8.0;
}

/// Clamp / sanitize column ratios (finite, within bounds). Invalid → defaults.
pub fn clamp_column_ratios(
    ratios: crate::domain::types::ColumnRatios,
) -> crate::domain::types::ColumnRatios {
    use crate::domain::types::ColumnRatios;
    let sanitize = |v: f64, default: f64| {
        if v.is_finite() {
            v.clamp(ColumnRatioPolicy::MIN, ColumnRatioPolicy::MAX)
        } else {
            default
        }
    };
    ColumnRatios {
        symbol: sanitize(ratios.symbol, ColumnRatioPolicy::DEFAULT_SYMBOL),
        spark: sanitize(ratios.spark, ColumnRatioPolicy::DEFAULT_SPARK),
        metrics: sanitize(ratios.metrics, ColumnRatioPolicy::DEFAULT_METRICS),
    }
}

/// Clamp window size to policy minimums (position unchanged).
pub fn clamp_geometry(
    geometry: &crate::domain::types::WindowGeometry,
) -> crate::domain::types::WindowGeometry {
    crate::domain::types::WindowGeometry {
        x: geometry.x,
        y: geometry.y,
        width: geometry.width.max(WindowPolicy::MIN_WIDTH),
        height: geometry.height.max(WindowPolicy::MIN_HEIGHT),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_opacity_bounds() {
        assert_eq!(clamp_opacity(0.0), OpacityPolicy::MIN);
        assert_eq!(clamp_opacity(0.1), OpacityPolicy::MIN);
        assert_eq!(clamp_opacity(OpacityPolicy::MIN), OpacityPolicy::MIN);
        assert_eq!(clamp_opacity(0.5), 0.5);
        assert_eq!(
            clamp_opacity(OpacityPolicy::DEFAULT),
            OpacityPolicy::DEFAULT
        );
        assert_eq!(clamp_opacity(OpacityPolicy::MAX), OpacityPolicy::MAX);
        assert_eq!(clamp_opacity(1.5), OpacityPolicy::MAX);
        assert_eq!(clamp_opacity(100.0), OpacityPolicy::MAX);
    }

    #[test]
    fn clamp_column_ratios_bounds_and_nan() {
        use crate::domain::types::ColumnRatios;
        let ok = clamp_column_ratios(ColumnRatios {
            symbol: 1.15,
            spark: 1.25,
            metrics: 2.0,
        });
        assert!((ok.symbol - 1.15).abs() < 1e-9);
        let low = clamp_column_ratios(ColumnRatios {
            symbol: 0.01,
            spark: 0.01,
            metrics: 0.01,
        });
        assert_eq!(low.symbol, ColumnRatioPolicy::MIN);
        assert_eq!(low.spark, ColumnRatioPolicy::MIN);
        assert_eq!(low.metrics, ColumnRatioPolicy::MIN);
        let high = clamp_column_ratios(ColumnRatios {
            symbol: 99.0,
            spark: 99.0,
            metrics: 99.0,
        });
        assert_eq!(high.symbol, ColumnRatioPolicy::MAX);
        let bad = clamp_column_ratios(ColumnRatios {
            symbol: f64::NAN,
            spark: f64::INFINITY,
            metrics: -1.0,
        });
        assert_eq!(bad.symbol, ColumnRatioPolicy::DEFAULT_SYMBOL);
        assert_eq!(bad.spark, ColumnRatioPolicy::DEFAULT_SPARK);
        assert_eq!(bad.metrics, ColumnRatioPolicy::MIN);
    }

    #[test]
    fn refresh_policy_durations() {
        assert_eq!(RefreshPolicy::TICK, Duration::from_millis(500));
        assert_eq!(
            RefreshPolicy::MIN_QUOTE_INTERVAL,
            Duration::from_millis(500)
        );
        assert_eq!(
            RefreshPolicy::SPARKLINE_MIN_INTERVAL,
            Duration::from_secs(300)
        );
    }

    #[test]
    fn clamp_quote_refresh_secs_bounds() {
        // 0 → default ms
        assert_eq!(
            clamp_quote_refresh_secs(0),
            RefreshPolicy::QUOTE_REFRESH_MS_DEFAULT
        );
        // Legacy whole seconds 1..=120 → ms
        assert_eq!(clamp_quote_refresh_secs(1), 1000);
        assert_eq!(clamp_quote_refresh_secs(3), 3000);
        // Explicit milliseconds
        assert_eq!(clamp_quote_refresh_secs(500), 500);
        assert_eq!(clamp_quote_refresh_secs(250), 250);
        assert_eq!(
            clamp_quote_refresh_secs(999_999),
            RefreshPolicy::QUOTE_REFRESH_MS_MAX
        );
    }

    #[test]
    fn clamp_geometry_enforces_min_size() {
        let g = clamp_geometry(&crate::domain::types::WindowGeometry {
            x: 10.0,
            y: 20.0,
            width: 1.0,
            height: 1.0,
        });
        assert_eq!(g.x, 10.0);
        assert_eq!(g.y, 20.0);
        assert_eq!(g.width, WindowPolicy::MIN_WIDTH);
        assert_eq!(g.height, WindowPolicy::MIN_HEIGHT);
    }
}
