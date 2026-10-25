#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:-$HOME/trust-cloud-topology-demo}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
BIN="${TRUST_RUNTIME_BIN:-${REPO_ROOT}/target/debug/trust-runtime}"
BOOTSTRAP_SCRIPT="${SCRIPT_DIR}/bootstrap-3-realistic.sh"
STOP_SCRIPT="${SCRIPT_DIR}/stop-3-realistic.sh"

runtime_ids=("a-control" "a-gateway" "b-control")

runtime_web_port() {
  case "$1" in
    "a-control") echo "19181" ;;
    "a-gateway") echo "19182" ;;
    "b-control") echo "19183" ;;
    *) return 1 ;;
  esac
}

ensure_bootstrapped_projects() {
  expected_host_group() {
    case "$1" in
      "a-control") echo "cell-a-edge-ipc" ;;
      "a-gateway") echo "cell-a-edge-ipc" ;;
      "b-control") echo "cell-b-edge-ipc" ;;
      *) echo "" ;;
    esac
  }

  local missing=0
  for id in "${runtime_ids[@]}"; do
    local project_dir="${ROOT}/runtime-${id}"
    if [[ ! -f "${project_dir}/runtime.toml" || ! -f "${project_dir}/io.toml" || ! -f "${project_dir}/program.tpi" ]]; then
      echo "[start-3] missing or incomplete project: ${project_dir}"
      missing=1
      continue
    fi
    local expected_group
    expected_group="$(expected_host_group "${id}")"
    if [[ -n "${expected_group}" ]]; then
      local current_group
      current_group="$(awk -F'"' '/^[[:space:]]*host_group[[:space:]]*=/ { print $2; exit }' "${project_dir}/runtime.toml" || true)"
      if [[ "${current_group}" != "${expected_group}" ]]; then
        echo "[start-3] stale host_group in ${project_dir}/runtime.toml: '${current_group}' -> '${expected_group}'"
        missing=1
      fi
    fi
  done

  if [[ "${missing}" -ne 0 || "${TRUST_TOPOLOGY_DEMO_REFRESH:-0}" == "1" ]]; then
    if [[ "${TRUST_TOPOLOGY_DEMO_REFRESH:-0}" == "1" ]]; then
      echo "[start-3] refreshing demo files (TRUST_TOPOLOGY_DEMO_REFRESH=1)."
    else
      echo "[start-3] bootstrapping missing demo projects."
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

validate_projects() {
  if [[ "${TRUST_TOPOLOGY_DEMO_VALIDATE:-0}" != "1" ]]; then
    return 0
  fi
  for id in "${runtime_ids[@]}"; do
    local project_dir="${ROOT}/runtime-${id}"
    echo "[start-3] validate runtime-${id}"
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

  echo "[start-3] runtime-${id} did not become ready on ${state_url}"
  if [[ -f "${log_file}" ]]; then
    echo "[start-3] last log lines (${log_file}):"
    tail -n 40 "${log_file}" || true
  fi
  return 1
}

wait_for_topology_convergence() {
  if ! command -v jq >/dev/null 2>&1; then
    return 0
  fi

  local deadline=$((SECONDS + 30))
  local state_url="http://127.0.0.1:19181/api/runtime-cloud/state"
  while (( SECONDS < deadline )); do
    local node_count
    node_count="$(curl -fsS "${state_url}" 2>/dev/null | jq -r '.topology.nodes | length' 2>/dev/null || true)"
    if [[ "${node_count}" == "3" ]]; then
      echo "[start-3] topology converged: 3 runtime nodes visible from runtime-a-control."
      return 0
    fi
    sleep 1
  done
  echo "[start-3] topology not fully converged yet (continuing; mesh may still be settling)."
}

if [[ ! -x "${BIN}" ]]; then
  BIN="$(command -v trust-runtime || true)"
fi

if [[ -z "${BIN}" || ! -x "${BIN}" ]]; then
  echo "[start-3] trust-runtime binary not found."
  echo "[start-3] build first (from repo root): cargo build -p trust-runtime"
  exit 1
fi

ensure_bootstrapped_projects
clear_stale_runtime_cloud_state
validate_projects

if "${STOP_SCRIPT}" "${ROOT}" >/dev/null 2>&1; then
  echo "[start-3] previous demo processes stopped."
fi

for id in "${runtime_ids[@]}"; do
  port="$(runtime_web_port "${id}")"
  log="/tmp/trust-topology-demo-${id}.log"
  pid="/tmp/trust-topology-demo-${id}.pid"
  nohup "${BIN}" play --no-console --project "${ROOT}/runtime-${id}" >"${log}" 2>&1 &
  echo $! >"${pid}"
  echo "[start-3] runtime-${id}: pid $(cat "${pid}") web http://127.0.0.1:${port}/ log ${log}"
  wait_for_runtime_web "${id}" "${port}"
done

wait_for_topology_convergence || true

echo "[start-3] realistic layout:"
echo "  cell-a-edge-ipc: runtime-a-control (realtime), runtime-a-gateway (non-realtime comms)"
echo "  cell-b-edge-ipc: runtime-b-control (realtime)"
echo "[start-3] link intent from runtime-a-control.toml:"
echo "  realtime: runtime-a-control -> runtime-b-control"
echo "  zenoh: runtime-a-control -> runtime-a-gateway (mesh remains for all runtimes)"
echo "[start-3] dashboards:"
for id in "${runtime_ids[@]}"; do
  port="$(runtime_web_port "${id}")"
  echo "  runtime-${id}: http://127.0.0.1:${port}/"
done
