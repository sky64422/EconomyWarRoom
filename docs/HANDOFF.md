# Session handoff — read this first

**Purpose:** After `git clone` (especially on **Windows**), a human or coding agent should read this file and be able to continue without re-deriving project context.

**Last updated:** 2026-07-23  
**Default branch:** `main`  
**Remote:** `https://github.com/sky64422/EconomyWarRoom.git`

---

## 1. What this product is

**EconomyWarRoom** = lightweight **floating desktop watchlist widget** (not a portfolio app).

- Tall always-on-top glass panel (content-hug height; min size tracks outer chrome)  
- US stocks + crypto (Yahoo chart API)  
- Rows: symbol · sparkline (1d/5m) · price · change %  
- In-widget add (**+** at bottom), remove, drag-reorder  
- Card **pastel tints**, multi-select (click / Ctrl / Shift), **Delete** to remove  
- Hide: UI button **or** `Ctrl+Shift+Space` (process stays alive; polling pauses)  
- Theme light/dark/system, opacity slider, **launch at login** toggle, **quote refresh** interval  
- Header **update check** icon · Tauri updater plugin (signed releases)  
- Stack: **Tauri 2 + Rust + vanilla TS/Vite**

**Not in scope:** portfolio, P&L, trades, SQLite history, broker sync, Finnhub-required realtime, Win11 Widgets board.

Contrast (do not merge architectures): sibling project **AssetStocker** is a full Flutter finance app — borrow *ideas* only.

---

## 2. Where you are (status)

| Area | Status |
|------|--------|
| MVP features | **Done** on `main` |
| Post-MVP UX | Card tint, multi-select, refresh interval, autostart UI, updater — **done** |
| Automated tests | Unit + integration + risk; ~**63** lib + e2e 4 + risk 7 |
| Coverage gate | ≥85% business logic (**~98%** last measured) |
| Windows machine toolchain | **Ready** — Node 24, npm, Rust stable-MSVC, VS Build Tools 2022, WebView2 |
| Diagnostics (Mode B) | **Done** — Settings → Copy diagnostics; command/scheduler notes + 30s throttle |
| Windows runtime smoke | **Still recommended** — `npm run run:exe` + P5-2/P5-3 checklist |
| Open product work | Manual smoke; then remaining Phase 6 (tray, remappable hotkey, file log) |

**Do not re-scaffold** Tauri or re-implement domain/scheduler from the MVP plan unless fixing bugs. Plan file is historical.

---

## 3. Doc map (read order for sync-up)

| Order | File | Why |
|------:|------|-----|
| 1 | **This file** (`docs/HANDOFF.md`) | Orientation + next actions |
| 2 | [windows-dev.md](./windows-dev.md) | Windows install, first run, traps |
| 3 | [ARCHITECTURE.md](./ARCHITECTURE.md) | Modules, data flow, events |
| 4 | [testing.md](./testing.md) | How to test / coverage rules |
| 5 | [TODO.md](./TODO.md) | Remaining checklist |
| 6 | [README.md](../README.md) | User-facing runbook |
| 7 | [release.md](./release.md) | Signed builds, `latest.json`, `npm run release:publish` |
| 8 | [superpowers/specs/2026-07-22-economy-war-room-design.md](./superpowers/specs/2026-07-22-economy-war-room-design.md) | Product decisions / non-goals |
| 9 | [superpowers/plans/2026-07-22-economy-war-room-mvp.md](./superpowers/plans/2026-07-22-economy-war-room-mvp.md) | Historical implementation plan (**complete**) |

---

## 4. First commands on a new machine

### Windows (primary)

See full detail: [windows-dev.md](./windows-dev.md).

```powershell
git clone https://github.com/sky64422/EconomyWarRoom.git
cd EconomyWarRoom
git checkout main
git pull

# Prerequisites already installed (Rust MSVC, Node 18+, WebView2, VS Build Tools)
npm install
npm run run:exe
```

### Tests (any OS with Rust)

```powershell
npm test
# or:
cd src-tauri
cargo test --lib
cargo test --test integration_e2e --test risk_scenarios
```

Coverage (bash / Git Bash / WSL):

```bash
npm run test:coverage
# or: bash scripts/coverage.sh
```

---

## 5. Architecture cheat sheet

```
src/ui/*          →  invoke/events  →  commands.rs (thin)
                                         ↓
                                    AppCore (application/service.rs)
                                         ↓
                         QuoteScheduler ←→ JSON store
                         YahooProvider
                         updater (Tauri plugin)
```

| Concern | Location |
|---------|----------|
| Types / constants / watchlist pure logic | `src-tauri/src/domain/` |
| Provider trait | `src-tauri/src/ports/market_data.rs` |
| Scheduler, queue, **AppCore** | `src-tauri/src/application/` |
| Yahoo HTTP + parse, JSON store, updater | `src-tauri/src/infrastructure/` |
| Tauri commands | `src-tauri/src/commands.rs` |
| Bootstrap (hotkey, autostart, tick loop, updater) | `src-tauri/src/lib.rs` |
| UI | `src/ui/*.ts`, `src/styles/*` |

**Rule:** Put business logic in `AppCore` / domain / scheduler — not in fat Tauri commands. Keep coverage gate green.

### Events (Rust → UI)

`watchlist-updated` · `quotes-updated` · `sparklines-updated` · `opacity-updated`

### Persistence

JSON under OS app data dir, file `economy-war-room-state.json` (via `infrastructure/store.rs`).

Includes: watchlist (`card_tint`, order), theme, opacity, geometry, hotkey, **autostart**, **quote_refresh_secs**.

### Serde / frontend

Rust fields are **snake_case** in JSON; TS types in `src/ui/types.ts` match snake_case.

---

## 6. Engineering conventions

1. **YAGNI** — no portfolio features without a new design approval.  
2. **Rate limits** — never per-symbol `setInterval`; use `QuoteScheduler` + backoff.  
3. **Hide ≠ quit** — hide pauses polling; quit only via Settings → Quit.  
4. **Tests required** for domain/service/scheduler/parse changes; prefer wiremock for HTTP.  
5. **Coverage:** `scripts/coverage.sh` fails under 85% on business logic; excludes `lib.rs`, `commands.rs`, `window_ctl.rs`, `main.rs` (GUI glue).  
6. **Windows is primary UX target**; Linux is fine for logic-only work.  
7. **Git:** prefer feature branches off `main`; don’t rewrite MVP history casually.  
8. **Widget weight** — pause sparkline UI ticker when document is hidden; hide pauses network.

### Suggested commit style

```
feat(scope): ...
fix(scope): ...
test(scope): ...
docs: ...
```

---

## 7. Good next tasks (pick one)

Priority for a **Windows handoff session**:

1. **P5-2 / P5-3 manual smoke** on Windows (`npm run run:exe`) — checklist in [TODO.md](./TODO.md).  
2. While smoking: Settings → **Copy diagnostics** once; header **↻** update check (release builds).  
3. Verify **Launch at login**, card tints, multi-select + Delete, quote refresh presets.  
4. Fix any Windows-only bugs found; update `windows-dev.md` Troubleshooting.  
5. Only then: remaining Phase 6 (remappable hotkey, tray, file log P6-8, etc.).

### Windows host notes (2026-07-22+)

- Repo path: `C:\dev\EconomyWarRoom` (branch usually `main`).  
- First-time setup issues seen: PowerShell blocked `npm.ps1` until `Set-ExecutionPolicy -Scope CurrentUser RemoteSigned`; Rust/VS Build Tools installed via winget.  
- Full agent observability / how to report runtime failures: [windows-dev.md §10](./windows-dev.md#10-defect-reporting--agent-visibility).  
- **Mode B:** Settings → **Copy diagnostics** (clipboard dump). Spec: [diagnostics-copy design](./superpowers/specs/2026-07-22-diagnostics-copy-design.md).

---

## 8. Agent prompt starter (copy-paste)

Use this when opening a new AI coding session on Windows:

```text
You are continuing EconomyWarRoom (Tauri 2 floating market watchlist widget).
Read docs/HANDOFF.md first, then docs/windows-dev.md and docs/ARCHITECTURE.md.
Branch: main. MVP is implemented. Prefer fixing Windows runtime issues or
manual-smoke gaps over re-scaffolding. Business logic goes in AppCore/domain/
scheduler. Run npm test (or cargo tests under src-tauri) before claiming done.
Do not add portfolio/P&L features. Hide must not quit the app.
```

---

## 9. Known caveats

| Topic | Note |
|-------|------|
| Opacity | No native Tauri 2 window alpha; slider sets **glass fill alpha** via CSS `--panel-opacity` (100% = solid / no desktop bleed) |
| Yahoo | Unofficial public endpoints; 429 → backoff; may fail from some networks |
| Hotkey | Best-effort register; may collide with other apps |
| Updater | Auto-check only in release builds; publish with `npm run release:publish` — see [release.md](./release.md) |
| Min window | Content-hug floor (~120px chrome); live `setMinSize` follows panel height so rows/+Add are not clipped |
| Coverage script | Bash; use Git Bash/WSL on Windows or run tarpaulin manually |
| Worktree | Optional `.worktrees/` on Linux dev host; Windows clone is usually a normal `main` checkout |

---

## 10. Definition of “synced”

You are synced when you can answer:

1. Product = floating watchlist widget, not AssetStocker.  
2. Code lives on `main`; AppCore owns business logic.  
3. `npm run run:exe` is the release-style app entry; `npm test` validates logic.  
4. Post-MVP UX (tint, multi-select, refresh, autostart UI, updater) is on `main`.  
5. Next human-valuable work = Windows smoke + bugfix, then remaining TODO Phase 6.  

Then implement only what the user asks, using the doc map above.
