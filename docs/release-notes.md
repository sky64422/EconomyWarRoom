# Release notes

## v0.1.47 - 2026-08-15

### Fixed

- Sparklines use regular-session bars against yesterday’s close. Premarket prints no longer pull the line above the baseline while the quote % is red (SPCX-shaped days).

### Changed

- Display rows (primary/secondary) are computed in Rust domain code.
- Watchlist UI and CSS split into smaller modules; quote refresh stored as milliseconds (`quote_refresh_ms`, JSON key still `quote_refresh_secs`).

### Tests

- Coverage gate still ≥85% business logic. Last tarpaulin: **91.22%**. Unit tests: 109. `updater.rs` excluded from the gate (Tauri plugin glue).

### Verification

- `cargo test --lib` (109)
- `cargo test --test integration_e2e --test risk_scenarios`
- `cargo tarpaulin --fail-under 85` (updater excluded)

## v0.1.46 - 2026-08-14

### Fixed

- Secondary (2nd) price row during LIVE no longer treats Yahoo `regularMarketPreviousClose` as the day-before-yesterday. That field is T-1 (yesterday’s close), same as `previousClose`.
- Yesterday’s % now comes from daily bars (T-1 vs T-2). TSLA example: `339.96 (+3.80%)`, not a rounded `340.0` with today’s regular move.
- Daily-bar matching skips today’s incomplete last bar so a flat open cannot be treated as yesterday.
- Equity prices from $1 show two decimals so official closes are not rounded away (`339.96` instead of `340.0`). Secondary % uses two decimals (`+3.80%`).

### Docs

- Documented PRE / LIVE / POST / CLOSED meaning of the secondary row in `docs/ARCHITECTURE.md` and the README snapshot.

### Verification

- `cargo test --lib` (82 tests)
- `cargo test --test integration_e2e --test risk_scenarios`
- `npx tsc --noEmit`

## v0.1.42 - 2026-08-13

### Fixed

- Corrected the secondary price row for pre-market quotes. It now shows the latest completed regular-session close and that session percentage change instead of the prior session close.
- Confirmed the SPCX pre-market case: the secondary row uses the 2026-08-12 regular close of 146.15 USD and the regular-session change of +9.65%.

### Verification

- npm run build completed successfully.
- npm test completed successfully.

## v0.1.43 - 2026-08-13

### Changed

- Reduced header update, settings, and hide icon glyphs from 16px to 14px while retaining 32px button hit areas.

## v0.1.44 - 2026-08-13

### Added

- Added a compact US regular-market status pill to the header. It shows LIVE, PRE, POST, CLOSED, or -- using equity quote market states.
- LIVE uses a subtle pulse that respects reduced-motion preferences.

### Changed

- Reduced the market status pill height, spacing, padding, and status-dot size to keep the header compact.

## v0.1.45 - 2026-08-13

### Changed

- Increased the gap between the WarRoom title and the market status pill to 6px for clearer separation while preserving the compact header.
