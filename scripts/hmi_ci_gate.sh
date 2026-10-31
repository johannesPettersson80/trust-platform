#!/usr/bin/env bash
# Deterministic HMI parity and browser gate; caller owns Cargo target leasing.
set -euo pipefail
cd "$(dirname "$0")/.."
mode=${1:-all}
case "$mode" in all|bundles|browser) ;; *) echo 'usage: hmi_ci_gate.sh [all|bundles|browser]' >&2; exit 2;; esac
if [[ "$mode" != browser ]]; then
  if [[ "${HMI_REUSE_EDITOR_BUILD:-0}" != 1 ]]; then
    npm --prefix editors/vscode ci
    npm --prefix editors/vscode run build:hmi-builder
  fi
  npm --prefix editors/vscode run build:hmi-drawing
  git diff --exit-code -- crates/trust-runtime/src/web/ui/modules/hmi-drawing.js crates/trust-runtime/src/web/ui/modules/hmi-display.js
fi
if [[ "$mode" != bundles ]]; then
  cargo build -p trust-runtime --bin trust-runtime
  export TRUST_RUNTIME="${CARGO_TARGET_DIR:-$PWD/target}/debug/trust-runtime"
  npm --prefix scripts/hmi-screen-tests ci
  cd scripts/hmi-screen-tests
  npx playwright install chromium
  npx playwright test
fi
