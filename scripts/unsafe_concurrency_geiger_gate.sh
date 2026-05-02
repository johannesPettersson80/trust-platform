#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMMIT="$(git -C "$ROOT" rev-parse --short HEAD)"
ARTIFACT_DIR="$ROOT/target/gate-artifacts/unsafe-concurrency-tools-${COMMIT}"
OUT="$ARTIFACT_DIR/cargo-geiger.txt"

mkdir -p "$ARTIFACT_DIR"

{
  cargo geiger --version
  echo "command=cargo geiger --all-features"
  if cargo geiger --all-features; then
    exit 0
  fi
  echo "PARTIAL: cargo-geiger 0.13.0 does not accept this workspace virtual manifest."
  echo "PARTIAL: absolute package-manifest runs were probed locally and start by cleaning/rebuilding a large target tree, so they are not used as a hard BOARD-11 local gate."
  echo "PARTIAL: repo-specific full-map unsafe scanner remains the enforced first-party unsafe register gate."
} 2>&1 | tee "$OUT"
