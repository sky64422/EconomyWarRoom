# EconomyWarRoom MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a lightweight Windows (and Linux-dev) Tauri floating watchlist widget: glass UI, US equity + crypto quotes/sparklines, DnD reorder, hotkey + in-UI hide, opacity/theme, rate-limited Yahoo-backed scheduler.

**Architecture:** Thin Tauri shell — web UI for glass list/interaction; Rust owns window/hotkey/autostart, JSON persistence, `RateLimitedQueue` + `QuoteScheduler`, and `MarketDataProvider` HTTP (Yahoo-first). Domain types and policy constants live in small focused modules; no SQLite, no portfolio features.

**Tech Stack:** Tauri 2, Rust (serde, reqwest, tokio, uuid), vanilla TypeScript + CSS (no heavy SPA framework for MVP), global-hotkey / tauri-plugin-global-shortcut, tauri-plugin-autostart, tauri-plugin-store or hand-rolled JSON file.

**Spec:** `docs/superpowers/specs/2026-07-22-economy-war-room-design.md`

---

## File map (create during tasks)

```
package.json
vite.config.ts
tsconfig.json
index.html
src/
  main.ts                 # boot UI, invoke commands, listen events
  styles/
    tokens.css            # theme tokens, glass
    app.css               # layout
  ui/
    app.ts                # root render + state
    header.ts             # drag region, hide, settings toggle
    watchlist.ts          # rows, DnD, + button
    sparkline.ts          # SVG path from points
    settings-panel.ts     # theme, opacity, quit
    types.ts              # TS mirrors of Rust DTOs
src-tauri/
  Cargo.toml
  tauri.conf.json
  capabilities/default.json
  icons/                  # generated or placeholder
  src/
    main.rs
    lib.rs                # tauri builder, plugins, managed state
    domain/
      mod.rs
      types.rs            # AssetKind, WatchlistItem, Quote, Sparkline, AppSettings
      constants.rs        # REFRESH, SPARKLINE, WINDOW, HOTKEY, OPACITY
      watchlist.rs        # append / remove / reorder pure functions
      sparkline_math.rs   # downsample
    ports/
      mod.rs
      market_data.rs      # MarketDataProvider trait + ProviderLimits
    application/
      mod.rs
      scheduler.rs        # QuoteScheduler
      queue.rs            # RateLimitedQueue
      cache.rs            # QuoteCache, SparklineCache
    infrastructure/
      mod.rs
      store.rs            # JSON load/save AppState file
      yahoo/
        mod.rs
        client.rs         # HTTP
        parse.rs          # parse chart JSON → Quote/Sparkline
      window_ctl.rs       # show/hide, opacity, always-on-top helpers
    commands.rs           # #[tauri::command] API for UI
  tests/                  # or #[cfg(test)] in modules
    fixtures/
      yahoo_chart_aapl.json
docs/TODO.md              # mark items done as you go
README.md                 # fill run instructions after scaffold
```

---

### Task 1: Scaffold Tauri 2 + Vite + TypeScript

**Files:**
- Create: project root scaffold via CLI (overwrites nothing critical if docs/ kept)
- Modify: ensure `docs/` and existing `README.md` preserved

- [ ] **Step 1: Install Rust toolchain if missing**

```bash
command -v rustc || curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustc --version
cargo --version
```

Expected: version prints (stable).

- [ ] **Step 2: Scaffold app in a temp dir then merge into repo root**

Repo already has `docs/` and `README.md`. Scaffold without destroying them:

```bash
cd /home/jyc/dev
npm create tauri-app@latest EconomyWarRoom-scaffold -- --template vanilla-ts --manager npm --yes
# If interactive flags differ, use:
# npm create tauri-app@latest
# name: EconomyWarRoom-scaffold, vanilla-ts, npm
```

Copy scaffold into `EconomyWarRoom` preserving docs:

```bash
cd /home/jyc/dev/EconomyWarRoom
# copy scaffold files except overwriting docs
rsync -a --exclude docs --exclude .git /home/jyc/dev/EconomyWarRoom-scaffold/ ./
# keep our README content: merge run section later; for now backup scaffold README
mv README.md README.scaffold.md 2>/dev/null || true
# restore product README if rsync overwrote — re-check
```

If `create-tauri-app` lands files only in scaffold dir, prefer:

```bash
cp -a /home/jyc/dev/EconomyWarRoom-scaffold/. /home/jyc/dev/EconomyWarRoom/
# then restore docs from git/staging if wiped
```

Ensure still present:

```bash
test -f docs/superpowers/specs/2026-07-22-economy-war-room-design.md && echo OK_SPEC
```

- [ ] **Step 3: Install JS deps and verify `tauri dev` builds**

```bash
cd /home/jyc/dev/EconomyWarRoom
npm install
npm run tauri build -- --help
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: `cargo check` succeeds (may download crates first).

- [ ] **Step 4: Commit scaffold**

```bash
git add -A
git status
git commit -m "chore: scaffold Tauri 2 vanilla-ts app"
```

---

### Task 2: Domain types and policy constants (Rust)

**Files:**
- Create: `src-tauri/src/domain/mod.rs`
- Create: `src-tauri/src/domain/types.rs`
- Create: `src-tauri/src/domain/constants.rs`
- Modify: `src-tauri/src/lib.rs` — `mod domain;`

- [ ] **Step 1: Add domain module files with types and constants**

`src-tauri/src/domain/types.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Equity,
    Crypto,
    Commodity,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WatchlistItem {
    pub id: String,
    pub symbol: String,
    pub display_name: Option<String>,
    pub asset_kind: AssetKind,
    pub sort_index: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quote {
    pub symbol: String,
    pub price: f64,
    pub currency: String,
    pub change_percent: Option<f64>,
    pub as_of: String, // RFC3339
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SparklinePoint {
    pub t: i64, // unix seconds
    pub close: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sparkline {
    pub symbol: String,
    pub points: Vec<SparklinePoint>,
    pub previous_close: Option<f64>,
    pub as_of: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    Light,
    Dark,
    System,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowGeometry {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub theme: ThemeMode,
    pub opacity: f64,
    pub window: WindowGeometry,
    pub hotkey: String,
    pub autostart: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersistedState {
    pub watchlist: Vec<WatchlistItem>,
    pub settings: AppSettings,
}
```

`src-tauri/src/domain/constants.rs`:

```rust
use std::time::Duration;

pub struct RefreshPolicy;
impl RefreshPolicy {
    pub const TICK: Duration = Duration::from_secs(1);
    pub const BATCH_SIZE: usize = 4;
    pub const MIN_QUOTE_INTERVAL: Duration = Duration::from_secs(10);
    pub const MAX_CONCURRENT: usize = 3;
    pub const SPARKLINE_MIN_INTERVAL: Duration = Duration::from_secs(300);
    pub const BACKOFF_INITIAL: Duration = Duration::from_secs(5);
    pub const BACKOFF_MAX: Duration = Duration::from_secs(120);
}

pub struct SparklinePolicy;
impl SparklinePolicy {
    pub const RANGE: &'static str = "1d";
    pub const INTERVAL: &'static str = "5m";
    pub const TARGET_POINTS: usize = 32;
}

pub struct WindowPolicy;
impl WindowPolicy {
    pub const DEFAULT_WIDTH: f64 = 320.0;
    pub const DEFAULT_HEIGHT: f64 = 640.0;
    pub const MIN_WIDTH: f64 = 260.0;
    pub const MIN_HEIGHT: f64 = 360.0;
}

pub struct HotkeyPolicy;
impl HotkeyPolicy {
    pub const DEFAULT: &'static str = "Ctrl+Shift+Space";
}

pub struct OpacityPolicy;
impl OpacityPolicy {
    pub const MIN: f64 = 0.35;
    pub const MAX: f64 = 1.0;
    pub const DEFAULT: f64 = 0.92;
}

pub fn clamp_opacity(value: f64) -> f64 {
    value.clamp(OpacityPolicy::MIN, OpacityPolicy::MAX)
}
```

`src-tauri/src/domain/mod.rs`:

```rust
pub mod constants;
pub mod types;
pub mod watchlist;
pub mod sparkline_math;
```

Wire in `lib.rs` with `mod domain;` (and leave other mods for later tasks).

- [ ] **Step 2: Unit test opacity clamp**

In `constants.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_opacity_bounds() {
        assert_eq!(clamp_opacity(0.0), OpacityPolicy::MIN);
        assert_eq!(clamp_opacity(2.0), OpacityPolicy::MAX);
        assert_eq!(clamp_opacity(0.8), 0.8);
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cd /home/jyc/dev/EconomyWarRoom/src-tauri && cargo test clamp_opacity_bounds -- --nocapture
```

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/domain
git commit -m "feat(domain): add types and policy constants"
```

---

### Task 3: Watchlist pure logic (TDD)

**Files:**
- Create: `src-tauri/src/domain/watchlist.rs`

- [ ] **Step 1: Write failing tests first**

`src-tauri/src/domain/watchlist.rs`:

```rust
use super::types::{AssetKind, WatchlistItem};
use uuid::Uuid;

pub fn normalize_symbol(raw: &str) -> String {
    raw.trim().to_uppercase()
}

pub fn next_sort_index(items: &[WatchlistItem]) -> u32 {
    items.iter().map(|i| i.sort_index).max().map(|m| m + 1).unwrap_or(0)
}

pub fn add_item(
    items: &mut Vec<WatchlistItem>,
    symbol: &str,
    asset_kind: AssetKind,
    display_name: Option<String>,
) -> Result<WatchlistItem, String> {
    let symbol = normalize_symbol(symbol);
    if symbol.is_empty() {
        return Err("symbol empty".into());
    }
    if items.iter().any(|i| i.symbol == symbol) {
        return Err(format!("duplicate symbol {symbol}"));
    }
    let item = WatchlistItem {
        id: Uuid::new_v4().to_string(),
        symbol,
        display_name,
        asset_kind,
        sort_index: next_sort_index(items),
    };
    items.push(item.clone());
    Ok(item)
}

pub fn remove_item(items: &mut Vec<WatchlistItem>, id: &str) -> bool {
    let before = items.len();
    items.retain(|i| i.id != id);
    if items.len() != before {
        reindex(items);
        true
    } else {
        false
    }
}

/// `ordered_ids` is the full list of ids in the new visual order.
pub fn reorder(items: &mut Vec<WatchlistItem>, ordered_ids: &[String]) -> Result<(), String> {
    if ordered_ids.len() != items.len() {
        return Err("ordered_ids length mismatch".into());
    }
    let mut map: std::collections::HashMap<String, WatchlistItem> =
        items.drain(..).map(|i| (i.id.clone(), i)).collect();
    let mut next = Vec::with_capacity(ordered_ids.len());
    for id in ordered_ids {
        let item = map.remove(id).ok_or_else(|| format!("unknown id {id}"))?;
        next.push(item);
    }
    if !map.is_empty() {
        return Err("ordered_ids missing some items".into());
    }
    *items = next;
    reindex(items);
    Ok(())
}

fn reindex(items: &mut [WatchlistItem]) {
    for (idx, item) in items.iter_mut().enumerate() {
        item.sort_index = idx as u32;
    }
}

pub fn sorted_clone(items: &[WatchlistItem]) -> Vec<WatchlistItem> {
    let mut v = items.to_vec();
    v.sort_by_key(|i| i.sort_index);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_appends_at_bottom() {
        let mut items = vec![];
        add_item(&mut items, "aapl", AssetKind::Equity, None).unwrap();
        add_item(&mut items, "btc-usd", AssetKind::Crypto, None).unwrap();
        assert_eq!(items[0].symbol, "AAPL");
        assert_eq!(items[0].sort_index, 0);
        assert_eq!(items[1].symbol, "BTC-USD");
        assert_eq!(items[1].sort_index, 1);
    }

    #[test]
    fn reject_duplicate() {
        let mut items = vec![];
        add_item(&mut items, "MSFT", AssetKind::Equity, None).unwrap();
        assert!(add_item(&mut items, "msft", AssetKind::Equity, None).is_err());
    }

    #[test]
    fn reorder_updates_sort_index() {
        let mut items = vec![];
        let a = add_item(&mut items, "A", AssetKind::Equity, None).unwrap();
        let b = add_item(&mut items, "B", AssetKind::Equity, None).unwrap();
        reorder(&mut items, &[b.id.clone(), a.id.clone()]).unwrap();
        assert_eq!(items[0].symbol, "B");
        assert_eq!(items[0].sort_index, 0);
        assert_eq!(items[1].symbol, "A");
        assert_eq!(items[1].sort_index, 1);
    }

    #[test]
    fn remove_reindexes() {
        let mut items = vec![];
        let a = add_item(&mut items, "A", AssetKind::Equity, None).unwrap();
        add_item(&mut items, "B", AssetKind::Equity, None).unwrap();
        assert!(remove_item(&mut items, &a.id));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].sort_index, 0);
        assert_eq!(items[0].symbol, "B");
    }
}
```

Add to `src-tauri/Cargo.toml` dependencies:

```toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4", "serde"] }
```

- [ ] **Step 2: Run tests**

```bash
cd /home/jyc/dev/EconomyWarRoom/src-tauri && cargo test domain::watchlist -- --nocapture
```

Expected: all PASS

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/domain/watchlist.rs src-tauri/Cargo.toml
git commit -m "feat(domain): watchlist append reorder remove"
```

---

### Task 4: Sparkline downsample (TDD)

**Files:**
- Create: `src-tauri/src/domain/sparkline_math.rs`

- [ ] **Step 1: Implement with tests**

```rust
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
}
```

- [ ] **Step 2: Run tests**

```bash
cd /home/jyc/dev/EconomyWarRoom/src-tauri && cargo test sparkline_math -- --nocapture
```

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/domain/sparkline_math.rs
git commit -m "feat(domain): sparkline downsample"
```

---

### Task 5: RateLimitedQueue (TDD)

**Files:**
- Create: `src-tauri/src/application/mod.rs`
- Create: `src-tauri/src/application/queue.rs`
- Modify: `src-tauri/src/lib.rs` — `mod application;`

- [ ] **Step 1: Implement coalesce + max concurrent queue**

`src-tauri/src/application/queue.rs`:

```rust
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify, oneshot};

type BoxFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

struct Job {
    key: String,
    priority: u8, // lower = higher priority
    work: BoxFuture,
    done: Option<oneshot::Sender<()>>,
}

/// Minimal rate-limited job queue: max concurrent workers, coalesce by key.
pub struct RateLimitedQueue {
    inner: Arc<Mutex<Inner>>,
    notify: Arc<Notify>,
}

struct Inner {
    max_concurrent: usize,
    running: usize,
    pending: VecDeque<Job>,
    pending_keys: HashMap<String, ()>,
    closed: bool,
}

impl RateLimitedQueue {
    pub fn new(max_concurrent: usize) -> Self {
        assert!(max_concurrent >= 1);
        let q = Self {
            inner: Arc::new(Mutex::new(Inner {
                max_concurrent,
                running: 0,
                pending: VecDeque::new(),
                pending_keys: HashMap::new(),
                closed: false,
            })),
            notify: Arc::new(Notify::new()),
        };
        for _ in 0..max_concurrent {
            q.spawn_worker();
        }
        q
    }

    fn spawn_worker(&self) {
        let inner = self.inner.clone();
        let notify = self.notify.clone();
        tokio::spawn(async move {
            loop {
                let job = {
                    let mut g = inner.lock().await;
                    if g.closed && g.pending.is_empty() {
                        break;
                    }
                    if g.running >= g.max_concurrent {
                        None
                    } else {
                        // pick highest priority (lowest number), FIFO within
                        let pos = g
                            .pending
                            .iter()
                            .enumerate()
                            .min_by_key(|(_, j)| j.priority)
                            .map(|(i, _)| i);
                        if let Some(i) = pos {
                            let job = g.pending.remove(i).unwrap();
                            g.pending_keys.remove(&job.key);
                            g.running += 1;
                            Some(job)
                        } else {
                            None
                        }
                    }
                };
                if let Some(job) = job {
                    (job.work).await;
                    if let Some(tx) = job.done {
                        let _ = tx.send(());
                    }
                    let mut g = inner.lock().await;
                    g.running -= 1;
                    notify.notify_waiters();
                } else {
                    notify.notified().await;
                }
            }
        });
    }

    /// Enqueue work. If `key` already pending, skip duplicate (coalesce).
    pub async fn enqueue<F, Fut>(&self, key: impl Into<String>, priority: u8, f: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let key = key.into();
        let (tx, rx) = oneshot::channel();
        {
            let mut g = self.inner.lock().await;
            if g.closed {
                return;
            }
            if g.pending_keys.contains_key(&key) {
                return;
            }
            g.pending_keys.insert(key.clone(), ());
            g.pending.push_back(Job {
                key,
                priority,
                work: Box::pin(f()),
                done: Some(tx),
            });
        }
        self.notify.notify_waiters();
        let _ = rx.await;
    }

    pub async fn pending_len(&self) -> usize {
        self.inner.lock().await.pending.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[tokio::test]
    async fn respects_max_concurrent() {
        let q = RateLimitedQueue::new(2);
        let current = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let mut handles = vec![];
        for i in 0..6 {
            let current = current.clone();
            let peak = peak.clone();
            let q = q.clone_for_test();
            handles.push(tokio::spawn(async move {
                q.enqueue(format!("k{i}"), 1, move || {
                    let current = current.clone();
                    let peak = peak.clone();
                    async move {
                        let c = current.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(c, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        current.fetch_sub(1, Ordering::SeqCst);
                    }
                })
                .await;
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        assert!(peak.load(Ordering::SeqCst) <= 2);
    }

    impl RateLimitedQueue {
        fn clone_for_test(&self) -> Self {
            Self {
                inner: self.inner.clone(),
                notify: self.notify.clone(),
            }
        }
    }

    #[tokio::test]
    async fn coalesces_same_key() {
        let q = RateLimitedQueue::new(1);
        let runs = Arc::new(AtomicUsize::new(0));
        let r1 = runs.clone();
        let r2 = runs.clone();
        let q1 = q.clone_for_test();
        let q2 = q.clone_for_test();
        let a = tokio::spawn(async move {
            q1.enqueue("same", 1, move || {
                let runs = r1;
                async move {
                    runs.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(30)).await;
                }
            })
            .await;
        });
        tokio::time::sleep(Duration::from_millis(5)).await;
        let b = tokio::spawn(async move {
            q2.enqueue("same", 1, move || {
                let runs = r2;
                async move {
                    runs.fetch_add(1, Ordering::SeqCst);
                }
            })
            .await;
        });
        a.await.unwrap();
        b.await.unwrap();
        assert_eq!(runs.load(Ordering::SeqCst), 1);
    }
}
```

`application/mod.rs`:

```rust
pub mod cache;
pub mod queue;
pub mod scheduler;
```

Add tokio to Cargo.toml:

```toml
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time", "sync"] }
```

Note: If `clone_for_test` feels awkward, implement `Clone` on `RateLimitedQueue` (Arc internals) for production and tests.

- [ ] **Step 2: Run queue tests**

```bash
cd /home/jyc/dev/EconomyWarRoom/src-tauri && cargo test application::queue -- --nocapture
```

Expected: PASS. If coalesce test is racy, increase first job sleep and keep second enqueue while first still pending (not running): enqueue two before worker picks — adjust test to lock timing by using max_concurrent=1 and enqueue both before notify, or test `pending_keys` via `pending_len` after double enqueue without awaiting completion.

Simpler coalesce test alternative (preferred if flaky):

```rust
#[tokio::test]
async fn coalesces_while_pending() {
    let q = RateLimitedQueue::new(1);
    // block worker with long first unique job
    let q_block = q.clone_for_test();
    tokio::spawn(async move {
        q_block
            .enqueue("block", 0, || async {
                tokio::time::sleep(Duration::from_millis(200)).await;
            })
            .await;
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    let runs = Arc::new(AtomicUsize::new(0));
    let r = runs.clone();
    let q1 = q.clone_for_test();
    let h1 = tokio::spawn(async move {
        q1.enqueue("sym", 1, move || {
            let runs = r;
            async move {
                runs.fetch_add(1, Ordering::SeqCst);
            }
        })
        .await;
    });
    tokio::time::sleep(Duration::from_millis(10)).await;
    let r2 = runs.clone();
    let q2 = q.clone_for_test();
    let h2 = tokio::spawn(async move {
        q2.enqueue("sym", 1, move || {
            let runs = r2;
            async move {
                runs.fetch_add(1, Ordering::SeqCst);
            }
        })
        .await;
    });
    h1.await.unwrap();
    h2.await.unwrap();
    assert_eq!(runs.load(Ordering::SeqCst), 1);
}
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/application/queue.rs src-tauri/src/application/mod.rs src-tauri/Cargo.toml
git commit -m "feat(app): rate-limited job queue with key coalesce"
```

---

### Task 6: Quote cache + scheduler core (TDD)

**Files:**
- Create: `src-tauri/src/application/cache.rs`
- Create: `src-tauri/src/application/scheduler.rs`
- Create: `src-tauri/src/ports/mod.rs`
- Create: `src-tauri/src/ports/market_data.rs`

- [ ] **Step 1: Provider trait**

`ports/market_data.rs`:

```rust
use crate::domain::types::{AssetKind, Quote, Sparkline};
use async_trait::async_trait;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ProviderLimits {
    pub max_concurrent: usize,
    pub min_interval: Duration,
    pub prefers_batch: bool,
}

#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn supports(&self, kind: AssetKind) -> bool;
    fn limits(&self) -> ProviderLimits;
    async fn fetch_quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, String>;
    async fn fetch_sparkline(
        &self,
        symbol: &str,
        range: &str,
        interval: &str,
    ) -> Result<Sparkline, String>;
}
```

Cargo.toml:

```toml
async-trait = "0.1"
```

- [ ] **Step 2: Cache**

```rust
use crate::domain::types::{Quote, Sparkline};
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct QuoteCache {
    map: HashMap<String, (Quote, Instant)>,
}

impl QuoteCache {
    pub fn get(&self, symbol: &str) -> Option<&Quote> {
        self.map.get(symbol).map(|(q, _)| q)
    }

    pub fn put(&mut self, quote: Quote) {
        let sym = quote.symbol.clone();
        self.map.insert(sym, (quote, Instant::now()));
    }

    pub fn age(&self, symbol: &str) -> Option<Duration> {
        self.map.get(symbol).map(|(_, t)| t.elapsed())
    }

    pub fn all(&self) -> Vec<Quote> {
        self.map.values().map(|(q, _)| q.clone()).collect()
    }
}

#[derive(Default)]
pub struct SparklineCache {
    map: HashMap<String, (Sparkline, Instant)>,
}

impl SparklineCache {
    pub fn get(&self, symbol: &str) -> Option<&Sparkline> {
        self.map.get(symbol).map(|(s, _)| s)
    }

    pub fn put(&mut self, spark: Sparkline) {
        let sym = spark.symbol.clone();
        self.map.insert(sym, (spark, Instant::now()));
    }

    pub fn age(&self, symbol: &str) -> Option<Duration> {
        self.map.get(symbol).map(|(_, t)| t.elapsed())
    }
}
```

- [ ] **Step 3: Scheduler — round-robin batch selection (pure + async loop)**

Implement `pick_batch` as pure function testable without network:

```rust
use crate::domain::constants::RefreshPolicy;
use crate::domain::types::WatchlistItem;
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub fn pick_batch(
    items: &[WatchlistItem],
    last_fetch: &HashMap<String, Instant>,
    now: Instant,
    min_interval: Duration,
    batch_size: usize,
    cursor: &mut usize,
    priority_symbol: Option<&str>,
) -> Vec<String> {
    if items.is_empty() || batch_size == 0 {
        return vec![];
    }
    let mut out = Vec::new();
    if let Some(sym) = priority_symbol {
        if items.iter().any(|i| i.symbol == sym) {
            let stale = last_fetch
                .get(sym)
                .map(|t| now.duration_since(*t) >= min_interval)
                .unwrap_or(true);
            if stale {
                out.push(sym.to_string());
            }
        }
    }
    let n = items.len();
    let start = *cursor % n;
    for offset in 0..n {
        if out.len() >= batch_size {
            break;
        }
        let idx = (start + offset) % n;
        let sym = &items[idx].symbol;
        if out.iter().any(|s| s == sym) {
            continue;
        }
        let stale = last_fetch
            .get(sym)
            .map(|t| now.duration_since(*t) >= min_interval)
            .unwrap_or(true);
        if stale {
            out.push(sym.clone());
        }
    }
    *cursor = start.wrapping_add(out.len().max(1));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::AssetKind;

    fn item(sym: &str, idx: u32) -> WatchlistItem {
        WatchlistItem {
            id: sym.to_string(),
            symbol: sym.to_string(),
            display_name: None,
            asset_kind: AssetKind::Equity,
            sort_index: idx,
        }
    }

    #[test]
    fn round_robin_respects_batch_and_staleness() {
        let items = vec![item("A", 0), item("B", 1), item("C", 2), item("D", 3)];
        let now = Instant::now();
        let mut last = HashMap::new();
        last.insert("A".into(), now); // fresh
        let mut cursor = 0;
        let batch = pick_batch(
            &items,
            &last,
            now,
            Duration::from_secs(10),
            2,
            &mut cursor,
            None,
        );
        assert!(!batch.contains(&"A".to_string()));
        assert_eq!(batch.len(), 2);
    }

    #[test]
    fn priority_symbol_first() {
        let items = vec![item("A", 0), item("B", 1), item("C", 2)];
        let now = Instant::now();
        let last = HashMap::new();
        let mut cursor = 0;
        let batch = pick_batch(
            &items,
            &last,
            now,
            Duration::from_secs(10),
            2,
            &mut cursor,
            Some("C"),
        );
        assert_eq!(batch[0], "C");
    }
}
```

Full `QuoteScheduler` struct (in same file) holds:

- `visible: bool` (pause when false)
- `watchlist: Vec<WatchlistItem>`
- `quote_cache`, `sparkline_cache`
- `last_quote_fetch`, `last_spark_fetch`
- `cursor`, `priority: Option<String>`
- `provider: Arc<dyn MarketDataProvider>`
- `queue: RateLimitedQueue`
- method `set_visible`, `set_watchlist`, `bump_priority(symbol)`
- `tick_once` for tests: if visible, pick_batch, enqueue fetches, update caches
- background `run` loop: sleep `RefreshPolicy::TICK`, call tick_once, emit callback/`tokio::sync::broadcast` of quotes

Keep `tick_once` testable with a mock provider:

```rust
struct MockProvider {
    quotes: Mutex<Vec<Quote>>,
}

#[async_trait]
impl MarketDataProvider for MockProvider {
    fn id(&self) -> &'static str { "mock" }
    fn supports(&self, _: AssetKind) -> bool { true }
    fn limits(&self) -> ProviderLimits {
        ProviderLimits {
            max_concurrent: 2,
            min_interval: Duration::from_secs(1),
            prefers_batch: true,
        }
    }
    async fn fetch_quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, String> {
        let all = self.quotes.lock().await;
        Ok(all.iter().filter(|q| symbols.contains(&q.symbol)).cloned().collect())
    }
    async fn fetch_sparkline(&self, symbol: &str, _: &str, _: &str) -> Result<Sparkline, String> {
        Ok(Sparkline {
            symbol: symbol.into(),
            points: vec![],
            previous_close: None,
            as_of: "2026-01-01T00:00:00Z".into(),
        })
    }
}
```

Test: hidden scheduler does not call provider (counter).

- [ ] **Step 4: Run tests**

```bash
cd /home/jyc/dev/EconomyWarRoom/src-tauri && cargo test application::scheduler pick_batch -- --nocapture
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/application src-tauri/src/ports src-tauri/Cargo.toml
git commit -m "feat(app): quote cache and scheduler batch picking"
```

---

### Task 7: Yahoo parse + client (fixtures, no live CI)

**Files:**
- Create: `src-tauri/src/infrastructure/mod.rs`
- Create: `src-tauri/src/infrastructure/yahoo/mod.rs`
- Create: `src-tauri/src/infrastructure/yahoo/parse.rs`
- Create: `src-tauri/src/infrastructure/yahoo/client.rs`
- Create: `src-tauri/tests/fixtures/yahoo_chart_aapl.json`

- [ ] **Step 1: Save a minimal fixture** (hand-written subset of chart response)

`src-tauri/tests/fixtures/yahoo_chart_aapl.json`:

```json
{
  "chart": {
    "result": [
      {
        "meta": {
          "currency": "USD",
          "symbol": "AAPL",
          "regularMarketPrice": 190.5,
          "previousClose": 188.0,
          "chartPreviousClose": 188.0
        },
        "timestamp": [1000, 1001, 1002],
        "indicators": {
          "quote": [
            {
              "close": [188.0, 189.0, 190.5]
            }
          ]
        }
      }
    ],
    "error": null
  }
}
```

- [ ] **Step 2: Parse implementation**

```rust
use crate::domain::sparkline_math::downsample;
use crate::domain::constants::SparklinePolicy;
use crate::domain::types::{Quote, Sparkline, SparklinePoint};
use serde_json::Value;

pub fn parse_quote_from_chart(json: &Value) -> Result<Quote, String> {
    let result = json
        .pointer("/chart/result/0")
        .ok_or("missing chart.result")?;
    let meta = result.get("meta").ok_or("missing meta")?;
    let symbol = meta
        .get("symbol")
        .and_then(|v| v.as_str())
        .ok_or("symbol")?
        .to_string();
    let price = meta
        .get("regularMarketPrice")
        .and_then(|v| v.as_f64())
        .ok_or("price")?;
    let currency = meta
        .get("currency")
        .and_then(|v| v.as_str())
        .unwrap_or("USD")
        .to_string();
    let prev = meta
        .get("previousClose")
        .or_else(|| meta.get("chartPreviousClose"))
        .and_then(|v| v.as_f64());
    let change_percent = prev.filter(|p| *p != 0.0).map(|p| (price - p) / p * 100.0);
    Ok(Quote {
        symbol,
        price,
        currency,
        change_percent,
        as_of: chrono::Utc::now().to_rfc3339(),
        source: "yahoo".into(),
    })
}

pub fn parse_sparkline_from_chart(json: &Value) -> Result<Sparkline, String> {
    let result = json
        .pointer("/chart/result/0")
        .ok_or("missing chart.result")?;
    let meta = result.get("meta").ok_or("missing meta")?;
    let symbol = meta
        .get("symbol")
        .and_then(|v| v.as_str())
        .ok_or("symbol")?
        .to_string();
    let prev = meta
        .get("previousClose")
        .or_else(|| meta.get("chartPreviousClose"))
        .and_then(|v| v.as_f64());
    let timestamps = result
        .get("timestamp")
        .and_then(|v| v.as_array())
        .ok_or("timestamp")?;
    let closes = result
        .pointer("/indicators/quote/0/close")
        .and_then(|v| v.as_array())
        .ok_or("close")?;
    let mut points = Vec::new();
    for (i, t) in timestamps.iter().enumerate() {
        let Some(ts) = t.as_i64() else { continue };
        let close = closes.get(i).and_then(|c| c.as_f64());
        if let Some(c) = close {
            points.push(SparklinePoint { t: ts, close: c });
        }
    }
    let points = downsample(&points, SparklinePolicy::TARGET_POINTS);
    Ok(Sparkline {
        symbol,
        points,
        previous_close: prev,
        as_of: chrono::Utc::now().to_rfc3339(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fixture_quote_and_spark() {
        let raw = include_str!("../../../tests/fixtures/yahoo_chart_aapl.json");
        let v: Value = serde_json::from_str(raw).unwrap();
        let q = parse_quote_from_chart(&v).unwrap();
        assert_eq!(q.symbol, "AAPL");
        assert!((q.price - 190.5).abs() < 1e-9);
        assert!(q.change_percent.unwrap() > 0.0);
        let s = parse_sparkline_from_chart(&v).unwrap();
        assert_eq!(s.points.len(), 3);
    }
}
```

Cargo.toml:

```toml
chrono = { version = "0.4", features = ["serde"] }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
```

- [ ] **Step 3: HTTP client**

```rust
use super::parse::{parse_quote_from_chart, parse_sparkline_from_chart};
use crate::domain::constants::SparklinePolicy;
use crate::domain::types::{AssetKind, Quote, Sparkline};
use crate::ports::market_data::{MarketDataProvider, ProviderLimits};
use async_trait::async_trait;
use std::time::Duration;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

pub struct YahooProvider {
    client: reqwest::Client,
}

impl YahooProvider {
    pub fn new() -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .user_agent(UA)
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { client })
    }

    async fn chart_json(&self, symbol: &str, range: &str, interval: &str) -> Result<serde_json::Value, String> {
        let url = format!(
            "https://query1.finance.yahoo.com/v8/finance/chart/{symbol}"
        );
        let resp = self
            .client
            .get(&url)
            .query(&[("range", range), ("interval", interval)])
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if resp.status().as_u16() == 429 {
            return Err("rate_limited".into());
        }
        if !resp.status().is_success() {
            return Err(format!("http {}", resp.status()));
        }
        resp.json().await.map_err(|e| e.to_string())
    }
}

#[async_trait]
impl MarketDataProvider for YahooProvider {
    fn id(&self) -> &'static str {
        "yahoo"
    }
    fn supports(&self, kind: AssetKind) -> bool {
        matches!(kind, AssetKind::Equity | AssetKind::Crypto | AssetKind::Other)
    }
    fn limits(&self) -> ProviderLimits {
        ProviderLimits {
            max_concurrent: 3,
            min_interval: Duration::from_secs(10),
            prefers_batch: false, // per-symbol chart
        }
    }
    async fn fetch_quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, String> {
        let mut out = Vec::new();
        for sym in symbols {
            let json = self
                .chart_json(sym, SparklinePolicy::RANGE, SparklinePolicy::INTERVAL)
                .await?;
            out.push(parse_quote_from_chart(&json)?);
        }
        Ok(out)
    }
    async fn fetch_sparkline(
        &self,
        symbol: &str,
        range: &str,
        interval: &str,
    ) -> Result<Sparkline, String> {
        let json = self.chart_json(symbol, range, interval).await?;
        parse_sparkline_from_chart(&json)
    }
}
```

On `rate_limited`, scheduler applies backoff (Task 6 loop).

- [ ] **Step 4: Run parse tests only**

```bash
cd /home/jyc/dev/EconomyWarRoom/src-tauri && cargo test yahoo::parse -- --nocapture
```

Expected: PASS (no network)

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/infrastructure src-tauri/tests/fixtures src-tauri/Cargo.toml
git commit -m "feat(data): Yahoo chart parse and provider"
```

---

### Task 8: JSON persistence

**Files:**
- Create: `src-tauri/src/infrastructure/store.rs`

- [ ] **Step 1: Implement load/save with defaults**

```rust
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
}
```

Cargo.toml dev-dep:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Test**

```bash
cd /home/jyc/dev/EconomyWarRoom/src-tauri && cargo test infrastructure::store -- --nocapture
```

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/infrastructure/store.rs src-tauri/Cargo.toml
git commit -m "feat(store): JSON persisted watchlist and settings"
```

---

### Task 9: Tauri window policy, commands, app state

**Files:**
- Create: `src-tauri/src/commands.rs`
- Create: `src-tauri/src/infrastructure/window_ctl.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tauri.conf.json` — transparent, decorations false, alwaysOnTop, size
- Modify: `src-tauri/Cargo.toml` — plugins

- [ ] **Step 1: Configure window in `tauri.conf.json`**

Set main window approximately:

```json
{
  "productName": "EconomyWarRoom",
  "identifier": "com.economywarroom.app",
  "app": {
    "windows": [
      {
        "title": "Economy War Room",
        "width": 320,
        "height": 640,
        "minWidth": 260,
        "minHeight": 360,
        "resizable": true,
        "decorations": false,
        "transparent": true,
        "alwaysOnTop": true,
        "visible": true,
        "center": false
      }
    ],
    "security": {
      "csp": null
    }
  }
}
```

(Align with actual Tauri 2 schema from scaffold.)

- [ ] **Step 2: Managed state + commands**

```rust
// commands.rs — signatures
#[tauri::command]
fn get_state(state: tauri::State<'_, AppHandleState>) -> PersistedState { ... }

#[tauri::command]
fn add_symbol(app: AppHandle, state: State<AppHandleState>, symbol: String, asset_kind: AssetKind) -> Result<WatchlistItem, String> { ... }

#[tauri::command]
fn remove_symbol(app: AppHandle, state: State<AppHandleState>, id: String) -> Result<(), String> { ... }

#[tauri::command]
fn reorder_symbols(app: AppHandle, state: State<AppHandleState>, ordered_ids: Vec<String>) -> Result<(), String> { ... }

#[tauri::command]
fn set_theme(state: State<AppHandleState>, theme: ThemeMode) -> Result<(), String> { ... }

#[tauri::command]
fn set_opacity(app: AppHandle, state: State<AppHandleState>, opacity: f64) -> Result<(), String> { ... }

#[tauri::command]
fn hide_widget(app: AppHandle, state: State<AppHandleState>) -> Result<(), String> { ... }

#[tauri::command]
fn toggle_widget_visibility(app: AppHandle, state: State<AppHandleState>) -> Result<bool, String> { ... }

#[tauri::command]
fn save_window_geometry(state: State<AppHandleState>, geometry: WindowGeometry) -> Result<(), String> { ... }

#[tauri::command]
fn get_quotes(state: State<AppHandleState>) -> Vec<Quote> { ... }

#[tauri::command]
fn get_sparklines(state: State<AppHandleState>) -> Vec<Sparkline> { ... }
```

`AppHandleState` holds `Mutex<PersistedState>`, path, scheduler handle, `visible: AtomicBool`.

On hide/show: `window.hide()` / `window.show()` + `scheduler.set_visible`.

Emit events:

```rust
app.emit("quotes-updated", quotes)?;
app.emit("sparklines-updated", sparks)?;
app.emit("watchlist-updated", watchlist)?;
```

- [ ] **Step 3: Wire plugins in lib.rs setup**

- `tauri-plugin-global-shortcut` register `Ctrl+Shift+Space` → `toggle_widget_visibility`
- `tauri-plugin-autostart` enable when `settings.autostart`
- On startup: load state, apply opacity, position window, start scheduler, show window

- [ ] **Step 4: Manual smoke**

```bash
cd /home/jyc/dev/EconomyWarRoom && npm run tauri dev
```

Expected: transparent-ish window opens, always on top; hotkey hides/shows (Linux/X11 may differ — note in README; Windows is target).

- [ ] **Step 5: Commit**

```bash
git add src-tauri
git commit -m "feat(app): window controls, commands, hotkey, autostart hooks"
```

---

### Task 10: Frontend types + glass shell + header hide

**Files:**
- Create: `src/ui/types.ts`
- Create: `src/styles/tokens.css`
- Create: `src/styles/app.css`
- Create: `src/ui/header.ts`
- Create: `src/ui/app.ts`
- Modify: `src/main.ts`
- Modify: `index.html`

- [ ] **Step 1: CSS tokens (glass)**

`src/styles/tokens.css`:

```css
:root {
  color-scheme: light dark;
  --bg-glass: rgba(255, 255, 255, 0.55);
  --bg-glass-dark: rgba(28, 28, 30, 0.55);
  --text: #1d1d1f;
  --text-secondary: rgba(0, 0, 0, 0.55);
  --up: #34c759;
  --down: #ff3b30;
  --radius: 16px;
  --blur: 20px;
  font-family: system-ui, -apple-system, "Segoe UI", sans-serif;
}

html[data-theme="dark"] {
  --bg-glass: var(--bg-glass-dark);
  --text: #f5f5f7;
  --text-secondary: rgba(255, 255, 255, 0.55);
}

@media (prefers-reduced-transparency: reduce) {
  :root {
    --bg-glass: rgba(255, 255, 255, 0.92);
    --blur: 0px;
  }
}
```

`app.css`: panel full viewport, rounded, `backdrop-filter: blur(var(--blur))`, padding, no body margin, `background: transparent` on html/body.

- [ ] **Step 2: Header with drag + hide**

```ts
// header.ts
import { invoke } from "@tauri-apps/api/core";

export function renderHeader(root: HTMLElement) {
  root.innerHTML = `
    <header class="header" data-tauri-drag-region>
      <span class="title">War Room</span>
      <div class="header-actions">
        <button type="button" class="icon-btn" id="btn-settings" aria-label="Settings">⚙</button>
        <button type="button" class="icon-btn" id="btn-hide" aria-label="Hide">−</button>
      </div>
    </header>
  `;
  root.querySelector("#btn-hide")!.addEventListener("click", () => {
    invoke("hide_widget");
  });
}
```

- [ ] **Step 3: Boot**

`main.ts` imports CSS, mounts app, `invoke("get_state")`, applies theme attribute.

- [ ] **Step 4: Dev visual check**

```bash
npm run tauri dev
```

Expected: glass panel, hide button works.

- [ ] **Step 5: Commit**

```bash
git add src index.html
git commit -m "feat(ui): glass shell and hide control"
```

---

### Task 11: Watchlist UI — rows, +, DnD, remove

**Files:**
- Create: `src/ui/watchlist.ts`
- Create: `src/ui/sparkline.ts`
- Modify: `src/ui/app.ts`

- [ ] **Step 1: Sparkline SVG**

```ts
// sparkline.ts
export function sparklinePath(
  points: { t: number; close: number }[],
  width: number,
  height: number
): string {
  if (points.length === 0) return "";
  const closes = points.map((p) => p.close);
  const min = Math.min(...closes);
  const max = Math.max(...closes);
  const span = max - min || 1;
  return points
    .map((p, i) => {
      const x = (i / Math.max(points.length - 1, 1)) * width;
      const y = height - ((p.close - min) / span) * height;
      return `${i === 0 ? "M" : "L"}${x.toFixed(2)} ${y.toFixed(2)}`;
    })
    .join(" ");
}
```

- [ ] **Step 2: List render + HTML5 DnD**

- Each row: `draggable="true"`, `data-id`
- `dragstart` / `dragover` / `drop` → compute new `ordered_ids` → `invoke("reorder_symbols", { orderedIds })`
- Bottom button `#btn-add` opens inline form (input + confirm)
- Submit → `invoke("add_symbol", { symbol, assetKind })` — for MVP: if symbol contains `-` or ends with `USD` treat as crypto else equity
- Remove: small `×` on row → `invoke("remove_symbol", { id })`

Row shows: symbol, svg sparkline, price (2–4 decimals), change % with class `up`/`down`.

- [ ] **Step 3: Listen for events**

```ts
import { listen } from "@tauri-apps/api/event";

await listen("quotes-updated", (e) => { /* merge into local map; re-render */ });
await listen("sparklines-updated", (e) => { /* ... */ });
await listen("watchlist-updated", (e) => { /* ... */ });
```

- [ ] **Step 4: Manual test**

Add `MSFT`, reorder above AAPL, remove, refresh persistence by restarting app.

- [ ] **Step 5: Commit**

```bash
git add src/ui
git commit -m "feat(ui): watchlist add remove drag-reorder sparklines"
```

---

### Task 12: Settings panel — theme, opacity, quit

**Files:**
- Create: `src/ui/settings-panel.ts`
- Modify: `src/ui/header.ts` / `app.ts`

- [ ] **Step 1: Panel UI**

- Theme segmented: Light / Dark / System → `invoke("set_theme", { theme })` + `document.documentElement.dataset.theme`
- Opacity range input 0.35–1 → `invoke("set_opacity", { opacity: Number(value) })`
- Quit button → `import { exit } from "@tauri-apps/plugin-process"` or `getCurrentWindow().close()` after confirming process exit path — use `import { exit } from '@tauri-apps/plugin-process'` if plugin added, else command `quit_app` calling `app.exit(0)`.

- [ ] **Step 2: Persist geometry on move/resize**

```ts
import { getCurrentWindow } from "@tauri-apps/api/window";
const w = getCurrentWindow();
w.onMoved(async () => { /* outerPosition + innerSize → save_window_geometry */ });
w.onResized(async () => { /* same */ });
```

- [ ] **Step 3: Commit**

```bash
git add src/ui src-tauri/src/commands.rs
git commit -m "feat(ui): theme opacity settings and geometry save"
```

---

### Task 13: Scheduler wiring end-to-end + backoff

**Files:**
- Modify: `src-tauri/src/application/scheduler.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: On each tick when visible**

1. `pick_batch` for quotes  
2. For each symbol enqueue `fetch_quotes([sym])` with priority 0 if priority symbol else 1  
3. Update cache; `emit("quotes-updated")`  
4. Separately, for symbols whose sparkline age > `SPARKLINE_MIN_INTERVAL`, enqueue sparkline fetch  

On error containing `rate_limited`: set global `backoff_until = now + backoff`, double backoff up to max; skip ticks until then.

- [ ] **Step 2: On `set_visible(true)`**

Clear soft cursor barrier; force all symbols stale; immediate tick.

- [ ] **Step 3: Manual sustained test (~2 min)**

Watch 4–6 symbols; confirm network not firing every second per symbol (use log lines `tracing` or `eprintln` behind log).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src
git commit -m "feat(app): wire scheduler to Yahoo with backoff and events"
```

---

### Task 14: Docs sync + verification checklist

**Files:**
- Modify: `README.md` — real install/run
- Modify: `docs/TODO.md` — mark completed phases
- Modify: `docs/superpowers/plans/README.md` — link this plan

- [ ] **Step 1: README runbook**

```markdown
## Develop

Requirements: Rust stable, Node 18+, platform WebView deps.

```bash
npm install
npm run tauri dev
```

Hotkey: Ctrl+Shift+Space toggles visibility.
Hide button only hides; use Settings → Quit to exit.
```

- [ ] **Step 2: Manual MVP checklist (all must pass)**

- [ ] Always on top floating glass window  
- [ ] Drag move + size persist after restart  
- [ ] Opacity + theme light/dark/system  
- [ ] Seed AAPL + BTC-USD load quotes/sparklines  
- [ ] Add symbol at bottom via +  
- [ ] DnD reorder persists  
- [ ] Remove symbol  
- [ ] Hide button hides; hotkey shows; polling pauses while hidden  
- [ ] Autostart registered when setting true (verify OS-specific)  

- [ ] **Step 3: Full test suite**

```bash
cd src-tauri && cargo test
```

Expected: all PASS

- [ ] **Step 4: Final commit**

```bash
git add README.md docs/
git commit -m "docs: MVP runbook and TODO progress"
```

---

## Spec coverage self-check

| Spec requirement | Task(s) |
|------------------|---------|
| Floating always-on-top widget | 9, 10 |
| US equity + crypto MVP | 7, 8 seed, 11 asset kind heuristic |
| Sparkline 1d/5m + price + change % | 4, 7, 11, 13 |
| In-widget add (bottom +), remove, DnD | 3, 11 |
| Hotkey Ctrl+Shift+Space | 9 |
| In-UI hide = hide only | 9, 10 |
| Opacity + theme L/D/system + glass | 10, 12 |
| Autostart + visible on launch | 8 defaults, 9 |
| Rate-limited scheduler/queue | 5, 6, 13 |
| Free API Yahoo-style | 7 |
| Clean layers + constants | 2, file map |
| JSON only, no portfolio | 8, non-goals |
| AssetStocker-light policies | 2 constants, 5–6, 13 |
| Tests: domain, parse, queue | 3–8 |
| Extensible provider port | 6 ports |

## Placeholder / consistency review

- Types use `WatchlistItem`, `sort_index`, `AssetKind`, `ThemeMode` consistently across tasks.  
- Commands use snake_case Rust / Tauri default camelCase conversion for JS — implementers must match `tauri::command` rename or use exact serde names; prefer `#[serde(rename_all = "camelCase")]` on DTOs at command boundary if JS uses camelCase.  
- If JS camelCase preferred, add to `PersistedState` and command args in Task 9:

```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchlistItem { ... }
```

Apply same rename on all UI-facing structs once in Task 2 or 9 (do not mix).

## Risks during execution

- Global shortcut on Wayland may fail — document; primary target Windows.  
- Yahoo may block datacenter IPs — backoff + cache; fixture tests still pass.  
- Transparent windows need platform compositor support.

---

**Plan complete and saved to `docs/superpowers/plans/2026-07-22-economy-war-room-mvp.md`.**

Two execution options:

1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks  
2. **Inline Execution** — this session executes tasks with checkpoints  

Which approach do you want?
