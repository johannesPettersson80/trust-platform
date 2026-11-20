#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

source "${ROOT_DIR}/scripts/runtime_host_codegen.sh"

OUT_DIR="${OUT_DIR:-target/gate-artifacts/runtime-motion-benchmark-breakdown}"
SAMPLES="${TRUST_MOTION_BREAKDOWN_SAMPLES:-128}"
WARMUP_CYCLES="${TRUST_MOTION_BREAKDOWN_WARMUP_CYCLES:-32}"
mkdir -p "${OUT_DIR}"

BUILD_MODE="$(trust_runtime_detect_host_codegen_mode)"
echo "[motion-breakdown] build mode: ${BUILD_MODE}"
trust_runtime_build_release_binary "${BUILD_MODE}"
BENCH_CMD=("$(trust_runtime_release_binary_path)")

common_watch=(
  --watch g_motion_bench_cycles
  --watch g_motion_bench_completed_sequences
  --watch g_motion_bench_last_error
  --watch g_motion_bench_power_on
  --watch g_motion_bench_is_homed
)

demo_watch=(
  --watch g_motion_demo_completed_sequences
  --watch g_motion_demo_last_error
  --watch g_motion_demo_limit_clamp_verified
  --watch g_motion_demo_power_on
  --watch g_motion_demo_is_homed
)

run_bench() {
  local name="$1"
  local project="$2"
  shift 2
  echo "[motion-breakdown] benchmarking ${name} (${project})"
  "${BENCH_CMD[@]}" bench project \
    --project "${project}" \
    --samples "${SAMPLES}" \
    --warmup-cycles "${WARMUP_CYCLES}" \
    "$@" \
    --output json > "${OUT_DIR}/${name}.json"
}

run_bench runtime_floor examples/plcopen_motion_single_axis_benchmarks/runtime_floor "${common_watch[@]}"
run_bench constants_only examples/plcopen_motion_single_axis_benchmarks/constants_only "${common_watch[@]}"
run_bench status_only examples/plcopen_motion_single_axis_benchmarks/status_only "${common_watch[@]}"
run_bench command_idle examples/plcopen_motion_single_axis_benchmarks/command_idle "${common_watch[@]}"
run_bench move_absolute_only examples/plcopen_motion_single_axis_benchmarks/move_absolute_only "${common_watch[@]}"
run_bench full_demo_constants_once examples/plcopen_motion_single_axis_benchmarks/full_demo_constants_once "${common_watch[@]}"
run_bench full_demo examples/plcopen_motion_single_axis_demo "${demo_watch[@]}"

BUILD_MODE_ENV="${BUILD_MODE}" OUT_DIR_ENV="${OUT_DIR}" python3 - <<'PY'
import json
import os
from pathlib import Path

out_dir = Path(os.environ['OUT_DIR_ENV'])
build_mode = os.environ['BUILD_MODE_ENV']
rows = []
order = [
    'runtime_floor',
    'constants_only',
    'status_only',
    'command_idle',
    'move_absolute_only',
    'full_demo_constants_once',
    'full_demo',
]
for name in order:
    path = out_dir / f'{name}.json'
    data = json.loads(path.read_text())
    report = data['report']
    watched = report.get('watched_globals', {})
    if path.stem == 'full_demo':
        completed = watched.get('g_motion_demo_completed_sequences')
        last_error = watched.get('g_motion_demo_last_error')
    else:
        completed = watched.get('g_motion_bench_completed_sequences')
        last_error = watched.get('g_motion_bench_last_error')
    rows.append({
        'name': path.stem,
        'p50_us': report['cycle_latency']['p50_us'],
        'p95_us': report['cycle_latency']['p95_us'],
        'p99_us': report['cycle_latency']['p99_us'],
        'max_us': report['cycle_latency']['max_us'],
        'overruns': report['budget_overruns'],
        'completed': completed,
        'last_error': last_error,
    })

floor = next(row for row in rows if row['name'] == 'runtime_floor')
floor_p50 = floor['p50_us'] or 1.0
for row in rows:
    row['p50_vs_floor'] = row['p50_us'] / floor_p50

runtime_floor = json.loads((out_dir / 'runtime_floor.json').read_text())['report']
header = '| profile | p50 us | p95 us | p99 us | max us | overruns | completed | last error | p50 vs floor |\n'
sep = '|---|---:|---:|---:|---:|---:|---:|---:|---:|\n'
body = ''.join(
    f"| {row['name']} | {row['p50_us']:.3f} | {row['p95_us']:.3f} | {row['p99_us']:.3f} | {row['max_us']:.3f} | {row['overruns']} | {row['completed']} | {row['last_error']} | {row['p50_vs_floor']:.1f}x |\n"
    for row in rows
)
summary = (
    '# Motion Benchmark Breakdown\n\n'
    f"- build mode: {build_mode}\n"
    f"- samples: {runtime_floor['samples']}\n"
    f"- warmup cycles: {runtime_floor['warmup_cycles']}\n\n"
    + header + sep + body
)
(out_dir / 'summary.md').write_text(summary)
(out_dir / 'summary.json').write_text(json.dumps({'build_mode': build_mode, 'rows': rows}, indent=2))
print(summary)
PY
