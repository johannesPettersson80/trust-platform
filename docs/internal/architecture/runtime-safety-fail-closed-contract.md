# Runtime Safety Fail-Closed Contract

Status: Blocking full-map gate active
Owner: runtime safety
Date: 2026-05-05

## Rule

Runtime-internal safety paths must fail closed by default. A runtime path that can affect physical outputs, retained state, safe state, audit/event evidence, runtime-cloud persisted state, mesh control snapshots, or debugger/control writes must not turn transport, persistence, parsing, timeout, init, or target-resolution failures into success defaults.

`Value::Null`, empty maps, default structs, degraded health, missing events, or ignored send/write results are not valid failure representations for safety-critical runtime paths unless the behavior is an explicit, named compatibility mode with tests and an allowlist entry.

## Enforced Doctor

The source-derived gate is `scripts/runtime_safety_fail_closed_ast_grep_gate.sh`. It is surfaced by `cargo run -p xtask -- architecture-doctor --full-map` as `FULLMAP-RUNTIMESAFE`.

The gate is blocking:

- The script records findings under `target/gate-artifacts/runtime-safety-fail-closed-<commit>/`.
- The script exits non-zero when findings are present.
- The full-map doctor reports `FULLMAP-RUNTIMESAFE` as `fail` when the script reports findings.
- CI runs the script and uploads runtime-safety artifacts.
- The gate may remain blocking only while every finding is fixed or narrowly allowlisted.

## Initial Rule Families

- `RUNTIMESAFE-INIT-NULL-FALLBACK`: runtime initialization or evaluator code falls back to `Value::Null`.
- `RUNTIMESAFE-DRIVER-FAULT-OK`: I/O drivers record degraded/fault state but still return `Ok(())`.
- `RUNTIMESAFE-DISCOVERY-CONFIG-POLICY-OPEN`: discovery/config/image-size failures are routed through warn/ignore policy or degraded health.
- `RUNTIMESAFE-IGNORED-FLUSH`: file/socket flush results are discarded.
- `RUNTIMESAFE-RETAIN-DIRECT-WRITE`: retain persistence writes directly to the target file instead of temp write, flush, fsync, atomic rename, and parent directory sync.
- `RUNTIMESAFE-RETAIN-NO-CHECKSUM`: retain codec lacks payload length, checksum, and trailer validation.
- `RUNTIMESAFE-EVALUATOR-SILENT-GLOBAL`: evaluator/debug assignment paths can create or overwrite globals instead of rejecting unknown targets.
- `RUNTIMESAFE-SAFE-STATE-DISCARD`: safe-state write failures are discarded.
- `RUNTIMESAFE-DEBUG-WRITE-DISCARD`: queued debug write failures are discarded.
- `RUNTIMESAFE-CLOUD-STATE-DEFAULT`: persisted runtime-cloud parse/write failures become defaults or ignored writes.
- `RUNTIMESAFE-AUDIT-EVENT-DROP`: audit/event send failures are discarded without counter or durable event.
- `RUNTIMESAFE-MESH-TIMEOUT-EMPTY`: mesh timeout or send failure is indistinguishable from a successful empty snapshot.
- `RUNTIMESAFE-RETAIN-COMMIT-ORDER`: physical output commit happens before due retain persistence.
- `RUNTIMESAFE-GPIO-NO-HEALTH`: GPIO read/write errors are not exposed through driver health.
- `RUNTIMESAFE-RETAIN-ORPHAN-SILENT`: retain orphan cleanup has no structured event.
- `RUNTIMESAFE-FEATURE-DISABLED-SILENT`: disabled debug surfaces lack structured `feature_disabled` behavior.
- `RUNTIMESAFE-COERCE-WARNING-ONLY`: HIR implicit-conversion warnings lack explicit runtime coercion/proof or rejection.

## Ownership Boundary

- I/O drivers own protocol/transport health and failure classification.
- Runtime cycle owns execution ordering and whether outputs may be committed.
- Retain store owns durable persistence guarantees.
- Retain codec/migration owns corruption and schema evolution decisions.
- Debug/control owns request responses and write observability.
- Runtime-cloud state modules own persisted state load/store errors.
- Mesh modules own timeout versus successful empty snapshot semantics.
- The doctor owns source-pattern enforcement; checklist text does not count as enforcement.

## Allowlist

Allowlist file: `docs/internal/architecture/runtime-safety-fail-closed-allowlist.toml`

Maximum entries: 5.

Every entry must name:

- `id`
- `rule`
- `owner`
- `rationale`
- `review_date`
- `test_evidence`

Optional match narrowing:

- `path`
- `path_prefix`
- `line_pattern`

Broad compatibility exceptions are not accepted. A compatibility path must also be named in code/config and covered by a failing-then-passing test before the gate can flip from warn-only to fail-class.
