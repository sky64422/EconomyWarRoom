# EconomyWarRoom — TODO

**Scope:** open **implementation** candidates only. Shipped work is not listed.  
**Product:** floating watchlist widget (not portfolio). See [ARCHITECTURE.md](./ARCHITECTURE.md), [README.md](../README.md).  
**Updated:** 2026-10-17

Do **not** start portfolio / P&L / SQLite / broker work without a new design.

---

## Open implementation candidates

| ID | Idea | When to do |
|----|------|------------|
| **T-2** | Rolling file log under app data (survives hard crash) | Only after hard crashes waste debugging time. Soft failures already covered by Copy diagnostics + agent-run logs. |

Keyboard controls and remappable hotkeys were removed from scope by the user on 2026-10-17. Core product does not block on T-2.

---

## Explicitly out of scope

- Portfolio, P&L, transactions, broker APIs  
- SQLite / historical snapshot system  
- Windows 11 official Widgets board  
- API-key-required realtime as a hard dependency  
- Separate multi-process quote proxy  
- Dedicated crypto-exchange / WebSocket provider (Yahoo is enough for now)  
- Commodities / KR equity providers (product expansion)  
- A broad frontend e2e platform (focused browser/native notch smoke scripts now cover the adopted UI)

---

## Not implementation (ops)

Windows runtime smoke checklists live in [windows-dev.md](./windows-dev.md) §5 — not tracked here as build work. Historical MVP plan: [superpowers/plans/](./superpowers/plans/).
