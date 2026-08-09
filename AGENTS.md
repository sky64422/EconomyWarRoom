# Agent instructions (EconomyWarRoom)

If you are an automated coding agent in a new session:

1. **On Windows:** [`docs/windows-dev.md`](docs/windows-dev.md)  
2. **Code map:** [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)  
3. **UI references:** [`docs/ui-references.md`](docs/ui-references.md) — [ui.shadcn.com](https://ui.shadcn.com), [impeccable.style](https://impeccable.style)  
4. **Tests:** [`docs/testing.md`](docs/testing.md)  
5. **Backlog:** [`docs/TODO.md`](docs/TODO.md)  
6. **Releases / updater:** [`docs/release.md`](docs/release.md)  
7. **Product overview:** [`README.md`](README.md)

## Hard constraints

- Product is a **floating watchlist widget**, not a portfolio manager.  
- **Do not** add portfolio / P&L / broker / SQLite history without a new design.  
- **Hide ≠ quit**; hide must pause quote polling.  
- Prefer **`AppCore`** / domain / scheduler for logic; keep `commands.rs` thin.  
- Keep automated tests green; respect coverage gate (≥85% business logic).  
- Default branch: **`main`**. MVP is already implemented — extend or fix, don’t re-scaffold.

## Verify before claiming done

```text
npm test
# or: cargo test --lib  +  integration_e2e  +  risk_scenarios under src-tauri
```

UI changes: `npm run tauri dev` on the target OS (Windows preferred for chrome/hotkey/autostart).
