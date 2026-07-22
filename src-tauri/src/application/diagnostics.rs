//! In-process diagnostics ring buffer for copy-to-clipboard reports.

use std::collections::VecDeque;

/// Default capacity for recent events kept in memory.
pub const DEFAULT_EVENT_CAPACITY: usize = 100;

/// Severity for a diagnostics event line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagLevel {
    Info,
    Warn,
    Error,
}

impl DiagLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            DiagLevel::Info => "info",
            DiagLevel::Warn => "warn",
            DiagLevel::Error => "error",
        }
    }
}

/// One diagnostics log line (timestamp is ISO-8601 UTC when recorded).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagEvent {
    pub timestamp: String,
    pub level: DiagLevel,
    pub message: String,
}

/// Fixed-capacity FIFO of recent diagnostics events.
#[derive(Debug, Clone)]
pub struct EventRing {
    capacity: usize,
    events: VecDeque<DiagEvent>,
}

impl EventRing {
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            capacity,
            events: VecDeque::with_capacity(capacity),
        }
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Append an event; drops oldest when over capacity.
    pub fn push(&mut self, level: DiagLevel, message: impl Into<String>) {
        let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        self.push_event(DiagEvent {
            timestamp,
            level,
            message: message.into(),
        });
    }

    /// Test/helper path with explicit event.
    pub fn push_event(&mut self, event: DiagEvent) {
        while self.events.len() >= self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    /// Oldest → newest lines: `timestamp level message`.
    pub fn lines(&self) -> Vec<String> {
        self.events
            .iter()
            .map(|e| format!("{} {} {}", e.timestamp, e.level.as_str(), e.message))
            .collect()
    }

    /// Last `n` lines (oldest of that window first). `n == 0` → empty.
    pub fn last_lines(&self, n: usize) -> Vec<String> {
        if n == 0 {
            return vec![];
        }
        let lines = self.lines();
        let start = lines.len().saturating_sub(n);
        lines[start..].to_vec()
    }
}

impl Default for EventRing {
    fn default() -> Self {
        Self::new(DEFAULT_EVENT_CAPACITY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_ring_has_no_lines() {
        let ring = EventRing::new(10);
        assert!(ring.is_empty());
        assert!(ring.lines().is_empty());
        assert!(ring.last_lines(50).is_empty());
    }

    #[test]
    fn push_order_oldest_first() {
        let mut ring = EventRing::new(10);
        ring.push_event(DiagEvent {
            timestamp: "t1".into(),
            level: DiagLevel::Info,
            message: "a".into(),
        });
        ring.push_event(DiagEvent {
            timestamp: "t2".into(),
            level: DiagLevel::Warn,
            message: "b".into(),
        });
        let lines = ring.lines();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("info a"));
        assert!(lines[1].contains("warn b"));
    }

    #[test]
    fn capacity_drops_oldest_fifo() {
        let mut ring = EventRing::new(2);
        ring.push_event(DiagEvent {
            timestamp: "t1".into(),
            level: DiagLevel::Info,
            message: "one".into(),
        });
        ring.push_event(DiagEvent {
            timestamp: "t2".into(),
            level: DiagLevel::Info,
            message: "two".into(),
        });
        ring.push_event(DiagEvent {
            timestamp: "t3".into(),
            level: DiagLevel::Error,
            message: "three".into(),
        });
        assert_eq!(ring.len(), 2);
        let lines = ring.lines();
        assert!(lines[0].contains("two"));
        assert!(lines[1].contains("three"));
        assert!(!lines.iter().any(|l| l.contains("one")));
    }

    #[test]
    fn last_lines_caps_window() {
        let mut ring = EventRing::new(10);
        for i in 0..5 {
            ring.push_event(DiagEvent {
                timestamp: format!("t{i}"),
                level: DiagLevel::Info,
                message: format!("m{i}"),
            });
        }
        let last = ring.last_lines(2);
        assert_eq!(last.len(), 2);
        assert!(last[0].contains("m3"));
        assert!(last[1].contains("m4"));
    }

    #[test]
    fn zero_capacity_constructor_becomes_one() {
        let mut ring = EventRing::new(0);
        ring.push(DiagLevel::Info, "only");
        assert_eq!(ring.len(), 1);
    }
}
