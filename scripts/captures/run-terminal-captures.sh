#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
OUTPUT_DIR="$ROOT_DIR/docs/public/assets/images/terminal"
VHS_IMAGE="${TRUST_CAPTURE_VHS_IMAGE:-ghcr.io/charmbracelet/vhs:latest}"
VHS_HOME_HOST=$(mktemp -d)
VHS_HOME_CONTAINER=/tmp/trust-vhs-home

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

require_cmd cargo
require_cmd docker

mkdir -p "$OUTPUT_DIR"
mkdir -p \
  "$VHS_HOME_HOST/.config" \
  "$VHS_HOME_HOST/.cache" \
  "$VHS_HOME_HOST/.runtime"

cleanup() {
  rm -rf "$VHS_HOME_HOST"
}

trap cleanup EXIT

cargo build -p trust-lsp -p trust-runtime -p trust-debug

for tape in "$ROOT_DIR"/scripts/captures/terminal/*.tape; do
  repo_relative_tape="${tape#$ROOT_DIR/}"
  docker run --rm \
    -u "$(id -u):$(id -g)" \
    -e HOME="$VHS_HOME_CONTAINER" \
    -e XDG_CONFIG_HOME="$VHS_HOME_CONTAINER/.config" \
    -e XDG_CACHE_HOME="$VHS_HOME_CONTAINER/.cache" \
    -e XDG_RUNTIME_DIR="$VHS_HOME_CONTAINER/.runtime" \
    -v "$VHS_HOME_HOST:$VHS_HOME_CONTAINER" \
    -v "$ROOT_DIR:/workspaces/trust-platform" \
    -w /workspaces/trust-platform \
    "$VHS_IMAGE" \
    "$repo_relative_tape"
done
