#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMMIT="$(git -C "$ROOT" rev-parse --short HEAD)"
ARTIFACT_DIR="$ROOT/target/gate-artifacts/architecture-external-safety-${COMMIT}"
OUT="$ARTIFACT_DIR/cargo-geiger.txt"
PROBE_TIMEOUT_SECONDS="${GEIGER_PROBE_TIMEOUT_SECONDS:-20}"

mkdir -p "$ARTIFACT_DIR"

run_probe() {
  local label="$1"
  shift

  echo
  echo "## $label"
  echo "command=$*"

  set +e
  timeout "$PROBE_TIMEOUT_SECONDS" "$@"
  local status=$?
  set -e

  case "$status" in
    0)
      echo "status=pass"
      ;;
    124)
      echo "status=partial"
      echo "blocker=probe exceeded ${PROBE_TIMEOUT_SECONDS}s timeout"
      ;;
    *)
      echo "status=partial"
      echo "exit_code=$status"
      ;;
  esac
}

{
  echo "# Architecture external safety geiger probe"
  echo "commit=$COMMIT"
  echo "generated_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "artifact=$OUT"
  echo "timeout_seconds=$PROBE_TIMEOUT_SECONDS"
  echo

  if command -v cargo-geiger >/dev/null 2>&1 || cargo geiger --version >/dev/null 2>&1; then
    cargo geiger --version
  else
    echo "status=partial"
    echo "blocker=cargo-geiger is not installed"
    echo "enforced_gate=cargo run -p xtask -- architecture-doctor --full-map"
    exit 0
  fi

  echo
  echo "decision=advisory-partial"
  echo "reason=cargo-geiger 0.13.0 does not handle the workspace virtual manifest as a package root; package-manifest probes are bounded because full geiger scans clean/rebuild target."
  echo "enforced_gate=cargo run -p xtask -- architecture-doctor --full-map"
  echo "enforced_check=FULLMAP-CHECK-09"

  run_probe \
    "root virtual-manifest probe" \
    cargo geiger --locked --forbid-only --output-format GitHubMarkdown

  run_probe \
    "trust-runtime package probe" \
    cargo geiger --manifest-path "$ROOT/crates/trust-runtime/Cargo.toml" --locked --forbid-only --output-format GitHubMarkdown

  run_probe \
    "trust-runtime-core package probe" \
    cargo geiger --manifest-path "$ROOT/crates/trust-runtime-core/Cargo.toml" --locked --forbid-only --output-format GitHubMarkdown

  run_probe \
    "trust-plcopen package probe" \
    cargo geiger --manifest-path "$ROOT/crates/trust-plcopen/Cargo.toml" --locked --forbid-only --output-format GitHubMarkdown

  run_probe \
    "trust-dev package probe" \
    cargo geiger --manifest-path "$ROOT/crates/trust-dev/Cargo.toml" --locked --forbid-only --output-format GitHubMarkdown

  echo
  echo "final_status=advisory-partial"
  echo "next_action=replace cargo-geiger or keep it advisory while FULLMAP-CHECK-09 remains the enforced unsafe register gate"
} 2>&1 | tee "$OUT"
