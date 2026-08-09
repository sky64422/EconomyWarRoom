# UI references

**Updated:** 2026-08-09  

External design references for EconomyWarRoom (and the sibling floating-widget family, e.g. TokenUsage).

| Site | URL | How we use it |
|------|-----|----------------|
| **shadcn/ui** | [https://ui.shadcn.com](https://ui.shadcn.com) | Control primitives & patterns: focus-visible rings, segmented controls, switches, sheets/overlays, list density, semantic naming. The app is **vanilla TS + CSS**, not a React/shadcn install — borrow patterns and tokens, not the full component stack. |
| **Impeccable** | [https://impeccable.style](https://impeccable.style) | Taste and anti-slop: Operate-mode glanceability, hierarchy, restrained motion, no nested-card “cardocalypse,” no decorative chrome. |

## Product constraints (override generic web patterns)

- Floating **watchlist widget**, not a portfolio dashboard or marketing landing.
- **Dark-only** glass panel; settings as an **opaque overlay** (no window grow).
- Glance: symbol · sparkline · price / change % — not heavy charts or OHLC terminals.
- Prefer [`README.md`](../README.md), [`ARCHITECTURE.md`](./ARCHITECTURE.md), and [`docs/superpowers/specs/2026-07-22-economy-war-room-design.md`](./superpowers/specs/2026-07-22-economy-war-room-design.md) when a generic shadcn/Impeccable page pattern conflicts with the widget form factor.

## Applied checklist (2026-08-09)

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
| P2–P3 | Header SVG icons, 32px hit targets |
| P4 | Secondary quote no double opacity |
| P6 | Price tick respects `prefers-reduced-motion` |

## Related code

| Path | Role |
|------|------|
| `src/styles/tokens.css` | Design tokens (opacity-linked surfaces, up/down, row grid) |
| `src/styles/app.css` | Layout + primitives |
| `src/ui/*` | Header, watchlist, settings |
