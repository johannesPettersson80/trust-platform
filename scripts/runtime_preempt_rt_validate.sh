#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

source "${ROOT_DIR}/scripts/runtime_host_codegen.sh"

PROJECT="${PROJECT:-examples/plcopen_motion_single_axis_demo}"
OUT_DIR="${OUT_DIR:-target/gate-artifacts/runtime-preempt-rt}"
SAMPLES="${TRUST_RT_BENCH_SAMPLES:-256}"
WARMUP_CYCLES="${TRUST_RT_BENCH_WARMUP_CYCLES:-64}"
SERVICE_NAME="${TRUST_RT_SERVICE:-trust-runtime}"
CYCLICTEST_INTERVAL_US="${TRUST_RT_CYCLICTEST_INTERVAL_US:-1000}"
CYCLICTEST_LOOPS="${TRUST_RT_CYCLICTEST_LOOPS:-60000}"
CYCLICTEST_PRIORITY="${TRUST_RT_CYCLICTEST_PRIORITY:-80}"
P95_LIMIT_US="${TRUST_RT_CYCLE_P95_MAX_US:-0}"
DECLARED_P95_LIMIT_US="${P95_LIMIT_US}"
MAX_OVERRUNS="${TRUST_RT_MAX_OVERRUNS:-0}"
CONTROL_PROJECT="${TRUST_RT_CONTROL_PROJECT:-}"
REQUIRE_PREEMPT_RT="${TRUST_RT_REQUIRE_PREEMPT:-0}"
SOAK_SECONDS="${TRUST_RT_SOAK_SECONDS:-0}"

mkdir -p "${OUT_DIR}"

BUILD_MODE="$(trust_runtime_detect_host_codegen_mode)"
trust_runtime_build_release_binary "${BUILD_MODE}"
RUNTIME_BIN="$(trust_runtime_release_binary_path)"

BENCH_JSON="${OUT_DIR}/bench-project.json"
SYSTEM_INFO_TXT="${OUT_DIR}/system-info.txt"
SYSTEMD_TXT="${OUT_DIR}/systemd-show.txt"
CHRT_TXT="${OUT_DIR}/chrt.txt"
TASKSET_TXT="${OUT_DIR}/taskset.txt"
VMLCK_TXT="${OUT_DIR}/vm-lck.txt"
CYCLICTEST_TXT="${OUT_DIR}/cyclictest.txt"
CONTROL_CONFIG_JSON="${OUT_DIR}/control-config.json"
SUMMARY_MD="${OUT_DIR}/summary.md"
SUMMARY_JSON="${OUT_DIR}/summary.json"

KERNEL_REALTIME="unknown"
KERNEL_REALTIME_SOURCE="unavailable"
VALIDATION_MODE="baseline"
KERNEL_REQUIREMENT_ERROR=""
THRESHOLD_REQUIREMENT_ERROR=""
CYCLICTEST_REQUIREMENT_ERROR=""
CYCLICTEST_WARNING=""
SOAK_REQUIREMENT_ERROR=""
THRESHOLD_WARNING=""
THRESHOLD_DECLARED="false"
CYCLICTEST_INSTALLED="false"

load_cycle_budget_us_from_project() {
  python3 - "$PROJECT" <<'PY'
from pathlib import Path
import sys
import tomllib

project = Path(sys.argv[1])
for candidate in (project / "runtime.toml", project / "src" / "runtime.toml"):
    if not candidate.is_file():
        continue
    with candidate.open("rb") as handle:
        data = tomllib.load(handle)
    cycle_ms = data.get("resource", {}).get("cycle_interval_ms")
    if cycle_ms is None:
        continue
    print(int(round(float(cycle_ms) * 1000.0)))
    sys.exit(0)
PY
}

required_samples_for_soak() {
  python3 - "$1" "$2" <<'PY'
import math
import sys

soak_seconds = float(sys.argv[1])
cycle_budget_us = float(sys.argv[2])
print(max(1, math.ceil((soak_seconds * 1_000_000.0) / cycle_budget_us)))
PY
}

detect_kernel_realtime() {
  if [[ -r /sys/kernel/realtime ]]; then
    KERNEL_REALTIME_SOURCE="/sys/kernel/realtime"
    if [[ "$(tr -d '[:space:]' </sys/kernel/realtime)" == "1" ]]; then
      KERNEL_REALTIME="true"
      VALIDATION_MODE="preempt-rt"
    else
      KERNEL_REALTIME="false"
    fi
    return
  fi

  local release=""
  local config_path=""
  if [[ -r /proc/sys/kernel/osrelease ]]; then
    release="$(tr -d '[:space:]' </proc/sys/kernel/osrelease)"
    config_path="/boot/config-${release}"
  fi
  if [[ -n "${config_path}" && -r "${config_path}" ]]; then
    KERNEL_REALTIME_SOURCE="${config_path}"
    if grep -q '^CONFIG_PREEMPT_RT=y$' "${config_path}"; then
      KERNEL_REALTIME="true"
      VALIDATION_MODE="preempt-rt"
      return
    fi
    if grep -q '^# CONFIG_PREEMPT_RT is not set$' "${config_path}"; then
      KERNEL_REALTIME="false"
      return
    fi
  fi

  if [[ -r /proc/version ]]; then
    local version_text
    version_text="$(cat /proc/version)"
    KERNEL_REALTIME_SOURCE="/proc/version"
    if grep -q 'PREEMPT_RT' <<<"${version_text}"; then
      KERNEL_REALTIME="true"
      VALIDATION_MODE="preempt-rt"
      return
    fi
    if grep -q ' PREEMPT ' <<<"${version_text}"; then
      KERNEL_REALTIME="false"
      return
    fi
  fi
}

detect_kernel_realtime

if [[ "${P95_LIMIT_US}" != "0" ]]; then
  THRESHOLD_DECLARED="true"
fi

if [[ "${SOAK_SECONDS}" != "0" ]]; then
  cycle_budget_us_from_project="$(load_cycle_budget_us_from_project)"
  if [[ -z "${cycle_budget_us_from_project}" ]]; then
    SOAK_REQUIREMENT_ERROR="TRUST_RT_SOAK_SECONDS requires a readable runtime.toml with resource.cycle_interval_ms"
  else
    required_samples="$(required_samples_for_soak "${SOAK_SECONDS}" "${cycle_budget_us_from_project}")"
    if [[ "${required_samples}" -gt "${SAMPLES}" ]]; then
      SAMPLES="${required_samples}"
    fi
  fi
fi

if [[ "${REQUIRE_PREEMPT_RT}" == "1" && "${KERNEL_REALTIME}" != "true" ]]; then
  KERNEL_REQUIREMENT_ERROR="TRUST_RT_REQUIRE_PREEMPT=1 but kernel evidence did not confirm PREEMPT_RT"
fi
if [[ "${REQUIRE_PREEMPT_RT}" == "1" && "${THRESHOLD_DECLARED}" != "true" ]]; then
  THRESHOLD_REQUIREMENT_ERROR="TRUST_RT_REQUIRE_PREEMPT=1 requires TRUST_RT_CYCLE_P95_MAX_US to declare a target-specific threshold"
elif [[ "${VALIDATION_MODE}" == "preempt-rt" && "${THRESHOLD_DECLARED}" != "true" ]]; then
  THRESHOLD_WARNING="kernel is PREEMPT_RT but no explicit TRUST_RT_CYCLE_P95_MAX_US threshold was declared; p95 falls back to the cycle budget and should not be treated as validation evidence"
fi

{
  echo "# PREEMPT_RT Validation"
  echo
  echo "date=$(date -Is)"
  echo "project=${PROJECT}"
  echo "build_mode=${BUILD_MODE}"
  echo "validation_mode=${VALIDATION_MODE}"
  echo "kernel_realtime=${KERNEL_REALTIME}"
  echo "kernel_realtime_source=${KERNEL_REALTIME_SOURCE}"
  echo
  echo "## uname"
  uname -a || true
  echo
  echo "## /sys/kernel/realtime"
  if [[ -r /sys/kernel/realtime ]]; then
    cat /sys/kernel/realtime
  else
    echo "unavailable"
  fi
  echo
  echo "## lscpu"
  if command -v lscpu >/dev/null 2>&1; then
    lscpu
  else
    echo "lscpu unavailable"
  fi
} > "${SYSTEM_INFO_TXT}"

echo "[preempt-rt] running trust-runtime bench project on ${PROJECT}"
"${RUNTIME_BIN}" \
  bench project \
  --project "${PROJECT}" \
  --samples "${SAMPLES}" \
  --warmup-cycles "${WARMUP_CYCLES}" \
  --output json \
  > "${BENCH_JSON}"

if command -v systemctl >/dev/null 2>&1; then
  systemctl show "${SERVICE_NAME}" \
    -p MainPID \
    -p CPUSchedulingPolicy \
    -p CPUSchedulingPriority \
    -p CPUAffinity \
    -p LimitMEMLOCK \
    -p LimitRTPRIO \
    > "${SYSTEMD_TXT}" || true
fi

main_pid=""
if [[ -f "${SYSTEMD_TXT}" ]]; then
  main_pid="$(awk -F= '$1=="MainPID" {print $2}' "${SYSTEMD_TXT}" | tail -n1)"
fi

if [[ -n "${main_pid}" && "${main_pid}" != "0" ]]; then
  if command -v chrt >/dev/null 2>&1; then
    chrt -p "${main_pid}" > "${CHRT_TXT}" || true
  fi
  if command -v taskset >/dev/null 2>&1; then
    taskset -pc "${main_pid}" > "${TASKSET_TXT}" || true
  fi
  if [[ -r "/proc/${main_pid}/status" ]]; then
    grep '^VmLck:' "/proc/${main_pid}/status" > "${VMLCK_TXT}" || true
  fi
fi

if command -v cyclictest >/dev/null 2>&1; then
  CYCLICTEST_INSTALLED="true"
  # Do not run cyclictest concurrently with the runtime bench. This script
  # sequences them deliberately so the higher-priority cyclictest thread does
  # not starve the runtime benchmark and corrupt the evidence run.
  echo "[preempt-rt] running cyclictest"
  cyclictest \
    -m \
    -p "${CYCLICTEST_PRIORITY}" \
    -i "${CYCLICTEST_INTERVAL_US}" \
    -l "${CYCLICTEST_LOOPS}" \
    -q \
    > "${CYCLICTEST_TXT}" || true
else
  echo "[preempt-rt] cyclictest not installed; skipping kernel baseline capture" > "${CYCLICTEST_TXT}"
  if [[ "${REQUIRE_PREEMPT_RT}" == "1" ]]; then
    CYCLICTEST_REQUIREMENT_ERROR="TRUST_RT_REQUIRE_PREEMPT=1 requires cyclictest from rt-tests to be installed"
  elif [[ "${VALIDATION_MODE}" == "preempt-rt" ]]; then
    CYCLICTEST_WARNING="cyclictest is missing; kernel-level RT evidence is incomplete"
  fi
fi

if [[ -n "${CONTROL_PROJECT}" ]]; then
  "${RUNTIME_BIN}" ctl --project "${CONTROL_PROJECT}" config-get > "${CONTROL_CONFIG_JSON}" || true
fi

read_json() {
  local query="$1"
  jq -r "${query}" "${BENCH_JSON}"
}

float_le() {
  local left="$1"
  local right="$2"
  awk -v a="${left}" -v b="${right}" 'BEGIN { exit (a <= b) ? 0 : 1 }'
}

multiply_to_seconds() {
  python3 - "$1" "$2" <<'PY'
import sys

samples = float(sys.argv[1])
cycle_budget_us = float(sys.argv[2])
print((samples * cycle_budget_us) / 1_000_000.0)
PY
}

cycle_budget_us="$(read_json '.report.cycle_budget_us')"
p50_us="$(read_json '.report.cycle_latency.p50_us')"
p95_us="$(read_json '.report.cycle_latency.p95_us')"
p99_us="$(read_json '.report.cycle_latency.p99_us')"
max_us="$(read_json '.report.cycle_latency.max_us')"
budget_overruns="$(read_json '.report.budget_overruns')"

if [[ "${P95_LIMIT_US}" == "0" ]]; then
  P95_LIMIT_US="${cycle_budget_us}"
fi

observed_window_seconds="$(multiply_to_seconds "${SAMPLES}" "${cycle_budget_us}")"

if [[ "${SOAK_SECONDS}" != "0" ]] && ! float_le "${SOAK_SECONDS}" "${observed_window_seconds}"; then
  SOAK_REQUIREMENT_ERROR="requested soak window ${SOAK_SECONDS}s exceeds measured window ${observed_window_seconds}s"
fi

result="pass"
if [[ "${budget_overruns}" -gt "${MAX_OVERRUNS}" ]]; then
  result="fail"
fi
if ! float_le "${p95_us}" "${P95_LIMIT_US}"; then
  result="fail"
fi
if [[ -n "${KERNEL_REQUIREMENT_ERROR}" ]]; then
  result="fail"
fi
if [[ -n "${THRESHOLD_REQUIREMENT_ERROR}" ]]; then
  result="fail"
fi
if [[ -n "${CYCLICTEST_REQUIREMENT_ERROR}" ]]; then
  result="fail"
fi
if [[ -n "${SOAK_REQUIREMENT_ERROR}" ]]; then
  result="fail"
fi

cat > "${SUMMARY_MD}" <<MD
# PREEMPT_RT Validation Summary

- project: ${PROJECT}
- build mode: ${BUILD_MODE}
- validation mode: ${VALIDATION_MODE}
- kernel realtime detected: ${KERNEL_REALTIME}
- kernel evidence source: ${KERNEL_REALTIME_SOURCE}
- samples: ${SAMPLES}
- warmup cycles: ${WARMUP_CYCLES}
- soak requested: ${SOAK_SECONDS} s
- approximate measurement window: ${observed_window_seconds} s
- cycle budget: ${cycle_budget_us} us
- cycle p50: ${p50_us} us
- cycle p95: ${p95_us} us
- cycle p99: ${p99_us} us
- cycle max: ${max_us} us
- threshold declared explicitly: ${THRESHOLD_DECLARED}
- declared cycle p95 limit: $(if [[ "${THRESHOLD_DECLARED}" == "true" ]]; then printf '%s us' "${DECLARED_P95_LIMIT_US}"; else printf 'n/a'; fi)
- cycle p95 limit: ${P95_LIMIT_US} us
- budget overruns: ${budget_overruns}
- max overruns allowed: ${MAX_OVERRUNS}
- cyclictest installed: ${CYCLICTEST_INSTALLED}
- service name: ${SERVICE_NAME}
- service main pid: ${main_pid:-unavailable}
- require PREEMPT_RT: ${REQUIRE_PREEMPT_RT}
- result: ${result}

$(if [[ -n "${KERNEL_REQUIREMENT_ERROR}" ]]; then
    printf '%s\n' "- kernel requirement error: ${KERNEL_REQUIREMENT_ERROR}"
  fi)
$(if [[ -n "${THRESHOLD_REQUIREMENT_ERROR}" ]]; then
    printf '%s\n' "- threshold requirement error: ${THRESHOLD_REQUIREMENT_ERROR}"
  fi)
$(if [[ -n "${THRESHOLD_WARNING}" ]]; then
    printf '%s\n' "- threshold warning: ${THRESHOLD_WARNING}"
  fi)
$(if [[ -n "${CYCLICTEST_REQUIREMENT_ERROR}" ]]; then
    printf '%s\n' "- cyclictest requirement error: ${CYCLICTEST_REQUIREMENT_ERROR}"
  fi)
$(if [[ -n "${CYCLICTEST_WARNING}" ]]; then
    printf '%s\n' "- cyclictest warning: ${CYCLICTEST_WARNING}"
  fi)
$(if [[ -n "${SOAK_REQUIREMENT_ERROR}" ]]; then
    printf '%s\n' "- soak requirement error: ${SOAK_REQUIREMENT_ERROR}"
  fi)

Artifacts:

- ${SYSTEM_INFO_TXT}
- ${SYSTEMD_TXT}
- ${CHRT_TXT}
- ${TASKSET_TXT}
- ${VMLCK_TXT}
- ${CYCLICTEST_TXT}
- ${BENCH_JSON}
- ${CONTROL_CONFIG_JSON}
MD

jq -n \
  --arg project "${PROJECT}" \
  --arg build_mode "${BUILD_MODE}" \
  --arg validation_mode "${VALIDATION_MODE}" \
  --arg kernel_realtime "${KERNEL_REALTIME}" \
  --arg kernel_realtime_source "${KERNEL_REALTIME_SOURCE}" \
  --arg kernel_requirement_error "${KERNEL_REQUIREMENT_ERROR}" \
  --arg threshold_requirement_error "${THRESHOLD_REQUIREMENT_ERROR}" \
  --arg threshold_warning "${THRESHOLD_WARNING}" \
  --arg require_preempt_rt "${REQUIRE_PREEMPT_RT}" \
  --argjson samples "${SAMPLES}" \
  --argjson warmup_cycles "${WARMUP_CYCLES}" \
  --arg soak_seconds "${SOAK_SECONDS}" \
  --arg observed_window_seconds "${observed_window_seconds}" \
  --arg service_name "${SERVICE_NAME}" \
  --arg main_pid "${main_pid:-}" \
  --arg result "${result}" \
  --arg p50_us "${p50_us}" \
  --arg p95_us "${p95_us}" \
  --arg p99_us "${p99_us}" \
  --arg max_us "${max_us}" \
  --arg threshold_declared "${THRESHOLD_DECLARED}" \
  --arg declared_p95_limit_us "${DECLARED_P95_LIMIT_US}" \
  --arg p95_limit_us "${P95_LIMIT_US}" \
  --arg cycle_budget_us "${cycle_budget_us}" \
  --arg budget_overruns "${budget_overruns}" \
  --arg max_overruns "${MAX_OVERRUNS}" \
  --arg cyclictest_installed "${CYCLICTEST_INSTALLED}" \
  --arg cyclictest_interval_us "${CYCLICTEST_INTERVAL_US}" \
  --arg cyclictest_loops "${CYCLICTEST_LOOPS}" \
  --arg cyclictest_priority "${CYCLICTEST_PRIORITY}" \
  --arg cyclictest_requirement_error "${CYCLICTEST_REQUIREMENT_ERROR}" \
  --arg cyclictest_warning "${CYCLICTEST_WARNING}" \
  --arg soak_requirement_error "${SOAK_REQUIREMENT_ERROR}" \
  --arg system_info "${SYSTEM_INFO_TXT}" \
  --arg systemd_show "${SYSTEMD_TXT}" \
  --arg chrt "${CHRT_TXT}" \
  --arg taskset "${TASKSET_TXT}" \
  --arg vm_lck "${VMLCK_TXT}" \
  --arg cyclictest "${CYCLICTEST_TXT}" \
  --arg bench_json "${BENCH_JSON}" \
  --arg control_config_json "${CONTROL_CONFIG_JSON}" \
  '{
    project: $project,
    build_mode: $build_mode,
    validation_mode: $validation_mode,
    kernel: {
      realtime_detected: $kernel_realtime,
      evidence_source: $kernel_realtime_source,
      require_preempt_rt: ($require_preempt_rt == "1"),
      requirement_error: (if ($kernel_requirement_error | length) > 0 then $kernel_requirement_error else null end)
    },
    samples: $samples,
    warmup_cycles: $warmup_cycles,
    thresholds: {
      declared: ($threshold_declared == "true"),
      declared_p95_limit_us: (if ($threshold_declared == "true") then ($declared_p95_limit_us | tonumber) else null end),
      effective_p95_limit_us: ($p95_limit_us | tonumber),
      requirement_error: (if ($threshold_requirement_error | length) > 0 then $threshold_requirement_error else null end),
      warning: (if ($threshold_warning | length) > 0 then $threshold_warning else null end)
    },
    soak: {
      requested_seconds: ($soak_seconds | tonumber),
      approximate_window_seconds: ($observed_window_seconds | tonumber),
      requirement_error: (if ($soak_requirement_error | length) > 0 then $soak_requirement_error else null end)
    },
    cyclictest: {
      installed: ($cyclictest_installed == "true"),
      interval_us: ($cyclictest_interval_us | tonumber),
      loops: ($cyclictest_loops | tonumber),
      priority: ($cyclictest_priority | tonumber),
      requirement_error: (if ($cyclictest_requirement_error | length) > 0 then $cyclictest_requirement_error else null end),
      warning: (if ($cyclictest_warning | length) > 0 then $cyclictest_warning else null end)
    },
    service: {
      name: $service_name,
      main_pid: $main_pid
    },
    bench: {
      cycle_budget_us: ($cycle_budget_us | tonumber),
      p50_us: ($p50_us | tonumber),
      p95_us: ($p95_us | tonumber),
      p99_us: ($p99_us | tonumber),
      max_us: ($max_us | tonumber),
      p95_limit_us: ($p95_limit_us | tonumber),
      budget_overruns: ($budget_overruns | tonumber),
      max_overruns: ($max_overruns | tonumber)
    },
    artifacts: {
      system_info: $system_info,
      systemd_show: $systemd_show,
      chrt: $chrt,
      taskset: $taskset,
      vm_lck: $vm_lck,
      cyclictest: $cyclictest,
      bench_json: $bench_json,
      control_config_json: $control_config_json
    },
    result: $result
  }' > "${SUMMARY_JSON}"

if [[ "${result}" != "pass" ]]; then
  echo "[preempt-rt] FAIL: see ${SUMMARY_MD}" >&2
  exit 1
fi

echo "[preempt-rt] PASS"
