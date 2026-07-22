#!/usr/bin/env bash
# Business-logic coverage gate for EconomyWarRoom.
# Excludes GUI bootstrap (lib.rs run(), window_ctl OS bindings, thin Tauri command adapters).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT/src-tauri"
# shellcheck disable=SC1090
source "${HOME}/.cargo/env" 2>/dev/null || true

EXCLUDE=(
  --exclude-files 'src/main.rs'
  --exclude-files 'src/lib.rs'
  --exclude-files 'src/infrastructure/window_ctl.rs'
  --exclude-files 'src/commands.rs'
)

echo "== cargo test (lib + integration) =="
cargo test --lib
cargo test --test integration_e2e --test risk_scenarios

echo "== tarpaulin (fail-under 85) =="
cargo tarpaulin --lib \
  --tests \
  --out Stdout \
  --out Html \
  --output-dir target/coverage \
  --timeout 180 \
  --fail-under 85 \
  "${EXCLUDE[@]}"

echo "Coverage HTML: $ROOT/src-tauri/target/coverage/tarpaulin-report.html"
