# Runtime Execution Backend Migration Policy

This note defines the MP-060 production backend contract after VM-default cutover.

## Production backend controls

- Startup configuration:
  - `runtime.execution_backend = "vm"` in `runtime.toml`.
  - `runtime.execution_backend = "interpreter"` is rejected by runtime config validation.
- CLI override:
  - `--execution-backend=vm` on `trust-runtime run` and `trust-runtime play`.
  - `--execution-backend=interpreter` is rejected by CLI argument parsing.
- Runtime control plane:
  - Live `config.set` writes to `runtime.execution_backend` are rejected.
  - Backend changes remain startup-boundary only.

## Interpreter policy

- Interpreter execution is retained only behind opt-in `legacy-interpreter` feature builds as a parity/test oracle.
- Allowed contexts:
  - differential suites and parity checks (`bytecode_vm_differential`, selected debug/runtime parity tests),
  - benchmark comparison surfaces that explicitly compare VM vs interpreter (`bench execution-backend`).
- Not allowed in production run/play startup selection.

## Enforcement and evidence

- Runtime/tests enforcing VM-only production selection:
  - `crates/trust-runtime/src/bin/trust-runtime/run/tests.rs`:
    - `execution_backend_selection_defaults_to_vm`
    - `execution_backend_selection_uses_bundle_when_cli_absent`
  - `crates/trust-runtime/src/config/tests.rs`:
    - `runtime_schema_rejects_interpreter_execution_backend_for_production`
    - `runtime_config_load_defaults_execution_backend_source_when_omitted`
  - `crates/trust-runtime/src/control/tests/core.rs`:
    - `config_set_rejects_runtime_backend_switch_during_live_control`
- CI release-evidence gates:
  - `.github/workflows/ci.yml` job `version-release-guard`.
  - `scripts/check_release_version_alignment.py` (workspace + VS Code version sync).
  - `scripts/check_version_release_evidence.py` (tag/workflow/release evidence on main/master version bumps).

## Operator migration expectations

1. Remove legacy `runtime.execution_backend = "interpreter"` from all production projects.
2. Use `runtime.execution_backend = "vm"` (or omit backend field and rely on VM default).
3. Treat backend mode as startup-only; no in-flight backend switching.
4. For legacy interpreter behavior investigation, run dedicated differential/benchmark workflows rather than production startup with interpreter.
   - use `--features legacy-interpreter` on dedicated parity/benchmark commands.
5. When comparing VM benchmark results across machines, pin portable builds with `TRUST_RUNTIME_HOST_CODEGEN=generic`. Use `TRUST_RUNTIME_HOST_CODEGEN=native` only for host-local tuning runs, because host-native binaries may not run on other CPUs.
