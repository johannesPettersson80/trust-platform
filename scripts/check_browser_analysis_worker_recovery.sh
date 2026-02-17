#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LEGACY_TEST_FILE="${ROOT_DIR}/docs/internal/prototypes/browser_analysis_wasm_spike/web/analysis-client.test.mjs"
DEMO_TEST_FILE="${ROOT_DIR}/docs/demo/lsp-position-resolver.test.mjs"

if ! command -v node >/dev/null 2>&1; then
  echo "error: node is required to run browser worker recovery tests."
  exit 1
fi

if [[ -f "${LEGACY_TEST_FILE}" ]]; then
  node --test "${LEGACY_TEST_FILE}"
else
  echo "warning: skipped missing legacy worker recovery test: ${LEGACY_TEST_FILE}"
fi

if [[ -f "${DEMO_TEST_FILE}" ]]; then
  node --test "${DEMO_TEST_FILE}"
else
  echo "error: missing demo LSP position fallback test: ${DEMO_TEST_FILE}"
  exit 1
fi

echo "Browser analysis worker recovery tests passed."
