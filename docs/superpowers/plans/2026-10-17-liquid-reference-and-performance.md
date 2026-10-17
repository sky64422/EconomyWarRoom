# Liquid reference material and capture performance

**Goal:** Both EconomyWarRoom and TokenUsage use the user's lock-screen reference for Glass; Liquid retains its previous appearance. Glass uses a softly translucent neutral surface, narrow edge refraction, restrained highlights, and sharp foreground. Reduce repeated image allocations while retaining 30Hz idle / 60Hz dragging and current geometry.

**Scope:** Existing `C:/dev/EconomyWarRoom` and `C:/dev/TokenUsage` working folders. Preserve existing work. No automatic commit, release, install or changes to real user preferences. Use isolated native identifiers for verification.

1. Add protocol/cache regression tests before implementation. A changed pixel must produce a frame; identical pixels can be skipped only for the same surface, generation and region. Legacy capture callers still receive BMP. Test raw RGBA header/dimension handling in the frontend.
2. Introduce a small Windows capture worker in each project's infrastructure. It owns at most two reusable DIB surfaces on its own thread, flushes GDI before reading, returns raw RGBA for the opt-in renderer protocol, and supports reset. Existing command/event names remain; optional surface/generation arguments enable the new path. Existing raw BMP response remains available to legacy probes. Affinity and region validation stay in the existing adapter.
3. Switch both live-glass renderers to raw pixels with a reusable canvas, exact unchanged-frame suppression, and generation checks. Clear backing stores/filter references when leaving Glass/Liquid or explicitly hidden. Preserve existing fractional timing, serialized reads and stale-frame rejection. TokenUsage also observes its native visibility event.
4. Adjust Glass material: soften neutral tint, remove stacked opaque foreground washes and label pills, narrow edge highlights, keep edge refraction with restrained RGB dispersion, and retain legibility floors. Restore Liquid's original CSS and optics, preserving Dark and all native geometry/settings sizing contracts. Remove tray Hide and app keyboard controls; preserve automatic folding and backend visibility lifecycle.
5. Run each project's complete tests and build. Run Windows coverage for EconomyWarRoom, shared renderer browser visual/lifecycle tests for both, TokenUsage settings first-open smoke, and native Liquid checks in separate profiles. Compare static/dynamic backdrop, actual drag, fold/hide and theme transitions; report measured CPU/memory/frame results and limits. Capture visual examples against the existing desktop. The temporary photo backdrop was closed at the user's request and must not be relaunched.
6. Independently review the scoped implementation and resolve material findings. Record final evidence in each project's testing/material documentation.

**Review focus:** stale unchanged frames after re-entry, buffer lifetime across async shutdown, GDI thread ownership, BGRA/RGBA order and alpha, DPI scaling, fallback-to-BMP compatibility, source settings first-open height, provider/quote polling semantics and native capture-affinity restoration.

## Final evidence

Implementation and independent read-only review completed. Both reviewer findings were resolved: invalidate detail generations on hidden transitions and emit explicit visibility through the show-only tray helper. Complete tests/builds and real-renderer regressions pass in both projects; EconomyWarRoom coverage is 94.23%. Native material/settings/raw/BMP/four-edge/drag/fold checks and current resource samples are recorded in `tmp/liquid-final-review.md`. No photo backdrop was reopened. High-rate drag remains a 10-11% debug CPU burst; release/presented FPS/power claims are outside the measured evidence.

## Latest user refinement

After visual feedback, Liquid displacement is halved (28 to 14) in both projects while its existing lens band, blur and colored rim remain. Glass keeps its neutral body, with the notch rim changed to a high-contrast neutral mirror gradient using the same layered band/specular-edge construction as Liquid. Native visual checks, renderer lifecycle smoke and builds pass. The cache now bulk-compares a reusable original BGRA buffer before conversion; exact key/source matches skip the pixel conversion loop too. Full suites and coverage pass after this refinement; latest Liquid samples are in `tmp/liquid-fast-cache-performance.json`.

## Confirmed zero-refraction Liquid

The user selected zero refraction permanently. Liquid now bypasses the SVG RGB/refraction filter and map creation instead of only setting its scale to zero. Capture/blur/material/rim remain. A failing-then-passing renderer regression enforces the bypass and Glass lens restoration; both full suites/builds pass. The short headless A/B and frozen-animation pixel comparison are recorded in `tmp/liquid-zero-benchmark.json`; they measure the renderer, not total native-app power or CPU.

## Follow-up: static capture and equal display data

The user approved reducing static-background work and repeated display updates in both apps, explicitly requesting stop/resume validation. Added 30/10/4 Hz adaptive capture (500 ms/2 s settling), keeping 60 Hz drag and immediate cadence reset on reveal/detail/drag transitions. Quote/provider polling intervals remain unchanged. EWR preserves unchanged chart DOM using full point/baseline/session/color keys; TU preserves unchanged quota cards without suppressing countdown or activity changes. Both guard repeated notch display writes. Virtual-clock and real Windows pattern-window stop/resume checks are documented in each testing guide. The user also requested removing the tiny black percentage backgrounds in both notches; only those decorations were removed. Independent review found no blocking issue; its countdown-test suggestion was added and passed.
