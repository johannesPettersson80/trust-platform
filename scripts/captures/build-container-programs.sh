#!/usr/bin/env bash
# Builds the programs that the captures run in Linux containers, the vhs image of the terminal
# captures and the code-server image of the VS Code captures, and prints the folder that holds them.
#
# Both images are Debian stable systems. A program built on this computer binds this computer's
# glibc and stops at start where that glibc is newer than the image's: on 14 November 2026 every
# terminal capture showed only "version `GLIBC_2.43' not found", and a language server built on the
# same system stops in the code-server image the same way. So the programs are built in cross for
# x86_64-unknown-linux-gnu, as the release builds its Linux packages (specification 22, "Docs
# Captures"). The repository's cargo configuration names sccache as the compiler wrapper, which the
# cross image does not have, so the build runs without one, as the release's does.
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)

if ! command -v cross >/dev/null 2>&1; then
  echo "Missing required command: cross" >&2
  exit 1
fi

# cargo resolves a relative CARGO_TARGET_DIR from the folder it runs in, which is the repository's.
target_dir="${CARGO_TARGET_DIR:-$ROOT_DIR/target}"
if [[ "$target_dir" != /* ]]; then
  target_dir="$ROOT_DIR/$target_dir"
fi

# The build reports on stderr, so that stdout carries only the folder.
(
  cd "$ROOT_DIR"
  CROSS_CONTAINER_OPTS="--env RUSTC_WRAPPER=" \
    cross build --target x86_64-unknown-linux-gnu -p trust-lsp -p trust-runtime -p trust-debug -p trust-dev >&2
)
echo "$target_dir/x86_64-unknown-linux-gnu/debug"
