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

emit_io_driver_fault_ok_findings() {
  python3 - "$ROOT" <<'PY' >>"$FINDINGS"
import pathlib
import re
import sys

root = pathlib.Path(sys.argv[1])
io_root = root / "crates/trust-runtime/src/io"

def line_for(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1

def function_bodies(text: str):
    for match in re.finditer(r"fn\s+(read_inputs|write_outputs|handle_io_error)\b[^{]*\{", text):
        depth = 1
        idx = match.end()
        while idx < len(text) and depth:
            if text[idx] == "{":
                depth += 1
            elif text[idx] == "}":
                depth -= 1
            idx += 1
        yield match.start(), body_lines(text, match.end(), idx - 1)

def body_lines(text: str, start: int, end: int):
    base_line = line_for(text, start)
    return [(base_line + offset, line) for offset, line in enumerate(text[start:end].splitlines())]

def health_then_ok_without_err(lines):
    for idx, (_, line) in enumerate(lines):
        if "IoDriverHealth::Degraded" not in line and "IoDriverHealth::Faulted" not in line:
            continue
        saw_err = False
        for _, next_line in lines[idx:idx + 12]:
            if "return Err" in next_line or re.search(r"\bErr\s*\(", next_line):
                saw_err = True
            if re.search(r"(?:return\s+)?Ok\s*\(\s*\(\s*\)\s*\)", next_line):
                if not saw_err:
                    return True
                break
    return False

for path in sorted(io_root.rglob("*.rs")):
    rel = path.relative_to(root).as_posix()
    if "/tests/" in rel or rel.endswith("/tests.rs"):
        continue
    text = path.read_text(encoding="utf-8")
    for start, lines in function_bodies(text):
        if health_then_ok_without_err(lines):
            print(
                "RUNTIMESAFE-DRIVER-FAULT-OK owner=runtime/IO "
                f"{rel}:{line_for(text, start)}: driver failure path records health but returns Ok(())"
            )
PY
}

emit_retain_commit_order_findings() {
  python3 - "$ROOT" <<'PY' >>"$FINDINGS"
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
path = root / "crates/trust-runtime/src/runtime/cycle.rs"
if not path.exists():
    raise SystemExit
text = path.read_text(encoding="utf-8")
rel = path.relative_to(root).as_posix()

def line_for(offset: int) -> int:
    return text.count("\n", 0, offset) + 1

save_pos = text.find("maybe_save_retain_store")
write_cycle_pos = text.find("write_cycle_outputs()")
if save_pos == -1:
    print(f"RUNTIMESAFE-RETAIN-COMMIT-ORDER owner=runtime/cycle {rel}:0: missing due retain save before output commit")
elif write_cycle_pos != -1 and write_cycle_pos < save_pos:
    print(f"RUNTIMESAFE-RETAIN-COMMIT-ORDER owner=runtime/cycle {rel}:{line_for(write_cycle_pos)}: output commit occurs before due retain save")

deadline_pos = text.find("check_output_commit_deadline()")
driver_write_pos = text.find("entry.driver.write_outputs")
if deadline_pos == -1:
    print(f"RUNTIMESAFE-RETAIN-COMMIT-ORDER owner=runtime/cycle {rel}:0: missing watchdog deadline check before output driver writes")
elif driver_write_pos != -1 and driver_write_pos < deadline_pos:
    print(f"RUNTIMESAFE-RETAIN-COMMIT-ORDER owner=runtime/cycle {rel}:{line_for(driver_write_pos)}: output driver write occurs before watchdog deadline check")
PY
}

emit_ethercat_policy_findings() {
  python3 - "$ROOT" <<'PY' >>"$FINDINGS"
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
path = root / "crates/trust-runtime/src/io/ethercat/driver.rs"
if not path.exists():
    raise SystemExit
text = path.read_text(encoding="utf-8")
rel = path.relative_to(root).as_posix()

def emit(line: int, message: str):
    print(f"RUNTIMESAFE-DISCOVERY-CONFIG-POLICY-OPEN owner=runtime/IO {rel}:{line}: {message}")

if 'handle_io_error("discover"' in text:
    line = text[:text.index('handle_io_error("discover"')].count("\n") + 1
    emit(line, "discovery error is routed through Warn/Ignore policy")

ensure_start = text.find("fn ensure_discovered")
ensure_end = text.find("\n    fn handle_io_error", ensure_start)
ensure_body = text[ensure_start:ensure_end] if ensure_start != -1 and ensure_end != -1 else ""
if "self.bus.discover" in ensure_body and "IoDriverHealth::Faulted" not in ensure_body:
    line = text[:ensure_start].count("\n") + 1 if ensure_start != -1 else 0
    emit(line, "discovery failure does not set faulted health")
if "discovery.input_bytes !=" in ensure_body:
    mismatch = ensure_body[ensure_body.find("discovery.input_bytes !="):]
    if "IoDriverHealth::Faulted" not in mismatch or "RuntimeError::IoAddress" not in mismatch:
        line = text[:ensure_start].count("\n") + 1 if ensure_start != -1 else 0
        emit(line, "image-size mismatch is not a faulting IoAddress path")
PY
}

emit_gpio_health_findings() {
  local path="crates/trust-runtime/src/io/gpio.rs"
  if [[ ! -e "$ROOT/$path" ]]; then
    return
  fi
  if ! rg -q 'health:\s*IoDriverHealth' "$ROOT/$path"; then
    echo "RUNTIMESAFE-GPIO-NO-HEALTH owner=runtime/IO ${path}:0: missing GPIO driver health field" >>"$FINDINGS"
  fi
  if ! rg -q 'fn health\(&self\) -> IoDriverHealth' "$ROOT/$path"; then
    echo "RUNTIMESAFE-GPIO-NO-HEALTH owner=runtime/IO ${path}:0: missing GPIO IoDriver health override" >>"$FINDINGS"
  fi
}

emit_findings \
  "RUNTIMESAFE-INIT-NULL-FALLBACK" \
  "runtime/init" \
  "crates/trust-runtime/src" \
  'unwrap_or\(Value::Null\)'

emit_io_driver_fault_ok_findings

emit_ethercat_policy_findings

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

emit_retain_commit_order_findings

emit_gpio_health_findings

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
