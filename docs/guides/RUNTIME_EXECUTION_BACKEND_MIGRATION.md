# Runtime Execution Backend Policy

This note defines the current `trust-runtime` execution-backend contract after legacy interpreter removal from the production runtime path.

## Current Runtime Contract

- Production task/program execution is bytecode-VM only.
- `runtime.execution_backend = "vm"` is accepted.
- `runtime.execution_backend = "interpreter"` is rejected by runtime config validation.
- `--execution-backend=vm` is accepted on `trust-runtime run` and `trust-runtime play`.
- `--execution-backend=interpreter` is rejected by CLI argument parsing.
- Live `config.set` writes to `runtime.execution_backend` remain rejected; backend mode is startup-only.

## What Still Uses Helper Evaluation

The runtime still has small storage-native helper evaluators for non-cycle flows:

- compile-time constant folding
- initializer/config/build evaluation
- debug watch/logpoint expression reads
- debug lvalue writes

Those helper paths do not constitute a second runtime backend and are not used for scheduled task/program execution.

## Enforcement and Evidence

- Runtime/tests enforcing VM-only production selection:
  - `crates/trust-runtime/src/bin/trust-runtime/run/tests.rs`
  - `crates/trust-runtime/src/config/tests.rs`
  - `crates/trust-runtime/src/control/tests/core.rs`
- CI/runtime guardrails:
  - `scripts/runtime_vm_production_backend_guard.sh`
  - `scripts/runtime_vm_bench_gate.sh`
  - `scripts/runtime_vm_determinism_reliability_gate.sh`
- Release evidence gates:
  - `.github/workflows/ci.yml`
  - `.github/workflows/release.yml`
  - `.github/workflows/nightly-reliability.yml`

## Operator Expectations

1. Remove any legacy `runtime.execution_backend = "interpreter"` setting from projects.
2. Use `runtime.execution_backend = "vm"` or omit the field and rely on the VM default.
3. Treat backend mode as startup-only.
4. For cross-machine performance comparisons, pin portable builds with `TRUST_RUNTIME_HOST_CODEGEN=generic`.
5. Use `TRUST_RUNTIME_HOST_CODEGEN=native` only for self-built, hardware-specific deployments where maximum local performance matters more than binary portability.
