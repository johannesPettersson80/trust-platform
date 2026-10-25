#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:-$HOME/trust-cloud-topology-demo}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEMPLATE_DIR="$(cd "${SCRIPT_DIR}/../../communication/multi_driver" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
BIN="${TRUST_RUNTIME_BIN:-${REPO_ROOT}/target/debug/trust-runtime}"
runtime_ids=("a-control" "a-gateway" "b-control")

echo "[bootstrap-3] target root: ${ROOT}"
mkdir -p "${ROOT}"

copy_runtime() {
  local id="$1"
  local project_dir="${ROOT}/runtime-${id}"

  mkdir -p "${project_dir}/src"
  cp "${SCRIPT_DIR}/runtime-${id}.toml" "${project_dir}/runtime.toml"
  cp "${SCRIPT_DIR}/io-${id}.toml" "${project_dir}/io.toml"
  cp "${TEMPLATE_DIR}/trust-lsp.toml" "${project_dir}/trust-lsp.toml"
  cp "${TEMPLATE_DIR}/src/main.st" "${project_dir}/src/main.st"
  cp "${TEMPLATE_DIR}/src/config.st" "${project_dir}/src/config.st"
  # Each project is compiled for this computer with its own runtime.toml and io.toml.
  "${BIN}" build --project "${project_dir}" >/dev/null

  # Always reset stale runtime-cloud local state for realistic TOML-only baseline.
  rm -f "${project_dir}/.trust/runtime-cloud/link-transport-state.json"
}

for id in "${runtime_ids[@]}"; do
  copy_runtime "${id}"
done

echo "[bootstrap-3] done. TOML/API-only topology (no manual JSON seeds)."
echo "[bootstrap-3] runtimes: runtime-a-control, runtime-a-gateway, runtime-b-control"
echo "[bootstrap-3] start with:"
echo "  ${SCRIPT_DIR}/start-3-realistic.sh ${ROOT}"
