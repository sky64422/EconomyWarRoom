# EconomyWarRoom

Lightweight **Windows floating widget** for watching markets at a glance — not a portfolio manager.

Add US stocks and crypto to a tall glass panel, see **sparklines**, **price**, and **change %**, reorder by drag-and-drop, and toggle visibility with a hotkey or an in-widget hide button.

> Contrast: [AssetStocker](../AssetStocker) is a full local-first finance app (holdings, imports, snapshots). EconomyWarRoom intentionally stays small: **watchlist + quotes only**.

## Product snapshot

| | |
|--|--|
| **Shape** | Tall floating panel, always on top, freely draggable |
| **MVP assets** | US equities + crypto (providers extensible later) |
| **Rows** | Symbol · intraday sparkline (1d/5m) · price · change % |
| **Watchlist** | Add via bottom **+** (appends) · remove · drag reorder |
| **Toggle** | `Ctrl+Shift+Space` **or** in-UI hide (hide only; app stays running) |
| **Look** | Light / dark / system · translucent **glass** · adjustable opacity |
| **Startup** | Autostart on login · widget visible on launch |
| **Stack** | [Tauri](https://tauri.app/) — Rust core + web UI |
| **Data** | Free public market APIs with a **rate-limited scheduler/queue** |

## Status

**MVP implementation on branch `feat/mvp-widget`.** Core widget, Yahoo-backed quotes/sparklines, glass UI, hotkey/hide, settings, and persistence are in place. Manual OS-level checks (sustained run, autostart on Windows) remain.

| Document | Purpose |
|----------|---------|
| [Design spec](docs/superpowers/specs/2026-07-22-economy-war-room-design.md) | Goals, architecture, scheduler, UI, non-goals |
| [TODO](docs/TODO.md) | Phased checklist (P0–P5 progress) |
| [MVP plan](docs/superpowers/plans/2026-07-22-economy-war-room-mvp.md) | Task breakdown (Tasks 1–14) |
| [Plans index](docs/superpowers/plans/) | Plan status |

## Architecture (short)

```
Web UI  ←→  Tauri  ←→  Rust
  glass list          window, hotkey, autostart
  DnD / + / hide      JSON settings + watchlist
                      QuoteScheduler + RateLimitedQueue
                      MarketDataProvider (Yahoo-first, …)
```

- **Domain / ports / application / infrastructure** separation; shared constants for refresh and sparkline policy.
- Network and rate limiting live in **Rust** (avoid webview CORS and centralize quotas).
- Persistence: **JSON only** for MVP (no SQLite).

Design details, scheduling policy, and AssetStocker borrow list: see the [design spec](docs/superpowers/specs/2026-07-22-economy-war-room-design.md).

## Non-goals

- Portfolio, P&L, trades, broker sync  
- Heavy historical DB / snapshot backfill  
- Windows 11 Widgets board  
- API-key-required feeds as MVP hard dependency  

## Develop

**Requirements:**

- **Rust** stable (`rustc`, `cargo`) — install via [rustup](https://rustup.rs/)
- **Node.js 18+** and npm
- **Tauri 2** system deps for your OS — see [Tauri prerequisites](https://tauri.app/start/prerequisites/)
  - **Linux:** WebKitGTK, etc. (e.g. `webkit2gtk`, `libgtk-3`, `librsvg`, build tools)
  - **Windows:** Microsoft Edge WebView2 (usually preinstalled on Win10/11)

**Primary target is Windows** (hotkey, transparent glass, always-on-top, autostart). Linux is fine for development; transparent windows and global hotkeys may be limited depending on compositor / Wayland.

```bash
npm install
npm run tauri dev
```

Frontend-only (no native shell):

```bash
npm run dev
```

Tests (unit + integration + risk scenarios; no live Yahoo required):

```bash
npm test
npm run test:coverage   # tarpaulin ≥ 85% business logic (~98% currently)
```

Details: [docs/testing.md](docs/testing.md)

Frontend typecheck + Vite build:

```bash
npm run build
```

### Controls

| Action | How |
|--------|-----|
| Toggle visibility | **`Ctrl+Shift+Space`** (global hotkey) |
| Hide widget | Header **hide** button (app keeps running; polling pauses) |
| Quit | **Settings → Quit** (hide alone does not exit) |
| Theme / opacity | Settings panel (light / dark / system; opacity slider) |
| Watchlist | Bottom **+** to add · drag to reorder · per-row remove |

Default seed watchlist: **AAPL**, **BTC-USD**.

## Configuration

Stored as JSON under the OS app data directory (Tauri `app_data_dir`), including:

- Watchlist symbols and order  
- Theme (`light` | `dark` | `system`)  
- Opacity  
- Window geometry  
- Autostart flag  
- Hotkey (default `Ctrl+Shift+Space`)  

## Design references

- UI motion/materials: [apple-design skill](https://github.com/emilkowalski/skills/tree/main/skills/apple-design)  
- Market-data patterns (ideas only): local **AssetStocker** (`YahooFinanceClient`, rate limiter, job queue, sparkline policy)  

## License

TBD.
