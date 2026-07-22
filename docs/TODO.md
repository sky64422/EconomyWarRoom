# EconomyWarRoom — TODO

Living checklist derived from the design spec  
(`docs/superpowers/specs/2026-07-22-economy-war-room-design.md`).

Status: `pending` · `in_progress` · `done` · `blocked`

**MVP code:** implemented and merged to **`main`** (plan Tasks 1–14).  
**Automated quality:** unit + integration + risk tests; coverage gate ≥85% (~98% business logic).  
  Last Windows run: `npm test` green (lib **~59** + e2e 4 + risk 7).  
**Windows host (2026-07-22):** toolchain ready (Node, Rust MSVC, VS Build Tools, WebView2); `npm install` done.  
**Diagnostics (Mode B):** Copy diagnostics + command/scheduler event hardening on `main` (`d4e9214`).  
**Remaining (highest priority):** **P5-2 / P5-3** manual smoke on Windows via `npm run tauri dev`.

**New session / Windows clone:** start at [`docs/HANDOFF.md`](HANDOFF.md) and [`docs/windows-dev.md`](windows-dev.md).

---

## Next up (priority order)

1. **P5-2** — Manual checklist below (`npm run tauri dev`).  
2. **P5-3** — Sustained-run smoke (rate limits / backoff healthy).  
3. Fix any Windows-only bugs found; update `windows-dev.md` Troubleshooting.  
4. Only then: Phase 6 product ideas (unless you explicitly prioritize a feature).  
5. Optional later: **P6-8** rolling file log (hard-crash recovery).

Do **not** start portfolio / P&L / SQLite work without a new design.

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
| P5-2 | Manual test checklist: hotkey, hide, DnD, autostart, opacity, theme | **pending** |
| P5-3 | Sustained-run smoke (rate limits under default constants) | **pending** |
| P5-4 | Quit path documented (and implemented if missing) | done |
| P5-5 | Spec/README/TODO updated after MVP | done |
| P5-6 | Automated coverage ≥85% + integration/risk suites | done |
| P5-7 | Diagnostics: Copy diagnostics + ring coverage / throttle | done |

### Manual verification checklist (P5-2 / P5-3)

Run with `npm run tauri dev` on the target OS (**Windows preferred**):

- [ ] Always-on-top floating glass window  
- [ ] Drag move + size persist after restart  
- [ ] Opacity + theme light / dark / system  
- [ ] Seed AAPL + BTC-USD load quotes and sparklines  
- [ ] Add symbol at bottom via **+** (try 5–8 symbols)  
- [ ] DnD reorder persists  
- [ ] Remove symbol  
- [ ] Hide button hides; **hotkey** `Ctrl+Shift+Space` shows again; polling pauses while hidden  
- [ ] Settings opens as **compact sheet above list** (watchlist still visible / scrollable)  
- [ ] **Tall window:** stretch height — extra space goes to the list, not empty settings chrome  
- [ ] Settings closed: list + `+` footer density looks good at default and max height  
- [ ] Settings → **Copy diagnostics** → paste looks complete (version, watchlist, events)  
- [ ] Settings → Quit exits the process  
- [ ] Autostart registered when setting true (verify OS-specific)  
- [ ] Sustained-run smoke: leave open long enough to confirm rate limits / backoff stay healthy under default constants  

## Phase 6 — Post-MVP ideas

| ID | Task | Status |
|----|------|--------|
| P6-1 | Remappable hotkey UI | pending |
| P6-2 | Dedicated crypto exchange provider (WebSocket optional) | pending |
| P6-3 | Commodities / KR equity providers | pending |
| P6-4 | Symbol search API (Yahoo autocomplete + substring) | done |
| P6-5 | Tray icon / alternate quit affordance | pending |
| P6-6 | Frontend automated e2e (e.g. Playwright) | pending |
| P6-7 | Copy diagnostics (Settings → clipboard dump for agents) | **done** |
| P6-8 | Rolling file log for hard-crash recovery | pending |
| P6-9 | Diagnostics hardening (command notes, scheduler throttle, dump 100 lines) | **done** |

## Out of scope (do not start without new design approval)

- Portfolio, P&L, transactions, broker APIs  
- SQLite / historical snapshot system  
- Windows 11 official Widgets board  
- Finnhub (or other) API-key-required realtime as MVP dependency  
- Separate multi-process quote proxy  

## Implementation history

| Phase | Result |
|-------|--------|
| P0 → P1 | Scaffold + window / hotkey / autostart |
| P2 → P3 | Domain, store, Yahoo, scheduler |
| P4 | Glass UI |
| P5 code | Docs, AppCore, tests, coverage gate |
| Diagnostics | Mode B Copy diagnostics + event-ring hardening (`main`) |
| P5 manual | **Open** — checklist above |

Detail task breakdown (historical):  
[`docs/superpowers/plans/2026-07-22-economy-war-room-mvp.md`](superpowers/plans/2026-07-22-economy-war-room-mvp.md)

Current structure: [`docs/ARCHITECTURE.md`](ARCHITECTURE.md)  
Testing: [`docs/testing.md`](testing.md)  
Defect reporting: [`windows-dev.md` §10](windows-dev.md#10-defect-reporting--agent-visibility)
