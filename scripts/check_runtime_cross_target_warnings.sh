#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

INSTALL_MISSING=0
REQUIRE_CROSS=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --install-missing)
      INSTALL_MISSING=1
      ;;
    --require-cross)
      REQUIRE_CROSS=1
      ;;
    *)
      echo "Unknown argument: $1"
      echo "Usage: $0 [--install-missing] [--require-cross]"
      exit 1
      ;;
  esac
  shift
done

WINDOWS_TARGET="x86_64-pc-windows-gnu"

ensure_target_installed() {
  local target="$1"
  if rustup target list --installed | grep -qx "${target}"; then
    return 0
  fi

  if [[ "${INSTALL_MISSING}" -eq 1 ]]; then
    echo "Installing missing Rust target: ${target}"
    rustup target add "${target}"
    return 0
  fi

  echo "Missing Rust target ${target}."
  echo "Install it with: rustup target add ${target}"
  return 1
}

has_windows_gnu_compiler() {
  command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1
}

echo "[runtime-warning-gate] Host target warning-deny check"
env RUSTFLAGS=-Dwarnings cargo check -p trust-runtime --all-targets

echo "[runtime-warning-gate] Windows cross-target warning-deny check (${WINDOWS_TARGET})"
if ! ensure_target_installed "${WINDOWS_TARGET}"; then
  if [[ "${REQUIRE_CROSS}" -eq 1 ]]; then
    exit 1
  fi
  echo "Skipping windows cross-target check: missing target ${WINDOWS_TARGET}."
  exit 0
fi

if ! has_windows_gnu_compiler; then
  if [[ "${REQUIRE_CROSS}" -eq 1 ]]; then
    echo "Missing cross-compiler x86_64-w64-mingw32-gcc for ${WINDOWS_TARGET}."
    echo "Install with your package manager (for example: apt-get install gcc-mingw-w64-x86-64)."
    exit 1
  fi
  echo "Skipping windows cross-target check: missing x86_64-w64-mingw32-gcc."
  exit 0
fi

# Clippy, not only the compiler, on the crates with Windows-only code: the Linux Clippy job never compiles that code,
# so Windows-only lints reached main (W40, specification 22).
env RUSTFLAGS=-Dwarnings cargo clippy -p trust-runtime -p trust-tcads-native -p trust-control -p trust-hir -p trust-lsp \
  --all-targets --target "${WINDOWS_TARGET}" -- -D warnings
