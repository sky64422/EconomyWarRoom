# Testing & Coverage

**Updated:** 2026-07-23

## Snapshot

| Metric | Value |
|--------|--------|
| Unit tests (`cargo test --lib`) | ~63 |
| Integration (`integration_e2e`) | 4 |
| Risk scenarios (`risk_scenarios`) | 7 |
| Coverage gate | **≥ 85%** business logic |
| Last measured | **~98%** business logic via tarpaulin |

## Layers

| Layer | Location | Purpose |
|-------|----------|---------|
| Unit | `src-tauri/src/**` `#[cfg(test)]` | Domain, scheduler, queue, parse, store, **AppCore**, Yahoo wiremock |
| HTTP mock | `yahoo/client` + **wiremock** | 200 / 429 / 5xx without live Yahoo |
| Integration | `src-tauri/tests/integration_e2e.rs` | Store + AppCore + scheduler + mock Yahoo HTTP pipeline |
| Risk | `src-tauri/tests/risk_scenarios.rs` | Rate limit, hide pause, corrupt JSON, invalid IDs |
| GUI smoke | Manual `npm run run:exe` | Hotkey, window chrome (not in automated coverage) |

## Commands

From repo root:

```bash
npm test                 # cargo test --lib + integration_e2e + risk_scenarios
npm run test:coverage    # scripts/coverage.sh (fail-under 85)
npm run build            # frontend tsc + vite
```

Equivalent cargo:

```bash
cd src-tauri
cargo test --lib
cargo test --test integration_e2e --test risk_scenarios
./../scripts/coverage.sh
```

HTML report: `src-tauri/target/coverage/tarpaulin-report.html`

## Coverage policy

**Included in the gate:**

- `domain/`
- `application/` (`cache`, `queue`, `scheduler`, **`service`**)
- `infrastructure/store`, `infrastructure/yahoo`
- `ports/`, `state/`

**Excluded from the gate (GUI / OS glue):**

| File | Why |
|------|-----|
| `src/main.rs` | Binary entry |
| `src/lib.rs` | Tauri `run()`, plugins, hotkey registration, tick loop wiring |
| `src/infrastructure/window_ctl.rs` | Needs live `WebviewWindow` |
| `src/commands.rs` | Thin adapters over `AppCore` (logic covered in `application/service.rs`) |

Rationale: Tauri WebView APIs do not run headlessly in this CI shape. Product risk for watchlist, rates, and persistence is covered by service + integration + risk tests.

## Risk scenarios covered

- API rate limit → backoff; **cache not wiped**  
- Widget hidden → **zero** provider calls  
- Corrupt / missing state JSON → safe defaults  
- Duplicate / empty symbol rejected  
- Unknown remove id / bad reorder → error, **list unchanged** (atomic reorder)  
- Geometry / opacity clamp  
- Yahoo **429** / **500** status mapping  
- Save creates missing parent directories  

## Integration scenarios covered

- Watchlist add / reorder / remove + **disk reload** after “restart”  
- Scheduler refresh while visible; pause when hidden  
- Yahoo mock HTTP end-to-end into quote + sparkline caches  
- Seed default state save/load  

## Frontend

- Automated UI e2e: **not yet**  
- Typecheck: `npm run build`  
- Optional future: Playwright against Vite + mocked `invoke`  

## Related

- [ARCHITECTURE.md](./ARCHITECTURE.md)  
- [TODO.md](./TODO.md) manual smoke (P5-2 / P5-3)  
