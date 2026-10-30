# EconomyWarRoom

A lightweight Windows **screen-edge watchlist widget** for US equities and crypto.
Its notch shows each instrument's icon and the percentage associated with its latest price. Click an icon to open that instrument's existing **symbol · sparkline · price/change** card, including the primary and secondary price rows.

Version **0.4.11** brings the TokenUsage-style edge notch, inward detail cards and settings, with Dark / Glass / Liquid materials, live desktop capture blur and adaptive foreground contrast.

## Controls

| Action | Control |
|---|---|
| Open / pin a stock card | Click the instrument icon; click again to close |
| Move / dock | Drag the notch to any screen edge or display; release to save |
| Open settings | Right-click the notch, including its empty surface |
| Add an instrument | Settings → **종목** → search and click a result or the input's **+** |
| Remove an instrument | Trash icon at the right end of its row in Settings → **종목** |
| Manage / reorder instruments | Settings → **종목**; drag a row to reorder, right-click for tint/remove |
| Open an overflow instrument | `⋯ / +N` cell → **종목** → click the instrument row |
| Change material | Settings → **모양** → 다크 / 글라스 / 리퀴드 |
| Change opacity / hover / folding / motion | Settings → **모양** |
| Refresh / autostart / updates / diagnostics | Settings → **일반** |
| Explicitly hide | Tray |
| Show again | Tray |
| Quit | Settings → **일반** → 종료, or tray Quit |

Up to six instrument cells fit along the notch. When needed, the final **⋯ / +N** cell gives access to the remaining instruments; even a one-slot notch can open the full list. Addition lives only in settings. An empty list leaves the notch surface available for right-click settings. Missing icons use ticker initials. The widget does not reorder instruments automatically as prices change.

Custom keyboard controls and the global visibility shortcut have been removed. Symbol typing and native text editing remain available; Enter does not add a symbol implicitly.

**Fold ≠ hide ≠ quit.** Folding leaves a small edge target and continues quote polling. Explicit hide stops new quote polling; the process stays running. Both fold and hide stop Glass/Liquid materials and contrast sampling. The tray provides Show and Quit; application keyboard shortcuts are not installed.

## Quote semantics

- The notch percentage is exactly `Quote.display.primary.change`, matching the detail's **top row**. There is no separate notch calculation or secondary-row fallback.
- The top row is the existing latest/session print; the second row is the existing last completed regular-session reference. PRE/LIVE/POST/CLOSED behavior is preserved.
- The existing sparkline uses its regular-session window and regular-session color policy. Its color can differ from the latest extended-hours percentage.
- The provider is the existing rate-limited Yahoo pipeline. Missing values are `—`; existing cache survives provider errors.

## Appearance

The notch has concave shoulders, inward detail/settings cards, transparent input regions, and four-edge docking. Rust owns physical placement and DPI; invisible canvas regions pass input to other applications. The detail/settings canvas stays fixed while card contents change.

Detail and settings use the same **260 DIP** width and **16 DIP** padding as TokenUsage, with the same monitor-aware scaling. Each detail is one surface: icon/name and primary/secondary prices above a full-width sparkline. It has no nested card, separate heading or timestamp footer. The notch has no add/remove controls. Rings stay neutral; only percentages use up/down colors.

The detail sparkline uses a 52 DIP chart area, a fixed 1.15px stroke, faint baseline/fill and a small last-sample marker. The marker stays at the last observed session time; a partially completed session is not stretched to the close. Empty/single-sample data does not fabricate a trend.

Dark is the default. Glass retains a neutral smoked surface. Liquid uses clear compositor blur, subtle cyan/pink reflections, a fine convex rim, and adaptive foreground contrast. Both use Windows Composition behind the WebView and remain visible in screenshots. Liquid samples only three-pixel-wide empty margins, serialized every 250ms after completion; bright surfaces use dark text and stronger up/down/chart colors. Hide/fold stops materials and contrast probes. Unsupported native materials retain a readable tint fallback.

Settings use **모양 / 종목 / 일반** with one intrinsic measured height across tabs, including the first opening after a short card. Normal contents fit without scrolling. There is no visible scrollbar or reserved scrollbar gutter, so header/tabs/body share the same center. Header/tabs stay fixed; exceptionally long lists remain wheel-scrollable inside screen bounds.

TokenUsage's icon tabs/sliding selection, grouped setting rows, compact switches, opacity track, and restrained press/loading/completion feedback are reused. The instruments tab has flat icon/ticker/name rows, always-visible search, and mouse reorder. There is no count, search-close control or detail-navigation button. Each row opens its detail; its trailing trash button removes only that instrument, with pending/error/retry feedback. Update and log-copy status stay inside settings; failures allow retry. See the [transfer inventory](docs/ui-references.md#tokenusage-transfer-inventory).

Tabs use TokenUsage's 37 DIP button height. General offers **1s / 10s / 60s** refresh choices; its neutral log-copy and softly tinted quit buttons stay at the bottom of the panel. Existing custom intervals are preserved until the user selects a preset. Notch snap/fold, tab spring/fade, switch/opacity response and copy/update icon transitions respect motion preferences. Detail reveal animates clipping/opacity without transforming the measured panel.

## Develop

Windows 10/11 with MSVC Build Tools, Rust stable, WebView2, and **Node 24+** for the built-in UI test runner.

```powershell
npm install
npm run tauri dev
```

```powershell
npm test                       # UI state/lifecycle + Rust unit/integration/risk/notch
npm run build                  # TypeScript + Vite
npm run test:coverage:windows   # business logic >=85%; cargo-llvm-cov required
```

`npm run dev` alone runs the frontend without native APIs. Focused browser smoke supplies mocked native APIs and synthetic prices; native smoke uses a separate application identifier. See [Windows development](docs/windows-dev.md) and [Testing](docs/testing.md).

Debug builds persist their own preferences but do not modify OS autostart entries. Tray controls remain available when the notch is hidden. No global keyboard shortcut is registered.

## Data and architecture

Watchlist, order, card tints, opacity, legacy column ratios, refresh presets, autostart and legacy window geometry remain in the existing `economy-war-room-state.json`. The legacy `hotkey` field is retained for file compatibility but has no runtime effect. The new two-tier detail no longer exposes the old three-column resizers. New theme/placement preferences use backward-compatible defaults. Existing files reopen without losing their watchlist; the former window's monitor is used when initializing placement.

This remains a **watchlist**, with no portfolio, P&L, broker integration or SQLite history. No TokenUsage quota/auth/activity collectors are imported.

| Document | Purpose |
|---|---|
| [Architecture](docs/ARCHITECTURE.md) | Current module map and contracts |
| [Windows development](docs/windows-dev.md) | Native launch, isolated preview and diagnostics |
| [Testing](docs/testing.md) | Automated and native evidence, coverage scope |
| [UI references](docs/ui-references.md) | Design references |
| [Adoption design](docs/superpowers/specs/2026-10-17-tokenusage-ui-adoption-design.md) | Agreed UI information hierarchy |
| [Implementation plan](docs/superpowers/plans/2026-10-17-tokenusage-ui-adoption.md) | Tasks and acceptance checks |
| [Releases/updater](docs/release.md) | Existing signed publication workflow |

Instrument marks are bundled from [Simple Icons 16.0.0](https://github.com/simple-icons/simple-icons/tree/16.0.0); source license and disclaimer are in `src/assets/marks/`. Missing brands use ticker text. Notch and material code is adapted from the user's local TokenUsage checkout; existing Lucian material attribution is retained.
