//! Physical screen placement; no OS calls, persistence, or vendor policy.
use serde::{Deserialize, Serialize};

pub const DEPTH: f64 = 56.;
pub const CELL: f64 = 72.;
pub const CELL_HORIZONTAL: f64 = 96.;
pub const SHOULDER: f64 = DEPTH / 2.;
pub const INNER_RADIUS: f64 = DEPTH - SHOULDER;
// Content nestles into the curved ends instead of starting after the entire curve.
pub const END_PADDING: f64 = 44.;
pub const DETAIL_RADIUS: f64 = 16.;
pub const DETAIL_WIDTH: f64 = 260.;
pub const MAX_CELLS: usize = 6;
pub const DETAIL_HEIGHT: f64 = 560.;
pub const MIN_DETAIL_HEIGHT: f64 = 48.;
pub const GAP: f64 = 8.;
const CONTENT_REFERENCE_SHORT_SIDE: f64 = 1440.;
pub const MIN_CONTENT_SCALE: f64 = 0.92;

pub fn watchlist_slot_count(items: usize) -> usize {
    items.clamp(1, MAX_CELLS)
}

pub fn calculate_content_scale(short_side: f64) -> f64 {
    let raw = (short_side / CONTENT_REFERENCE_SHORT_SIDE).sqrt();
    (1.0 - (1.0 - raw.min(1.0)) * 0.5).clamp(MIN_CONTENT_SCALE, 1.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NotchEdge {
    Top,
    #[default]
    Right,
    Bottom,
    Left,
}
impl NotchEdge {
    pub fn vertical(self) -> bool {
        matches!(self, Self::Right | Self::Left)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotchPlacement {
    pub edge: NotchEdge,
    pub monitor_hint: Option<String>,
    pub offset: f64,
}
impl Default for NotchPlacement {
    fn default() -> Self {
        Self {
            edge: NotchEdge::Right,
            monitor_hint: None,
            offset: 0.5,
        }
    }
}
impl NotchPlacement {
    /// Old geometry is logical; resolve it against each monitor's physical DPI.
    pub fn restore_monitor(
        &self,
        window: &super::types::WindowGeometry,
        monitors: &[MonitorArea],
    ) -> Self {
        if self.monitor_hint.is_some() {
            return self.clone();
        }
        let mut placement = self.clone();
        placement.monitor_hint = monitors
            .iter()
            .find(|m| m.bounds.contains(window.x * m.scale, window.y * m.scale))
            .or(monitors.first())
            .map(|m| m.name.clone());
        placement
    }
    pub fn validate(&self) -> Result<(), String> {
        if !self.offset.is_finite() || !(0. ..=1.).contains(&self.offset) {
            return Err("Notch offset must be between 0 and 1".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl Rect {
    pub fn contains(self, x: f64, y: f64) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }
    fn union(self, r: Self) -> Self {
        let x = self.x.min(r.x);
        let y = self.y.min(r.y);
        Self {
            x,
            y,
            width: (self.x + self.width).max(r.x + r.width) - x,
            height: (self.y + self.height).max(r.y + r.height) - y,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MonitorArea {
    pub name: String,
    pub bounds: Rect,
    pub work: Rect,
    pub scale: f64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnchorX {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnchorY {
    Top,
    Bottom,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NotchLayout {
    pub edge: NotchEdge,
    pub anchor_x: AnchorX,
    pub anchor_y: AnchorY,
    pub notch: Rect,
    pub detail: Option<Rect>,
    pub window: Rect,
    pub scale: f64,
    pub monitor: String,
    pub metrics: NotchMetrics,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NotchMetrics {
    pub slot_count: usize,
    pub content_scale: f64,
    pub depth: f64,
    pub cell: f64,
    pub shoulder: f64,
    pub inner_radius: f64,
    pub inset: f64,
    pub detail_radius: f64,
    pub drag_threshold: f64,
    pub rest_depth: f64,
    pub rest_length: f64,
}
impl NotchLayout {
    pub fn hit(&self, x: f64, y: f64) -> bool {
        if let Some(d) = self.detail {
            if d.contains(x, y) {
                let r = (DETAIL_RADIUS * self.scale)
                    .min(d.width / 2.)
                    .min(d.height / 2.);
                let dx = x - x.clamp(d.x + r, d.x + d.width - r);
                let dy = y - y.clamp(d.y + r, d.y + d.height - r);
                if dx * dx + dy * dy <= r * r {
                    return true;
                }
            }
            // A narrow bridge keeps hover alive while crossing to the detail.
            let n = self.notch;
            let gap = (GAP * self.scale).round();
            let bridge = if self.edge.vertical() {
                Rect {
                    x: n.x.min(d.x + d.width),
                    y: n.y.max(d.y),
                    width: gap,
                    height: (n.y + n.height).min(d.y + d.height) - n.y.max(d.y),
                }
            } else {
                Rect {
                    x: n.x.max(d.x),
                    y: n.y.min(d.y + d.height),
                    width: (n.x + n.width).min(d.x + d.width) - n.x.max(d.x),
                    height: gap,
                }
            };
            let bridge = match self.edge {
                NotchEdge::Left => Rect {
                    x: n.x + n.width,
                    ..bridge
                },
                NotchEdge::Top => Rect {
                    y: n.y + n.height,
                    ..bridge
                },
                _ => bridge,
            };
            if bridge.contains(x, y) {
                return true;
            }
        }
        if !self.notch.contains(x, y) {
            return false;
        }
        let n = self.notch;
        let s = self.scale;
        let depth = self.metrics.depth;
        let shoulder = self.metrics.shoulder;
        let inner = self.metrics.inner_radius;
        // The HWND rounds depth to physical pixels; mirror the SVG's cross-axis stretch.
        let across_scale = if self.edge.vertical() {
            n.width
        } else {
            n.height
        } / depth;
        let (across, along, length) = match self.edge {
            NotchEdge::Right => ((x - n.x) / across_scale, (y - n.y) / s, n.height / s),
            NotchEdge::Left => (
                (n.x + n.width - x) / across_scale,
                (y - n.y) / s,
                n.height / s,
            ),
            NotchEdge::Bottom => ((y - n.y) / across_scale, (x - n.x) / s, n.width / s),
            NotchEdge::Top => (
                (n.y + n.height - y) / across_scale,
                (x - n.x) / s,
                n.width / s,
            ),
        };
        let end = along.min(length - along);
        let boundary = if end < shoulder {
            depth - shoulder + (shoulder.powi(2) - end.powi(2)).max(0.).sqrt()
        } else if end < shoulder + inner {
            inner
                - (inner.powi(2) - (end - shoulder - inner).powi(2))
                    .max(0.)
                    .sqrt()
        } else {
            0.
        };
        across >= boundary
    }
}

pub fn calculate_layout(
    m: &MonitorArea,
    p: &NotchPlacement,
    count: usize,
    expanded: bool,
    detail_height: f64,
) -> Result<NotchLayout, String> {
    calculate_layout_target(m, p, count, expanded, detail_height, None)
}

pub fn calculate_layout_target(
    m: &MonitorArea,
    p: &NotchPlacement,
    count: usize,
    expanded: bool,
    detail_height: f64,
    target: Option<f64>,
) -> Result<NotchLayout, String> {
    p.validate()?;
    if !m.scale.is_finite() || m.scale <= 0. || count == 0 || count > MAX_CELLS {
        return Err("Invalid notch dimensions".into());
    }
    let b = m.bounds;
    let w = m.work;
    let s = m.scale;
    let content_scale = calculate_content_scale(b.width.min(b.height) / s);
    let available = |e| match e {
        NotchEdge::Right => w.x + w.width >= b.x + b.width,
        NotchEdge::Left => w.x <= b.x,
        NotchEdge::Top => w.y <= b.y,
        // User-selected bottom docking overlays the taskbar at the physical screen edge.
        NotchEdge::Bottom => true,
    };
    let edge = [
        p.edge,
        NotchEdge::Right,
        NotchEdge::Left,
        NotchEdge::Top,
        NotchEdge::Bottom,
    ]
    .into_iter()
    .find(|e| available(*e))
    .ok_or("No screen edge available")?;
    let cell = if edge.vertical() {
        CELL
    } else {
        CELL_HORIZONTAL
    } * content_scale;
    let inset = END_PADDING * content_scale;
    let span = if edge.vertical() { w.height } else { w.width };
    let capacity = (((span + 0.499999) / s - inset * 2.) / cell)
        .floor()
        .max(0.) as usize;
    let count = count.min(capacity);
    if count == 0 {
        return Err("Display is too small for the notch".into());
    }
    let len = ((count as f64 * cell + inset * 2.) * s).round();
    let depth = (DEPTH * content_scale * s).round();
    let gap = (GAP * s).round();
    let span = if edge.vertical() { w.height } else { w.width };
    if len > span || depth > w.width.min(w.height) {
        return Err("Display is too small for the notch".into());
    }
    let offset = ((span - len) * p.offset).round();
    let notch = match edge {
        NotchEdge::Right => Rect {
            x: b.x + b.width - depth,
            y: w.y + offset,
            width: depth,
            height: len,
        },
        NotchEdge::Left => Rect {
            x: b.x,
            y: w.y + offset,
            width: depth,
            height: len,
        },
        NotchEdge::Top => Rect {
            x: w.x + offset,
            y: b.y,
            width: len,
            height: depth,
        },
        NotchEdge::Bottom => Rect {
            x: w.x + offset,
            y: b.y + b.height - depth,
            width: len,
            height: depth,
        },
    };
    let anchor_x = match edge {
        NotchEdge::Left => AnchorX::Left,
        NotchEdge::Right => AnchorX::Right,
        NotchEdge::Top | NotchEdge::Bottom => {
            if p.offset <= 0.5 {
                AnchorX::Left
            } else {
                AnchorX::Right
            }
        }
    };
    let anchor_y = match edge {
        NotchEdge::Top | NotchEdge::Left | NotchEdge::Right => AnchorY::Top,
        NotchEdge::Bottom => AnchorY::Bottom,
    };
    let detail = if expanded {
        let dw = ((DETAIL_WIDTH * content_scale) * s)
            .ceil()
            .min(w.width - if edge.vertical() { depth + gap } else { 0. });
        let dh = (detail_height.clamp(
            MIN_DETAIL_HEIGHT * content_scale,
            DETAIL_HEIGHT * content_scale,
        ) * s)
            .ceil()
            .min(w.height - if edge.vertical() { 0. } else { depth + gap });
        if dw <= 0. || dh <= 0. {
            return Err("Display is too small for usage details".into());
        }
        let (dx, dy) = if edge.vertical() {
            let x = match edge {
                NotchEdge::Right => notch.x - gap - dw,
                NotchEdge::Left => notch.x + depth + gap,
                _ => unreachable!(),
            };
            let y = if let Some(t) = target {
                let center = notch.y + t * s;
                let ideal = center - dh / 2.0;
                if dh <= notch.height {
                    ideal.clamp(notch.y, notch.y + notch.height - dh)
                } else {
                    ideal.clamp(w.y, (w.y + w.height - dh).max(w.y))
                }
            } else {
                let center = notch.y + notch.height / 2.0;
                let ideal = center - dh / 2.0;
                ideal.clamp(w.y, (w.y + w.height - dh).max(w.y))
            };
            (x, y)
        } else {
            let y = match edge {
                NotchEdge::Top => notch.y + depth + gap,
                NotchEdge::Bottom => notch.y - gap - dh,
                _ => unreachable!(),
            };
            let x = if let Some(t) = target {
                let center = notch.x + t * s;
                let ideal = center - dw / 2.0;
                if dw <= notch.width {
                    ideal.clamp(notch.x, notch.x + notch.width - dw)
                } else {
                    ideal.clamp(w.x, (w.x + w.width - dw).max(w.x))
                }
            } else {
                let center = notch.x + notch.width / 2.0;
                let ideal = center - dw / 2.0;
                ideal.clamp(w.x, (w.x + w.width - dw).max(w.x))
            };
            (x, y)
        };
        let d = Rect {
            x: dx.round(),
            y: dy.round(),
            width: dw,
            height: dh,
        };
        Some(d)
    } else {
        None
    };
    // A stable transparent canvas prevents HWND moves racing the WebView's local
    // notch offset when a taller provider card extends above the rail.
    let window = surface_canvas(notch, w, edge, s, content_scale);
    Ok(NotchLayout {
        edge,
        anchor_x,
        anchor_y,
        notch,
        detail,
        window,
        scale: s,
        monitor: m.name.clone(),
        metrics: NotchMetrics {
            slot_count: count,
            content_scale,
            depth: DEPTH * content_scale,
            cell,
            shoulder: SHOULDER * content_scale,
            inner_radius: INNER_RADIUS * content_scale,
            inset,
            detail_radius: DETAIL_RADIUS * content_scale,
            drag_threshold: DRAG_THRESHOLD * content_scale,
            rest_depth: super::reveal::REST_DEPTH * content_scale,
            rest_length: super::reveal::REST_LENGTH * content_scale,
        },
    })
}

fn surface_canvas(
    notch: Rect,
    work: Rect,
    edge: NotchEdge,
    scale: f64,
    content_scale: f64,
) -> Rect {
    let gap = (GAP * scale).round();
    let width = ((DETAIL_WIDTH * content_scale) * scale).ceil().min(
        work.width
            - if edge.vertical() {
                notch.width + gap
            } else {
                0.
            },
    );
    let height = ((DETAIL_HEIGHT * content_scale) * scale).ceil().min(
        work.height
            - if edge.vertical() {
                0.
            } else {
                notch.height + gap
            },
    );
    if width <= 0. || height <= 0. {
        return notch;
    }
    let reserve = if edge.vertical() {
        let center = notch.y + notch.height / 2.;
        let top = (center - height)
            .floor()
            .clamp(work.y, work.y + work.height - height);
        let bottom = (center + height).ceil().min(work.y + work.height);
        Rect {
            x: if edge == NotchEdge::Left {
                notch.x + notch.width + gap
            } else {
                notch.x - gap - width
            },
            y: top,
            width,
            height: bottom - top,
        }
    } else {
        let center = notch.x + notch.width / 2.;
        let left = (center - width)
            .floor()
            .clamp(work.x, work.x + work.width - width);
        let right = (center + width).ceil().min(work.x + work.width);
        Rect {
            x: left,
            y: if edge == NotchEdge::Top {
                notch.y + notch.height + gap
            } else {
                notch.y - gap - height
            },
            width: right - left,
            height,
        }
    };
    notch.union(reserve)
}

mod drag;
pub use drag::NotchDrag;
use drag::DRAG_THRESHOLD;

const MAX_CAPTURE_PIXELS: u64 = 1_048_576;
pub fn valid_region(
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    canvas_width: u32,
    canvas_height: u32,
) -> Result<bool, String> {
    let (Some(right), Some(bottom)) = (x.checked_add(width), y.checked_add(height)) else {
        return Err("Liquid capture region overflow".into());
    };
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_CAPTURE_PIXELS {
        return Err("Invalid Liquid capture dimensions".into());
    }
    Ok(right <= canvas_width && bottom <= canvas_height)
}

#[cfg(test)]
mod tests {
    use super::valid_region;
    #[test]
    fn detail_uses_tokenusage_width_at_native_dpi() {
        use super::*;
        for scale in [1., 1.25, 1.5, 2.] {
            let bounds = Rect {
                x: 0.,
                y: 0.,
                width: 2560. * scale,
                height: 1440. * scale,
            };
            let monitor = MonitorArea {
                name: "reference".into(),
                bounds,
                work: bounds,
                scale,
            };
            for edge in [
                NotchEdge::Right,
                NotchEdge::Left,
                NotchEdge::Top,
                NotchEdge::Bottom,
            ] {
                let placement = NotchPlacement {
                    edge,
                    ..Default::default()
                };
                let layout = calculate_layout(&monitor, &placement, 4, true, 140.).unwrap();
                assert_eq!(layout.detail.unwrap().width, (260_f64 * scale).ceil());
            }
        }
    }
    #[test]
    fn bounds_reject_zero_overflow_and_unbounded_pixel_allocations() {
        assert_eq!(valid_region(268, 372, 56, 376, 324, 1120), Ok(true));
        assert_eq!(valid_region(300, 0, 56, 1, 324, 1120), Ok(false));
        for (x, y, w, h) in [(0, 0, 0, 1), (u32::MAX, 0, 1, 1), (0, 0, 2048, 2048)] {
            assert!(valid_region(x, y, w, h, 324, 1120).is_err());
        }
    }
}
