#!/bin/bash
# Quick runner for local iterative tests

set -euo pipefail

cd "$(dirname "$0")" || exit 1

echo "=== Recompilando y ejecutando ==="
rm -rf .trust-lsp program.stbc
trust-runtime build --project . --sources src

echo "Iniciando runtime..."
echo "Presiona Ctrl+C para detener"
echo ""

trust-runtime run --project .
