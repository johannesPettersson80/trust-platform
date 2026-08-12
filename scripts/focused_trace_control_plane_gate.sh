#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
exec python3 -m unittest \
  scripts.verification.focused_trace_authentication_tests \
  scripts.verification.coverage_trace_cli_tests \
  scripts.verification.coverage_trace_document_tests \
  scripts.verification.coverage_trace_tools_tests \
  scripts.verification.mapping_candidates_cli_tests
