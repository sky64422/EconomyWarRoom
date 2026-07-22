# EconomyWarRoom — TODO

Living checklist derived from the design spec  
(`docs/superpowers/specs/2026-07-22-economy-war-room-design.md`).

Status: `pending` · `in_progress` · `done` · `blocked`

---

## Phase 0 — Project foundation

| ID | Task | Status |
|----|------|--------|
| P0-1 | Scaffold Tauri (Rust + web) app for Windows | pending |
| P0-2 | Folder layout: domain / ports / application / infrastructure / ui | pending |
| P0-3 | Shared constants module (`REFRESH`, `SPARKLINE`, `WINDOW`, `HOTKEY`, `OPACITY`) | pending |
| P0-4 | App data path + JSON load/save skeleton | pending |
| P0-5 | Keep README and this TODO in sync as decisions change | pending |

## Phase 1 — Window shell & OS integration

| ID | Task | Status |
|----|------|--------|
| P1-1 | Frameless (or lightly chrome) tall floating window, min size | pending |
| P1-2 | Always on top (default on) | pending |
| P1-3 | Draggable reposition; persist x/y/w/h | pending |
| P1-4 | Window opacity API wired to settings | pending |
| P1-5 | Show / hide window commands | pending |
| P1-6 | Global hotkey `Ctrl+Shift+Space` → toggle visibility | pending |
| P1-7 | Login autostart (default on) | pending |
| P1-8 | Launch with widget visible | pending |

## Phase 2 — Domain & persistence

| ID | Task | Status |
|----|------|--------|
| P2-1 | Types: `WatchlistItem`, `Quote`, `Sparkline`, `AppSettings`, `AssetKind` | pending |
| P2-2 | Watchlist CRUD: add (append bottom), remove, reorder by sortIndex | pending |
| P2-3 | Persist watchlist + settings JSON | pending |
| P2-4 | In-memory `QuoteCache` / `SparklineCache` | pending |

## Phase 3 — Market data & scheduler

| ID | Task | Status |
|----|------|--------|
| P3-1 | `MarketDataProvider` trait/interface + limits metadata | pending |
| P3-2 | Yahoo (or equivalent free) quote provider with UA + error mapping | pending |
| P3-3 | Sparkline fetch 1d/5m + downsample | pending |
| P3-4 | `RateLimitedQueue` (max concurrent, job-key coalesce) | pending |
| P3-5 | `QuoteScheduler`: tick, round-robin batch, min interval | pending |
| P3-6 | Pause polling when hidden; resume + immediate refresh when shown | pending |
| P3-7 | Priority boost for newly added symbol | pending |
| P3-8 | Backoff on 429/network failure; keep last good quote | pending |
| P3-9 | Fixture-based unit tests for parse + scheduler (no live API in CI) | pending |

## Phase 4 — Web UI (Apple-like glass)

| ID | Task | Status |
|----|------|--------|
| P4-1 | Glass panel shell (blur, radius, light/dark/system) | pending |
| P4-2 | Watchlist row: symbol, sparkline, price, change % | pending |
| P4-3 | Bottom **+** add flow (symbol input; search depth per open item) | pending |
| P4-4 | Drag-and-drop reorder; sync to Rust store | pending |
| P4-5 | Remove symbol control | pending |
| P4-6 | Header **hide** button (= hotkey hide) | pending |
| P4-7 | Theme selector (light / dark / system) | pending |
| P4-8 | Opacity control in UI | pending |
| P4-9 | Subscribe to quote/sparkline updates from Rust events | pending |
| P4-10 | Reduced motion / reduced transparency where practical | pending |

## Phase 5 — Polish & verification

| ID | Task | Status |
|----|------|--------|
| P5-1 | Default sample watchlist (optional, few US + crypto symbols) | pending |
| P5-2 | Manual test checklist: hotkey, hide, DnD, autostart, opacity, theme | pending |
| P5-3 | Sustained-run smoke (rate limits under default constants) | pending |
| P5-4 | Quit path documented (and implemented if missing) | pending |
| P5-5 | Spec/README/TODO updated after MVP | pending |

## Out of scope (do not start without new design approval)

- Portfolio, P&L, transactions, broker APIs
- SQLite / historical snapshot system
- Windows 11 official Widgets board
- Finnhub (or other) API-key-required realtime as MVP dependency
- Commodities / KR market providers (post-MVP extension slots only)
- Separate multi-process quote proxy

## Suggested implementation order

1. P0 → P1 (see a glass window + hotkey hide before data)
2. P2 → P3 (quotes flowing with scheduler)
3. P4 (bind UI)
4. P5 (harden)

Detail task breakdown:  
[`docs/superpowers/plans/2026-07-22-economy-war-room-mvp.md`](superpowers/plans/2026-07-22-economy-war-room-mvp.md) (Tasks 1–14, TDD steps).
