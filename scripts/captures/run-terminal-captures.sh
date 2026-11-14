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

require_cmd cross
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

# The tapes run the programs in the vhs image, a Debian stable system. A program built on this
# computer binds this computer's glibc and stops at start where that glibc is newer than the
# image's: on 14 November 2026 every terminal capture showed only "version `GLIBC_2.43' not
# found". So the programs are built in cross, as the release builds its Linux packages, and the
# container sees that build as target/debug, the folder the tapes name (specification 22, "Docs
# Captures"). The repository's cargo configuration names sccache as the compiler wrapper, which the
# cross image does not have, so the build runs without one, as the release's does.
CROSS_CONTAINER_OPTS="--env RUSTC_WRAPPER=" \
  cross build --target x86_64-unknown-linux-gnu -p trust-lsp -p trust-runtime -p trust-debug -p trust-dev
programs="${CARGO_TARGET_DIR:-$ROOT_DIR/target}/x86_64-unknown-linux-gnu/debug"
# The folder the build is mounted over, made here so that Docker does not make it as root.
mkdir -p "$ROOT_DIR/target/debug"

# A capture of a program that does not start shows only the loader's error, so each program that
# the tapes run starts once in the image first, and the captures stop when one does not.
for program in trust-runtime trust-dev; do
  if ! docker run --rm \
    -v "$programs:/workspaces/trust-platform/target/debug:ro" \
    --entrypoint "/workspaces/trust-platform/target/debug/$program" \
    "$VHS_IMAGE" \
    --version >/dev/null; then
    echo "$program does not start in $VHS_IMAGE" >&2
    exit 1
  fi
done

# The captures that build a program need the toolchain of specification 44 §4.6. The container
# sees the folder TRUST_TOOLCHAIN names, and the Rust toolchain its compiler and linker link into,
# at the same paths. Without TRUST_TOOLCHAIN the list stays empty, and Bash 3.2 (macOS's /bin/bash)
# calls an empty array unbound under `set -u`, so the list is expanded only when it has elements.
toolchain_args=()
if [[ -n "${TRUST_TOOLCHAIN:-}" ]]; then
  sysroot=$(rustc --print sysroot)
  toolchain_args=(
    -e "TRUST_TOOLCHAIN=$TRUST_TOOLCHAIN"
    -v "$TRUST_TOOLCHAIN:$TRUST_TOOLCHAIN:ro"
    -v "$sysroot:$sysroot:ro"
  )
fi

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
    -v "$programs:/workspaces/trust-platform/target/debug" \
    -w /workspaces/trust-platform \
    ${toolchain_args[@]+"${toolchain_args[@]}"} \
    "$VHS_IMAGE" \
    "$repo_relative_tape"
done
