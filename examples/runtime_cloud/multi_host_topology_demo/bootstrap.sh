#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:-$HOME/trust-cloud-topology-demo}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEMPLATE_DIR="$(cd "${SCRIPT_DIR}/../../communication/multi_driver" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
BIN="${TRUST_RUNTIME_BIN:-${REPO_ROOT}/target/debug/trust-runtime}"
runtime_ids=(a b c d e)

echo "[bootstrap] target root: ${ROOT}"
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

  # Default mode is TOML/API-only. Remove stale runtime-cloud local state from prior runs.
  rm -f "${project_dir}/.trust/runtime-cloud/link-transport-state.json"
}

for id in "${runtime_ids[@]}"; do
  copy_runtime "${id}"
done

if [[ "${TRUST_TOPOLOGY_DEMO_SEED_LINK_JSON:-0}" == "1" ]]; then
  mkdir -p "${ROOT}/runtime-a/.trust/runtime-cloud"
  cp \
    "${SCRIPT_DIR}/link-transport.runtime-a.json" \
    "${ROOT}/runtime-a/.trust/runtime-cloud/link-transport-state.json"
fi

echo "[bootstrap] done."
echo "[bootstrap] topology is TOML/API driven."
if [[ "${TRUST_TOPOLOGY_DEMO_SEED_LINK_JSON:-0}" == "1" ]]; then
  echo "[bootstrap] seeded optional link transport JSON on runtime-a."
else
  echo "[bootstrap] link transports come from runtime-a.toml [runtime.cloud.links]."
fi
echo "[bootstrap] start with:"
echo "  ${SCRIPT_DIR}/start-5.sh ${ROOT}"
