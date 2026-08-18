use super::types::SparklinePoint;

/// Keep first/last; sample evenly to at most `target` points.
pub fn downsample(points: &[SparklinePoint], target: usize) -> Vec<SparklinePoint> {
    if target == 0 || points.is_empty() {
        return vec![];
    }
    if points.len() <= target {
        return points.to_vec();
    }
    if target == 1 {
        return vec![points[points.len() - 1].clone()];
    }
    let mut out = Vec::with_capacity(target);
    let last = points.len() - 1;
    for i in 0..target {
        let idx = if i == target - 1 {
            last
        } else {
            (i * last) / (target - 1)
        };
        out.push(points[idx].clone());
    }
    out
}

/// Horizontal fraction of a regular-session window `[start, end)`.
/// Early bars sit near 0 so the sparkline fills left-to-right through the day.
pub fn session_x(t: i64, start: i64, end: i64) -> f64 {
    if end <= start {
        return 1.0;
    }
    let span = (end - start) as f64;
    ((t - start) as f64 / span).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(n: usize) -> Vec<SparklinePoint> {
        (0..n)
            .map(|i| SparklinePoint {
                t: i as i64,
                close: i as f64,
            })
            .collect()
    }

    #[test]
    fn short_list_unchanged() {
        let p = pts(3);
        assert_eq!(downsample(&p, 10).len(), 3);
    }

    #[test]
    fn respects_target_and_endpoints() {
        let p = pts(100);
        let d = downsample(&p, 10);
        assert_eq!(d.len(), 10);
        assert_eq!(d[0].t, 0);
        assert_eq!(d[9].t, 99);
    }

    #[test]
    fn empty_or_zero_target_yields_empty() {
        assert!(downsample(&pts(5), 0).is_empty());
        assert!(downsample(&[], 10).is_empty());
    }

    #[test]
    fn target_one_keeps_last_only() {
        let d = downsample(&pts(5), 1);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].t, 4);
    }

    #[test]
    fn session_x_is_zero_at_open() {
        assert!((session_x(2000, 2000, 3000) - 0.0).abs() < 1e-12);
    }

    #[test]
    fn session_x_is_half_at_midpoint() {
        assert!((session_x(2500, 2000, 3000) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn session_x_is_not_one_before_close() {
        let x = session_x(2900, 2000, 3000);
        assert!(x > 0.89 && x < 1.0);
    }

    #[test]
    fn session_x_clamps_outside_window() {
        assert_eq!(session_x(1000, 2000, 3000), 0.0);
        assert_eq!(session_x(4000, 2000, 3000), 1.0);
    }

    #[test]
    fn session_x_invalid_window_is_one() {
        assert_eq!(session_x(10, 5, 5), 1.0);
        assert_eq!(session_x(10, 8, 3), 1.0);
    }
}
