#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUNDLE="$ROOT/docs/internal/testing/bundles/sample-runtime"
SOCK="/tmp/trust-runtime-smoke.sock"
BIN="$ROOT/target/debug/trust-runtime"
DO_GEN=1
DO_BUILD=1

while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-gen)
      DO_GEN=0
      shift
      ;;
    --no-build)
      DO_BUILD=0
      shift
      ;;
    --bundle|--project)
      BUNDLE="$2"
      shift 2
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

if [[ "$DO_GEN" -eq 1 ]]; then
  if [[ "$DO_BUILD" -eq 1 ]]; then
    "$ROOT/scripts/gen-sample-bundle.sh" --project "$BUNDLE" >/dev/null
  else
    "$ROOT/scripts/gen-sample-bundle.sh" --no-build --project "$BUNDLE" >/dev/null
  fi
fi

if [[ ! -f "$BUNDLE/program.tpi" ]]; then
  echo "Missing $BUNDLE/program.tpi" >&2
  exit 1
fi

rm -f "$SOCK"

cleanup() {
  if [[ -n "${PID:-}" ]] && kill -0 "$PID" 2>/dev/null; then
    "$BIN" ctl shutdown --project "$BUNDLE" >/dev/null 2>&1 || true
    kill "$PID" >/dev/null 2>&1 || true
  fi
  rm -f "$SOCK"
}
trap cleanup EXIT

if [[ "$DO_BUILD" -eq 1 ]]; then
  cargo build -p trust-runtime >/dev/null
fi

"$BIN" run --project "$BUNDLE" >/dev/null 2>&1 &
PID=$!

for _ in $(seq 1 50); do
  if [[ -S "$SOCK" ]]; then
    break
  fi
  sleep 0.1
done

"$BIN" ctl --project "$BUNDLE" --token trust-demo-token status
# `shutdown` alone stops the program and keeps the runtime running; `--exit` ends the process too.
"$BIN" ctl --project "$BUNDLE" --token trust-demo-token shutdown --exit >/dev/null
wait "$PID"
