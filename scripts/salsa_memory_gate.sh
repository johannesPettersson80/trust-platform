#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-run}"
BASELINE_FILE="${SALSA_MEMORY_BASELINE_FILE:-docs/reports/salsa-memory-baseline.env}"
AUTO_BASELINE="${SALSA_MEMORY_RECORD_IF_MISSING:-0}"
SAMPLES="${SALSA_MEMORY_SAMPLES:-5}"
WARMUP_RUNS="${SALSA_MEMORY_WARMUP_RUNS:-1}"
MAX_REGRESSION_PCT="${SALSA_MEMORY_MAX_REGRESSION_PCT:-10}"
MAX_RSS_END_KB="${SALSA_MEMORY_MAX_RSS_END_KB:-524288}"
MAX_RSS_DELTA_KB="${SALSA_MEMORY_MAX_RSS_DELTA_KB:-131072}"
MAX_RETAINED_BYTES_PER_ITER="${SALSA_MEMORY_MAX_RETAINED_BYTES_PER_ITER:-50000}"

export ST_LSP_PERF_EDIT_LOOP_ITERS="${ST_LSP_PERF_EDIT_LOOP_ITERS:-120}"
export ST_LSP_PERF_EDIT_LOOP_AVG_MS="${ST_LSP_PERF_EDIT_LOOP_AVG_MS:-80}"
export ST_LSP_PERF_EDIT_LOOP_P95_MS="${ST_LSP_PERF_EDIT_LOOP_P95_MS:-140}"
export ST_LSP_PERF_EDIT_LOOP_CPU_MS="${ST_LSP_PERF_EDIT_LOOP_CPU_MS:-70}"

if ! [[ "$SAMPLES" =~ ^[1-9][0-9]*$ ]]; then
  echo "[salsa-memory] FAIL: SALSA_MEMORY_SAMPLES must be a positive integer"
  exit 1
fi
if ! [[ "$WARMUP_RUNS" =~ ^[0-9]+$ ]]; then
  echo "[salsa-memory] FAIL: SALSA_MEMORY_WARMUP_RUNS must be an integer >= 0"
  exit 1
fi

current_benchmark_id() {
  echo "trust-lsp.perf_edit_loop_budget.perf_alloc_metrics.v1"
}

current_rustc_version() {
  if command -v rustc >/dev/null 2>&1; then
    rustc --version | sed -E 's/[^A-Za-z0-9._-]+/_/g'
  else
    echo "unknown"
  fi
}

current_lock_hash() {
  if [[ -f Cargo.lock ]]; then
    sha256sum Cargo.lock | awk '{print $1}'
  else
    echo "missing"
  fi
}

current_system() {
  uname -srm 2>/dev/null | sed -E 's/[^A-Za-z0-9._-]+/_/g' || echo "unknown"
}

extract_metric_series() {
  local file="$1"
  local key="$2"
  grep -Eo "(^|[[:space:]])${key}=[0-9]+(\\.[0-9]+)?" "$file" | sed -E 's/^[[:space:]]*//' | cut -d '=' -f 2
}

metric_median() {
  local file="$1"
  local key="$2"
  extract_metric_series "$file" "$key" | sort -n | awk '
    { values[++n] = $1 }
    END {
      if (n == 0) {
        exit 2
      }
      if (n % 2 == 1) {
        print values[(n + 1) / 2]
      } else {
        printf "%.2f\n", (values[n / 2] + values[n / 2 + 1]) / 2
      }
    }
  '
}

metric_list() {
  local file="$1"
  local key="$2"
  extract_metric_series "$file" "$key" | paste -sd',' -
}

percent_change() {
  local baseline="$1"
  local current="$2"
  awk -v baseline="$baseline" -v current="$current" '
    BEGIN {
      if (baseline == 0) {
        if (current == 0) {
          printf "0.00"
        } else {
          printf "INF"
        }
      } else {
        printf "%.2f", ((current - baseline) / baseline) * 100
      }
    }
  '
}

run_samples() {
  local perf_log
  perf_log="$(mktemp)"
  trap 'rm -f "$perf_log"' RETURN

  for warmup in $(seq 1 "$WARMUP_RUNS"); do
    echo "[salsa-memory] warmup ${warmup}/${WARMUP_RUNS}"
    cargo test -p trust-lsp --features perf_alloc_metrics perf_edit_loop_budget -- --ignored --nocapture >/dev/null
  done

  : > "$perf_log"
  for sample in $(seq 1 "$SAMPLES"); do
    echo "[salsa-memory] sample ${sample}/${SAMPLES}"
    cargo test -p trust-lsp --features perf_alloc_metrics perf_edit_loop_budget -- --ignored --nocapture | tee -a "$perf_log"
  done

  CURRENT_AVG_MEDIAN="$(metric_median "$perf_log" "avg_ms")"
  CURRENT_P95_MEDIAN="$(metric_median "$perf_log" "p95_ms")"
  CURRENT_CPU_MEDIAN="$(metric_median "$perf_log" "cpu_ms_per_iter")"
  CURRENT_ALLOC_CALLS_MEDIAN="$(metric_median "$perf_log" "alloc_calls_per_iter")"
  CURRENT_ALLOC_BYTES_MEDIAN="$(metric_median "$perf_log" "alloc_bytes_per_iter")"
  CURRENT_RETAINED_BYTES_PER_ITER_MEDIAN="$(metric_median "$perf_log" "retained_bytes_per_iter")"
  CURRENT_RSS_END_MEDIAN="$(metric_median "$perf_log" "rss_end_kb")"
  CURRENT_RSS_DELTA_MEDIAN="$(metric_median "$perf_log" "rss_delta_kb")"

  CURRENT_AVG_SAMPLES="$(metric_list "$perf_log" "avg_ms")"
  CURRENT_P95_SAMPLES="$(metric_list "$perf_log" "p95_ms")"
  CURRENT_CPU_SAMPLES="$(metric_list "$perf_log" "cpu_ms_per_iter")"
  CURRENT_ALLOC_CALLS_SAMPLES="$(metric_list "$perf_log" "alloc_calls_per_iter")"
  CURRENT_ALLOC_BYTES_SAMPLES="$(metric_list "$perf_log" "alloc_bytes_per_iter")"
  CURRENT_RETAINED_BYTES_PER_ITER_SAMPLES="$(metric_list "$perf_log" "retained_bytes_per_iter")"
  CURRENT_RSS_END_SAMPLES="$(metric_list "$perf_log" "rss_end_kb")"
  CURRENT_RSS_DELTA_SAMPLES="$(metric_list "$perf_log" "rss_delta_kb")"
}

print_current_metrics() {
  echo "[salsa-memory] avg_samples=[${CURRENT_AVG_SAMPLES}] p95_samples=[${CURRENT_P95_SAMPLES}] cpu_samples=[${CURRENT_CPU_SAMPLES}]"
  echo "[salsa-memory] alloc_calls_samples=[${CURRENT_ALLOC_CALLS_SAMPLES}] alloc_bytes_samples=[${CURRENT_ALLOC_BYTES_SAMPLES}] retained_bytes_per_iter_samples=[${CURRENT_RETAINED_BYTES_PER_ITER_SAMPLES}]"
  echo "[salsa-memory] rss_end_samples=[${CURRENT_RSS_END_SAMPLES}] rss_delta_samples=[${CURRENT_RSS_DELTA_SAMPLES}]"
  echo "[salsa-memory] medians avg=${CURRENT_AVG_MEDIAN}ms p95=${CURRENT_P95_MEDIAN}ms cpu=${CURRENT_CPU_MEDIAN}ms/op alloc_calls=${CURRENT_ALLOC_CALLS_MEDIAN}/op alloc_bytes=${CURRENT_ALLOC_BYTES_MEDIAN}B/op retained=${CURRENT_RETAINED_BYTES_PER_ITER_MEDIAN}B/op rss_end=${CURRENT_RSS_END_MEDIAN}KB rss_delta=${CURRENT_RSS_DELTA_MEDIAN}KB"
}

record_baseline() {
  mkdir -p "$(dirname "$BASELINE_FILE")"
  local now
  local git_rev
  local benchmark_id
  local rustc_version
  local cargo_lock_sha256
  local system
  now="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  git_rev="$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")"
  benchmark_id="$(current_benchmark_id)"
  rustc_version="$(current_rustc_version)"
  cargo_lock_sha256="$(current_lock_hash)"
  system="$(current_system)"

  cat > "$BASELINE_FILE" <<EOF
# Generated by scripts/salsa_memory_gate.sh
recorded_at=${now}
git_rev=${git_rev}
benchmark_id=${benchmark_id}
rustc_version=${rustc_version}
cargo_lock_sha256=${cargo_lock_sha256}
system=${system}
samples=${SAMPLES}
avg_ms=${CURRENT_AVG_MEDIAN}
p95_ms=${CURRENT_P95_MEDIAN}
cpu_ms_per_iter=${CURRENT_CPU_MEDIAN}
alloc_calls_per_iter=${CURRENT_ALLOC_CALLS_MEDIAN}
alloc_bytes_per_iter=${CURRENT_ALLOC_BYTES_MEDIAN}
retained_bytes_per_iter=${CURRENT_RETAINED_BYTES_PER_ITER_MEDIAN}
rss_end_kb=${CURRENT_RSS_END_MEDIAN}
rss_delta_kb=${CURRENT_RSS_DELTA_MEDIAN}
avg_samples=${CURRENT_AVG_SAMPLES}
p95_samples=${CURRENT_P95_SAMPLES}
cpu_samples=${CURRENT_CPU_SAMPLES}
alloc_calls_samples=${CURRENT_ALLOC_CALLS_SAMPLES}
alloc_bytes_samples=${CURRENT_ALLOC_BYTES_SAMPLES}
retained_bytes_per_iter_samples=${CURRENT_RETAINED_BYTES_PER_ITER_SAMPLES}
rss_end_samples=${CURRENT_RSS_END_SAMPLES}
rss_delta_samples=${CURRENT_RSS_DELTA_SAMPLES}
EOF

  echo "[salsa-memory] baseline recorded at ${BASELINE_FILE}"
}

compare_metric() {
  local label="$1"
  local baseline="$2"
  local current="$3"
  local delta
  delta="$(percent_change "$baseline" "$current")"
  if [[ "$delta" == "INF" ]]; then
    echo "[salsa-memory] compare ${label}: baseline=${baseline} current=${current} delta=INF% (invalid zero baseline)"
    return 1
  fi
  echo "[salsa-memory] compare ${label}: baseline=${baseline} current=${current} delta=${delta}%"
  if awk -v delta="$delta" -v max="$MAX_REGRESSION_PCT" 'BEGIN { exit(delta > max) }'; then
    return 0
  fi
  return 1
}

absolute_cap_check() {
  local label="$1"
  local value="$2"
  local cap="$3"
  echo "[salsa-memory] cap ${label}: value=${value} cap=${cap}"
  if awk -v value="$value" -v cap="$cap" 'BEGIN { exit(value > cap) }'; then
    return 0
  fi
  return 1
}

compare_with_baseline() {
  if [[ ! -f "$BASELINE_FILE" ]]; then
    if [[ "$AUTO_BASELINE" == "1" ]]; then
      echo "[salsa-memory] WARN: baseline file not found at ${BASELINE_FILE}; recording ad-hoc baseline for this environment."
      record_baseline
    else
      echo "[salsa-memory] FAIL: baseline file not found at ${BASELINE_FILE}"
      echo "[salsa-memory] Run: $0 record"
      exit 1
    fi
  fi

  # shellcheck disable=SC1090
  source "$BASELINE_FILE"

  if [[ "${SALSA_MEMORY_IGNORE_METADATA:-0}" != "1" ]]; then
    if ! validate_baseline_metadata; then
      if [[ "$AUTO_BASELINE" == "1" ]]; then
        echo "[salsa-memory] WARN: baseline metadata mismatch; recording ad-hoc baseline for this environment."
        record_baseline
        # shellcheck disable=SC1090
        source "$BASELINE_FILE"
      else
        echo "[salsa-memory] Run: $0 record"
        exit 1
      fi
    fi
  fi

  local fail=0
  compare_metric "avg_ms" "$avg_ms" "$CURRENT_AVG_MEDIAN" || fail=1
  compare_metric "p95_ms" "$p95_ms" "$CURRENT_P95_MEDIAN" || fail=1
  compare_metric "cpu_ms_per_iter" "$cpu_ms_per_iter" "$CURRENT_CPU_MEDIAN" || fail=1
  compare_metric "alloc_calls_per_iter" "$alloc_calls_per_iter" "$CURRENT_ALLOC_CALLS_MEDIAN" || fail=1
  compare_metric "alloc_bytes_per_iter" "$alloc_bytes_per_iter" "$CURRENT_ALLOC_BYTES_MEDIAN" || fail=1
  compare_metric "retained_bytes_per_iter" "$retained_bytes_per_iter" "$CURRENT_RETAINED_BYTES_PER_ITER_MEDIAN" || fail=1
  compare_metric "rss_delta_kb" "$rss_delta_kb" "$CURRENT_RSS_DELTA_MEDIAN" || fail=1

  absolute_cap_check "rss_end_kb" "$CURRENT_RSS_END_MEDIAN" "$MAX_RSS_END_KB" || fail=1
  absolute_cap_check "rss_delta_kb" "$CURRENT_RSS_DELTA_MEDIAN" "$MAX_RSS_DELTA_KB" || fail=1
  absolute_cap_check "retained_bytes_per_iter" "$CURRENT_RETAINED_BYTES_PER_ITER_MEDIAN" "$MAX_RETAINED_BYTES_PER_ITER" || fail=1

  if [[ "$fail" -ne 0 ]]; then
    echo "[salsa-memory] FAIL: baseline regression or absolute memory cap exceeded"
    exit 1
  fi

  echo "[salsa-memory] PASS: baseline compare and memory caps satisfied"
}

validate_baseline_metadata() {
  local baseline_benchmark_id="${benchmark_id:-}"
  local baseline_rustc_version="${rustc_version:-}"
  local baseline_lock_hash="${cargo_lock_sha256:-}"
  local baseline_system="${system:-}"

  if [[ -z "$baseline_benchmark_id" || -z "$baseline_rustc_version" || -z "$baseline_lock_hash" || -z "$baseline_system" ]]; then
    echo "[salsa-memory] FAIL: baseline metadata missing (benchmark_id/rustc_version/cargo_lock_sha256/system)."
    return 1
  fi

  local expected_benchmark_id
  local expected_rustc_version
  local expected_lock_hash
  local expected_system
  expected_benchmark_id="$(current_benchmark_id)"
  expected_rustc_version="$(current_rustc_version)"
  expected_lock_hash="$(current_lock_hash)"
  expected_system="$(current_system)"

  local mismatch=0
  if [[ "$baseline_benchmark_id" != "$expected_benchmark_id" ]]; then
    echo "[salsa-memory] FAIL: benchmark_id mismatch (baseline=${baseline_benchmark_id}, current=${expected_benchmark_id})."
    mismatch=1
  fi
  if [[ "$baseline_rustc_version" != "$expected_rustc_version" ]]; then
    echo "[salsa-memory] FAIL: rustc_version mismatch (baseline=${baseline_rustc_version}, current=${expected_rustc_version})."
    mismatch=1
  fi
  if [[ "$baseline_lock_hash" != "$expected_lock_hash" ]]; then
    echo "[salsa-memory] FAIL: cargo_lock_sha256 mismatch (baseline=${baseline_lock_hash}, current=${expected_lock_hash})."
    mismatch=1
  fi
  if [[ "$baseline_system" != "$expected_system" ]]; then
    echo "[salsa-memory] FAIL: system mismatch (baseline=${baseline_system}, current=${expected_system})."
    mismatch=1
  fi

  if [[ "$mismatch" -ne 0 ]]; then
    echo "[salsa-memory] Baseline is stale for current benchmark environment."
    return 1
  fi

  return 0
}

run_samples
print_current_metrics

case "$MODE" in
  run)
    echo "[salsa-memory] PASS"
    ;;
  record)
    record_baseline
    ;;
  compare)
    compare_with_baseline
    ;;
  *)
    echo "Usage: $0 [run|record|compare]"
    exit 1
    ;;
esac
