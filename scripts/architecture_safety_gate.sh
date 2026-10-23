#!/usr/bin/env bash
set -euo pipefail

# Respect the caller's pinned tools; keep Cargo's default bin as a fallback.
export PATH="$PATH:${CARGO_HOME:-$HOME/.cargo}/bin"

if ! command -v rg >/dev/null 2>&1; then
  sudo apt-get update
  sudo apt-get install -y ripgrep
fi

if ! command -v ast-grep >/dev/null 2>&1; then
  cargo install ast-grep --version 0.50.0 --locked
fi

if ! cargo public-api --version >/dev/null 2>&1; then
  cargo install cargo-public-api --version 0.52.0 --locked
fi

./scripts/architecture_external_safety_ast_grep_gate.sh
./scripts/runtime_boundary_fail_closed_ast_grep_gate.sh
./scripts/runtime_safety_fail_closed_ast_grep_gate.sh
cargo run -p xtask -- architecture-doctor --full-map
