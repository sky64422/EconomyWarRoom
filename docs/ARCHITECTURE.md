# Architecture (as implemented)

**Updated:** 2026-07-23 (v0.1.7)  
**Branch of truth:** `main`

This document describes the **current codebase**, not only the original design sketch.

## Runtime

```
┌─────────────────────────────────────────────┐
│  Web UI  src/                               │
│  · glass panel, rows, SVG sparklines        │
│  · DnD reorder, select / multi-select       │
│  · pastel card tints, bottom +, hide        │
│  · settings: theme, opacity, refresh, login │
└──────────────────┬──────────────────────────┘
                   │ invoke / listen (events)
┌──────────────────▼──────────────────────────┐
│  commands.rs  (thin Tauri adapters)        │
│  lib.rs        (setup: plugins, hotkey,     │
│                 tick loop, updater, state)  │
└──────────────────┬──────────────────────────┘
                   │
┌──────────────────▼──────────────────────────┐
│  AppCore  application/service.rs            │
│  · watchlist CRUD + card_tint + persist     │
│  · theme / opacity / geometry / autostart   │
│  · quote_refresh_secs → scheduler           │
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
```

## Source layout

### Rust (`src-tauri/src/`)

| Path | Role |
|------|------|
| `domain/` | Types (`WatchlistItem`, `CardTint`, `AppSettings`), policy constants, watchlist pure logic, sparkline downsample |
| `ports/market_data.rs` | `MarketDataProvider` + `ProviderLimits` |
| `application/cache.rs` | In-memory quote / sparkline caches |
| `application/queue.rs` | `RateLimitedQueue` (max concurrent, key coalesce, priority) |
| `application/scheduler.rs` | Round-robin batch pick, configurable min quote interval, backoff, sparkline cadence |
| `application/service.rs` | **`AppCore`** — testable app use cases |
| `infrastructure/yahoo/` | `YahooProvider` (mockable base URL), chart parse |
| `infrastructure/store.rs` | Load/save `economy-war-room-state.json` |
| `infrastructure/window_ctl.rs` | Show/hide/geometry/opacity; **content min-size** (physical) + resize clamp |
| `infrastructure/updater.rs` | Startup auto-check + manual install path |
| `commands.rs` | `#[tauri::command]` handlers (incl. `set_content_min_size`) |
| `state.rs` | `AppHandleState { core, content_min_w/h }` |
| `lib.rs` | Tauri `run()`, autostart, hotkey, tick loop, updater, **on_window_event Resized** |

### Web (`src/`)

| Path | Role |
|------|------|
| `ui/app.ts` | Boot, state, geometry persist, **content-hug min** measure → `set_content_min_size` |
| `ui/header.ts` | Drag region, update check, settings, hide |
| `ui/watchlist.ts` | Rows (symbol · spark · price), multi-select, DnD, tint menu, add/remove |
| `ui/sparkline.ts` | SVG path helper |
| `ui/settings-panel.ts` | Theme, opacity, price refresh, launch-at-login, diagnostics, quit |
| `ui/types.ts` | TS mirrors of Rust DTOs (snake_case) |
| `styles/tokens.css`, `app.css` | Glass / theme / pastel tint tokens |

### Tests

| Path | Role |
|------|------|
| `src/**` `#[cfg(test)]` | Unit tests (~63) |
| `tests/integration_e2e.rs` | Store + AppCore + scheduler + Yahoo mock HTTP |
| `tests/risk_scenarios.rs` | Rate limit, hide, corrupt JSON, invalid input |

## Key policies (constants)

Defined in `domain/constants.rs` (names approximate):

| Policy | Defaults |
|--------|----------|
| Tick | 1s |
| Batch size | 4 |
| Quote refresh | **5–120s** user setting (default **10s**); scheduler uses `min_quote_interval` |
| Max concurrent (provider hint) | 3 |
| Sparkline | range `1d`, interval `5m`, target points 32; min refresh ~300s |
| Backoff | 5s initial → double up to 120s |
| Opacity | 0.35–1.0, default ~0.92 |
| Window | default 320×640; policy floor 260×120; **runtime min = measured content** (OS physical min + Resized clamp) |
| Hotkey | `Ctrl+Shift+Space` |
| Card tints | `none`, `rose`, `peach`, `mint`, `sky`, `lavender`, `lemon` |
| Card layout | Left→right: **symbol · sparkline · price/change**; remove **x** on hover |

## Commands (selected)

| Command | Role |
|---------|------|
| `add_symbol` / `remove_symbol` / `remove_symbols` | Watchlist mutations |
| `set_card_tint` | Persist pastel row highlight |
| `reorder_symbols` | DnD order |
| `set_theme` / `set_opacity` / `set_autostart` | Settings |
| `set_quote_refresh_secs` | Persist + apply scheduler interval |
| `set_content_min_size` | OS min from UI content measure; optional grow if content grew |
| `check_for_updates` | Manual updater path |
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
- **Right-click** opens pastel tint menu.  
- Drag-reorder starts after a small pointer movement threshold so clicks stay clicks.  
- Sparkline 1s UI ticker pauses when `document.hidden`.  
- **Window min height** follows content (add/remove cards); near the floor, frameless Windows may show slight resize jitter (accepted limitation).  
- List scrolls only when content truly overflows (not at content-hug min).

## Extending markets

1. Implement `MarketDataProvider` for the new source.  
2. Register / select provider in setup (today: Yahoo only).  
3. Keep `AssetKind` and UI row model market-agnostic.

## Related docs

- **Session sync-up:** [HANDOFF.md](./HANDOFF.md)  
- **Windows setup:** [windows-dev.md](./windows-dev.md)  
- Product decisions: [superpowers/specs/2026-07-22-economy-war-room-design.md](./superpowers/specs/2026-07-22-economy-war-room-design.md)  
- Implementation history: [superpowers/plans/2026-07-22-economy-war-room-mvp.md](./superpowers/plans/2026-07-22-economy-war-room-mvp.md)  
- Testing: [testing.md](./testing.md)  
