#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
TRUST_RUNTIME_BIN="${TRUST_RUNTIME_BIN:-$ROOT_DIR/target/debug/trust-runtime}"
PROJECT_DIR="${TRUST_CAPTURE_IDE_PROJECT:-$ROOT_DIR/examples/tutorials/12_hmi_pid_process_dashboard}"
LISTEN_ADDR="${TRUST_CAPTURE_IDE_LISTEN:-127.0.0.1:18080}"

if [[ ! -x "$TRUST_RUNTIME_BIN" ]]; then
  echo "Missing trust-runtime binary at $TRUST_RUNTIME_BIN. Run cargo build -p trust-runtime first." >&2
  exit 1
fi

exec "$TRUST_RUNTIME_BIN" ide serve --project "$PROJECT_DIR" --listen "$LISTEN_ADDR"
