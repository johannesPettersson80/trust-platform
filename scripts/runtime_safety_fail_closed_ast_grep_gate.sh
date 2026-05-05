#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMMIT="$(git -C "$ROOT" rev-parse --short HEAD)"
ARTIFACT_DIR="$ROOT/target/gate-artifacts/runtime-safety-fail-closed-${COMMIT}"
FINDINGS="$ARTIFACT_DIR/runtime-safety-findings.txt"
SUMMARY="$ARTIFACT_DIR/runtime-safety-summary.txt"
ALLOWLIST="$ROOT/docs/internal/architecture/runtime-safety-fail-closed-allowlist.toml"

mkdir -p "$ARTIFACT_DIR"
: >"$FINDINGS"

emit_findings() {
  local rule="$1"
  local owner="$2"
  local path="$3"
  local pattern="$4"
  shift 4
  local extra_args=("$@")

  if [[ ! -e "$ROOT/$path" ]]; then
    return
  fi

  rg -n --with-filename --glob '!**/tests/**' --glob '!**/*tests.rs' --glob '!**/target/**' \
    "${extra_args[@]}" "$pattern" "$ROOT/$path" \
    | sed "s#^$ROOT/##; s#^#${rule} owner=${owner} #" >>"$FINDINGS" || true
}

emit_absence_if_missing() {
  local rule="$1"
  local owner="$2"
  local path="$3"
  local required_pattern="$4"
  local evidence="$5"

  if [[ ! -e "$ROOT/$path" ]]; then
    return
  fi
  if ! rg -q "$required_pattern" "$ROOT/$path"; then
    echo "${rule} owner=${owner} ${path}:0: missing ${evidence}" >>"$FINDINGS"
  fi
}

emit_findings \
  "RUNTIMESAFE-INIT-NULL-FALLBACK" \
  "runtime/init" \
  "crates/trust-runtime/src" \
  'unwrap_or\(Value::Null\)'

emit_findings \
  "RUNTIMESAFE-DRIVER-FAULT-OK" \
  "runtime/IO" \
  "crates/trust-runtime/src/io" \
  'health[^;]*=( HealthState::)?(Degraded|Faulted)|record_.*(fault|error)|last_error[^;]*='

emit_findings \
  "RUNTIMESAFE-DISCOVERY-CONFIG-POLICY-OPEN" \
  "runtime/IO" \
  "crates/trust-runtime/src/io/ethercat/driver.rs" \
  'IoDriverErrorPolicy::Warn|IoDriverErrorPolicy::Ignore|health = IoDriverHealth::Degraded'

emit_findings \
  "RUNTIMESAFE-IGNORED-FLUSH" \
  "runtime/IO" \
  "crates/trust-runtime/src" \
  'flush\(\)\.ok\(\)|let _ = [^;]*flush\(\)'

emit_findings \
  "RUNTIMESAFE-RETAIN-DIRECT-WRITE" \
  "runtime/retain" \
  "crates/trust-runtime/src/retain" \
  'File::create|OpenOptions::new\(\)[^;]*truncate|fs::write'

emit_findings \
  "RUNTIMESAFE-RETAIN-NO-CHECKSUM" \
  "runtime/retain" \
  "crates/trust-runtime/src/retain" \
  'bincode|postcard|serde_json::to_(vec|string)|serde_json::from_(slice|str)'

emit_absence_if_missing \
  "RUNTIMESAFE-RETAIN-NO-CHECKSUM" \
  "runtime/retain" \
  "crates/trust-runtime/src/retain.rs" \
  'crc|checksum|trailer' \
  "retain codec checksum/trailer validation"

emit_absence_if_missing \
  "RUNTIMESAFE-RETAIN-NO-CHECKSUM" \
  "runtime/retain" \
  "crates/trust-runtime/src/retain/codec.rs" \
  'is_finished|remaining|trailing|offset == .*len|len\(\) == .*offset' \
  "retain decoder trailing-data rejection"

emit_findings \
  "RUNTIMESAFE-EVALUATOR-SILENT-GLOBAL" \
  "runtime/eval" \
  "crates/trust-runtime/src/host/helper_eval/storage_lvalue.rs" \
  'storage(_mut)?\(\)?\.set_global\(|storage\.set_global\('

emit_findings \
  "RUNTIMESAFE-EVALUATOR-SILENT-GLOBAL" \
  "runtime/eval" \
  "crates/trust-runtime/src/host/eval" \
  'storage(_mut)?\(\)?\.set_global\(|storage\.set_global\('

emit_findings \
  "RUNTIMESAFE-EVALUATOR-SILENT-GLOBAL" \
  "runtime/eval" \
  "crates/trust-runtime/src/runtime/cycle.rs" \
  'storage(_mut)?\(\)?\.set_global\(|storage\.set_global\('

emit_findings \
  "RUNTIMESAFE-SAFE-STATE-DISCARD" \
  "runtime/cycle" \
  "crates/trust-runtime/src" \
  'let _ = [^;]*apply_safe_state|apply_safe_state\(\)\.ok\(\)'

emit_findings \
  "RUNTIMESAFE-DEBUG-WRITE-DISCARD" \
  "runtime/debug-control" \
  "crates/trust-runtime/src/host/debug/control" \
  'let _ = [^;]*(queue|debug|write)[^;]*write|debug[^;]*write[^;]*\.ok\(\)'

emit_findings \
  "RUNTIMESAFE-DEBUG-WRITE-DISCARD" \
  "runtime/debug-control" \
  "crates/trust-runtime/src/runtime/cycle.rs" \
  'let _ = [^;]*(queue|debug|write)[^;]*write|debug[^;]*write[^;]*\.ok\(\)'

emit_findings \
  "RUNTIMESAFE-CLOUD-STATE-DEFAULT" \
  "runtime-cloud" \
  "crates/trust-runtime/src/web/runtime_cloud_state" \
  'from_str\([^;]*\)\.unwrap_or_default\(\)|serde_json::from_str\([^;]*\)\.unwrap_or'

emit_findings \
  "RUNTIMESAFE-AUDIT-EVENT-DROP" \
  "runtime/audit-event" \
  "crates/trust-runtime/src/control.rs" \
  '\.send\([^;]*\)\.ok\(\)|let _ = [^;]*\.send\('

emit_findings \
  "RUNTIMESAFE-AUDIT-EVENT-DROP" \
  "runtime/audit-event" \
  "crates/trust-runtime/src/control" \
  '\.send\([^;]*\)\.ok\(\)|let _ = [^;]*\.send\('

emit_findings \
  "RUNTIMESAFE-AUDIT-EVENT-DROP" \
  "runtime/audit-event" \
  "crates/trust-runtime/src/host/debug" \
  '\.send\([^;]*\)\.ok\(\)|let _ = [^;]*\.send\('

emit_findings \
  "RUNTIMESAFE-MESH-TIMEOUT-EMPTY" \
  "runtime/mesh" \
  "crates/trust-runtime/src/host/mesh/mapping.rs" \
  'recv_timeout\([^;]*\)\.unwrap_or_default\(\)|MeshSnapshot'

emit_findings \
  "RUNTIMESAFE-RETAIN-COMMIT-ORDER" \
  "runtime/cycle" \
  "crates/trust-runtime/src/runtime/cycle.rs" \
  'write_outputs\(|maybe_save_retain_store'

emit_findings \
  "RUNTIMESAFE-GPIO-NO-HEALTH" \
  "runtime/IO" \
  "crates/trust-runtime/src/io/gpio.rs" \
  'pub struct GpioDriver|impl IoDriver for GpioDriver'

emit_findings \
  "RUNTIMESAFE-RETAIN-ORPHAN-SILENT" \
  "runtime/retain" \
  "crates/trust-runtime/src/runtime/retain_store.rs" \
  'retain.*orphan|orphan.*retain|retain.*drop|drop.*retain'

emit_absence_if_missing \
  "RUNTIMESAFE-RETAIN-ORPHAN-SILENT" \
  "runtime/retain" \
  "crates/trust-runtime/src/runtime/retain_store.rs" \
  'orphan|RetainOrphan' \
  "retain orphan event/reporting"

emit_findings \
  "RUNTIMESAFE-FEATURE-DISABLED-SILENT" \
  "runtime/debug-control" \
  "crates/trust-runtime/src/control" \
  'debug disabled|cfg\(not\(feature = "debug"\)\)|feature_disabled'

emit_findings \
  "RUNTIMESAFE-FEATURE-DISABLED-SILENT" \
  "runtime/debug-control" \
  "crates/trust-runtime/src/bin/trust-runtime.rs" \
  'debug disabled|cfg\(not\(feature = "debug"\)\)|feature_disabled'

emit_findings \
  "RUNTIMESAFE-COERCE-WARNING-ONLY" \
  "runtime/HIR" \
  "crates/trust-hir/src" \
  'implicit.*conversion|conversion.*warning|allowed.*widen'

emit_findings \
  "RUNTIMESAFE-COERCE-WARNING-ONLY" \
  "runtime/HIR" \
  "crates/trust-runtime/src/host/harness/lower" \
  'implicit.*conversion|conversion.*warning|allowed.*widen'

finding_count="$(grep -c . "$FINDINGS" || true)"
allowlisted_count=0
max_allowlist_entries=5

if [[ -f "$ALLOWLIST" ]]; then
  configured_max="$(rg -n '^\s*max_entries\s*=' "$ALLOWLIST" | sed -E 's/.*=\s*([0-9]+).*/\1/' | tail -n 1 || true)"
  if [[ -n "$configured_max" ]]; then
    max_allowlist_entries="$configured_max"
  fi
  allowlisted_count="$({ rg '^\s*\[\[entries\]\]' "$ALLOWLIST" || true; } | awk 'END { print NR + 0 }')"
fi

{
  echo "gate=runtime-safety-fail-closed"
  echo "phase=warn_only_inventory"
  echo "commit=$COMMIT"
  echo "finding_count=$finding_count"
  echo "allowlisted_count=$allowlisted_count"
  echo "max_allowlist_entries=$max_allowlist_entries"
  echo "findings=$FINDINGS"
  echo "allowlist=$ALLOWLIST"
} >"$SUMMARY"

if (( allowlisted_count > max_allowlist_entries )); then
  echo "runtime safety fail-closed gate: allowlist exceeds max entries" >&2
  cat "$SUMMARY" >&2
  exit 1
fi

if [[ "$finding_count" == "0" ]]; then
  echo "runtime safety fail-closed gate: no findings"
else
  echo "runtime safety fail-closed gate: findings"
fi
cat "$SUMMARY"
