# Release notes

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
