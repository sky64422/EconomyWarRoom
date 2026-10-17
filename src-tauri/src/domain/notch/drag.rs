use super::{calculate_layout, MonitorArea, NotchEdge, NotchLayout, NotchPlacement};

pub(super) const DRAG_THRESHOLD: f64 = 5.;
const EDGE_ENTRY: f64 = 64.;
const EDGE_HYSTERESIS: f64 = 16.;
const MONITOR_ENTRY: f64 = 24.;
const EDGE_ORDER: [NotchEdge; 4] = [
    NotchEdge::Right,
    NotchEdge::Left,
    NotchEdge::Top,
    NotchEdge::Bottom,
];

/// Pure drag selection in physical desktop coordinates. Persistence belongs to the controller.
pub struct NotchDrag {
    pub placement: NotchPlacement,
    pub active: bool,
    start: (f64, f64),
    previous: (f64, f64),
    start_scale: f64,
    grab_ratio: f64,
    count: usize,
}
impl NotchDrag {
    pub fn new(layout: &NotchLayout, x: f64, y: f64, count: usize) -> Self {
        let n = layout.notch;
        let (along, origin, len) = if layout.edge.vertical() {
            (y, n.y, n.height)
        } else {
            (x, n.x, n.width)
        };
        Self {
            placement: NotchPlacement {
                edge: layout.edge,
                monitor_hint: Some(layout.monitor.clone()),
                offset: 0.5,
            },
            active: false,
            start: (x, y),
            previous: (x, y),
            start_scale: layout.scale * layout.metrics.content_scale,
            grab_ratio: ((along - origin) / len).clamp(0., 1.),
            count,
        }
    }
    pub fn update(&mut self, monitors: &[MonitorArea], x: f64, y: f64) -> Result<(), String> {
        if !x.is_finite() || !y.is_finite() {
            return Err("Invalid drag coordinates".into());
        }
        let previous = std::mem::replace(&mut self.previous, (x, y));
        if !self.active {
            if (x - self.start.0).hypot(y - self.start.1) < DRAG_THRESHOLD * self.start_scale {
                return Ok(());
            }
            self.active = true;
        }
        let current = monitors
            .iter()
            .find(|m| Some(&m.name) == self.placement.monitor_hint.as_ref())
            .ok_or("Drag display disappeared")?;
        let mut target = current;
        let mut edge = self.placement.edge;
        if let Some(other) = monitors.iter().find(|m| {
            m.name != current.name && m.bounds.contains(x, y) && entered_monitor(current, m, x, y)
        }) {
            if let Some(candidate) = self
                .closest_edge(other, x, y)
                .or_else(|| self.crossed_entry(current, other, previous, (x, y)))
            {
                target = other;
                edge = candidate;
            }
        } else if current.bounds.contains(x, y) {
            if let Some(candidate) = self.closest_edge(current, x, y) {
                if candidate != edge
                    && edge_distance(current, candidate, x, y) + EDGE_HYSTERESIS
                        <= edge_distance(current, edge, x, y)
                {
                    edge = candidate;
                }
            }
        }
        let mut next = NotchPlacement {
            edge,
            monitor_hint: Some(target.name.clone()),
            offset: 0.,
        };
        let l = calculate_layout(target, &next, self.count, false, 100.)?;
        let (pointer, origin, span, len) = if edge.vertical() {
            (y, target.work.y, target.work.height, l.notch.height)
        } else {
            (x, target.work.x, target.work.width, l.notch.width)
        };
        let travel = span - len;
        next.offset = if travel > 0. {
            ((pointer - self.grab_ratio * len - origin) / travel).clamp(0., 1.)
        } else {
            0.
        };
        self.placement = next;
        Ok(())
    }
    // A native sample may jump over the entire 24..64 DIP docking band.
    // Test the swept segment at entry instead of depending on sampling speed.
    fn crossed_entry(
        &self,
        from: &MonitorArea,
        to: &MonitorArea,
        previous: (f64, f64),
        next: (f64, f64),
    ) -> Option<NotchEdge> {
        EDGE_ORDER
            .into_iter()
            .filter_map(|edge| {
                let before = edge_distance(to, edge, previous.0, previous.1);
                let after = edge_distance(to, edge, next.0, next.1);
                if before >= MONITOR_ENTRY || after < MONITOR_ENTRY {
                    return None;
                }
                let t = (MONITOR_ENTRY - before) / (after - before);
                let x = previous.0 + (next.0 - previous.0) * t;
                let y = previous.1 + (next.1 - previous.1) * t;
                // The destination and hysteresis still apply, including diagonal seams.
                if !to.bounds.contains(x, y) || !entered_monitor(from, to, x, y) {
                    return None;
                }
                self.closest_edge(to, x, y).map(|candidate| (t, candidate))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, edge)| edge)
    }
    fn closest_edge(&self, m: &MonitorArea, x: f64, y: f64) -> Option<NotchEdge> {
        EDGE_ORDER
            .into_iter()
            .filter(|e| {
                let d = edge_distance(m, *e, x, y);
                let p = NotchPlacement {
                    edge: *e,
                    monitor_hint: Some(m.name.clone()),
                    offset: 0.,
                };
                (0.0..=EDGE_ENTRY).contains(&d)
                    && calculate_layout(m, &p, self.count, false, 100.).is_ok_and(|l| l.edge == *e)
            })
            .min_by(|a, b| edge_distance(m, *a, x, y).total_cmp(&edge_distance(m, *b, x, y)))
    }
}
fn edge_distance(m: &MonitorArea, edge: NotchEdge, x: f64, y: f64) -> f64 {
    let b = m.bounds;
    (match edge {
        NotchEdge::Left => x - b.x,
        NotchEdge::Right => b.x + b.width - x,
        NotchEdge::Top => y - b.y,
        NotchEdge::Bottom => b.y + b.height - y,
    }) / m.scale
}

fn entered_monitor(from: &MonitorArea, to: &MonitorArea, x: f64, y: f64) -> bool {
    const EPSILON: f64 = 1e-9;
    let a = from.bounds;
    let b = to.bounds;
    let inset = MONITOR_ENTRY * to.scale;
    // Only the crossed seam needs hysteresis; the pointer may hug an outer edge.
    if b.x + b.width <= a.x && b.x + b.width - x + EPSILON < inset {
        return false;
    }
    if b.x >= a.x + a.width && x - b.x + EPSILON < inset {
        return false;
    }
    if b.y + b.height <= a.y && b.y + b.height - y + EPSILON < inset {
        return false;
    }
    if b.y >= a.y + a.height && y - b.y + EPSILON < inset {
        return false;
    }
    true
}
