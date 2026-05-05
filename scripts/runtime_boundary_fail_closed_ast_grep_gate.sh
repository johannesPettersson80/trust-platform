#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMMIT="$(git -C "$ROOT" rev-parse --short HEAD)"
ARTIFACT_DIR="$ROOT/target/gate-artifacts/runtime-boundary-fail-closed-${COMMIT}"
FINDINGS="$ARTIFACT_DIR/runtime-boundary-findings.txt"
SUMMARY="$ARTIFACT_DIR/runtime-boundary-summary.txt"

mkdir -p "$ARTIFACT_DIR"
: >"$FINDINGS"

if ! command -v rg >/dev/null 2>&1; then
  echo "error: ripgrep (rg) is required for the runtime boundary fail-closed gate" >&2
  exit 2
fi

scan_file() {
  local file="$1"
  local pattern="$2"
  local label="$3"
  if [[ -f "$ROOT/$file" ]]; then
    rg -n "$pattern" "$ROOT/$file" \
      | sed "s#^$ROOT/##; s#^#$label #" >>"$FINDINGS" || true
  fi
}

boundary_fallback_pattern='unwrap_or\((JsonValue|Value)::Null\)|serde_json::to_(value|string)\([^[:cntrl:]]+\)\.ok\(\)|\.ok\(\)\.unwrap_or_default\(\)'
silent_create_pattern='storage_mut\(\)\.set_global\(name'

scan_file "crates/trust-runtime/src/host/harness/harness.rs" "$silent_create_pattern" "silent-create"
scan_file "crates/trust-runtime/src/host/harness/protocol.rs" "$boundary_fallback_pattern" "silent-boundary-fallback"
scan_file "crates/trust-runtime/src/bin/trust-harness.rs" "$boundary_fallback_pattern" "silent-json-fallback"
scan_file "crates/trust-dev/src/agent/harness.rs" "$boundary_fallback_pattern" "silent-agent-fallback"
scan_file "crates/trust-runtime/src/control/debug_handlers_variables.rs" 'current_frame\.unwrap_or|unknown variables reference.*ok|\.lock\(\)\.ok\(\)|unwrap_or_default\(\)' "silent-debug-variable-fallback"
scan_file "crates/trust-runtime/src/control/debug_handlers_eval.rs" '\.unwrap_or_default\(\)' "silent-debug-eval-fallback"
scan_file "crates/trust-runtime/src/control.rs" 'serde_json::to_string\([^[:cntrl:]]+\)\.ok\(\)' "silent-control-json-fallback"
scan_file "crates/trust-runtime/src/web/hmi_ws.rs" 'serde_json::to_value\([^[:cntrl:]]+\)\.ok\(\)|\.ok\(\)\?' "silent-hmi-ws-fallback"
scan_file "crates/trust-runtime/src/web/runtime_cloud_routes/control_proxy.rs" 'serde_json::to_value\(&control_response\)\.unwrap_or' "silent-runtime-cloud-control-fallback"
scan_file "crates/trust-runtime/src/web/ui_routes.rs" 'serde_json::to_value\(schema_response\)\.unwrap_or' "silent-hmi-export-fallback"

finding_count="$(grep -c . "$FINDINGS" || true)"
{
  echo "gate=runtime-boundary-fail-closed"
  echo "commit=$COMMIT"
  echo "finding_count=$finding_count"
  echo "scanned=crates/trust-runtime/src/host/harness/harness.rs"
  echo "scanned=crates/trust-runtime/src/host/harness/protocol.rs"
  echo "scanned=crates/trust-runtime/src/bin/trust-harness.rs"
  echo "scanned=crates/trust-dev/src/agent/harness.rs"
  echo "scanned=crates/trust-runtime/src/control/debug_handlers_variables.rs"
  echo "scanned=crates/trust-runtime/src/control/debug_handlers_eval.rs"
  echo "scanned=crates/trust-runtime/src/control.rs"
  echo "scanned=crates/trust-runtime/src/web/hmi_ws.rs"
  echo "scanned=crates/trust-runtime/src/web/runtime_cloud_routes/control_proxy.rs"
  echo "scanned=crates/trust-runtime/src/web/ui_routes.rs"
} >"$SUMMARY"

if [[ "$finding_count" != "0" ]]; then
  echo "runtime boundary fail-closed gate: findings" >&2
  cat "$FINDINGS" >&2
  echo "artifact=$FINDINGS" >&2
  exit 1
fi

echo "runtime boundary fail-closed gate: no findings"
echo "artifact=$SUMMARY"
