#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LEGACY_TEST_FILE="${ROOT_DIR}/docs/internal/prototypes/browser_analysis_wasm_spike/web/analysis-client.test.mjs"
DEMO_TEST_FILE="${ROOT_DIR}/docs/demo/lsp-position-resolver.test.mjs"
WALKTHROUGH_TEST_FILE="${ROOT_DIR}/docs/demo/walkthrough-actions.test.mjs"
MONACO_BUNDLE_IMPORTS_TEST_FILE="${ROOT_DIR}/crates/trust-runtime/web/ide-frontend/src/monaco-bundle-imports.test.mjs"
MONACO_BUNDLE_ARTIFACT_TEST_FILE="${ROOT_DIR}/docs/demo/monaco-bundle-artifact.test.mjs"
WASM_READY_RACE_TEST_FILE="${ROOT_DIR}/docs/demo/wasm-ready-race.test.mjs"

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

if [[ -f "${WALKTHROUGH_TEST_FILE}" ]]; then
  node --test "${WALKTHROUGH_TEST_FILE}"
else
  echo "error: missing demo walkthrough contract test: ${WALKTHROUGH_TEST_FILE}"
  exit 1
fi

if [[ -f "${MONACO_BUNDLE_IMPORTS_TEST_FILE}" ]]; then
  node --test "${MONACO_BUNDLE_IMPORTS_TEST_FILE}"
else
  echo "error: missing monaco bundle import contract test: ${MONACO_BUNDLE_IMPORTS_TEST_FILE}"
  exit 1
fi

if [[ -f "${MONACO_BUNDLE_ARTIFACT_TEST_FILE}" ]]; then
  node --test "${MONACO_BUNDLE_ARTIFACT_TEST_FILE}"
else
  echo "error: missing monaco bundle artifact contract test: ${MONACO_BUNDLE_ARTIFACT_TEST_FILE}"
  exit 1
fi

if [[ -f "${WASM_READY_RACE_TEST_FILE}" ]]; then
  node --test "${WASM_READY_RACE_TEST_FILE}"
else
  echo "error: missing demo WASM ready race regression test: ${WASM_READY_RACE_TEST_FILE}"
  exit 1
fi

echo "Browser analysis worker recovery tests passed."
