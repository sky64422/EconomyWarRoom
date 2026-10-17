# Agent instructions (EconomyWarRoom)

공통 규칙: Rules clone의 `ENGINEERING.md` / `RELEASE.md` (Windows `C:\dev\Rules`, WSL `/mnt/c/dev/Rules`). 이 파일은 **이 제품만**. 충돌하면 여기가 이긴다.

새 세션이면 이 순서로 연다:

1. **On Windows:** [`docs/windows-dev.md`](docs/windows-dev.md)
2. **Code map:** [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
3. **UI references:** [`docs/ui-references.md`](docs/ui-references.md) — [ui.shadcn.com](https://ui.shadcn.com), [impeccable.style](https://impeccable.style)
4. **Tests:** [`docs/testing.md`](docs/testing.md)
5. **Backlog:** [`docs/TODO.md`](docs/TODO.md)
6. **Releases / updater:** [`docs/release.md`](docs/release.md)
7. **Product overview:** [`README.md`](README.md)

## Hard constraints

- **Floating watchlist widget**, not a portfolio manager.
- UI is a screen-edge notch: instrument icon + latest `Quote.display.primary.change`; click opens that instrument's existing sparkline and primary/secondary rows.
- Rust owns placement, DPI and hit regions. Do not restore the former frontend content-hug resize loop.
- Themes: Dark / Glass / Liquid. Glass and Liquid capture visible desktop regions; stop capture on explicit hide and fold, and restore capture affinity on exit/reload/theme change.
- Settings: `모양 / 종목 / 일반`, one measured height across tabs, including first opening after a short detail. Scroll the body in constrained work areas.
- **Do not** add portfolio / P&L / broker / SQLite history without a new design.
- **Hide ≠ quit**; hide must pause quote polling.
- Fold is distinct from hide: the edge tab remains visible and quote polling continues.
- Prefer **`AppCore`** / domain / scheduler for logic; keep `commands.rs` thin.
- Coverage gate: ≥85% business logic.
- Default branch: **`main`**. MVP is already implemented — extend or fix, don’t re-scaffold.
- Signing private key `tmp/updater.key` stays out of git.

## Verify before claiming done

```text
npm test
# or: cargo test --lib  +  integration_e2e  +  risk_scenarios under src-tauri
```

`npm test` includes Node 24 UI state/lifecycle tests and Rust notch geometry tests. Also run `npm run build`. Windows coverage: `npm run test:coverage:windows` (cargo-llvm-cov + llvm-tools-preview).

UI: `npm run tauri dev` on the target OS (Windows preferred for chrome/hotkey/autostart).
