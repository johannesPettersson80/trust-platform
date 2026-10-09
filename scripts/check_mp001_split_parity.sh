#!/usr/bin/env bash
set -euo pipefail

# Force byte-wise sort semantics so baseline ordering is stable across locales.
export LC_ALL=C

MODE="${1:---verify}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASELINE_DIR="${ROOT_DIR}/tests/fixtures/mp001"
TMP_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

capture_test_discovery() {
  local tag="$1"
  "${ROOT_DIR}/scripts/cargo_test_fast_link.sh" test -p trust-lsp handlers::tests:: -- --list > "${TMP_DIR}/lsp-${tag}.raw"
  grep ': test$' "${TMP_DIR}/lsp-${tag}.raw" | sed 's/\r$//' | sort > "${TMP_DIR}/lsp-${tag}.list"

  "${ROOT_DIR}/scripts/cargo_test_fast_link.sh" test -p trust-hir --test semantic_type_checking -- --list > "${TMP_DIR}/hir-${tag}.raw"
  grep ': test$' "${TMP_DIR}/hir-${tag}.raw" | sed 's/\r$//' | sort > "${TMP_DIR}/hir-${tag}.list"
}

capture_snapshot_inventory() {
  find "${ROOT_DIR}/crates/trust-lsp/src/handlers/snapshots" -type f -name '*.snap' \
    | sed "s#^${ROOT_DIR}/##" \
    | sort \
    > "${TMP_DIR}/lsp-snapshots.list"
  find "${ROOT_DIR}/crates/trust-hir/tests/snapshots" -type f -name '*.snap' \
    | sed "s#^${ROOT_DIR}/##" \
    | sort \
    > "${TMP_DIR}/hir-snapshots.list"
}

capture_all() {
  local tag="$1"
  capture_test_discovery "${tag}"
  capture_snapshot_inventory
}

write_baseline() {
  mkdir -p "${BASELINE_DIR}"
  cp "${TMP_DIR}/lsp-baseline.list" "${BASELINE_DIR}/lsp-test-discovery-baseline.list"
  cp "${TMP_DIR}/hir-baseline.list" "${BASELINE_DIR}/hir-type-checking-test-discovery-baseline.list"
  cp "${TMP_DIR}/lsp-snapshots.list" "${BASELINE_DIR}/lsp-snapshots-baseline.list"
  cp "${TMP_DIR}/hir-snapshots.list" "${BASELINE_DIR}/hir-snapshots-baseline.list"
  echo "Captured MP-001 baseline into ${BASELINE_DIR}"
}

ensure_baseline_exists() {
  local file="$1"
  if [[ ! -f "${file}" ]]; then
    echo "missing baseline file: ${file}"
    echo "run: scripts/check_mp001_split_parity.sh --capture-baseline"
    exit 1
  fi
}

if [[ "${MODE}" == "--capture-baseline" ]]; then
  capture_all "baseline"
  write_baseline
  exit 0
fi

if [[ "${MODE}" != "--verify" ]]; then
  echo "usage: scripts/check_mp001_split_parity.sh [--verify|--capture-baseline]"
  exit 2
fi

cd "${ROOT_DIR}"
capture_all "run1"

ensure_baseline_exists "${BASELINE_DIR}/lsp-test-discovery-baseline.list"
ensure_baseline_exists "${BASELINE_DIR}/hir-type-checking-test-discovery-baseline.list"
ensure_baseline_exists "${BASELINE_DIR}/lsp-snapshots-baseline.list"
ensure_baseline_exists "${BASELINE_DIR}/hir-snapshots-baseline.list"

diff -u "${BASELINE_DIR}/lsp-test-discovery-baseline.list" "${TMP_DIR}/lsp-run1.list"
diff -u "${BASELINE_DIR}/hir-type-checking-test-discovery-baseline.list" "${TMP_DIR}/hir-run1.list"
diff -u "${BASELINE_DIR}/lsp-snapshots-baseline.list" "${TMP_DIR}/lsp-snapshots.list"
diff -u "${BASELINE_DIR}/hir-snapshots-baseline.list" "${TMP_DIR}/hir-snapshots.list"

capture_test_discovery "run2"
capture_test_discovery "run3"

diff -u "${TMP_DIR}/lsp-run1.list" "${TMP_DIR}/lsp-run2.list"
diff -u "${TMP_DIR}/lsp-run1.list" "${TMP_DIR}/lsp-run3.list"
diff -u "${TMP_DIR}/hir-run1.list" "${TMP_DIR}/hir-run2.list"
diff -u "${TMP_DIR}/hir-run1.list" "${TMP_DIR}/hir-run3.list"

echo "MP-001 parity check passed:"
echo "  - LSP handler tests: $(wc -l < "${TMP_DIR}/lsp-run1.list") cases"
echo "  - HIR semantic type-checking tests: $(wc -l < "${TMP_DIR}/hir-run1.list") cases"
