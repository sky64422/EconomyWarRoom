# EconomyWarRoom — TODO

Living checklist derived from the design spec  
(`docs/superpowers/specs/2026-07-22-economy-war-room-design.md`).

Status: `pending` · `in_progress` · `done` · `blocked`

**MVP code path:** implemented on branch `feat/mvp-widget` (see plan Tasks 1–13). Docs/runbook: Task 14.

---

## Phase 0 — Project foundation

| ID | Task | Status |
|----|------|--------|
| P0-1 | Scaffold Tauri (Rust + web) app for Windows | done |
| P0-2 | Folder layout: domain / ports / application / infrastructure / ui | done |
| P0-3 | Shared constants module (`REFRESH`, `SPARKLINE`, `WINDOW`, `HOTKEY`, `OPACITY`) | done |
| P0-4 | App data path + JSON load/save skeleton | done |
| P0-5 | Keep README and this TODO in sync as decisions change | done |

## Phase 1 — Window shell & OS integration

| ID | Task | Status |
|----|------|--------|
| P1-1 | Frameless (or lightly chrome) tall floating window, min size | done |
| P1-2 | Always on top (default on) | done |
| P1-3 | Draggable reposition; persist x/y/w/h | done |
| P1-4 | Window opacity API wired to settings | done |
| P1-5 | Show / hide window commands | done |
| P1-6 | Global hotkey `Ctrl+Shift+Space` → toggle visibility | done |
| P1-7 | Login autostart (default on) | done |
| P1-8 | Launch with widget visible | done |

## Phase 2 — Domain & persistence

| ID | Task | Status |
|----|------|--------|
| P2-1 | Types: `WatchlistItem`, `Quote`, `Sparkline`, `AppSettings`, `AssetKind` | done |
| P2-2 | Watchlist CRUD: add (append bottom), remove, reorder by sortIndex | done |
| P2-3 | Persist watchlist + settings JSON | done |
| P2-4 | In-memory `QuoteCache` / `SparklineCache` | done |

## Phase 3 — Market data & scheduler

| ID | Task | Status |
|----|------|--------|
| P3-1 | `MarketDataProvider` trait/interface + limits metadata | done |
| P3-2 | Yahoo (or equivalent free) quote provider with UA + error mapping | done |
| P3-3 | Sparkline fetch 1d/5m + downsample | done |
| P3-4 | `RateLimitedQueue` (max concurrent, job-key coalesce) | done |
| P3-5 | `QuoteScheduler`: tick, round-robin batch, min interval | done |
| P3-6 | Pause polling when hidden; resume + immediate refresh when shown | done |
| P3-7 | Priority boost for newly added symbol | done |
| P3-8 | Backoff on 429/network failure; keep last good quote | done |
| P3-9 | Fixture-based unit tests for parse + scheduler (no live API in CI) | done |

## Phase 4 — Web UI (Apple-like glass)

| ID | Task | Status |
|----|------|--------|
| P4-1 | Glass panel shell (blur, radius, light/dark/system) | done |
| P4-2 | Watchlist row: symbol, sparkline, price, change % | done |
| P4-3 | Bottom **+** add flow (symbol input; search depth per open item) | done |
| P4-4 | Drag-and-drop reorder; sync to Rust store | done |
| P4-5 | Remove symbol control | done |
| P4-6 | Header **hide** button (= hotkey hide) | done |
| P4-7 | Theme selector (light / dark / system) | done |
| P4-8 | Opacity control in UI | done |
| P4-9 | Subscribe to quote/sparkline updates from Rust events | done |
| P4-10 | Reduced motion / reduced transparency where practical | done |

## Phase 5 — Polish & verification

| ID | Task | Status |
|----|------|--------|
| P5-1 | Default sample watchlist (optional, few US + crypto symbols) | done |
| P5-2 | Manual test checklist: hotkey, hide, DnD, autostart, opacity, theme | pending |
| P5-3 | Sustained-run smoke (rate limits under default constants) | pending |
| P5-4 | Quit path documented (and implemented if missing) | done |
| P5-5 | Spec/README/TODO updated after MVP | done |

### Manual verification checklist (P5-2 / P5-3)

Run with `npm run tauri dev` on the target OS (Windows preferred):

- [ ] Always-on-top floating glass window
- [ ] Drag move + size persist after restart
- [ ] Opacity + theme light / dark / system
- [ ] Seed AAPL + BTC-USD load quotes and sparklines
- [ ] Add symbol at bottom via **+**
- [ ] DnD reorder persists
- [ ] Remove symbol
- [ ] Hide button hides; hotkey shows; polling pauses while hidden
- [ ] Settings → Quit exits the process
- [ ] Autostart registered when setting true (verify OS-specific)
- [ ] Sustained-run smoke: leave open long enough to confirm rate limits / backoff stay healthy under default constants

## Out of scope (do not start without new design approval)

- Portfolio, P&L, transactions, broker APIs
- SQLite / historical snapshot system
- Windows 11 official Widgets board
- Finnhub (or other) API-key-required realtime as MVP dependency
- Commodities / KR market providers (post-MVP extension slots only)
- Separate multi-process quote proxy

## Suggested implementation order

1. P0 → P1 (see a glass window + hotkey hide before data) — **done**
2. P2 → P3 (quotes flowing with scheduler) — **done**
3. P4 (bind UI) — **done**
4. P5 (harden) — code done; **manual smoke remaining**

Detail task breakdown:  
[`docs/superpowers/plans/2026-07-22-economy-war-room-mvp.md`](superpowers/plans/2026-07-22-economy-war-room-mvp.md) (Tasks 1–14, TDD steps).
