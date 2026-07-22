# Architecture (as implemented)

**Updated:** 2026-07-22  
**Branch of truth:** `main` (MVP)

This document describes the **current codebase**, not only the original design sketch.

## Runtime

```
┌─────────────────────────────────────────────┐
│  Web UI  src/                               │
│  · glass panel, rows, SVG sparklines        │
│  · DnD reorder, bottom +, hide, settings    │
└──────────────────┬──────────────────────────┘
                   │ invoke / listen (events)
┌──────────────────▼──────────────────────────┐
│  commands.rs  (thin Tauri adapters)        │
│  lib.rs        (setup: plugins, hotkey,     │
│                 tick loop, manage state)    │
└──────────────────┬──────────────────────────┘
                   │
┌──────────────────▼──────────────────────────┐
│  AppCore  application/service.rs            │
│  · watchlist CRUD + persist                 │
│  · theme / opacity / geometry               │
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
```

## Source layout

### Rust (`src-tauri/src/`)

| Path | Role |
|------|------|
| `domain/` | Types, policy constants, watchlist pure logic, sparkline downsample |
| `ports/market_data.rs` | `MarketDataProvider` + `ProviderLimits` |
| `application/cache.rs` | In-memory quote / sparkline caches |
| `application/queue.rs` | `RateLimitedQueue` (max concurrent, key coalesce, priority) |
| `application/scheduler.rs` | Round-robin batch pick, backoff, sparkline cadence |
| `application/service.rs` | **`AppCore`** — testable app use cases |
| `infrastructure/yahoo/` | `YahooProvider` (mockable base URL), chart parse |
| `infrastructure/store.rs` | Load/save `economy-war-room-state.json` |
| `infrastructure/window_ctl.rs` | Show/hide/geometry/opacity emit (OS / Tauri) |
| `commands.rs` | `#[tauri::command]` handlers |
| `state.rs` | `AppHandleState { core: Arc<AppCore> }` |
| `lib.rs` | Tauri `run()`, autostart, global shortcut, tick loop |

### Web (`src/`)

| Path | Role |
|------|------|
| `ui/app.ts` | Boot, state, geometry save, theme |
| `ui/header.ts` | Drag region, hide, settings toggle |
| `ui/watchlist.ts` | Rows, DnD, add/remove |
| `ui/sparkline.ts` | SVG path helper |
| `ui/settings-panel.ts` | Theme, opacity, quit |
| `ui/types.ts` | TS mirrors of Rust DTOs (snake_case) |
| `styles/tokens.css`, `app.css` | Glass / theme tokens |

### Tests

| Path | Role |
|------|------|
| `src/**` `#[cfg(test)]` | Unit tests (~51) |
| `tests/integration_e2e.rs` | Store + AppCore + scheduler + Yahoo mock HTTP |
| `tests/risk_scenarios.rs` | Rate limit, hide, corrupt JSON, invalid input |

## Key policies (constants)

Defined in `domain/constants.rs` (names approximate):

| Policy | Defaults (MVP) |
|--------|----------------|
| Tick | 1s |
| Batch size | 4 |
| Min quote interval | 10s |
| Max concurrent (provider hint) | 3 |
| Sparkline | range `1d`, interval `5m`, target points 32; min refresh ~300s |
| Backoff | 5s initial → double up to 120s |
| Opacity | 0.35–1.0, default ~0.92 |
| Window | default 320×640, min 260×360 |
| Hotkey | `Ctrl+Shift+Space` |

## Events (Rust → UI)

| Event | Payload |
|-------|---------|
| `watchlist-updated` | Ordered watchlist items |
| `quotes-updated` | Quote list from cache |
| `sparklines-updated` | Sparkline list from cache |
| `opacity-updated` | Clamped opacity (CSS; Tauri has no native set_opacity) |

## Extending markets

1. Implement `MarketDataProvider` for the new source.  
2. Register / select provider in setup (today: Yahoo only).  
3. Keep `AssetKind` and UI row model market-agnostic.

## Related docs

- Product decisions: [superpowers/specs/2026-07-22-economy-war-room-design.md](./superpowers/specs/2026-07-22-economy-war-room-design.md)  
- Implementation history: [superpowers/plans/2026-07-22-economy-war-room-mvp.md](./superpowers/plans/2026-07-22-economy-war-room-mvp.md)  
- Testing: [testing.md](./testing.md)  
