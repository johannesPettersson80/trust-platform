#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
CAPTURE_DIR="$ROOT_DIR/scripts/captures"
MODE="${1:-all}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

require_cmd cargo
require_cmd npm

if [[ ! -d "$CAPTURE_DIR/node_modules" ]]; then
  npm --prefix "$CAPTURE_DIR" ci
fi

if [[ "$MODE" == "browser" || "$MODE" == "all" ]]; then
  cargo build -p trust-runtime
  "$ROOT_DIR/scripts/build_browser_analysis_wasm_spike.sh"
  npm --prefix "$CAPTURE_DIR" run capture:browser
fi

if [[ "$MODE" == "vscode" || "$MODE" == "all" ]]; then
  require_cmd docker
  cargo build -p trust-lsp
  npm --prefix "$ROOT_DIR/editors/vscode" ci
  npm --prefix "$ROOT_DIR/editors/vscode" run compile
  docker rm -f "${TRUST_CAPTURE_CODESERVER_CONTAINER:-trust-docs-code-server}" >/dev/null 2>&1 || true
  npm --prefix "$CAPTURE_DIR" run capture:vscode
fi

if [[ "$MODE" != "browser" && "$MODE" != "vscode" && "$MODE" != "all" ]]; then
  echo "Unknown capture mode: $MODE" >&2
  exit 1
fi
