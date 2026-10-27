#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:-$HOME/trust-cloud-topology-demo}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
BIN="${TRUST_RUNTIME_BIN:-${REPO_ROOT}/target/debug/trust-runtime}"
BOOTSTRAP_SCRIPT="${SCRIPT_DIR}/bootstrap.sh"
STOP_SCRIPT="${SCRIPT_DIR}/stop-5.sh"

runtime_ids=(a b c d e)

runtime_web_port() {
  case "$1" in
    a) echo "19081" ;;
    b) echo "19082" ;;
    c) echo "19083" ;;
    d) echo "19084" ;;
    e) echo "19085" ;;
    *) return 1 ;;
  esac
}

ensure_bootstrapped_projects() {
  local missing=0
  for id in "${runtime_ids[@]}"; do
    local project_dir="${ROOT}/runtime-${id}"
    if [[ ! -f "${project_dir}/runtime.toml" || ! -f "${project_dir}/io.toml" || ! -f "${project_dir}/program.tpi" ]]; then
      echo "[start] missing or incomplete project: ${project_dir}"
      missing=1
    fi
  done

  if [[ "${missing}" -ne 0 || "${TRUST_TOPOLOGY_DEMO_REFRESH:-0}" == "1" ]]; then
    if [[ "${TRUST_TOPOLOGY_DEMO_REFRESH:-0}" == "1" ]]; then
      echo "[start] refreshing demo files (TRUST_TOPOLOGY_DEMO_REFRESH=1)."
    else
      echo "[start] bootstrapping missing demo projects."
    fi
    "${BOOTSTRAP_SCRIPT}" "${ROOT}"
  fi
}

clear_stale_runtime_cloud_state() {
  for id in "${runtime_ids[@]}"; do
    local project_dir="${ROOT}/runtime-${id}"
    rm -f "${project_dir}/.trust/runtime-cloud/link-transport-state.json"
  done
}

apply_optional_seeds() {
  if [[ "${TRUST_TOPOLOGY_DEMO_SEED_LINK_JSON:-0}" == "1" ]]; then
    mkdir -p "${ROOT}/runtime-a/.trust/runtime-cloud"
    cp "${SCRIPT_DIR}/link-transport.runtime-a.json" \
      "${ROOT}/runtime-a/.trust/runtime-cloud/link-transport-state.json"
  fi
}

validate_projects() {
  if [[ "${TRUST_TOPOLOGY_DEMO_VALIDATE:-0}" != "1" ]]; then
    return 0
  fi
  for id in "${runtime_ids[@]}"; do
    local project_dir="${ROOT}/runtime-${id}"
    echo "[start] validate runtime-${id}"
    "${BIN}" validate --project "${project_dir}"
  done
}

wait_for_runtime_web() {
  local id="$1"
  local port="$2"
  local deadline=$((SECONDS + 20))
  local state_url="http://127.0.0.1:${port}/api/runtime-cloud/state"
  local log_file="/tmp/trust-topology-demo-${id}.log"

  while (( SECONDS < deadline )); do
    if curl -fsS "${state_url}" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done

  echo "[start] runtime-${id} did not become ready on ${state_url}"
  if [[ -f "${log_file}" ]]; then
    echo "[start] last log lines (${log_file}):"
    tail -n 40 "${log_file}" || true
  fi
  return 1
}

wait_for_topology_convergence() {
  if ! command -v jq >/dev/null 2>&1; then
    return 0
  fi

  local deadline=$((SECONDS + 30))
  local state_url="http://127.0.0.1:19081/api/runtime-cloud/state"
  while (( SECONDS < deadline )); do
    local node_count
    node_count="$(curl -fsS "${state_url}" 2>/dev/null | jq -r '.topology.nodes | length' 2>/dev/null || true)"
    if [[ "${node_count}" == "5" ]]; then
      echo "[start] topology converged: 5 runtime nodes visible from runtime-a."
      return 0
    fi
    sleep 1
  done
  echo "[start] topology not fully converged yet (continuing; mesh may still be settling)."
}

if [[ ! -x "${BIN}" ]]; then
  BIN="$(command -v trust-runtime || true)"
fi

if [[ -z "${BIN}" || ! -x "${BIN}" ]]; then
  echo "[start] trust-runtime binary not found."
  echo "[start] build first (from repo root): cargo build -p trust-runtime"
  exit 1
fi

ensure_bootstrapped_projects
if [[ "${TRUST_TOPOLOGY_DEMO_SEED_LINK_JSON:-0}" == "1" ]]; then
  apply_optional_seeds
else
  clear_stale_runtime_cloud_state
fi
validate_projects

if "${STOP_SCRIPT}" "${ROOT}" >/dev/null 2>&1; then
  echo "[start] previous demo processes stopped."
fi

for id in "${runtime_ids[@]}"; do
  port="$(runtime_web_port "${id}")"
  log="/tmp/trust-topology-demo-${id}.log"
  pid="/tmp/trust-topology-demo-${id}.pid"
  nohup "${BIN}" play --no-console --project "${ROOT}/runtime-${id}" >"${log}" 2>&1 &
  echo $! >"${pid}"
  echo "[start] runtime-${id}: pid $(cat "${pid}") web http://127.0.0.1:${port}/ log ${log}"
  wait_for_runtime_web "${id}" "${port}"
done

wait_for_topology_convergence || true

echo "[start] mesh pattern (explicit runtime.mesh.connect):"
echo "  runtime-a -> runtime-b, runtime-d, runtime-e"
echo "  runtime-b -> runtime-a, runtime-c"
echo "  runtime-c -> runtime-b, runtime-d"
echo "  runtime-d -> runtime-a, runtime-c, runtime-e"
echo "  runtime-e -> runtime-a, runtime-d"
echo "[start] link transports configured in runtime-a.toml:"
echo "  realtime: runtime-a->runtime-b"
echo "  zenoh:    runtime-a->runtime-c, runtime-a->runtime-d, runtime-a->runtime-e"
echo "[start] dashboards:"
for id in "${runtime_ids[@]}"; do
  port="$(runtime_web_port "${id}")"
  echo "  runtime-${id}: http://127.0.0.1:${port}/"
done
