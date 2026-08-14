# Architecture (as implemented)

**Updated:** 2026-08-15 (v0.1.46)  
**Branch of truth:** `main`

This document describes the **current codebase**, not only the original design sketch.

## Runtime

```
┌─────────────────────────────────────────────┐
│  Web UI  src/                               │
│  · glass panel, rows, SVG sparklines        │
│  · DnD reorder, select / multi-select       │
│  · pastel card tints, bottom +, hide        │
│  · resizable columns (persisted ratios)     │
│  · extended/pre-post primary + secondary %  │
│  · settings overlay (list fades under sheet)│
└──────────────────┬──────────────────────────┘
                   │ invoke / listen (events)
┌──────────────────▼──────────────────────────┐
│  commands.rs  (thin Tauri adapters)        │
│  lib.rs        (setup: plugins, hotkey,     │
│                 tray, tick loop, updater)   │
└──────────────────┬──────────────────────────┘
                   │
┌──────────────────▼──────────────────────────┐
│  AppCore  application/service.rs            │
│  · watchlist CRUD + card_tint + persist     │
│  · opacity / geometry / autostart           │
│  · quote_refresh_ms (JSON: quote_refresh_secs) │
│  · column_ratios                            │
│  · visibility flag → scheduler              │
│  · quote / sparkline cache reads            │
└──────────┬─────────────────┬────────────────┘
           │                 │
           ▼                 ▼
   QuoteScheduler      JSON store
   + RateLimitedQueue  infrastructure/store.rs
           │
           ▼
   MarketDataProvider
   infrastructure/yahoo (HTTP + parse)

   infrastructure/updater  (Tauri updater plugin)
   system tray (show / hide / quit; skip taskbar)
```

## Source layout

### Rust (`src-tauri/src/`)

| Path | Role |
|------|------|
| `domain/` | Types (`WatchlistItem`, `CardTint`, `Quote` + `PriceRows` display, `ColumnRatios`, `AppSettings`), policy constants, watchlist + **display rows**, sparkline downsample |
| `ports/market_data.rs` | `MarketDataProvider` + `ProviderLimits` |
| `application/cache.rs` | In-memory quote / sparkline caches |
| `application/queue.rs` | `RateLimitedQueue` (max concurrent, key coalesce, priority) |
| `application/scheduler.rs` | Pipeline / RR workers, configurable min quote interval (ms), backoff, sparkline cadence |
| `application/service.rs` | **`AppCore`** — testable app use cases |
| `application/diagnostics.rs` | In-process event ring for Copy diagnostics |
| `infrastructure/yahoo/` | `YahooProvider` (mockable base URL), chart parse, search, prior-day enrich |
| `infrastructure/store.rs` | Load/save `economy-war-room-state.json` |
| `infrastructure/window_ctl.rs` | Show/hide/geometry/opacity; **content min-size** (physical) + resize clamp; clean glass edge |
| `infrastructure/updater.rs` | Startup auto-check + manual install + restart path |
| `commands.rs` | `#[tauri::command]` handlers (incl. `set_content_min_size`, `set_column_ratios`) |
| `state.rs` | `AppHandleState { core, content_min_w/h }` |
| `lib.rs` | Tauri `run()`, autostart, hotkey, **system tray**, tick loop, updater, **on_window_event Resized** |

### Web (`src/`)

| Path | Role |
|------|------|
| `ui/app.ts` | Boot, state, geometry persist, **content-hug min** measure → `set_content_min_size` |
| `ui/header.ts` | Drag region, update check, settings, hide |
| `ui/watchlist/` | Rows, multi-select, DnD, tint/remove, column resize, add (`index` + `metrics` / `columns` / `search` / `dnd`) |
| `ui/sparkline.ts` | SVG path helper; tone from regular-session move |
| `ui/settings-panel.ts` | Overlay settings: opacity, refresh presets, autostart, Copy Log / Quit |
| `ui/types.ts` | TS mirrors of Rust DTOs (snake_case) |
| `styles/` | `tokens.css`, `fonts.css`, `app.css` (barrel) + `base` / `header` / `layout` / `watchlist` / `settings` / `tint-menu` |

### Tests

| Path | Role |
|------|------|
| `src/**` `#[cfg(test)]` | Unit tests (~78) |
| `tests/integration_e2e.rs` | Store + AppCore + scheduler + Yahoo mock HTTP |
| `tests/risk_scenarios.rs` | Rate limit, hide, corrupt JSON, invalid input |

## Key policies (constants)

Defined in `domain/constants.rs` (names approximate):

| Policy | Defaults |
|--------|----------|
| Tick | **500ms** |
| Batch / workers | batch size 4; max concurrent provider ~3 |
| Quote refresh | UI presets **0.25s / 1s / 10s / 1m** (Rust field `quote_refresh_ms`; JSON key still `quote_refresh_secs`; clamp **250ms–120s**, default **500ms**; legacy 1..=120 = whole seconds) |
| Sparkline | range `1d`, interval `5m`, target points 32; min refresh ~300s |
| Backoff | 5s initial → double up to 120s |
| Opacity | 0.35–1.0, default ~0.92 |
| Window | default 320×640; policy floor 260×120; **runtime min = measured content** (OS physical min + Resized clamp) |
| Hotkey | `Ctrl+Shift+Space` |
| Card tints | `none`, `rose`, `peach`, `mint`, `sky`, `lavender`, `lemon` |
| Column ratios | default symbol **1.15** / spark **1.25** / metrics **2.0** `fr` (clamped ~0.45–8) |
| Card layout | Left→right: **symbol · sparkline · price/change**; remove via **context menu** |

## Quote model (extended)

`Quote` carries more than a single last price:

| Field | Role |
|-------|------|
| `price` / `change_percent` | Latest display fields (often live/extended) |
| `regular_price` / `regular_change_percent` | Official regular session |
| `extended_price` / `extended_change_percent` | Pre/post when applicable |
| `previous_close` / `prior_close` / `previous_day_change_percent` | T-1 close / T-2 close / T-1 session % (daily bars; Yahoo `regularMarketPreviousClose` is T-1, never T-2) |
| `market_state` | Yahoo hint: `regular`, `pre`, `post`, `closed`, … |

UI **primary** = the print for the current session. UI **secondary** = the last **completed** regular session (not a repeat of the primary %). Sparkline color uses regular-session move when extended.

| Header | Secondary price | Secondary % |
|--------|-----------------|-------------|
| **PRE** | Last completed regular close (usually yesterday) | That regular session’s day move |
| **LIVE** | Yesterday’s regular close | Yesterday vs T-2 (`previous_day_change_percent`) |
| **POST** | Today’s official regular close | Today’s regular-session % |
| **CLOSED** | Last completed regular close (Friday if weekend) | That regular session’s day move |

LIVE cannot put “today’s close” on the secondary row — the regular session is still open — so the row steps back one trading day. Prices ≥ $1 show two decimals (e.g. `339.96`, not `340.0`).

## Commands (selected)

| Command | Role |
|---------|------|
| `add_symbol` / `remove_symbol` / `remove_symbols` | Watchlist mutations |
| `set_card_tint` | Persist pastel row highlight |
| `reorder_symbols` | DnD order |
| `set_opacity` / `set_autostart` | Settings |
| `set_quote_refresh_ms` / `set_quote_refresh_secs` | Persist + apply scheduler interval (**ms**; `_secs` is the historical IPC name) |
| `set_column_ratios` | Persist proportional column widths |
| `set_content_min_size` | OS min from UI content measure; optional grow if content grew |
| `search_symbols` | Yahoo autocomplete (+ local fallback in UI) |
| `check_for_updates` | Manual updater path (install + restart) |
| `get_diagnostics` / `hide_widget` / `quit_app` | Ops |

## Events (Rust → UI)

| Event | Payload |
|-------|---------|
| `watchlist-updated` | Ordered watchlist items (includes `card_tint`) |
| `quotes-updated` | Quote list from cache |
| `sparklines-updated` | Sparkline list from cache |
| `opacity-updated` | Clamped opacity (CSS; Tauri has no native set_opacity) |

## UI interaction notes

- **Click** selects a card; **Ctrl/Cmd+click** toggles; **Shift+click** range-selects.  
- **Delete / Backspace** removes selection (not while typing in the add input).  
- **Right-click** opens menu: pastel tint + **Remove**.  
- Drag-reorder starts after a small pointer movement threshold so clicks stay clicks.  
- Column edges between symbol|spark and spark|metrics are draggable; ratios persist.  
- Sparkline 1s UI ticker pauses when `document.hidden`.  
- Quote DOM updates skip unchanged cells (quieter UI).  
- **Window min height** follows content (add/remove cards); near the floor, frameless Windows may show slight resize jitter (accepted limitation).  
- List scrolls only when content truly overflows (not at content-hug min).  
- **Tray:** left-click toggles visibility; menu Show / Hide / Quit. Window has **no taskbar button**.

## Extending markets

1. Implement `MarketDataProvider` for the new source.  
2. Register / select provider in setup (today: Yahoo only).  
3. Keep `AssetKind` and UI row model market-agnostic.

## Related docs

- **Windows setup:** [windows-dev.md](./windows-dev.md)  
- Product decisions: [superpowers/specs/2026-07-22-economy-war-room-design.md](./superpowers/specs/2026-07-22-economy-war-room-design.md)  
- Implementation history: [superpowers/plans/2026-07-22-economy-war-room-mvp.md](./superpowers/plans/2026-07-22-economy-war-room-mvp.md)  
- Testing: [testing.md](./testing.md)  
- Backlog: [TODO.md](./TODO.md)  

