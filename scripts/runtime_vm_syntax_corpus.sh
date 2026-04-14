#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

source "${ROOT_DIR}/scripts/runtime_host_codegen.sh"

CORPUS_DIR="${RUNTIME_VM_SYNTAX_CORPUS_DIR:-${ROOT_DIR}/docs/internal/testing/local/runtime_vm_syntax_corpus}"
OUT_BASE_DIR="${OUT_DIR:-target/gate-artifacts/runtime-vm-syntax-corpus}"
SAMPLES="${TRUST_VM_SYNTAX_CORPUS_SAMPLES:-128}"
WARMUP_CYCLES="${TRUST_VM_SYNTAX_CORPUS_WARMUP_CYCLES:-32}"
TIER="${TRUST_VM_SYNTAX_CORPUS_TIER:-default}"
OUT_DIR="${OUT_BASE_DIR}/${TIER}"
mkdir -p "${OUT_DIR}"

BUILD_MODE="$(trust_runtime_detect_host_codegen_mode)"

if [[ ! -d "${CORPUS_DIR}" ]]; then
  echo "[syntax-corpus] local corpus missing, bootstrapping ${CORPUS_DIR}"
  "${ROOT_DIR}/scripts/bootstrap_runtime_vm_syntax_corpus_local.sh" >/dev/null
fi

echo "[syntax-corpus] build mode: ${BUILD_MODE}"
trust_runtime_build_release_binary "${BUILD_MODE}"
BENCH_CMD=("$(trust_runtime_release_binary_path)")
COMMON_WATCH=(
  --watch g_motion_bench_cycles
  --watch g_motion_bench_completed_sequences
  --watch g_motion_bench_last_error
  --watch g_motion_bench_power_on
  --watch g_motion_bench_is_homed
)
DEMO_WATCH=(
  --watch g_motion_demo_completed_sequences
  --watch g_motion_demo_last_error
  --watch g_motion_demo_limit_clamp_verified
  --watch g_motion_demo_power_on
  --watch g_motion_demo_is_homed
)
TIER1_FLAG=()
if [[ "${TRUST_VM_SYNTAX_CORPUS_TIER1:-0}" != "0" ]]; then
  TIER1_FLAG=(--tier1)
fi

RUN_LIST=()
run_default() {
  RUN_LIST+=("runtime_floor|examples/plcopen_motion_single_axis_benchmarks/runtime_floor|common")
  RUN_LIST+=("scalar_globals_only|examples/plcopen_motion_single_axis_benchmarks/scalar_globals_only|common")
  RUN_LIST+=("loop_arith|${CORPUS_DIR}/loop_arith|common")
  RUN_LIST+=("branch_control|${CORPUS_DIR}/branch_control|common")
  RUN_LIST+=("call_binding|${CORPUS_DIR}/call_binding|common")
  RUN_LIST+=("trivial_fb_no_params|examples/plcopen_motion_single_axis_benchmarks/trivial_fb_no_params|common")
  RUN_LIST+=("trivial_fb_low_params|examples/plcopen_motion_single_axis_benchmarks/trivial_fb_low_params|common")
  RUN_LIST+=("trivial_fb_high_params|examples/plcopen_motion_single_axis_benchmarks/trivial_fb_high_params|common")
  RUN_LIST+=("dynamic_refs|examples/plcopen_motion_single_axis_benchmarks/dynamic_refs|common")
  RUN_LIST+=("composite_updates|${CORPUS_DIR}/composite_updates|common")
  RUN_LIST+=("bitwise_conversions|${CORPUS_DIR}/bitwise_conversions|common")
  RUN_LIST+=("string_stdlib|${CORPUS_DIR}/string_stdlib|common")
  RUN_LIST+=("status_only|examples/plcopen_motion_single_axis_benchmarks/status_only|common")
  RUN_LIST+=("command_idle|examples/plcopen_motion_single_axis_benchmarks/command_idle|common")
  RUN_LIST+=("move_absolute_only|examples/plcopen_motion_single_axis_benchmarks/move_absolute_only|common")
  RUN_LIST+=("full_demo|examples/plcopen_motion_single_axis_demo|demo")
}
run_extended() {
  RUN_LIST+=("refs_sizeof|${CORPUS_DIR}/refs_sizeof|common")
  RUN_LIST+=("call_heavy_callee_arith|${CORPUS_DIR}/call_heavy_callee_arith|common")
  RUN_LIST+=("time_date_stdlib|${CORPUS_DIR}/time_date_stdlib|common")
  RUN_LIST+=("method_receiver|${CORPUS_DIR}/method_receiver|common")
  RUN_LIST+=("full_demo_constants_once|examples/plcopen_motion_single_axis_benchmarks/full_demo_constants_once|common")
  RUN_LIST+=("constants_only|examples/plcopen_motion_single_axis_benchmarks/constants_only|common")
}
case "${TIER}" in
  default) run_default ;;
  extended) run_extended ;;
  all) run_default; run_extended ;;
  *) echo "[syntax-corpus] unknown tier: ${TIER}" >&2; exit 1 ;;
esac

FILTERED_RUN_LIST=()
for entry in "${RUN_LIST[@]}"; do
  IFS='|' read -r name project watchset <<<"${entry}"
  if [[ -d "${project}" ]]; then
    FILTERED_RUN_LIST+=("${name}|${project}|${watchset}")
  else
    echo "[syntax-corpus] skipping missing project ${project}" >&2
  fi
done
RUN_LIST=("${FILTERED_RUN_LIST[@]}")
if [[ "${#RUN_LIST[@]}" -eq 0 ]]; then
  echo "[syntax-corpus] no benchmark projects found for tier ${TIER}" >&2
  exit 1
fi

printf '{
  "tier": "%s",
  "build_mode": "%s",
  "samples": %s,
  "warmup_cycles": %s,
  "order": [' "${TIER}" "${BUILD_MODE}" "${SAMPLES}" "${WARMUP_CYCLES}" > "${OUT_DIR}/_meta.json"
for i in "${!RUN_LIST[@]}"; do
  IFS='|' read -r name _project _watch <<<"${RUN_LIST[$i]}"
  if [[ "$i" -gt 0 ]]; then printf ', ' >> "${OUT_DIR}/_meta.json"; fi
  printf '"%s"' "$name" >> "${OUT_DIR}/_meta.json"
done
printf ']
}
' >> "${OUT_DIR}/_meta.json"

START_NS="$(date +%s%N)"
run_bench() {
  local name="$1"
  local project="$2"
  local watchset="$3"
  local watch_args=()
  case "${watchset}" in
    common) watch_args=("${COMMON_WATCH[@]}") ;;
    demo) watch_args=("${DEMO_WATCH[@]}") ;;
    *) echo "[syntax-corpus] unknown watchset: ${watchset}" >&2; exit 1 ;;
  esac
  echo "[syntax-corpus] benchmarking ${name} (${project})"
  "${BENCH_CMD[@]}" bench project     --project "${project}"     --samples "${SAMPLES}"     --warmup-cycles "${WARMUP_CYCLES}"     "${TIER1_FLAG[@]}"     "${watch_args[@]}"     --output json > "${OUT_DIR}/${name}.json"
}
for entry in "${RUN_LIST[@]}"; do
  IFS='|' read -r name project watchset <<<"${entry}"
  run_bench "$name" "$project" "$watchset"
done
END_NS="$(date +%s%N)"
SUITE_MS="$(( (END_NS - START_NS) / 1000000 ))"
OUT_DIR_ENV="${OUT_DIR}" SUITE_MS_ENV="${SUITE_MS}" python3 - <<'PY2'
import json
import os
from pathlib import Path

out_dir = Path(os.environ['OUT_DIR_ENV'])
meta_path = out_dir / '_meta.json'
meta = json.loads(meta_path.read_text())
meta['suite_wall_clock_ms'] = int(os.environ['SUITE_MS_ENV'])
meta_path.write_text(json.dumps(meta, indent=2))
PY2

OUT_DIR_ENV="${OUT_DIR}" python3 - <<'PY2'
import json
import os
from pathlib import Path

out_dir = Path(os.environ['OUT_DIR_ENV'])
summary_meta = json.loads((out_dir / '_meta.json').read_text())
rows = []
for name in summary_meta['order']:
    report = json.loads((out_dir / f'{name}.json').read_text())['report']
    watched = report.get('watched_globals', {})
    if name == 'full_demo':
        completed = watched.get('g_motion_demo_completed_sequences')
        last_error = watched.get('g_motion_demo_last_error')
    else:
        completed = watched.get('g_motion_bench_completed_sequences')
        last_error = watched.get('g_motion_bench_last_error')
    vm_profile = report.get('vm_profile') or {}
    fallbacks = vm_profile.get('register_program_fallbacks', 0)
    fallback_reasons = vm_profile.get('fallback_reasons') or []
    tier1 = vm_profile.get('tier1_specialized_executor')
    if isinstance(tier1, dict):
        vm_highlight = (
            f"tier1 exec={tier1.get('block_executions', 0)} "
            f"cf={tier1.get('compile_failures', 0)} "
            f"deopt={tier1.get('deopt_count', 0)}"
        )
    elif fallback_reasons:
        vm_highlight = f"fallback:{fallback_reasons[0].get('reason', 'unknown')}"
    else:
        vm_highlight = 'vm-clean'
    rows.append({
        'name': name,
        'project': report['project'],
        'p50_us': report['cycle_latency']['p50_us'],
        'p95_us': report['cycle_latency']['p95_us'],
        'p99_us': report['cycle_latency']['p99_us'],
        'max_us': report['cycle_latency']['max_us'],
        'overruns': report['budget_overruns'],
        'completed': completed,
        'last_error': last_error,
        'fallbacks': fallbacks,
        'vm_highlight': vm_highlight,
        'measured_duration_ms': report.get('measured_duration_ms', 0.0),
    })
header = '| workload | p50 us | p95 us | p99 us | max us | overruns | completed | last error | fallbacks | vm highlight | measured ms |\n'
sep = '|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|\n'
body = ''.join(
    f"| {row['name']} | {row['p50_us']:.3f} | {row['p95_us']:.3f} | {row['p99_us']:.3f} | {row['max_us']:.3f} | {row['overruns']} | {row['completed']} | {row['last_error']} | {row['fallbacks']} | {row['vm_highlight']} | {row['measured_duration_ms']:.3f} |\n"
    for row in rows
)
total_measured = sum(row['measured_duration_ms'] for row in rows)
summary = (
    '# Runtime VM Syntax Corpus\n\n'
    f"- tier: {summary_meta['tier']}\n"
    f"- build mode: {summary_meta.get('build_mode', 'generic')}\n"
    f"- samples: {summary_meta['samples']}\n"
    f"- warmup cycles: {summary_meta['warmup_cycles']}\n"
    f"- suite wall-clock ms: {summary_meta['suite_wall_clock_ms']}\n"
    f"- total measured benchmark ms: {total_measured:.3f}\n\n"
    + header + sep + body
)
(out_dir / 'summary.md').write_text(summary)
(out_dir / 'summary.json').write_text(json.dumps({'tier': summary_meta['tier'], 'build_mode': summary_meta.get('build_mode', 'generic'), 'suite_wall_clock_ms': summary_meta['suite_wall_clock_ms'], 'rows': rows}, indent=2))
print(summary)
PY2
