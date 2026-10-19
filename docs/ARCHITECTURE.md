# Architecture

Updated: 2026-10-17. Version 0.2.0 includes the TokenUsage UI adoption.

```text
Edge notch: instrument icon + primary change
  ├─ click → single-instrument detail: symbol | sparkline | primary/secondary rows
  └─ context menu → appearance / instruments / general settings
                   │ invoke / events
Thin Tauri commands│
                   ▼
AppCore ── QuoteScheduler + RateLimitedQueue ── YahooProvider
  └─ atomic preference/watchlist persistence to existing JSON store

domain/notch + reveal ── infrastructure/notch_window ── fixed transparent HWND
Glass / Liquid -> Windows Composition; narrow Liquid contrast probes (250ms)
```

## Rust modules

| Path under `src-tauri/src` | Responsibility |
|---|---|
| `domain/types.rs`, `constants.rs` | Existing market DTOs, settings, refresh/opacity/column policies; theme defaults |
| `domain/display.rs` | Existing primary/secondary rows; remains the sole quote display calculation |
| `domain/watchlist.rs`, `sparkline_math.rs` | Existing watchlist and chart rules |
| `domain/notch.rs`, `notch/drag.rs` | Physical geometry, edge selection, scaling, hit regions, dynamic cell capacity, capture bounds |
| `domain/reveal.rs` | Fold/reveal timing and physical edge target; accepts supplied time |
| `application/service.rs` | AppCore use cases; cloned state is saved before committing in memory |
| `application/scheduler.rs`, `queue.rs`, `cache.rs` | Existing rate-limited quote/sparkline pipeline and cache |
| `application/diagnostics.rs` | Existing bounded diagnostics and error throttling |
| `infrastructure/notch_window.rs` | Native layout, pointer/input polling, drag session and monitor reconciliation |
| `infrastructure/liquid_glass.rs`, `liquid_capture.rs` | Ordered material sessions; `native_material.rs` owns same-HWND backdrop visuals. Dedicated reusable GDI surfaces serve only narrow contrast probes and diagnostics |
| `infrastructure/topmost.rs` | Reasserts the widget's topmost band without activating it |
| `infrastructure/window_ctl.rs` | Show/hide, transparent edge; legacy geometry adapters retained |
| `infrastructure/store.rs`, `yahoo/`, `updater.rs` | Existing JSON, market provider and signed updater |
| `commands.rs`, `lib.rs`, `state.rs` | Thin adapters, setup/tray/tick wiring and managed state |

The former content-hug observer is no longer mounted, and the native Resized clamp is no longer active. Rust owns physical geometry. DWM rounding is disabled for the custom SVG silhouette.

## Frontend modules

| Path under `src` | Responsibility |
|---|---|
| `ui/app.ts` | Snapshot/event subscriptions, shell lifecycle, selected card/settings composition |
| `ui/notch.ts`, `notch-state.ts` | Instrument icons, primary percentages, overflow, mouse pin/hover state |
| `ui/notch-drag.ts`, `notch-surface.ts` | Gesture sessions; serialized geometry requests and stale-reply suppression |
| `ui/stock-detail.ts` | One instrument in a two-tier grid, reusing existing quote metrics and sparkline policies |
| `ui/watchlist/metrics.ts`, `format.ts`, `columns.ts`, `ui/sparkline.ts` | Shared existing quote presentation and chart policies |
| `ui/settings-panel.ts` | Three tabs, shared intrinsic height, appearance saves and instrument management |
| `ui/general-settings.ts` | Existing refresh/autostart/updater/diagnostics/quit controls, embedded in General |
| `ui/watchlist/index.ts`, `dnd.ts`, `search.ts`, `tint.ts` | Existing watchlist CRUD/selection/reorder/tint; management rows in Instruments |
| `ui/liquid-glass.ts`, `liquid-contrast.ts`, `liquid-frame.ts` | Ordered native geometry, clear tint and 250ms serialized margin contrast probes |
| `ui/stock-marks.ts`, `assets/marks/` | Bundled instrument icon registry and ticker fallback |
| `styles/notch.css`, `notch-widget.css` | Adopted silhouette/material and market-specific cards/settings |

The app uses vanilla TS/CSS/SVG/canvas, with no new product package dependency.

## Data contracts

- **Notch = `Quote.display.primary.change`**. Missing/nonfinite primary is unknown; explicit zero stays zero. Do not substitute secondary/regular percentages.
- Detail top/bottom rows remain `display.primary` / `display.secondary`. The existing `resolve_price_rows` calculation remains unchanged.
- During LIVE the second row describes the previous completed regular session. Extended sessions retain their existing primary/secondary distinction.
- Equity sparklines remain RTH-only, with the existing prior regular window during PRE and the current regular window during LIVE. Their regular-session color can differ from an extended primary percentage.
- Settings offer 1s / 10s / 60s. Existing custom/legacy intervals remain unchanged on opening; they are not shown as a different selected preset. Rust's 500ms default and legacy bounds remain compatible. `quote_refresh_secs` is still the persisted millisecond key, including legacy seconds normalization.

## Placement and lifecycle contracts

- Default depth 56 DIP; vertical/horizontal cells 72/96 DIP; ends 44 DIP; detail/settings width 260 DIP and maximum height 560 DIP, matching TokenUsage. Monitor content scale is 0.92–1.0, separate from physical Windows DPI.
- At most six cells fit; native geometry reduces capacity for small work areas. All slots show instruments until overflow requires a final list cell. A one-slot notch shows the single instrument, or a list button when overflowing. Addition is available only in settings; an empty list keeps a right-clickable notch surface. All watchlist instruments stay in the scheduler.
- Reserve a stable transparent canvas for details/settings. Changing selected card or tab does not move the HWND/notch origin. Only visible surfaces/bridge take pointer input.
- Default click pin, optional hover detail, 180ms leave grace. Fold after 800ms outside unless held by detail/settings/drag/always-show. Custom keyboard handlers and global shortcuts are not installed.
- **Fold:** 10×80 DIP edge target remains; quotes continue, Glass/Liquid materials and contrast probes stop.
- **Explicit hide:** core visibility false; new quote polling and Glass/Liquid materials and contrast probes stop. `widget-visibility` explicitly informs the renderer because WebView2 does not reliably change `document.hidden` on hide.
- **Show:** existing scheduler resumes; native materials and contrast probes can resume. **Quit** terminates the process.
- Glass and Liquid use same-HWND compositor visuals, with ordered session/revision updates and no capture exclusion. Liquid contrast reads only empty three-pixel-wide margins, at most four cycles per second after completion, with separate rail/detail modes and hysteresis. Hidden/Dark requests are rejected; stale contrast results are discarded. Geometry and capture bounds remain validated.

## Persistence

Existing watchlist IDs/order/tints, window geometry, column ratios, opacity, hotkey, autostart and refresh fields are retained. New fields: `theme`, `notch`, `hover_detail`, `always_show_notch`, `show_animation`, with defaults dark/right-centre/false/false/true.

Legacy column ratios and the inactive hotkey field remain persisted for compatibility; the current detail uses an intrinsic identity/price row above a full-width chart. Instrument rings stay neutral; notch cells have no add/remove actions. The Symbols tab uses flat rows and mouse reorder, with always-visible search; no count, search-close control, detail-navigation button, Ctrl/Shift selection, Delete or implicit Enter submission remains. Row pointer release opens the detail unless a drag or cancellation occurred. The trailing trash button stops pointer propagation and owns removal, pending/error/retry feedback and duplicate-request suppression.

Missing placement monitor is initialized from the old window's logical coordinates and the current monitor DPI, with a primary-monitor fallback. Explicit placement is retained. Preference/placement saves clone, validate, persist, then commit; a failed write leaves memory and UI at the committed value.

Settings measure all three tabs, including inactive ones, using actual header/tab/gap/padding/border sizes. Active-tab selection does not control panel height. Monitor/content changes can recompute height. Normal contents fit without scrolling; constrained work areas or long lists scroll the body without visible scrollbars or reserved gutter. The body therefore stays horizontally centered with the header and tabs.

Tab buttons match TokenUsage's 37 DIP height. General stretches to the available body height with its two action buttons at the bottom; intrinsic measurement temporarily clears panel min-height/flex growth so the footer cannot feed back into window sizing. Tab spring/fade, switch/opacity and copy/update icon transitions share motion preferences. Four-edge detail reveal uses clip-path/opacity only; its native/DOM rectangles stay fixed throughout. Detail chart projection and session/baseline policy remain in `sparkline.ts`; the detail variant only changes presentation and adds the last sample's exact projected marker.

See [testing.md](testing.md) for evidence and native limitations.
