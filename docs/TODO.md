# EconomyWarRoom — TODO

Living checklist derived from the design spec  
(`docs/superpowers/specs/2026-07-22-economy-war-room-design.md`).

Status: `pending` · `in_progress` · `done` · `blocked`

**MVP code:** implemented and merged to **`main`** (plan Tasks 1–14).  
**Automated quality:** unit + integration + risk tests; coverage gate ≥85% (~98% business logic).  
  Last measured: lib **~63** + e2e 4 + risk 7.  
**Windows host:** toolchain ready (Node, Rust MSVC, VS Build Tools, WebView2).  
**Diagnostics (Mode B):** Copy diagnostics + command/scheduler event hardening on `main`.  
**Post-MVP (2026-07-23):** updater, card tints, multi-select, quote refresh, autostart UI,  
content-hug min window, card layout (symbol·spark·price), release tooling — **done** (shipped through **v0.1.7**).  
**Remaining (highest priority):** **P5-2 / P5-3** manual smoke on Windows via `npm run run:exe`.

**New session / Windows clone:** start at [`docs/HANDOFF.md`](HANDOFF.md) and [`docs/windows-dev.md`](windows-dev.md).

---

## Next up (priority order)

1. **P5-2** — Manual checklist below (`npm run run:exe`).  
2. **P5-3** — Sustained-run smoke (rate limits / backoff healthy).  
3. Fix any Windows-only bugs found; update `windows-dev.md` Troubleshooting.  
4. Only then: remaining Phase 6 product ideas (unless you explicitly prioritize a feature).  
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
| P1-7 | Login autostart (default on) + Settings toggle | done |
| P1-8 | Launch with widget visible | done |

## Phase 2 — Domain & persistence

| ID | Task | Status |
|----|------|--------|
| P2-1 | Types: `WatchlistItem`, `Quote`, `Sparkline`, `AppSettings`, `AssetKind`, `CardTint` | done |
| P2-2 | Watchlist CRUD: add (append bottom), remove, multi-remove, reorder by sortIndex | done |
| P2-3 | Persist watchlist + settings JSON (incl. tint, quote interval, autostart) | done |
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
| P3-10 | User-configurable quote refresh interval (5–120s) | done |

## Phase 4 — Web UI (Apple-like glass)

| ID | Task | Status |
|----|------|--------|
| P4-1 | Glass panel shell (blur, radius, light/dark/system) | done |
| P4-2 | Watchlist row: symbol, sparkline, price, change % | done |
| P4-3 | Bottom **+** add flow (symbol input; search depth per open item) | done |
| P4-4 | Drag-and-drop reorder; sync to Rust store | done |
| P4-5 | Remove symbol control + keyboard Delete multi-remove | done |
| P4-6 | Header **hide** button (= hotkey hide) | done |
| P4-7 | Theme selector (light / dark / system) | done |
| P4-8 | Opacity control in UI | done |
| P4-9 | Subscribe to quote/sparkline updates from Rust events | done |
| P4-10 | Reduced motion / reduced transparency where practical | done |
| P4-11 | Card selection / Ctrl / Shift multi-select | done |
| P4-12 | Pastel card tint picker (right-click) | done |
| P4-13 | Content-hug min height (outer chrome floor) | done |

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

Run with `npm run run:exe` on the target OS (**Windows preferred**):

- [ ] Always-on-top floating glass window  
- [ ] Drag move + size persist after restart  
- [ ] Opacity + theme light / dark / system  
- [ ] Seed AAPL + BTC-USD load quotes and sparklines  
- [ ] Add symbol at bottom via **+** (try 5–8 symbols)  
- [ ] DnD reorder persists  
- [ ] Remove symbol (row **x** and **Delete** on selection)  
- [ ] Multi-select: click, Ctrl+click, Shift+click  
- [ ] Right-click card → pastel tint persists after restart  
- [ ] Hide button hides; **hotkey** `Ctrl+Shift+Space` shows again; polling pauses while hidden  
- [ ] Settings opens as **compact sheet above list** (watchlist still visible / scrollable)  
- [ ] Settings → **Price refresh** presets change cadence  
- [ ] Settings → **Launch at login** toggle registers/unregisters autostart  
- [ ] Header **↻** update check (release build; may no-op in dev)  
- [ ] Window cannot shrink below content (rows + **+ Add** stay visible)  
- [ ] Settings → **Copy diagnostics** → paste looks complete (version, watchlist, events)  
- [ ] Settings → Quit exits the process  
- [ ] Sustained-run smoke: leave open long enough to confirm rate limits / backoff stay healthy  

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
| P6-10 | In-app self-update (Tauri 2 updater plugin + auto-check + header icon) | **done** |
| P6-11 | Card pastel tint, multi-select + Delete, quote interval setting | **done** |
| P6-12 | Settings launch-at-login toggle | **done** |
| P6-13 | Content-hug OS min-size (hard wall; no rubber-band) + no spurious scrollbar at min | **done** |
| P6-14 | Card layout symbol · sparkline · price; spacing / bottom inset polish | **done** |
| P6-15 | `npm run release:publish` + [release.md](./release.md) | **done** |

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
| Post-MVP UX | Updater, tints, multi-select, refresh, autostart UI (2026-07-23) |
| Window / card polish | Min-size wall, layout order, spacing, bottom inset; releases **v0.1.1–v0.1.7** |
| P5 manual | **Open** — checklist above |

Detail task breakdown (historical):  
[`docs/superpowers/plans/2026-07-22-economy-war-room-mvp.md`](superpowers/plans/2026-07-22-economy-war-room-mvp.md)
