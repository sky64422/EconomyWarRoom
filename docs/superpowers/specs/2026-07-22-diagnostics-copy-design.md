# Design: Copy diagnostics (Mode B)

**Status:** approved for implementation  
**Date:** 2026-07-22  
**Product:** EconomyWarRoom floating watchlist widget

## Problem

Two defect-analysis modes:

| Mode | Who runs the app | How the agent sees failures |
|------|------------------|-----------------------------|
| **A** | Agent via `npm run tauri dev` | Live process stdout/stderr |
| **B** | User via release/dev exe | Needs a pasteable diagnostics dump |

Mode B has no dump path today. Sparse `eprintln!` / `console.error` are not user-facing.

## Goals

1. Settings → **Copy diagnostics** copies a text report to the clipboard.
2. Report is enough for an agent to diagnose soft failures (settings, watchlist, quote cache, recent in-process events).
3. Business logic (ring buffer + formatting) is unit-tested; commands stay thin.

## Non-goals

- Rolling file logs / post-crash recovery (future)
- Telemetry, network upload, OS minidumps
- Capturing WebView pixel UI
- Portfolio or P&L data

## UX

- Button **Copy diagnostics** above **Quit** in the settings panel.
- Success: label briefly shows `Copied`.
- Failure: label shows `Failed`; error also goes to the event ring when possible.

## Report format

Plain text (markdown-friendly):

```text
### EWR diagnostics
- captured_at: <ISO-8601 UTC>
- app_version: <Cargo package version>
- os: <std::env::consts::OS / ARCH>
- visible: true|false
- app_data_dir: <path>
- settings: theme, opacity, autostart, hotkey, window {x,y,w,h}
- watchlist: ordered lines `sort_index symbol kind id`
- quotes: per cached quote `symbol price change% as_of` or `(none)`
- scheduler: visible-flag, backoff_active, last_error if any
- recent_events: up to 50 lines `timestamp level message` (oldest→newest)
```

No API keys exist in MVP; watchlist symbols are included intentionally.

## Architecture

```
commands / lib setup / AppCore ops
        │
        ▼
  EventRing (capacity 100)  ──┐
                              ├── format_diagnostics() → String
  PersistedState + quotes ────┘
        │
        ▼
  get_diagnostics command → UI clipboard.writeText
```

| Piece | Location |
|-------|----------|
| `EventRing`, `DiagLevel` | `application/diagnostics.rs` |
| Ring + `note` + `format_diagnostics` | `AppCore` |
| Last scheduler error string | `QuoteScheduler` fields, set on fetch Err |
| `get_diagnostics` | `commands.rs` (thin) |
| Button | `settings-panel.ts` + CSS |

## Clipboard

1. Primary: WebView `navigator.clipboard.writeText` after `invoke("get_diagnostics")`.
2. Fallback: temporary `<textarea>` + `document.execCommand('copy')` if clipboard API fails.

No new Tauri clipboard plugin in v1.

## Event sources (minimal)

- App start (info)
- Autostart / hotkey register failures (warn/error) — from setup after state is managed
- Command failures that already surface to the user (add_symbol, etc.) — note on Err in UI or command layer
- Scheduler quote/sparkline errors → store `last_error` + optional ring note from core if wired later

Avoid per-tick success spam.

## Testing

- Ring: empty, push order, capacity drop (FIFO)
- `format_diagnostics` contains version, settings keys, watchlist symbols, ring lines
- Existing suites remain green

## Docs

Update `windows-dev.md` §10: Mode A vs Mode B + Copy diagnostics usage.

## Success criteria

1. One click produces a clipboard string matching the format above.
2. Agent can act on Mode B pastes without live process access.
3. Coverage gate still ≥85% on business logic.
