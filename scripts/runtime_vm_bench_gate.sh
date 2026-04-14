#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

BENCH_OUT_DIR="${TRUST_VM_BENCH_ARTIFACT_DIR:-${OUT_DIR:-target/gate-artifacts/runtime-vm-bench}}"
PROFILE="${TRUST_VM_BENCH_PROFILE:-quick}"
TIER="${TRUST_VM_BENCH_TIER:-default}"
HOST_CODEGEN="${TRUST_VM_BENCH_HOST_CODEGEN:-generic}"

case "${PROFILE}" in
  quick)
    SAMPLES="${TRUST_VM_BENCH_SAMPLES:-32}"
    WARMUP_CYCLES="${TRUST_VM_BENCH_WARMUP_CYCLES:-8}"
    ;;
  full)
    SAMPLES="${TRUST_VM_BENCH_SAMPLES:-128}"
    WARMUP_CYCLES="${TRUST_VM_BENCH_WARMUP_CYCLES:-32}"
    ;;
  *)
    echo "[vm-bench-gate] FAIL: unsupported profile '${PROFILE}' (expected quick|full)"
    exit 1
    ;;
esac

mkdir -p "${BENCH_OUT_DIR}"

# Avoid leaking gate OUT_DIR into cargo/rustc build script env.
unset OUT_DIR

echo "[vm-bench-gate] capturing VM syntax corpus benchmark (profile=${PROFILE}, tier=${TIER}, host_codegen=${HOST_CODEGEN})"
TRUST_RUNTIME_HOST_CODEGEN="${HOST_CODEGEN}" OUT_DIR="${BENCH_OUT_DIR}" TRUST_VM_SYNTAX_CORPUS_SAMPLES="${SAMPLES}" TRUST_VM_SYNTAX_CORPUS_WARMUP_CYCLES="${WARMUP_CYCLES}" TRUST_VM_SYNTAX_CORPUS_TIER="${TIER}" ./scripts/runtime_vm_syntax_corpus.sh | tee "${BENCH_OUT_DIR}/gate.log"

BENCH_OUT_DIR_ENV="${BENCH_OUT_DIR}" PROFILE_ENV="${PROFILE}" TIER_ENV="${TIER}" SAMPLES_ENV="${SAMPLES}" WARMUP_ENV="${WARMUP_CYCLES}" python3 - <<'PY2'
import json
import os
from pathlib import Path

out_dir = Path(os.environ['BENCH_OUT_DIR_ENV'])
profile = os.environ['PROFILE_ENV']
tier = os.environ['TIER_ENV']
samples = int(os.environ['SAMPLES_ENV'])
warmup = int(os.environ['WARMUP_ENV'])
corpus_dir = out_dir / tier
summary = json.loads((corpus_dir / 'summary.json').read_text())
rows = summary['rows']
worst_p95 = max((row['p95_us'] for row in rows), default=0.0)
summary_doc = f'''# Runtime VM Benchmark Gate

- profile: {profile}
- tier: {tier}
- build mode: {summary.get('build_mode', 'generic')}
- samples: {samples}
- warmup cycles: {warmup}
- suite wall-clock ms: {summary.get('suite_wall_clock_ms', 0)}
- workloads: {len(rows)}
- worst p95 us: {worst_p95:.3f}
- syntax corpus summary: `{tier}/summary.md`

Result: RECORDED
'''
(out_dir / 'summary.md').write_text(summary_doc)
(out_dir / 'summary.json').write_text(json.dumps({
    'profile': profile,
    'tier': tier,
    'build_mode': summary.get('build_mode', 'generic'),
    'samples': samples,
    'warmup_cycles': warmup,
    'suite_wall_clock_ms': summary.get('suite_wall_clock_ms', 0),
    'workloads': len(rows),
    'worst_p95_us': worst_p95,
    'corpus_summary_path': f'{tier}/summary.json',
    'result': 'recorded',
}, indent=2))
PY2

echo "[vm-bench-gate] RECORDED"
