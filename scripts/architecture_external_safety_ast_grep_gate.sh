#!/usr/bin/env bash
# Rust owns scanner scope and the unsafe admission decision; AST tools supply facts.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
exec cargo run --locked -p xtask -- architecture-external-safety
