#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

OUT_DIR="${OUT_DIR:-target/gate-artifacts/runtime-motion-benchmark-breakdown}"
SAMPLES="${TRUST_MOTION_BREAKDOWN_SAMPLES:-128}"
WARMUP_CYCLES="${TRUST_MOTION_BREAKDOWN_WARMUP_CYCLES:-32}"
mkdir -p "${OUT_DIR}"

if [[ -x target/release/trust-runtime ]]; then
  BENCH_CMD=(target/release/trust-runtime)
else
  BENCH_CMD=(cargo run --release -p trust-runtime --bin trust-runtime --)
fi

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

python3 - <<'PY'
import json
from pathlib import Path

out_dir = Path('target/gate-artifacts/runtime-motion-benchmark-breakdown')
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
    f"- samples: {runtime_floor['samples']}\n"
    f"- warmup cycles: {runtime_floor['warmup_cycles']}\n\n"
    + header + sep + body
)
profile_names = ['status_only', 'command_idle', 'move_absolute_only', 'full_demo']
profile_lines = ['\n## VM Profile Highlights\n']
for name in profile_names:
    report = json.loads((out_dir / f'{name}.json').read_text())['report']
    vm_profile = report.get('vm_profile') or {}
    if not vm_profile:
        continue
    profile_lines.append(f'\n### {name}\n')
    profile_lines.append(
        f"- executed: {vm_profile.get('register_programs_executed', 0)}\n"
    )
    profile_lines.append(
        f"- fallbacks: {vm_profile.get('register_program_fallbacks', 0)}\n"
    )
    lowering_cache = vm_profile.get('register_lowering_cache', {})
    if lowering_cache:
        profile_lines.append(
            '- lowering cache: '
            f"hits={lowering_cache.get('hits', 0)} "
            f"misses={lowering_cache.get('misses', 0)} "
            f"hit_ratio={lowering_cache.get('hit_ratio', 0.0):.4f} "
            f"cached={lowering_cache.get('cached_entries', 0)}/"
            f"{lowering_cache.get('cache_capacity', 0)}\n"
        )
    hot_blocks = vm_profile.get('hot_blocks', [])
    if hot_blocks:
        profile_lines.append('- top hot blocks:\n')
        for block in hot_blocks[:5]:
            profile_lines.append(
                f"  - pou={block['pou_id']} block={block['block_id']} "
                f"pc={block['start_pc']} hits={block['hits']}\n"
            )
    fallback_reasons = vm_profile.get('fallback_reasons', [])
    if fallback_reasons:
        profile_lines.append('- fallback reasons:\n')
        for reason in fallback_reasons[:5]:
            profile_lines.append(
                f"  - {reason['reason']}: {reason['count']}\n"
            )
summary += ''.join(profile_lines)
(out_dir / 'summary.md').write_text(summary)
(out_dir / 'summary.json').write_text(json.dumps(rows, indent=2))
print(summary)
PY
