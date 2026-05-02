#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMMIT="$(git -C "$ROOT" rev-parse --short HEAD)"
ARTIFACT_DIR="$ROOT/target/gate-artifacts/unsafe-concurrency-tools-${COMMIT}"
OUT="$ARTIFACT_DIR/valgrind-startup.txt"

mkdir -p "$ARTIFACT_DIR"

{
  valgrind --version
  cargo build -p trust-runtime --bin trust-runtime
  valgrind --error-exitcode=99 --leak-check=summary \
    "$ROOT/target/debug/trust-runtime" --help
} 2>&1 | tee "$OUT"
