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

**Design phase complete (spec written).** Implementation not started yet.

| Document | Purpose |
|----------|---------|
| [Design spec](docs/superpowers/specs/2026-07-22-economy-war-room-design.md) | Goals, architecture, scheduler, UI, non-goals |
| [TODO](docs/TODO.md) | Phased checklist toward MVP |
| [Plans](docs/superpowers/plans/) | Implementation plans (after planning step) |

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

## Development

Scaffolded as **Tauri 2 + Vite + vanilla TypeScript**. Widget features land in later tasks (Phase 0 in [TODO](docs/TODO.md)).

Expected toolchain:

- Rust stable  
- Node.js (frontend tooling)  
- Tauri CLI 2.x (via `@tauri-apps/cli`)  
- Linux system deps for local GUI dev (`webkit2gtk`, etc.) — see [Tauri prerequisites](https://tauri.app/start/prerequisites/)  
- Windows target for production widget behavior (hotkey, autostart, always-on-top)

```bash
npm install
npm run tauri dev
# or frontend only:
npm run dev
```

## Configuration (planned)

Stored under the OS app data directory (JSON), including:

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
