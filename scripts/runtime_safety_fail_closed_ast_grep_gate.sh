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

if ! command -v rg >/dev/null 2>&1; then
  echo "error: ripgrep (rg) is required for the runtime safety fail-closed gate" >&2
  exit 2
fi

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

emit_retain_durability_findings() {
  python3 - "$ROOT" <<'PY' >>"$FINDINGS"
import pathlib
import re
import sys

root = pathlib.Path(sys.argv[1])
store = root / "crates/trust-runtime/src/retain/store.rs"
if not store.exists():
    raise SystemExit
text = store.read_text(encoding="utf-8")
rel = store.relative_to(root).as_posix()

if re.search(r"File::create\s*\(\s*path\s*\)|fs::write\s*\(\s*path\s*,", text):
    print(
        "RUNTIMESAFE-RETAIN-DIRECT-WRITE owner=runtime/retain "
        f"{rel}:0: retain store writes directly to final path"
    )
if "OpenOptions::new" not in text:
    print(
        "RUNTIMESAFE-RETAIN-DIRECT-WRITE owner=runtime/retain "
        f"{rel}:0: retain store does not use an explicit temp-file writer"
    )
if "fs::rename" not in text:
    print(
        "RUNTIMESAFE-RETAIN-DIRECT-WRITE owner=runtime/retain "
        f"{rel}:0: retain store does not atomically rename temp file"
    )
if "sync_all" not in text:
    print(
        "RUNTIMESAFE-RETAIN-DIRECT-WRITE owner=runtime/retain "
        f"{rel}:0: retain store does not fsync temp file or parent directory"
    )
PY
}

emit_evaluator_silent_global_findings() {
  python3 - "$ROOT" <<'PY' >>"$FINDINGS"
import pathlib
import re
import sys

root = pathlib.Path(sys.argv[1])
targets = [
    root / "crates/trust-runtime/src/host/helper_eval/storage_lvalue.rs",
    root / "crates/trust-runtime/src/host/eval/expr/access.rs",
    root / "crates/trust-runtime/src/runtime/cycle.rs",
]

def line_for(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1

def function_body(text: str, name: str):
    match = re.search(rf"fn\s+{re.escape(name)}\b[^\{{]*\{{", text)
    if not match:
        return None, ""
    depth = 1
    idx = match.end()
    while idx < len(text) and depth:
        if text[idx] == "{":
            depth += 1
        elif text[idx] == "}":
            depth -= 1
        idx += 1
    return match.start(), text[match.end():idx - 1]

for path in targets:
    if not path.exists():
        continue
    rel = path.relative_to(root).as_posix()
    text = path.read_text(encoding="utf-8")
    if rel.endswith("storage_lvalue.rs"):
        start, body = function_body(text, "write_name")
        if start is not None and "storage.set_global(name.clone(), value)" in body and "UndefinedVariable" not in body:
            print(
                "RUNTIMESAFE-EVALUATOR-SILENT-GLOBAL owner=runtime/eval "
                f"{rel}:{line_for(text, start)}: helper evaluator creates missing global on assignment"
            )
    elif rel.endswith("access.rs"):
        start, body = function_body(text, "write_name")
        if start is not None and "ctx.storage.set_global(name.clone(), value)" in body and "UndefinedVariable" not in body:
            print(
                "RUNTIMESAFE-EVALUATOR-SILENT-GLOBAL owner=runtime/eval "
                f"{rel}:{line_for(text, start)}: evaluator creates missing global on assignment"
            )
    elif rel.endswith("cycle.rs"):
        for fn_name in ("apply_pending_debug_writes", "apply_forced_values"):
            start, body = function_body(text, fn_name)
            if start is None:
                print(
                    "RUNTIMESAFE-EVALUATOR-SILENT-GLOBAL owner=runtime/eval "
                    f"{rel}:0: missing debug write validation function {fn_name}"
                )
                continue
            if "set_global" in body and "get_global" not in body:
                print(
                    "RUNTIMESAFE-EVALUATOR-SILENT-GLOBAL owner=runtime/eval "
                    f"{rel}:{line_for(text, start)}: debug write path can create missing global"
                )
PY
}

emit_coerce_warning_only_findings() {
  python3 - "$ROOT" <<'PY' >>"$FINDINGS"
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
hir_root = root / "crates/trust-hir/src"

warning_site = None
if hir_root.exists():
    for path in sorted(hir_root.rglob("*.rs")):
        rel = path.relative_to(root).as_posix()
        if "/tests/" in rel or rel.endswith("/tests.rs"):
            continue
        lines = path.read_text(encoding="utf-8").splitlines()
        for idx, line in enumerate(lines, start=1):
            if "warn_implicit_conversion" in line:
                warning_site = (rel, idx)
                break
        if warning_site is not None:
            break

if warning_site is None:
    raise SystemExit

def contains(path: pathlib.Path, needle: str) -> bool:
    return path.exists() and needle in path.read_text(encoding="utf-8")

proof = root / "crates/trust-runtime/tests/coercion_proof.rs"
lowering = root / "crates/trust-runtime/src/host/harness/lower/expr/lowering.rs"
literals = root / "crates/trust-runtime/src/host/harness/lower/expr/literals.rs"
stmt = root / "crates/trust-runtime/src/host/harness/lower/stmt.rs"

required = [
    (proof, "function_input_parameter_widening", "function input widening proof"),
    (proof, "function_output_parameter_widening", "function output widening proof"),
    (proof, "assignment_widening", "assignment widening runtime proof"),
    (proof, "initializer_widening", "initializer widening runtime proof"),
    (proof, "return_value_widening", "return-value widening runtime proof"),
    (proof, "inout_narrowing", "InOut narrowing rejection proof"),
    (proof, "narrowing_assignment", "narrowing rejection proof"),
    (lowering, "lower_expr_with_context", "contextual expression lowering hook"),
    (lowering, "binary_operand_context", "contextual binary operand lowering policy"),
    (literals, "lower_literal_with_context", "contextual literal lowering hook"),
    (literals, "coerce_value_to_type(value, type_id)?", "literal target-type coercion"),
    (stmt, "lower_expr_with_context(&exprs[1], ctx, target_type)?", "assignment target-type lowering"),
]

rel, line = warning_site
for path, needle, evidence in required:
    if contains(path, needle):
        continue
    print(
        "RUNTIMESAFE-COERCE-WARNING-ONLY owner=runtime/HIR "
        f"{rel}:{line}: implicit-conversion warnings require runtime proof/evidence: missing {evidence}"
    )
PY
}

emit_mesh_timeout_empty_findings() {
  python3 - "$ROOT" <<'PY' >>"$FINDINGS"
import pathlib
import re
import sys

root = pathlib.Path(sys.argv[1])
path = root / "crates/trust-runtime/src/host/mesh/mapping.rs"
if not path.exists():
    raise SystemExit
text = path.read_text(encoding="utf-8")
rel = path.relative_to(root).as_posix()

def line_for(pattern: str) -> int:
    idx = text.find(pattern)
    if idx == -1:
        return 0
    return text.count("\n", 0, idx) + 1

if re.search(r"recv_timeout\([^;]*\)\.unwrap_or_default\(\)", text, re.S):
    print(
        "RUNTIMESAFE-MESH-TIMEOUT-EMPTY owner=runtime/mesh "
        f"{rel}:{line_for('recv_timeout')}: mesh snapshot timeout becomes empty map"
    )
if "let _ = resource.send_command(ResourceCommand::MeshSnapshot" in text:
    print(
        "RUNTIMESAFE-MESH-TIMEOUT-EMPTY owner=runtime/mesh "
        f"{rel}:{line_for('ResourceCommand::MeshSnapshot')}: mesh snapshot command failure is ignored"
    )
match = re.search(r"fn\s+snapshot_globals\b[^{]*", text)
if match and "Result<IndexMap" not in match.group(0):
    print(
        "RUNTIMESAFE-MESH-TIMEOUT-EMPTY owner=runtime/mesh "
        f"{rel}:{text.count(chr(10), 0, match.start()) + 1}: mesh snapshot API cannot distinguish timeout from empty snapshot"
    )
PY
}

emit_feature_disabled_findings() {
  python3 - "$ROOT" <<'PY' >>"$FINDINGS"
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
control = root / "crates/trust-runtime/src/control.rs"
types = root / "crates/trust-runtime/src/host/debug/types.rs"

def emit(path: pathlib.Path, message: str, needle: str = "debug disabled"):
    rel = path.relative_to(root).as_posix()
    text = path.read_text(encoding="utf-8") if path.exists() else ""
    idx = text.find(needle)
    line = text.count("\n", 0, idx) + 1 if idx != -1 else 0
    print(f"RUNTIMESAFE-FEATURE-DISABLED-SILENT owner=runtime/debug-control {rel}:{line}: {message}")

if control.exists():
    text = control.read_text(encoding="utf-8")
    if "debug disabled" in text and "feature_disabled" not in text:
        emit(control, "debug-disabled control response lacks structured feature_disabled code")
    if "debug disabled" in text and "RuntimeEvent::FeatureDisabled" not in text:
        emit(control, "debug-disabled control response lacks observable FeatureDisabled event")

if types.exists() and "FeatureDisabled" not in types.read_text(encoding="utf-8"):
    emit(types, "runtime event taxonomy lacks FeatureDisabled")
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

emit_init_null_fallback_findings() {
  python3 - "$ROOT" <<'PY' >>"$FINDINGS"
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
paths = [
    root / "crates/trust-runtime/src/host/instance.rs",
    root / "crates/trust-runtime/src/runtime/vm/local_init.rs",
    root / "crates/trust-runtime/src/host/harness/config/globals.rs",
    root / "crates/trust-runtime/src/host/eval/bindings.rs",
    root / "crates/trust-runtime/src/host/eval/locals.rs",
    root / "crates/trust-runtime/src/host/eval/calls.rs",
]
patterns = ("unwrap_or(Value::Null)", "or(Ok(Value::Null))")
for path in paths:
    if not path.exists():
        continue
    rel = path.relative_to(root).as_posix()
    for idx, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if any(pattern in line for pattern in patterns):
            print(
                "RUNTIMESAFE-INIT-NULL-FALLBACK owner=runtime/init "
                f"{rel}:{idx}: {line.strip()}"
            )
PY
}

emit_init_null_fallback_findings

emit_io_driver_fault_ok_findings

emit_ethercat_policy_findings

emit_findings \
  "RUNTIMESAFE-IGNORED-FLUSH" \
  "runtime/IO" \
  "crates/trust-runtime/src" \
  'flush\(\)\.ok\(\)|let _ = [^;]*flush\(\)'

emit_retain_durability_findings

emit_findings \
  "RUNTIMESAFE-RETAIN-NO-CHECKSUM" \
  "runtime/retain" \
  "crates/trust-runtime/src/retain" \
  'bincode|postcard|serde_json::to_(vec|string)|serde_json::from_(slice|str)'

emit_absence_if_missing \
  "RUNTIMESAFE-RETAIN-NO-CHECKSUM" \
  "runtime/retain" \
  "crates/trust-runtime/src/retain.rs" \
  'crc|checksum|trailer|TRAILER|crc32fast' \
  "retain codec checksum/trailer validation"

emit_absence_if_missing \
  "RUNTIMESAFE-RETAIN-NO-CHECKSUM" \
  "runtime/retain" \
  "crates/trust-runtime/src/retain/codec.rs" \
  'is_finished|expect_finished|remaining|trailing|offset == .*len|len\(\) == .*offset' \
  "retain decoder trailing-data rejection"

emit_evaluator_silent_global_findings

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

emit_mesh_timeout_empty_findings

emit_retain_commit_order_findings

emit_gpio_health_findings

emit_absence_if_missing \
  "RUNTIMESAFE-RETAIN-ORPHAN-SILENT" \
  "runtime/retain" \
  "crates/trust-runtime/src/runtime/restart.rs" \
  'orphan|RetainOrphan' \
  "retain orphan event/reporting"

emit_feature_disabled_findings

emit_coerce_warning_only_findings

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
  echo "phase=fail_class"
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

if [[ "$finding_count" != "0" ]]; then
  exit 1
fi
