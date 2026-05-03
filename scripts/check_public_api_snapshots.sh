#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
SHORT_COMMIT=$(git -C "$ROOT_DIR" rev-parse --short HEAD 2>/dev/null || echo "unknown")
BASELINE_DIR="$ROOT_DIR/docs/internal/architecture/public-api"
ARTIFACT_DIR="$ROOT_DIR/target/gate-artifacts/public-api-snapshots-$SHORT_COMMIT"
UPDATE=0

if [[ "${1:-}" == "--update" ]]; then
  UPDATE=1
fi

mkdir -p "$BASELINE_DIR" "$ARTIFACT_DIR"

cargo public-api --version >"$ARTIFACT_DIR/cargo-public-api-version.txt"
rustc --version >"$ARTIFACT_DIR/rustc-version.txt"

check_snapshot() {
  local package="$1"
  local manifest="$2"
  local baseline="$BASELINE_DIR/$package.txt"
  local actual="$ARTIFACT_DIR/$package.txt"
  local diff_file="$ARTIFACT_DIR/$package.diff"

  cargo public-api --manifest-path "$manifest" -sss --color never >"$actual"

  if [[ "$UPDATE" -eq 1 ]]; then
    cp "$actual" "$baseline"
    : >"$diff_file"
    echo "updated $baseline"
    return 0
  fi

  if [[ ! -s "$baseline" ]]; then
    echo "missing public API baseline: $baseline" | tee "$diff_file"
    return 1
  fi

  if ! diff -u "$baseline" "$actual" >"$diff_file"; then
    echo "public API snapshot drift for $package; see $diff_file"
    return 1
  fi

  echo "public API snapshot ok: $package"
}

check_snapshot trust-runtime "$ROOT_DIR/crates/trust-runtime/Cargo.toml"
check_snapshot trust-plcopen "$ROOT_DIR/crates/trust-plcopen/Cargo.toml"
check_snapshot trust-runtime-core "$ROOT_DIR/crates/trust-runtime-core/Cargo.toml"
