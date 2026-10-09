#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
CAPTURE_DIR="$ROOT_DIR/scripts/captures"
MODE="${1:-all}"
OWNED_CAPTURE_PID=""
CODESERVER_OWNED=0

cleanup() {
  if [[ -n "$OWNED_CAPTURE_PID" ]]; then
    kill -TERM "$OWNED_CAPTURE_PID" >/dev/null 2>&1 || true
    wait "$OWNED_CAPTURE_PID" >/dev/null 2>&1 || true
    OWNED_CAPTURE_PID=""
  fi
  if [[ "$CODESERVER_OWNED" -eq 1 ]]; then
    docker rm -f "${TRUST_CAPTURE_CODESERVER_CONTAINER:-trust-docs-code-server}" >/dev/null 2>&1 || true
    CODESERVER_OWNED=0
  fi
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

require_cmd cargo
require_cmd npm
require_cmd setsid

run_owned_capture() {
  local status

  "$CAPTURE_DIR/run-owned-command.sh" "$@" &
  OWNED_CAPTURE_PID=$!
  set +e
  wait "$OWNED_CAPTURE_PID"
  status=$?
  set -e
  OWNED_CAPTURE_PID=""
  return "$status"
}

if [[ ! -d "$CAPTURE_DIR/node_modules" ]]; then
  npm --prefix "$CAPTURE_DIR" ci
fi

if [[ "$MODE" == "browser" || "$MODE" == "all" ]]; then
  cargo build -p trust-runtime
  "$ROOT_DIR/scripts/build_browser_analysis_wasm_spike.sh"
  run_owned_capture npm --prefix "$CAPTURE_DIR" run capture:browser
fi

if [[ "$MODE" == "vscode" || "$MODE" == "all" ]]; then
  require_cmd docker
  cargo build -p trust-lsp
  npm --prefix "$ROOT_DIR/editors/vscode" ci
  npm --prefix "$ROOT_DIR/editors/vscode" run compile
  CODESERVER_OWNED=1
  docker rm -f "${TRUST_CAPTURE_CODESERVER_CONTAINER:-trust-docs-code-server}" >/dev/null 2>&1 || true
  run_owned_capture npm --prefix "$CAPTURE_DIR" run capture:vscode
  docker rm -f "${TRUST_CAPTURE_CODESERVER_CONTAINER:-trust-docs-code-server}" >/dev/null 2>&1 || true
  CODESERVER_OWNED=0
fi

if [[ "$MODE" != "browser" && "$MODE" != "vscode" && "$MODE" != "all" ]]; then
  echo "Unknown capture mode: $MODE" >&2
  exit 1
fi
