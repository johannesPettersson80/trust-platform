# Runtime Boundary Fail-Closed Contract

Status: Active
Owner: runtime/harness-control
Date: 2026-05-03

## Rule

Any name or path submitted through an external runtime observation or control surface must resolve to a declared entity. Missing, ambiguous, unsupported, or invalid paths must return a structured error. `Value::Null` is a declared runtime value only; it must not be used as a missing-name fallback.

## Trigger

GitHub issue #77 reported downstream watch paths such as `arr[1]` and `fb.IN` returning invented defaults under a `CONFIGURATION` setup. Current main did not reproduce the exact typed-default behavior, but it did contain the same fail-open class:

- `set_input("drrive", ...)` silently created a new global instead of rejecting the typo.
- `bind_direct("drrive", "%IX0.0")` silently registered an undeclared binding target.
- watch snapshots converted unresolved names to `Value::Null` inside an `ok=true` response.
- `CONFIGURATION` with a declared top-level `PROGRAM` but no `PROGRAM ... WITH` binding skipped that program without a load-time error.

## Locked Decisions

- Unbound `PROGRAM` under `CONFIGURATION` is a hard compile/load error by default.
- `CompileSession::with_extra_program_instances(...)` remains the explicit test-builder opt-in for programs that should be instantiated outside the normal configuration binding list.
- `trust-harness` defaults to JSON protocol version 2 for watch envelopes and accepts `--protocol-version 1` or `TRUST_HARNESS_PROTOCOL_VERSION=1` for one-release legacy watch output.
- Runtime-internal evaluator `Value::Null` propagation is not covered by this boundary contract; it belongs to a separate runtime-internal-init-safety board.

## Error Shape

Boundary errors carry stable codes:

- `unresolved_name`
- `unbound_program`
- `ambiguous_name`
- `unsupported_path_syntax`
- `wrong_kind`
- `undeclared_binding`
- `internal_lock_failure`
- `internal_failure`

Watch protocol v2 uses a per-entry envelope:

```json
{"status":"ok","value":{"type":"INT","value":42}}
{"status":"error","code":"unresolved_name","message":"boundary path 'drrive' did not resolve to a declared value","path":"drrive","candidates":[]}
```

## Enforced Surfaces

- `crates/trust-runtime/src/host/harness/harness.rs`
- `crates/trust-runtime/src/host/harness/protocol.rs`
- `crates/trust-runtime/src/bin/trust-harness.rs`
- `crates/trust-dev/src/agent/harness.rs`
- `crates/trust-runtime/src/control/debug_handlers_variables.rs`
- `crates/trust-runtime/src/control/debug_handlers_eval.rs`
- `crates/trust-runtime/src/control.rs`
- `crates/trust-runtime/src/web/hmi_ws.rs`
- `crates/trust-runtime/src/web/runtime_cloud_routes/control_proxy.rs`
- `crates/trust-runtime/src/web/ui_routes.rs`

The enforced gate is `scripts/runtime_boundary_fail_closed_ast_grep_gate.sh`, surfaced through `FULLMAP-RUNTIMEBOUND` in `cargo run -p xtask -- architecture-doctor --full-map`.

## Separate Board

Runtime-internal evaluator `Value::Null` propagation, including `host/instance.rs` initialization chains, remains outside this boundary contract. That work requires a separate runtime-internal-init-safety board because those paths are interpreter internals rather than external observation/control protocol surfaces.
