#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMMIT="$(git -C "$ROOT" rev-parse --short HEAD)"
ARTIFACT_DIR="$ROOT/target/gate-artifacts/unsafe-concurrency-tools-${COMMIT}"
OUT="$ARTIFACT_DIR/miri-focused.txt"

mkdir -p "$ARTIFACT_DIR"

{
  echo "command=cargo +nightly miri test -p trust-runtime-core --lib --no-default-features"
  cargo +nightly miri --version
  cargo +nightly miri setup
  cargo +nightly miri test -p trust-runtime-core --lib --no-default-features
} 2>&1 | tee "$OUT"
