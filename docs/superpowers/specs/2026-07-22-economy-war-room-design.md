# EconomyWarRoom — Design Spec

**Date:** 2026-07-22  
**Status:** Draft (approved in brainstorming; UI section included from agreed product decisions)  
**Product type:** Lightweight floating market-watch widget (not a portfolio app)

## 1. Problem & product one-liner

Desktop users want a **tall floating Windows widget** that shows a personal watchlist of US equities and crypto with **sparklines**, **last price**, and **change %**, toggleable via **hotkey** and an in-widget **hide** control — without the weight of a full finance/portfolio application (contrast: AssetStocker).

## 2. Goals

- Glanceable vertical watchlist with stock-app-like rows (sparkline + price + change %).
- Add / remove / reorder symbols inside the widget; new items append at the bottom; drag-and-drop reorder; **+** control under the last row.
- Global hotkey **Ctrl+Shift+Space** toggles visibility; in-UI hide button does the **same** (hide only — process stays alive).
- Free drag positioning + **always on top**.
- Theme: light / dark / system + **translucent glass**.
- Opacity user-adjustable.
- Autostart on login; widget **visible** on launch.
- Near-real-time feel (1–5s UI) with **rate-limited scheduling / queuing** so free APIs are not hammered.
- Clean architecture, OOP, shared constants/types; **extensible** providers (commodities, KR stocks later).
- Apple-like UI principles ([apple-design skill](https://github.com/emilkowalski/skills/tree/main/skills/apple-design)): materials, restraint, fluid motion where cheap.

## 3. Non-goals (explicit)

- Portfolio, P&L, transactions, broker sync, CSV import.
- Heavy local DB / multi-year history / snapshot backfill (AssetStocker-scale).
- Order execution.
- Mandatory API keys for MVP (free public sources first).
- OS Windows 11 Widgets board integration.
- Multi-process local quote proxy.

## 4. Decisions log

| Topic | Decision |
|-------|----------|
| Form factor | Floating desktop panel (not Win11 widget board, not tray-only) |
| MVP assets | US equities first + crypto; design for extension |
| Stack | **Tauri** (Rust + web UI) |
| Data approach | Free public APIs; learn from **AssetStocker** policies, reimplement slim |
| Reference project | `../AssetStocker` (Flutter portfolio app) — concepts only, no Dart port |
| Refresh feel | Near real-time with scheduler + queue + backoff |
| Sparkline | Intraday **1d / 5m** default |
| Watchlist UX | In-widget add/remove/reorder; append bottom; DnD; bottom **+** |
| Hotkey | `Ctrl+Shift+Space` |
| Hide button | Hide widget only (same as hotkey), not quit |
| Window | Freely draggable, always on top |
| Theme | Light / dark / system + glass |
| Opacity | User adjustable |
| Startup | Login autostart + show widget immediately |
| Architecture style | Approach 1: thin Tauri shell + web UI; **network + scheduler in Rust** |

## 5. Architecture

### 5.1 Runtime

```
┌─────────────────────────────────────────┐
│  Web UI (HTML/CSS/TS)                   │
│  · glass panel, list, sparkline SVG     │
│  · DnD reorder, bottom +, hide button   │
│  · theme / opacity                      │
└─────────────────┬───────────────────────┘
                  │ Tauri commands / events
┌─────────────────▼───────────────────────┐
│  Rust core                              │
│  · Window: always-on-top, position,     │
│    opacity, show/hide, autostart        │
│  · Global hotkey                        │
│  · Settings + Watchlist store (JSON)    │
│  · QuoteScheduler + RateLimitedQueue    │
│  · MarketDataProvider(s) → HTTP         │
└─────────────────────────────────────────┘
```

### 5.2 Layers

| Layer | Responsibility | Examples |
|-------|----------------|----------|
| Domain | Market-agnostic models & rules | `AssetKind`, `Quote`, `Sparkline`, `WatchlistItem` |
| Ports | Interfaces | `MarketDataProvider`, `WatchlistStore`, `Clock` |
| Application | Use cases | AddSymbol, Reorder, RefreshQuotes, ToggleVisibility |
| Infrastructure | Implementations | YahooProvider, JsonStore, window/hotkey adapters |
| UI | Presentation only | Rows, chrome, settings controls |

### 5.3 Shared policy constants (single place)

Shrink of AssetStocker’s `PriceUpdateCadence` idea — **few** named constants, not a forest:

- `REFRESH`: tick interval, batch size, per-symbol min interval, max concurrent
- `SPARKLINE`: range `1d`, interval `5m`, target point count
- `WINDOW`: default size, min size
- `HOTKEY`: default accelerator
- `OPACITY`: min/max/default

## 6. Data model

```
WatchlistItem
  id, symbol, displayName?, assetKind, sortIndex

Quote
  symbol, price, currency, changePercent, asOf, source

Sparkline
  symbol, points: [{ t, close }], previousClose?, asOf

AppSettings
  theme: light | dark | system
  opacity: number   // e.g. 0.3 … 1.0
  window: { x, y, width, height }
  hotkey: string    // default Ctrl+Shift+Space; structure open for later
  autostart: bool   // default true
```

**Persistence:** one JSON file under app data (watchlist + settings + window geometry). No SQLite for MVP. Quote/sparkline caches **in-memory** only (optional later).

**Ordering:** `sortIndex` defines list order. New items get `max(sortIndex)+1` (bottom).

## 7. Market data & scheduling

### 7.1 Provider port

```
MarketDataProvider
  id
  supports(AssetKind) -> bool
  limits: { maxConcurrent, minInterval, prefersBatch }
  fetchQuotes(symbols) -> Quote[]
  fetchSparkline(symbol, range, interval) -> Sparkline
```

**MVP:** Yahoo-style public chart/search endpoints (as proven in AssetStocker `YahooFinanceClient`): User-Agent required, chart for price + history, careful concurrency.

**Later:** dedicated crypto exchange provider, commodities, KR equities — same port.

### 7.2 QuoteScheduler (rate-limit aware)

Goals: UI feels 1–5s fresh; network stays under free-API limits.

| Policy | MVP default (tunable constants) |
|--------|----------------------------------|
| Scheduler tick | ~1s |
| Batch size per tick | 3–5 symbols (round-robin by sortIndex) |
| Min re-fetch per symbol (quotes) | ~5–15s |
| Sparkline refresh | ~5 minute bucket / slower than quotes |
| Widget hidden | **Stop polling** |
| Widget shown again | Immediate full refresh, then normal ticks |
| 429 / errors | Exponential backoff; keep last good cache |
| Priority | Just-added symbol > rolling refresh |
| Dedup | Job key coalesce (same symbol not double-fetched) |

**Queue:** single `RateLimitedQueue` (AssetStocker `NetworkJobQueue` + `YfRateLimiter` concepts, **minimal**): max concurrent + optional priority; no per-symbol `setInterval`.

**UI vs network separation:** UI may re-render on a short cadence from cache; network cadence is coarser via batch round-robin.

### 7.3 Sparklines

- Default: **intraday 1d / 5m**
- Downsample to a small point budget (~20–40) for cheap SVG
- Off-session: keep last curve; no heavy market-calendar subsystem in MVP

## 8. UI / UX

### 8.1 Layout

- Tall narrow floating panel, large corner radius, glass (backdrop blur + translucent fill).
- Rows: symbol (and optional name) · sparkline · price · change % (color up/down).
- Bottom of list: **+** control to add symbol.
- Header chrome: minimal title/drag region + **hide** control (same as hotkey).
- Opacity and theme accessible via lightweight settings affordance (popover or small sheet — keep simple).

### 8.2 Interactions

| Action | Behavior |
|--------|----------|
| Add | + → input/search-lite → append bottom → priority quote fetch |
| Reorder | Drag-and-drop rows; persist sortIndex |
| Remove | Row action (swipe or menu) — exact chrome in implementation; must be one clear path |
| Drag window | Drag header/empty chrome; persist position |
| Hide | Header button or hotkey → hide window, pause quotes |
| Show | Hotkey → show, resume quotes + immediate refresh |
| Opacity | Slider or equivalent; apply to window/panel |

### 8.3 Visual direction

- Apple-inspired: system fonts, size-specific tracking, glass hierarchy, critically damped motion for hide/show and list changes; bounce only if gesture momentum warrants it.
- `prefers-reduced-motion` / reduced transparency respected when feasible.
- Reference skill: emilkowalski `apple-design` (web-oriented; maps well to Tauri webview).

## 9. Window, OS integration

| Feature | Behavior |
|---------|----------|
| Always on top | Default on |
| Position/size | User drag/resize within min bounds; persisted |
| Opacity | User-controlled; persisted |
| Hotkey | `Ctrl+Shift+Space` show/hide toggle |
| Autostart | Enabled by default (login) |
| Launch | Widget visible immediately |
| Quit | Not the hide button; provide a clear quit path later (tray menu or settings) if needed |

## 10. What we borrow from AssetStocker

**Borrow (ideas / policies only — reimplement in Rust/TS):**

- `MarketDataService` / multi-source `PriceSource` idea → `MarketDataProvider`
- Yahoo chart + UA + concurrency guards
- Priority queue / job coalesce patterns
- Cadence constants object (shrunk)
- Sparkline 1d/5m + point budget + pacing ideas

**Do not borrow:**

- Portfolio, transactions, KIS, Drift schema, Flutter UI, Finnhub-as-required path, snapshot/backfill services

## 11. Testing strategy (lightweight)

- Domain/unit: sortIndex append/reorder, scheduler pick order, backoff, coalesce keys
- Provider: parse fixtures for Yahoo-like JSON (no live network in CI)
- Manual: hotkey, hide button, autostart, glass themes, DnD, opacity

## 12. Risks & mitigations

| Risk | Mitigation |
|------|------------|
| Yahoo/public API flaky or rate limits | Queue, batch, backoff, cache; provider swappable |
| CORS if fetch from webview | Fetch from Rust |
| “Almost realtime” vs free limits | Round-robin batch; UI tick ≠ full-list network tick |
| Scope creep toward AssetStocker | Non-goals list; JSON-only; no portfolio |

## 13. Success criteria (MVP)

1. User can pin a vertical always-on-top glass widget, drag it, set opacity/theme.
2. Add US stock and crypto symbols via bottom +; reorder by drag; remove.
3. Each row shows sparkline (1d/5m), price, change %.
4. Hotkey and hide button both hide; hotkey shows again; polling pauses while hidden.
5. Autostart shows widget after login.
6. Extended idle use does not trip obvious free-API bans under default constants.
7. Codebase has clear domain/provider/scheduler boundaries and shared constants.

## 14. Open items (intentionally small)

- Exact Yahoo (or alternate) endpoint set and symbol search UX depth (type-in vs search API).
- Quit / tray icon: optional post-MVP if users cannot find how to exit.
- Remappable hotkey UI: structure ready; MVP may ship fixed default.
- Commodity / KR equity providers: post-MVP.
- Whether sparkline previousClose for % coloring matches Yahoo meta only (prefer simple).
---

## Appendix A — Suggested repo layout (implementation)

```
src-tauri/          Rust: window, hotkey, store, scheduler, providers
src/                Web UI
docs/
  superpowers/specs/   this document
  TODO.md
README.md
```

## Appendix B — Brainstorming process notes

Collaborative decisions recorded in session 2026-07-22; this file is the durable source of truth for the approved design direction before implementation planning.
