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
- **Do not** add portfolio / P&L / broker / SQLite history without a new design.
- **Hide ≠ quit**; hide must pause quote polling.
- Prefer **`AppCore`** / domain / scheduler for logic; keep `commands.rs` thin.
- Coverage gate: ≥85% business logic.
- Default branch: **`main`**. MVP is already implemented — extend or fix, don’t re-scaffold.
- Signing private key `tmp/updater.key` stays out of git.

## Verify before claiming done

```text
npm test
# or: cargo test --lib  +  integration_e2e  +  risk_scenarios under src-tauri
```

UI: `npm run tauri dev` on the target OS (Windows preferred for chrome/hotkey/autostart).
