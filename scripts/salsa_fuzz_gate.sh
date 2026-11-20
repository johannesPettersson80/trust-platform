#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-smoke}"
FUZZ_DIR="${SALSA_FUZZ_DIR:-fuzz}"
SMOKE_SECONDS="${SALSA_FUZZ_SMOKE_SECONDS:-30}"
EXTENDED_SECONDS="${SALSA_FUZZ_EXTENDED_SECONDS:-300}"
RSS_LIMIT_MB="${SALSA_FUZZ_RSS_LIMIT_MB:-4096}"
HARD_TIMEOUT_PADDING_SECONDS="${SALSA_FUZZ_HARD_TIMEOUT_PADDING_SECONDS:-600}"
SMOKE_CORPUS_LIMIT="${SALSA_FUZZ_SMOKE_CORPUS_LIMIT:-2000}"
EXTENDED_CORPUS_LIMIT="${SALSA_FUZZ_EXTENDED_CORPUS_LIMIT:-10000}"
FUZZ_TARGET_TRIPLE="${SALSA_FUZZ_TARGET_TRIPLE:-$(rustc -vV | awk '/^host: / { print $2 }')}"

declare -a TEMP_CORPUS_DIRS=()

cleanup_temp_corpora() {
  if [[ ${#TEMP_CORPUS_DIRS[@]} -eq 0 ]]; then
    return
  fi
  for dir in "${TEMP_CORPUS_DIRS[@]}"; do
    rm -rf "$dir"
  done
}

trap cleanup_temp_corpora EXIT

if [[ ! -d "$FUZZ_DIR" ]]; then
  echo "[salsa-fuzz] FAIL: fuzz dir not found at ${FUZZ_DIR}"
  exit 1
fi

if ! command -v cargo-fuzz >/dev/null 2>&1; then
  echo "[salsa-fuzz] FAIL: cargo-fuzz is required (cargo install cargo-fuzz)"
  exit 1
fi

case "$MODE" in
  smoke)
    MAX_TOTAL_TIME="$SMOKE_SECONDS"
    ;;
  extended)
    MAX_TOTAL_TIME="$EXTENDED_SECONDS"
    ;;
  *)
    echo "Usage: $0 [smoke|extended]"
    exit 1
    ;;
esac

corpus_limit_for_mode() {
  case "$MODE" in
    smoke)
      printf '%s\n' "$SMOKE_CORPUS_LIMIT"
      ;;
    extended)
      printf '%s\n' "$EXTENDED_CORPUS_LIMIT"
      ;;
    *)
      printf '%s\n' "0"
      ;;
  esac
}

prepare_corpus_dir() {
  local target="$1"
  local source_corpus_dir="${FUZZ_DIR}/corpus/${target}"
  if [[ ! -d "$source_corpus_dir" ]]; then
    local generated_dir
    generated_dir="$(mktemp -d "/tmp/salsa-fuzz-${target}-${MODE}-seed-XXXXXX")"
    : > "${generated_dir}/seed"
    TEMP_CORPUS_DIRS+=("$generated_dir")
    echo "[salsa-fuzz] warn: corpus dir missing for target=${target} at ${source_corpus_dir}; using generated seed corpus (${generated_dir})" >&2
    printf '%s\n' "$generated_dir"
    return 0
  fi

  local corpus_limit
  corpus_limit="$(corpus_limit_for_mode)"
  if [[ ! "$corpus_limit" =~ ^[0-9]+$ ]] || (( corpus_limit <= 0 )); then
    printf '%s\n' "$source_corpus_dir"
    return 0
  fi

  local file_count
  file_count="$(find "$source_corpus_dir" -type f | wc -l | tr -d '[:space:]')"
  if (( file_count <= corpus_limit )); then
    printf '%s\n' "$source_corpus_dir"
    return 0
  fi

  local capped_dir
  capped_dir="$(mktemp -d "/tmp/salsa-fuzz-${target}-${MODE}-XXXXXX")"
  while IFS= read -r file; do
    local rel_path="${file#${source_corpus_dir}/}"
    local out_path="${capped_dir}/${rel_path}"
    mkdir -p "$(dirname "$out_path")"
    cp "$file" "$out_path"
  done < <(find "$source_corpus_dir" -type f | LC_ALL=C sort | head -n "$corpus_limit")

  TEMP_CORPUS_DIRS+=("$capped_dir")
  echo "[salsa-fuzz] info: using capped corpus target=${target} files=${corpus_limit}/${file_count}" >&2
  printf '%s\n' "$capped_dir"
}

resolve_fuzz_binary_path() {
  local target="$1"
  local candidate="target/${FUZZ_TARGET_TRIPLE}/release/${target}"
  if [[ -x "$candidate" ]]; then
    printf '%s\n' "$candidate"
    return 0
  fi
  candidate="target/release/${target}"
  if [[ -x "$candidate" ]]; then
    printf '%s\n' "$candidate"
    return 0
  fi
  local discovered
  discovered="$(find target -type f -path "*/release/${target}" | head -n 1)"
  if [[ -n "$discovered" && -x "$discovered" ]]; then
    printf '%s\n' "$discovered"
    return 0
  fi
  echo "[salsa-fuzz] FAIL: could not locate fuzz binary for target=${target}" >&2
  return 1
}

run_target() {
  local target="$1"
  local hard_timeout_seconds
  local corpus_dir
  corpus_dir="$(prepare_corpus_dir "$target")"
  hard_timeout_seconds="${SALSA_FUZZ_HARD_TIMEOUT_SECONDS:-$((MAX_TOTAL_TIME + HARD_TIMEOUT_PADDING_SECONDS))}"
  echo "[salsa-fuzz] running target=${target} mode=${MODE} max_total_time=${MAX_TOTAL_TIME}s hard_timeout=${hard_timeout_seconds}s corpus=${corpus_dir}"
  (
    cd "$FUZZ_DIR"
    cargo +nightly fuzz build "$target"
    local binary_path
    binary_path="$(resolve_fuzz_binary_path "$target")"
    local artifact_dir="artifacts/${target}"
    mkdir -p "$artifact_dir"
    local -a cmd=(
      "$binary_path"
      "-artifact_prefix=${PWD}/${artifact_dir}/"
      -max_total_time="${MAX_TOTAL_TIME}"
      -rss_limit_mb="${RSS_LIMIT_MB}"
      -verbosity=0
      -timeout=10
      -max_len=4096
      "$corpus_dir"
    )
    if command -v timeout >/dev/null 2>&1; then
      set +e
      timeout "${hard_timeout_seconds}s" "${cmd[@]}"
      local status=$?
      set -e
      if [[ $status -eq 124 ]]; then
        echo "[salsa-fuzz] FAIL: target=${target} exceeded wall-clock timeout (${hard_timeout_seconds}s)"
        return 124
      fi
      if [[ $status -ne 0 ]]; then
        echo "[salsa-fuzz] FAIL: target=${target} exited with status=${status}"
        return "$status"
      fi
      return 0
    fi
    "${cmd[@]}"
  )
}

run_target "syntax_parse"
run_target "hir_semantic"

echo "[salsa-fuzz] PASS"
