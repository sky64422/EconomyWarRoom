# EconomyWarRoom

Lightweight **Windows floating widget** for watching markets at a glance — not a portfolio manager.

Add US stocks and crypto to a tall glass panel, see **sparklines**, **price**, and **change %**, reorder by drag-and-drop, highlight cards with soft colors, and toggle visibility with a hotkey or an in-widget hide button.

> Contrast: [AssetStocker](../AssetStocker) is a full local-first finance app (holdings, imports, snapshots). EconomyWarRoom intentionally stays small: **watchlist + quotes only**.

## Product snapshot

| | |
|--|--|
| **Shape** | Tall floating panel, always on top, freely draggable (content-hug min height) |
| **MVP assets** | US equities + crypto (providers extensible later) |
| **Rows** | Symbol · intraday sparkline (1d/5m) · price · change % |
| **Watchlist** | Add via bottom **+** · remove · drag reorder · multi-select · pastel tints |
| **Toggle** | `Ctrl+Shift+Space` **or** in-UI hide (hide only; app stays running) |
| **Look** | Light / dark / system · translucent **glass** · adjustable opacity |
| **Settings** | Theme · opacity · **price refresh** · **launch at login** · diagnostics · quit |
| **Updates** | In-app updater (header ↻ + release auto-check) |
| **Startup** | Autostart on login (toggleable) · widget visible on launch |
| **Stack** | [Tauri](https://tauri.app/) 2 — Rust core + vanilla TypeScript / Vite UI |
| **Data** | Free Yahoo-style chart API + **rate-limited scheduler** (backoff on 429) |

## Status

**MVP + post-MVP UX on `main`.** Design, implementation plan (Tasks 1–14), unit/integration/risk tests, and ≥85% business-logic coverage gate are in place.

| Area | State |
|------|--------|
| Core widget + glass UI | Done |
| Yahoo quotes / sparklines + scheduler | Done |
| Hotkey / hide / settings / JSON persist | Done |
| Card tint · multi-select · quote interval · autostart UI · updater | Done |
| Automated tests + coverage gate | Done (~98% business logic; ~63 unit tests) |
| Manual OS smoke (Windows long run) | Still recommended — see [TODO](docs/TODO.md) P5-2 / P5-3 |

### Continuing on a new machine (especially Windows)

| Start here | Purpose |
|------------|---------|
| **[docs/HANDOFF.md](docs/HANDOFF.md)** | **Read first** — project sync-up for humans & AI agents |
| [docs/windows-dev.md](docs/windows-dev.md) | Windows prerequisites, first run, troubleshooting |
| [AGENTS.md](AGENTS.md) | Short rules for coding agents |

```powershell
git clone https://github.com/sky64422/EconomyWarRoom.git
cd EconomyWarRoom
npm install
npm run run:exe
```

| Document | Purpose |
|----------|---------|
| [Architecture](docs/ARCHITECTURE.md) | Current module layout and data flow |
| [Design spec](docs/superpowers/specs/2026-07-22-economy-war-room-design.md) | Goals, decisions, non-goals |
| [MVP plan](docs/superpowers/plans/2026-07-22-economy-war-room-mvp.md) | Implementation task breakdown (complete) |
| [TODO](docs/TODO.md) | Phase checklist + remaining manual smoke |
| [Testing](docs/testing.md) | Unit / integration / risk / coverage policy |

## Architecture (short)

```
Web UI (src/)          Tauri bridge              Rust (src-tauri/)
  glass list      ←→   commands / events   ←→   AppCore service
  DnD / select / +      invoke + emit            QuoteScheduler + queue
  theme / opacity                                MarketDataProvider (Yahoo)
  refresh / login                                JSON store (app data dir)
  update icon                                    updater plugin
```

- **Layers:** `domain` → `ports` → `application` (scheduler, queue, **AppCore**) → `infrastructure` (Yahoo, store, window, updater).
- Commands are thin adapters; business logic lives in `AppCore` (unit-tested).
- Network and rate limits run in **Rust** (no webview CORS).
- Persistence: **one JSON file** under Tauri `app_data_dir` (no SQLite).

More detail: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Non-goals

- Portfolio, P&L, trades, broker sync  
- Heavy historical DB / snapshot backfill  
- Windows 11 Widgets board  
- API-key-required feeds as MVP hard dependency  

## Develop

**Requirements:**

- **Rust** stable (`rustc`, `cargo`) — [rustup](https://rustup.rs/)
- **Node.js 18+** and npm
- **Tauri 2** OS deps — [prerequisites](https://tauri.app/start/prerequisites/)
  - **Linux:** WebKitGTK, GTK, etc.
  - **Windows:** Edge WebView2 (usually preinstalled)

**Primary target is Windows** (hotkey, glass, always-on-top, autostart). Linux works for development; transparent windows and global hotkeys may vary by compositor / Wayland.

```bash
npm install
npm run run:exe
```

Frontend-only (no native shell; `invoke` will fail outside Tauri):

```bash
npm run dev
```

Development shell with hot reload:

```bash
npm run tauri dev
```

### Tests & coverage

```bash
npm test                 # unit + integration_e2e + risk_scenarios
npm run test:coverage    # tarpaulin ≥ 85% business logic (currently ~98%)
npm run build            # tsc + Vite production bundle
```

See [docs/testing.md](docs/testing.md).

### Updates

The app checks for updates at startup (release builds) through Tauri's updater plugin.
Manual check: header **↻** icon. To publish an installable release, build with a signing
key and upload the generated updater manifest/artifacts to the configured release endpoint.

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY_PATH="C:\path\to\economy-war-room.key"
npm run run:exe
```

### Controls

| Action | How |
|--------|-----|
| Toggle visibility | **`Ctrl+Shift+Space`** (global hotkey) |
| Hide widget | Header **hide** button (process keeps running; polling pauses) |
| Check for updates | Header **↻** (left of settings) |
| Quit | **Settings → Quit** (hide alone does not exit) |
| Theme / opacity / refresh / login | Settings panel |
| Select cards | Click · **Ctrl** toggle · **Shift** range |
| Delete selected | **Delete** or **Backspace** |
| Card color | Right-click card → pastel swatch |
| Watchlist | Bottom **+** · drag to reorder · per-row **x** |

Default seed watchlist: **AAPL**, **BTC-USD**.

## Configuration

JSON under the OS app data directory (Tauri `app_data_dir`), file name roughly `economy-war-room-state.json`:

- Watchlist symbols, order, and **card_tint**  
- Theme (`light` \| `dark` \| `system`)  
- Opacity (clamped ~0.35–1.0)  
- Window geometry  
- **Autostart** flag (launch at login)  
- **quote_refresh_secs** (5–120, default 10)  
- Hotkey string (default `Ctrl+Shift+Space`)  

## Design references

- UI motion/materials: [apple-design skill](https://github.com/emilkowalski/skills/tree/main/skills/apple-design)  
- Market-data patterns (ideas only): **AssetStocker** rate limiter, job queue, Yahoo chart + sparkline policy  

## License

TBD.
