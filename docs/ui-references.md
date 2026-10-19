# UI references

**Updated:** 2026-10-17

External design references for EconomyWarRoom (and the sibling floating-widget family, e.g. TokenUsage).

| Site | URL | How we use it |
|------|-----|----------------|
| **shadcn/ui** | [https://ui.shadcn.com](https://ui.shadcn.com) | Control primitives & patterns: focus-visible rings, segmented controls, switches, sheets/overlays, list density, semantic naming. The app is **vanilla TS + CSS**, not a React/shadcn install — borrow patterns and tokens, not the full component stack. |
| **Impeccable** | [https://impeccable.style](https://impeccable.style) | Taste and anti-slop: Operate-mode glanceability, hierarchy, restrained motion, no nested-card “cardocalypse,” no decorative chrome. |

## Product constraints (override generic web patterns)

- Floating **watchlist widget**, not a portfolio dashboard or marketing landing.
- **Dark / Glass / Liquid** screen-edge notch; inward single-instrument detail and a shared-height settings panel. See the [adoption design](superpowers/specs/2026-10-17-tokenusage-ui-adoption-design.md).
- Glance: symbol · sparkline · price / change % — not heavy charts or OHLC terminals.
- The notch shows an instrument icon and its existing latest primary percentage. Addition/removal live only in Settings → **종목**. Clicked details match TokenUsage's 260 DIP width: identity/prices above a full-width sparkline.
- Prefer [`README.md`](../README.md), [`ARCHITECTURE.md`](./ARCHITECTURE.md), and [`docs/superpowers/specs/2026-07-22-economy-war-room-design.md`](./superpowers/specs/2026-07-22-economy-war-room-design.md) when a generic shadcn/Impeccable page pattern conflicts with the widget form factor.

## TokenUsage transfer inventory

Source inspected locally at `C:/dev/TokenUsage`, commit `251df6bfdceda1bf29f1696e503b72e3cce907e1`. The source project is unchanged. Existing Lucian attribution in the borrowed notch/material files is retained.

| Compatible element | EconomyWarRoom adaptation |
|---|---|
| Concave edge notch, folding/reveal, four-edge docking | Rust geometry/native hit regions and TS shell; dynamic watchlist instead of fixed providers |
| Stable transparent canvas and monitor/DPI scaling | 260 DIP details/settings, 16 DIP padding, 0.92–1 content scale; changing detail/tab does not move the window |
| Dark / Glass / Liquid surfaces | Same-HWND compositor material from TokenUsage db5ecbc, with clear prism reflections and adaptive foreground contrast |
| Shared-height settings with fixed chrome | All tabs measured intrinsically; normal contents fit without scrolling; no scrollbar/gutter, with body wheel scrolling retained for long lists |
| Icon tabs and sliding selection | 모양 / 종목 / 일반; source 37 DIP height, spring indicator, hover/press and panel fade |
| Grouped rows, compact spring switches | Appearance/system controls share sizing and separators; existing settings persistence retained |
| Opacity meter | Animated neutral fill, direct mouse slider and stable percentage; existing readability floors retained |
| Save/retry feedback | Pending lock and rollback for settings; add/remove errors keep the item/query and allow retry |
| Update and log-copy feedback | Spinning reload and copy/check icon morphs, completion tint/glow, version badge and localized status; existing updater/diagnostics backend |
| Bottom action row | Log-copy neutral glass and subtly tinted quit colors/hover states; both stay at the panel bottom |
| Compact information hierarchy | Icon/ticker/name rows with trailing trash; row click opens detail; no nested detail card |

Not transferred: quota meters, vendor authentication, provider minimum-selection restrictions, token/activity collectors, quota-specific refresh cadence, keyboard shortcuts and destructive-action confirmation flows. These do not serve the current mouse-operated watchlist or conflict with the user's explicit direction. The notch has no extra add/remove controls, and the Symbols tab has no count, cancel header or separate detail-navigation button.

Notch fold/reveal/snap and material motion were already adapted. Detail reveal now retains the source timing/direction while animating clip/opacity only; scaling a measured/native panel would move its visible/hit bounds. All added motion respects the app switch and OS reduced-motion setting. Quota/activity orbit, quota warning breathing and quit-confirmation countdown are not stock-price animations and are not attached to quotes.

## Historical checklist (2026-08-09)

This records the older list UI; its keyboard controls were explicitly removed on 2026-10-17. Current interaction contracts are in README and ARCHITECTURE.

shadcn + Impeccable gaps closed in code (vanilla TS/CSS only):

| ID | Item |
|----|------|
| S1 | Global `focus-visible` ring + row focus |
| S2 | Refresh segmented `aria-pressed` |
| S3 | Settings `role="dialog"`, Esc, Tab cycle, backdrop `inert` |
| S4 | Update button title ↔ aria-label sync |
| S5 | Copy Log `aria-live` |
| I1 | Opacity alpha floors (fg/accent/chrome + text/border tokens) |
| I2 | Row `aria-label` (symbol · price · change) |
| I4 | Pending quote/spark placeholders (`—`, muted) |
| P2–P3 | Header SVG icons (download/settings/hide), 24px hit targets |
| P4 | Secondary quote no double opacity |
| P6 | Price tick respects `prefers-reduced-motion` |

## Related code

| Path | Role |
|------|------|
| `src/styles/tokens.css` | Design tokens (opacity-linked surfaces, up/down, row grid) |
| `src/styles/app.css` | Layout + primitives |
| `src/ui/*` | Header, watchlist, settings |
