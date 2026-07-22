/// Quote / refresh scheduler policy.
pub struct RefreshPolicy;

impl RefreshPolicy {
    pub const TICK: u64 = 1;
    pub const BATCH_SIZE: usize = 4;
    pub const MIN_QUOTE_INTERVAL: u64 = 10;
    pub const MAX_CONCURRENT: usize = 3;
    pub const SPARKLINE_MIN_INTERVAL: u64 = 300;
    pub const BACKOFF_INITIAL: u64 = 5;
    pub const BACKOFF_MAX: u64 = 120;
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
    pub const MIN_HEIGHT: f64 = 360.0;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_opacity_bounds() {
        assert_eq!(clamp_opacity(0.0), OpacityPolicy::MIN);
        assert_eq!(clamp_opacity(0.1), OpacityPolicy::MIN);
        assert_eq!(clamp_opacity(OpacityPolicy::MIN), OpacityPolicy::MIN);
        assert_eq!(clamp_opacity(0.5), 0.5);
        assert_eq!(clamp_opacity(OpacityPolicy::DEFAULT), OpacityPolicy::DEFAULT);
        assert_eq!(clamp_opacity(OpacityPolicy::MAX), OpacityPolicy::MAX);
        assert_eq!(clamp_opacity(1.5), OpacityPolicy::MAX);
        assert_eq!(clamp_opacity(100.0), OpacityPolicy::MAX);
    }
}
