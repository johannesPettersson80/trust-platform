#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
CORPUS_DIR="${RUNTIME_VM_SYNTAX_CORPUS_DIR:-${ROOT_DIR}/docs/internal/testing/local/runtime_vm_syntax_corpus}"
python3 "${ROOT_DIR}/scripts/runtime_vm_syntax_corpus_local_bootstrap.py" "${CORPUS_DIR}"
