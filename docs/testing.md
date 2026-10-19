## Clear Liquid material adoption (2026-10-19)

Adapted from TokenUsage `db5ecbc`: same-HWND Windows Composition, clear prism tint,
250ms serialized foreground probes, separate rail/detail contrast and market colors.
`npm test` includes contrast hysteresis and pending-capture cancellation checks.
Run `python scripts/liquid-cadence-smoke.py --port 1521` and
`python scripts/liquid-renderer-smoke.py --port 1521` for geometry lifecycle checks.
Run `python scripts/liquid-contrast-native-smoke.py` against preview CDP 9251 for
blue/white/blue transitions on every monitor and screenshot inclusion (affinity 0).
The native probe restores theme and placement. `notch-browser-smoke.py` checks
unchanged quote/settings behavior; `notch-native-smoke.py` covers four edges and themes.
Verified: 13 UI tests, 143 Rust unit tests, 4 integration, 9 risk, 28 notch and
2 frame tests; production build; browser material lifecycle and market UI smokes;
native four-edge settings heights (434 DIP), focus/click-through/fold, all themes,
and six blue/white/blue checks across two monitors with capture affinity 0.

Older capture-affinity/renderer evidence below describes the previous implementation.

# Testing and coverage

Updated: 2026-10-17. Tests cover the version 0.2.0 notch UI source.

## Theme-switch geometry regression (2026-10-17)

`python scripts/theme-transition-smoke.py --port 1420` runs the real app with mocked IPC/capture and a virtual clock. It opens a short detail then settings, records the first and repeated transitions across all six directed theme pairs, holds persistence pending for 64 ms, and rejects a save before immediately switching tabs. Cases cover DPR 1/2 and content scale 1/0.92. It asserts fixed notch/icon/card/tab/scroll geometry, selected-theme rollback and absence of save notices. Opening CSS animations are finished separately because the virtual clock controls timers/rAF rather than the document animation timeline.

Before the fix, the pending status paragraph added 28–29 px to the settings height, moving the real Windows settings tabs by 14–15 px. The user subsequently requested removing all save notices, including pending/failed/restored messages in appearance and general settings. Those nodes and strings are removed; pending locks, rollback and console diagnostics remain, preserving the original idle height. Both projects pass the regression and existing browser/settings suites. Native Windows recordings of six changes per app confirm unchanged HWND, notch, icon and settings-tab rectangles. Evidence: `tmp/theme-transition-results.json`, `tmp/native-theme-jitter-before.json`, `tmp/native-theme-jitter.json`. The user's original theme and layout were preserved; no full-screen backdrop was opened.

## Adaptive backdrop and unchanged displays (2026-10-17)

Glass and Liquid now capture at 30 Hz while changing, 10 Hz after 500 ms unchanged, and 4 Hz after 2 s unchanged. Dragging keeps 60 Hz; reduced motion keeps 4 Hz. Visible/detail/drag transitions reset the cadence. Quote polling is unchanged. Equal notch text/attributes and unchanged detail charts avoid DOM replacement; chart keys include all points, baseline, session bounds and color, so corrections to earlier points still render. The user-requested black percentage badge has been removed without changing font size or spacing.

With Vite running, run `python scripts/display-update-smoke.py --port 1420`, `python scripts/liquid-cadence-smoke.py --port 1420` and again with `--theme glass`, plus the existing renderer/browser suites. The cadence test uses Playwright's virtual clock: two surfaces together settle at 8 captures/s, moving backgrounds recover to 60 captures/s, dragging to 120 captures/s, detail reopening paints within 50 ms, and folding stops capture. Three changes immediately after a slow read exercise the least favorable phase; observed first paint was at most 240 ms. This bounds background detection latency rather than claiming 30 Hz detection while idle.

Real Windows Liquid checks used a small temporary striped window behind the notch/detail, removed in `finally`, and three stop/resume cycles per app. Moving detail paints stayed around 30 Hz and settled reads returned to 8/s. Evidence: `tmp/adaptive-native-results.json` (includes TokenUsage), `tmp/liquid-cadence-liquid-results.json`, `tmp/liquid-cadence-glass-results.json`. Native first-change and paint timings include IPC/GDI; these are short debug runs, not presented-FPS, GPU/power, release-build or long-term leak measurements. A fixture initially measured its background before the asynchronous detail placement finished; position the pattern only after rail/detail no longer overlap. Keep process-enumeration helpers outside timing windows.

## Current verification

- Current suites: **195 passed** (11 Node UI; 141 Rust library, 4 integration, 28 notch, 9 risk, 2 raw-frame/cache).
- `npm run build`: passed.
- `npm run test:coverage:windows`: **94.23%** business-logic lines, gate 85%.
- Browser smoke: page errors 0. Windows native evidence is listed below.

## Glass/Liquid update (2026-10-17)

Zero refraction is now the permanent Liquid material. The renderer skips the SVG filter URL and distance/displacement-map allocation, while retaining capture, blur, saturation, brightness and the existing reflected rim. Glass still uses its own refraction. Both real-renderer lifecycle fixtures enforce no map builds/no SVG filter for Liquid and restore the Glass lens when switching. Both full suites and production builds pass; native debug inspection confirms a blur-only filter with no lens-image references.

A short headless Edge A/B (baseline zero-strength SVG versus bypass, ABBA order, two six-second samples per 30/60 Hz workload, synthetic changing pixels, two surfaces) measured whole-machine-normalized browser-group CPU 2.12% -> 1.52% at 30 Hz and 2.87% -> 1.63% at 60 Hz. These are renderer-only workload reductions, not native whole-app CPU/power savings; desktop capture is not part of this fixture. A separate static screenshot comparison with animations disabled differs by at most 2/255 per channel (mean 0.0146/255), consistent with SVG color rounding. Evidence: `C:/dev/EconomyWarRoom/tmp/liquid-zero-benchmark.json` and `liquid-zero-pixels.json`.

Latest visual adjustment: Glass notch rims now use Liquid's layered 3px reflective band plus a 0.7px specular edge, with a neutral high-contrast mirror gradient. Liquid's displacement is fixed at 0: SVG refraction and displacement-map generation are bypassed; blur and the colored rim remain. Both renderer smokes and production builds pass after the change; native notch screenshots were inspected (`tmp/final-glass-mirror-notch.png`).

The final cache also keeps a reusable original BGRA buffer: an exact key/source match skips per-pixel conversion as well as IPC and painting. Reserved-alpha changes still use the alpha-insensitive RGBA comparison. Complete suites and coverage were rerun successfully. Fresh Liquid samples after this refinement are in `C:/dev/EconomyWarRoom/tmp/liquid-fast-cache-performance.json`; prior tables are earlier snapshots.

Current verification: EconomyWarRoom 195 tests, TokenUsage 166 tests (one live vendor test ignored), both production frontend builds pass. EconomyWarRoom business coverage is 94.23%. Native first-open settings heights are 434/440 DIP respectively; raw RGBA and legacy BMP work, four-edge card changes keep the canvas fixed, automatic fold restores capture affinity, and Dark theme exit releases it. Native reference-material screenshots were visually inspected.

Eight-second static samples on the existing desktop: Liquid CPU 1.95/3.01/4.41% for EconomyWarRoom rail/detail/settings and 1.82/2.42/3.22% for TokenUsage. Private commit was approximately 265-339 MiB for the live materials. Actual high-rate drag produced about 50-54 changed background paints/sec and 10-11% total-machine CPU in debug builds. rAF p99 was 7.1 ms at 144 Hz; this is not compositor-presented FPS. Earlier CPU/memory runs are not a controlled A/B comparison. Full samples, limitations and transient harness observations: `C:/dev/EconomyWarRoom/tmp/liquid-final-review.md` with `liquid-final-performance.json` / `liquid-final-drag.json`.

Glass now uses the supplied neutral lock-screen reference. Liquid keeps its original material CSS and optics; both share the optimized capture worker. The tray has Show/Quit only; application keyboard handlers (including Escape drag cancellation and custom tab navigation) are removed. Native smoke reset helpers now use mouse controls. Earlier keyboard and painted-Glass evidence is historical.

`npm test` includes raw RGBA/cache regressions. `python scripts/liquid-renderer-smoke.py --port PORT` with Vite on that port checks unchanged-frame suppression, a deliberately delayed detail frame across close/reopen, enabled Glass/Liquid switching, explicit visibility buffer release/resume and destroy. The delayed-frame case failed before detail-generation invalidation and passes after the fix. Both projects pass this real-renderer fixture without page errors. It mocks native pixels and does not prove desktop capture by itself.

Windows validation uses isolated app profiles, existing desktop pixels and no fullscreen test backdrop. Raw RGBA avoids per-frame Blob/BMP/image decode; the dedicated worker reuses two DIB buffers and returns empty packets for unchanged pixels. The legacy BMP command remains available to diagnostics. Capture affinity stays excluded during pending reads and restores on leaving both live materials, reload or stop.

## Commands

```powershell
npm test
npm run build
```

`npm test` runs Node 24's built-in runner for pure UI state/capture tests, then Rust library tests, `integration_e2e`, `risk_scenarios`, `notch`, and `liquid_frames`. Tests use synthetic values/mock HTTP and supplied time; they do not require live Yahoo prices.

```powershell
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked
npm run test:coverage:windows
```

The Windows coverage script enforces **>=85% business-logic lines** and writes `tmp/coverage-summary.json`. The existing `npm run test:coverage` uses tarpaulin through Bash, with the same business/native distinction.

The raw-frame cache module is also included; its new Windows worker branches are not excluded to inflate the gate.

Included: domain (including notch geometry/drag/reveal/capture bounds), application, ports/state, JSON store and Yahoo. Excluded: bootstrap, thin commands, window control/updater, native notch/topmost/capture OS adapters. Do not copy TokenUsage's broader application exclusions.

## Startup input regression (2026-10-17)

A native debug launch reproduced an expanded DOM notch while `get_notch_reveal` was false and Win32 input covered only the folded edge target. Touching that small target restored interaction. The initial `Promise.all` sampled reveal before quote/sparkline reads finished; those reads can wait for the scheduler's network work. A newer native fold event could update the DOM, then the late bootstrap snapshot reopened it without changing native input coverage.

The app now counts native reveal events and applies the initial reveal snapshot only if no newer event arrived during bootstrap. No geometry or input-region rules changed. `notch-browser-smoke.py` defers initial quotes, delivers a newer fold/reveal, then releases the stale initial state; both directions pass. The folded case failed before this fix with expected false / actual true after quote completion.

Windows verification against the running normal debug app held a real quote response through the IPC callback map, let the native notch fold, then released the response. DOM folding and inert state stayed correct. Physical pointer reveal, detail click, settings right-click and drag save all passed; the script restored its starting placement and removed the diagnostic hook by reloading. Evidence: `tmp/startup-input-native-result.json`. The temporary test code lives in `tmp/startup-input-native.py` and is not part of the product.

`npm test` passes 195 tests using the separate existing `src-tauri/target/llvm-cov-target` output directory, because the running debug executable locks the normal target on Windows. `npm run build` and the full browser regression pass. The debug app remains running with the updated frontend.

## Regression coverage

- Legacy settings/watchlist/column ratios/refresh preserved; new theme/placement defaults; old-window monitor restoration.
- Failed filesystem writes leave committed state unchanged; retry persists.
- Notch primary percentage matches the existing top row; secondary/regular values cannot replace it. Missing/nonfinite is unknown; explicit zero stays zero.
- Sparkline endpoint preserves observed session time, including partial sessions; empty/single-sample data does not fabricate a trend.
- Dynamic cells: all slots show instruments; 10 items in 6 slots → 5 icons and +5 overflow; six items use all six slots; one-slot overflow opens the list; empty notch still opens settings by right-click.
- Pin/hover/settings/removal transitions; serialized native surface requests and stale response suppression. Keyboard controls/global hotkey were removed at the user's request.
- Capture serialization, stale-theme frame disposal, stop/release and failure/retry.
- Four edges, negative coordinates, 100/125/150/200% geometric scaling, small display capacity, detailed hit regions, stable canvas, seam/drag selection and placement rollback.
- Existing quote/sparkline policies, rate-limit backoff, hidden scheduler zero requests, JSON, search and watchlist operations remain covered.

## Browser smoke

```powershell
npm run dev -- --port 1521
# Another terminal:
python scripts/notch-browser-smoke.py
python scripts/notch-motion-smoke.py
```

Requires Python Playwright and Edge. Uses mocked native APIs and synthetic primary/secondary/sparkline values. Checks card switching, matching percentages, settings shared height, delayed first paint, menu containment/mouse cleanup, add/remove pending/duplicate suppression/failure/retry, mouse reorder, row detail opening, update/copy feedback, save rollback, overflow and empty-list access. Esc/Delete/arrows/implicit Enter submission are verified inactive. The notch has no add/remove buttons; Symbols has no count/cancel/detail-navigation button. Screenshot: `tmp/notch-browser-glass.png`. This does not prove native capture, focus or click-through.

## Windows native smoke

Use a temporary Tauri config with identifier **`com.economywarroom.notch-preview`**, main window title **`Economy War Room Preview`**, `beforeDevCommand` empty and `devUrl` `http://localhost:1521`. This gives a separate app data directory. Point `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` at `--remote-debugging-port=9251`.

```powershell
# Vite must already run on 1521.
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9251'
npm run tauri dev -- --config tmp/native-preview.json
# Another terminal: creates a controlled external backdrop, not a DOM background.
python scripts/native-backdrop.py
# Another terminal:
python scripts/notch-native-smoke.py
python scripts/notch-native-lifecycle.py
python scripts/notch-native-menu.py
```

Native scripts assert the preview ID before writing fixture preferences. They use the existing seed instruments and add NVDA only in that preview. The menu regression adds seven temporary instruments, removing them in a finally block. No global shortcut is registered. The backdrop is a temporary external window; close it after the smoke. Python dependencies: Playwright, pyautogui, Pillow, tkinter. Debug builds do not modify OS autostart entries. These scripts were updated for the current mouse controls and syntax-checked; the complete isolated native suite has not been rerun after the latest Symbols-tab changes.

Historical initial adoption smoke (superseded controls/material contracts below are not current regressions) recorded at the former 320 DIP width, primary 2560×1440 at 100%, negative-x secondary 1024×1280 at 100%:

- Four physical edges; 20 selected-card transitions preserve canvas and notch origin.
- Primary-monitor settings tab heights: 391 DIP across appearance/instruments/general. Negative-x monitor: 382 DIP across all tabs in the final run, with monitor-aware content scaling.
- Hover preserves the foreground window; a transparent corner targets another HWND; edge fold/reveal works.
- Physical drag saves placement; Escape cancels an active physical drag. Keyboard can reveal/access the folded rail.
- External patterned-window capture returns BMP data with the expected multiple source colors; Liquid renders successfully and Glass rejects capture.
- Short native idle samples: approximately 30 rail paints/sec. This is a short sample, not a sustained 60fps claim or power benchmark.
- The last-row menu stays inside the detail region; physical mouse removal and Escape cleanup pass. The preview's separate shortcut hides and restores its window.
- The final notch **+** opens and focuses the input; adding MSFT through that form and removing it from its detail footer both pass. Failed detail removal preserves the item and re-enables its button in the browser regression.
- Liquid uses display affinity 17 while active. Explicit hide and switching to Glass restore affinity 0; show resumes rendering. This regression caught and fixed WebView2's unchanged `document.hidden` on hide.

Evidence: `tmp/native-smoke-result.json`, `tmp/native-extra-result.json`, native Glass/Liquid screenshots. Hardware mixed-DPI, monitor unplug/replug, sustained drag 60fps, GPU/power, and signed updater installation are not established by these checks. Geometric scaling is unit-tested separately from hardware validation.

The fresh reviewer reproduced two issues (first settings paint at the previous card height, and a context menu outside native hit coverage). Both received failing-then-passing regressions and native follow-up. That review agent reached its usage limit before completing a full report; the remaining final read-through was performed by the author.

Current follow-up: detail/settings match TokenUsage's 260 DIP width and 16 DIP padding. Identity and two price rows occupy the upper row; the chart fills the lower row. Prices remain at the user's reduced sizes. Rings stay neutral. No add/remove buttons remain on the notch. Symbols always shows search above flat logo/ticker/name rows; its only trailing action is an accessible trash button. Row click opens detail; drag reorders. No count, cancel header, separate detail-navigation button or custom keyboard/global shortcut remains. Add/remove failure keeps query/item and permits retry; pending actions reject duplicate submissions.

Current browser regression passes the full mouse add/remove/reorder/overflow/settings path, update/copy icon morphs, 1s/10s/60s selection/persistence/rollback, unchanged legacy interval on opening, inactive keyboard shortcuts, neutral rings and delayed settings first paint. Synthetic common settings heights are 434 DIP across all three tabs. Fourteen additional layout cases cover 0.92/1.0 content scaling, 0/1/4/6/10/20 instruments, long names/tickers and one-slot overflow. Search/body centers match the card, there is no scrollbar gutter or visible scrollbar, and up to six rows fit without vertical scrolling. Longer lists remain scrollable with their final trash button reachable. No horizontal overflow occurred. Previous detail render checks cover four edges, large prices and long ticker labels. Rust tests cover 260 DIP at four physical DPI scales and every edge.

Motion smoke seeks the actual CSS reveal animation through five points at each edge. DOM/native-canvas rects remain fixed while clipping changes, with no panel transform. It verifies 37 DIP tab height, footer anchoring, the app's motion switch and OS reduced-motion handling. Output: `tmp/notch-motion-result.json`. Twelve chart renders cover rising/falling/flat/partial/single/empty data at 1.0/0.92 scale, fixed 1.15px stroke and a last-observation marker; partial sessions retain unused future width. Output: `tmp/sparkline-redesign-result.json` and `tmp/sparkline-*.png`.

Detail charts use a contrasting dashed baseline above a 12% translucent fill between the observed line and that baseline. Check both rising and falling cases; the fill must remain visible below the baseline and must not extend into unused future session time.

Continuous reorder regression: moving the captured row in the DOM released pointer capture after the first position change. Pointer capture and gesture listeners now belong to the stable list container. The browser test runs with animations enabled, crosses three rows in each direction, verifies one persistence call per release and no leftover ghost. It scopes row order to the list, excluding the body-level cloned ghost that made the earlier test falsely pass. Before/after event traces: `tmp/watchlist-drag-trace-before.json`, `tmp/watchlist-drag-trace.json`. The decorative grip has been removed; rows remain draggable. The Liquid capture explanation was removed from appearance settings at the user's request.

The normal Windows development app was visually inspected after the current changes at 0.92 content scale (240 physical-pixel card). Physical right-click/settings-tab/search input work, with the current four instruments retained. This read-only native check does not exercise removal against the user's real watchlist. Evidence: `tmp/symbols-after.png`, `tmp/symbols-results-after.png`, `tmp/symbols-native-final.png`, `tmp/settings-polish-layout-result.json`, plus previous `tmp/token-width-layout-result.json` / `tmp/token-width-native-result.json` detail evidence.

With the user's debug app running, the earlier `npm test` could not replace its locked executable (Windows error 5). The current complete Rust suite runs in the separate llvm-cov target and passes (182); current Node UI tests pass (8), and production build passes. The user's running app was left open.

Keep verification counts/results current from command output. Native glue is verified by the matching native path; the coverage percentage alone is not desktop validation.
