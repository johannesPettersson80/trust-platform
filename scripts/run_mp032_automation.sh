#!/usr/bin/env bash
set -euo pipefail

ST_RUNTIME="${ST_RUNTIME:-./target/debug/trust-runtime}"
PROJECT="${1:-tests/fixtures/runtime_reliability_bundle}"
LOAD_DURATION_SECONDS="${LOAD_DURATION_SECONDS:-120}"
LOAD_INTERVAL_SECONDS="${LOAD_INTERVAL_SECONDS:-1}"
SOAK_DURATION_SECONDS="${SOAK_DURATION_SECONDS:-300}"
SOAK_INTERVAL_SECONDS="${SOAK_INTERVAL_SECONDS:-5}"
LOAD_MAX_CYCLE_MS="${LOAD_MAX_CYCLE_MS:-20}"
LOAD_MAX_JITTER_MS="${LOAD_MAX_JITTER_MS:-20}"
SOAK_MAX_RSS_KB="${SOAK_MAX_RSS_KB:-262144}"
SOAK_MAX_CPU_PCT="${SOAK_MAX_CPU_PCT:-95}"
OUT_DIR="${OUT_DIR:-target/mp032/$(date +%Y%m%d_%H%M%S)}"

mkdir -p "$OUT_DIR"

if [ ! -d "$PROJECT" ]; then
  echo "project folder not found: $PROJECT"
  exit 1
fi

echo "Building trust-runtime binary..."
cargo build -p trust-runtime --bin trust-runtime >/dev/null

echo "Running load test..."
OUT="$OUT_DIR/load.log" \
DURATION="$LOAD_DURATION_SECONDS" \
INTERVAL="$LOAD_INTERVAL_SECONDS" \
ST_RUNTIME="$ST_RUNTIME" \
scripts/runtime_load_test.sh "$PROJECT"

echo "Running soak test..."
OUT="$OUT_DIR/soak.log" \
DURATION_SECONDS="$SOAK_DURATION_SECONDS" \
INTERVAL_SEC="$SOAK_INTERVAL_SECONDS" \
ST_RUNTIME="$ST_RUNTIME" \
scripts/runtime_soak_test.sh "$PROJECT"

echo "Summarizing and enforcing reliability gates..."
python3 scripts/summarize_runtime_reliability.py \
  --load-log "$OUT_DIR/load.log" \
  --soak-log "$OUT_DIR/soak.log" \
  --output-json "$OUT_DIR/reliability-summary.json" \
  --output-md "$OUT_DIR/reliability-summary.md" \
  --enforce-gates \
  --max-load-max-ms "$LOAD_MAX_CYCLE_MS" \
  --max-load-jitter-ms "$LOAD_MAX_JITTER_MS" \
  --max-soak-rss-kb "$SOAK_MAX_RSS_KB" \
  --max-soak-cpu-pct "$SOAK_MAX_CPU_PCT"

echo "MP-032 automation complete: $OUT_DIR"
