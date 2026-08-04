# EconomyWarRoom — TODO

**Scope:** open **implementation** candidates only. Shipped work is not listed.  
**Product:** floating watchlist widget (not portfolio). See [ARCHITECTURE.md](./ARCHITECTURE.md), [README.md](../README.md).  
**Updated:** 2026-08-04

Do **not** start portfolio / P&L / SQLite / broker work without a new design.

---

## Open implementation candidates

| ID | Idea | When to do |
|----|------|------------|
| **T-1** | Remappable hotkey UI (change `Ctrl+Shift+Space` in Settings) | Only if hotkey collisions are a real pain. Default + tray already cover show/hide; power users can edit JSON `hotkey` today. |
| **T-2** | Rolling file log under app data (survives hard crash) | Only after hard crashes waste debugging time. Soft failures already covered by Copy diagnostics + agent-run logs. |

If you implement one, prefer **T-2** then **T-1** by maintainer ROI. Core product does not block on either.

---

## Explicitly out of scope

- Portfolio, P&L, transactions, broker APIs  
- SQLite / historical snapshot system  
- Windows 11 official Widgets board  
- API-key-required realtime as a hard dependency  
- Separate multi-process quote proxy  
- Dedicated crypto-exchange / WebSocket provider (Yahoo is enough for now)  
- Commodities / KR equity providers (product expansion)  
- Frontend Playwright/Tauri e2e suite (Rust tests + manual smoke cover the risk)

---

## Not implementation (ops)

Windows runtime smoke checklists live in [windows-dev.md](./windows-dev.md) §5 — not tracked here as build work. Historical MVP plan: [superpowers/plans/](./superpowers/plans/).
