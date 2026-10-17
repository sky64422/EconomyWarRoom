//! Rail auto-hide timing and the physical edge target; no window movement.
use super::notch::{NotchEdge, NotchLayout, Rect};
use std::time::Duration;

pub const REST_DEPTH: f64 = 10.;
pub const REST_LENGTH: f64 = 80.;
pub const HIDE_GRACE: Duration = Duration::from_millis(800);

pub struct NotchReveal {
    shown: bool,
    outside_since: Option<Duration>,
}
impl Default for NotchReveal {
    fn default() -> Self {
        Self {
            shown: true,
            outside_since: None,
        }
    }
}
impl NotchReveal {
    pub fn shown(&self) -> bool {
        self.shown
    }

    pub fn open(&mut self) -> Option<bool> {
        self.outside_since = None;
        if self.shown {
            return None;
        }
        self.shown = true;
        Some(true)
    }

    pub fn update(&mut self, now: Duration, inside: bool, held: bool) -> Option<bool> {
        if inside || held {
            return self.open();
        }
        if !self.shown {
            return None;
        }
        let outside_since = *self.outside_since.get_or_insert(now);
        if now.saturating_sub(outside_since) < HIDE_GRACE {
            return None;
        }
        self.shown = false;
        self.outside_since = None;
        Some(false)
    }
}

pub fn folded_rect(layout: &NotchLayout) -> Rect {
    let n = layout.notch;
    let depth = layout.metrics.rest_depth * layout.scale;
    let length = layout.metrics.rest_length * layout.scale;
    if layout.edge.vertical() {
        Rect {
            x: if layout.edge == NotchEdge::Right {
                n.x + n.width - depth
            } else {
                n.x
            },
            y: n.y + (n.height - length) / 2.,
            width: depth,
            height: length,
        }
    } else {
        Rect {
            x: n.x + (n.width - length) / 2.,
            y: if layout.edge == NotchEdge::Bottom {
                n.y + n.height - depth
            } else {
                n.y
            },
            width: length,
            height: depth,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::notch::{calculate_layout, MonitorArea, NotchPlacement};

    #[test]
    fn starts_shown_then_folds_only_after_full_outside_grace() {
        let mut state = NotchReveal::default();
        assert!(state.shown());
        assert_eq!(state.update(Duration::ZERO, false, false), None);
        assert_eq!(
            state.update(HIDE_GRACE - Duration::from_millis(1), false, false),
            None
        );
        assert_eq!(state.update(HIDE_GRACE, false, false), Some(false));
        assert!(!state.shown());
        assert_eq!(state.update(HIDE_GRACE * 2, false, false), None);
    }

    #[test]
    fn returning_pointer_cancels_close_and_folded_target_reveals_once() {
        let mut state = NotchReveal::default();
        state.update(Duration::ZERO, false, false);
        state.update(HIDE_GRACE / 2, true, false);
        assert_eq!(state.update(HIDE_GRACE, false, false), None);
        assert_eq!(state.update(HIDE_GRACE * 2, false, false), Some(false));
        assert_eq!(state.update(HIDE_GRACE * 3, true, false), Some(true));
        assert_eq!(state.update(HIDE_GRACE * 4, true, false), None);
    }

    #[test]
    fn detail_drag_or_keyboard_hold_opens_and_restarts_grace_after_release() {
        let mut state = NotchReveal::default();
        state.update(Duration::ZERO, false, false);
        state.update(HIDE_GRACE, false, false);
        assert_eq!(state.update(HIDE_GRACE * 2, false, true), Some(true));
        assert_eq!(state.update(HIDE_GRACE * 20, false, true), None);
        assert_eq!(state.update(HIDE_GRACE * 21, false, false), None);
        assert_eq!(state.update(HIDE_GRACE * 22, false, false), Some(false));
    }

    #[test]
    fn folded_targets_follow_each_physical_edge_and_scale_without_moving_layout() {
        for scale in [1., 1.25, 1.5, 2.] {
            let bounds = Rect {
                x: -2560.,
                y: -400.,
                width: 2560.,
                height: 1600.,
            };
            let monitor = MonitorArea {
                name: "left".into(),
                bounds,
                work: bounds,
                scale,
            };
            for edge in [
                NotchEdge::Left,
                NotchEdge::Right,
                NotchEdge::Top,
                NotchEdge::Bottom,
            ] {
                let placement = NotchPlacement {
                    edge,
                    offset: 0.25,
                    monitor_hint: None,
                };
                let layout = calculate_layout(&monitor, &placement, 3, false, 0.).unwrap();
                let before = layout.clone();
                let rest = folded_rect(&layout);
                let n = layout.notch;
                if edge.vertical() {
                    assert_eq!(
                        rest.width,
                        REST_DEPTH * layout.metrics.content_scale * scale
                    );
                    assert_eq!(
                        rest.height,
                        REST_LENGTH * layout.metrics.content_scale * scale
                    );
                    assert!((rest.y + rest.height / 2. - n.y - n.height / 2.).abs() < 1e-9);
                } else {
                    assert_eq!(
                        rest.width,
                        REST_LENGTH * layout.metrics.content_scale * scale
                    );
                    assert_eq!(
                        rest.height,
                        REST_DEPTH * layout.metrics.content_scale * scale
                    );
                    assert!((rest.x + rest.width / 2. - n.x - n.width / 2.).abs() < 1e-9);
                }
                match edge {
                    NotchEdge::Left => assert_eq!(rest.x, bounds.x),
                    NotchEdge::Right => {
                        assert!((rest.x + rest.width - bounds.x - bounds.width).abs() < 1e-9)
                    }
                    NotchEdge::Top => assert_eq!(rest.y, bounds.y),
                    NotchEdge::Bottom => {
                        assert!((rest.y + rest.height - bounds.y - bounds.height).abs() < 1e-9)
                    }
                }
                assert!(rest.contains(rest.x + rest.width / 2., rest.y + rest.height / 2.));
                assert!(!rest.contains(n.x + 1., n.y + 1.));
                assert_eq!(layout, before);
            }
        }
    }
}
