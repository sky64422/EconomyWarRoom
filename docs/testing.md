# Testing & Coverage

## Layers

| Layer | Location | Purpose |
|-------|----------|---------|
| Unit | `src-tauri/src/**` `#[cfg(test)]` | Domain, scheduler, parse, store, service |
| HTTP mock | `yahoo/client` + wiremock | 200 / 429 / 5xx without live Yahoo |
| Integration | `src-tauri/tests/integration_e2e.rs` | Store + AppCore + scheduler + mock/Yahoo HTTP |
| Risk | `src-tauri/tests/risk_scenarios.rs` | Rate limit, hide pause, corrupt JSON, invalid IDs |
| GUI smoke | Manual `npm run tauri dev` | Hotkey, window chrome (not in CI coverage) |

## Commands

```bash
# Unit + integration
cd src-tauri
cargo test --lib
cargo test --test integration_e2e --test risk_scenarios

# Coverage gate (≥ 85% business logic)
./scripts/coverage.sh
```

## Coverage policy

**Included:** `domain/`, `application/`, `infrastructure/store`, `infrastructure/yahoo`, `ports/`, `state/`

**Excluded from gate (GUI / OS glue):**

- `src/main.rs` — binary entry
- `src/lib.rs` — Tauri `run()` bootstrap, plugins, hotkey registration
- `src/infrastructure/window_ctl.rs` — requires live `WebviewWindow`
- `src/commands.rs` — thin adapters over `AppCore` (logic covered via `application/service.rs`)

Rationale: Tauri WebView APIs cannot run headlessly in this CI shape; business risk is covered by service + integration tests.

## Risk scenarios covered

- API rate limit → backoff; cache not wiped
- Widget hidden → zero provider calls
- Corrupt / missing state JSON → safe defaults
- Duplicate / empty symbol rejected
- Unknown remove id / bad reorder → error, no data loss
- Geometry / opacity clamp
- Yahoo 429 / 500 mapping

## Frontend

No automated UI e2e yet. Typecheck via `npm run build`. Optional future: Playwright against Vite mock.
