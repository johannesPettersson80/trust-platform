#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
OUTPUT_DIR="$ROOT_DIR/docs/public/assets/images/terminal"
VHS_IMAGE="${TRUST_CAPTURE_VHS_IMAGE:-ghcr.io/charmbracelet/vhs:latest}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

require_cmd cargo
require_cmd docker

mkdir -p "$OUTPUT_DIR"

cargo build -p trust-lsp -p trust-runtime -p trust-debug

for tape in "$ROOT_DIR"/scripts/captures/terminal/*.tape; do
  repo_relative_tape="${tape#$ROOT_DIR/}"
  docker run --rm \
    -u "$(id -u):$(id -g)" \
    -v "$ROOT_DIR:/workspaces/trust-platform" \
    -w /workspaces/trust-platform \
    "$VHS_IMAGE" \
    "$repo_relative_tape"
done
