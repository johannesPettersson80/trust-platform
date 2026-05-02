#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMMIT="$(git -C "$ROOT" rev-parse --short HEAD)"
ARTIFACT_DIR="$ROOT/target/gate-artifacts/unsafe-concurrency-tools-${COMMIT}"
OUT="$ARTIFACT_DIR/sanitizer-smoke.txt"
HOST="$(rustc +nightly -Vv | awk '/^host:/ { print $2 }')"

mkdir -p "$ARTIFACT_DIR"

{
  echo "host=$HOST"
  echo "command=RUSTFLAGS=-Zsanitizer=address cargo +nightly test -Zbuild-std --target $HOST -p trust-runtime-core --lib --no-default-features"
  rustc +nightly -Vv
  if ! rustc +nightly -Z help 2>/dev/null | rg -q "sanitizer"; then
    echo "PARTIAL: nightly rustc does not expose -Z sanitizer on this host"
    exit 0
  fi
  RUSTFLAGS="-Zsanitizer=address" cargo +nightly test -Zbuild-std --target "$HOST" \
    -p trust-runtime-core --lib --no-default-features
} 2>&1 | tee "$OUT"
