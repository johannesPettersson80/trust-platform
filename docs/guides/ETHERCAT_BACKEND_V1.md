# EtherCAT Backend v1 (EtherCAT Driver)

This guide describes `io.driver = "ethercat"` in truST runtime.

v1 scope focuses on deterministic process-image exchange for common digital
I/O module chains (for example Beckhoff-style `EK1100` + `EL1008` + `EL2008`).

## Scope Boundaries

- Included in v1:
  - EtherCAT driver profile with deterministic module-chain mapping.
  - Startup/discovery diagnostics with explicit discovered module summary.
  - Cycle-time health telemetry (`ok`/`degraded`/`faulted`) surfaced via control/web status.
  - EtherCrab-backed hardware transport for non-mock adapters on unix targets.
  - Deterministic mocked transport mode (`adapter = "mock"`) for CI and offline validation.
- Explicitly out of scope in v1:
  - Functional safety claims.
  - SIL certification claims.
  - Advanced motion profile support.

## io.toml Example (Hardware)

```toml
[io]
driver = "ethercat"

[io.params]
adapter = "eth0"
timeout_ms = 250
cycle_warn_ms = 5
on_error = "fault"

[[io.params.modules]]
model = "EK1100"
slot = 0

[[io.params.modules]]
model = "EL1008"
slot = 1
channels = 8

[[io.params.modules]]
model = "EL2008"
slot = 2
channels = 8

[[io.safe_state]]
address = "%QX0.0"
value = "FALSE"
```

## io.toml Example (Deterministic Mock)

```toml
[io]
driver = "ethercat"

[io.params]
adapter = "mock"
timeout_ms = 250
cycle_warn_ms = 5
on_error = "fault"
mock_inputs = ["01", "00"]
```

## Hardware Setup Checklist (Preparation)

1. Connect coupler and digital modules in physical order matching `io.params.modules`.
2. Bind a dedicated EtherCAT NIC and prepare interface naming (example: `eth0`).
3. Keep task interval and expected bus cycle budget aligned (`cycle_warn_ms` is an early-warning threshold).
4. Start in `on_error = "fault"` for production fault containment.
5. Validate before launch:

```bash
trust-runtime validate --project <project-folder>
trust-runtime --project <project-folder>
```

## Diagnostics and Health

- Discovery diagnostics: driver reports discovered module chain and process image sizes.
- Health states:
  - `ok`: discovery and cycle exchange within thresholds.
  - `degraded`: recoverable issue (for example warn/ignore policy error or cycle budget exceed).
  - `faulted`: non-recoverable driver fault under `on_error = "fault"`.
- Health is visible from `status`/`health` control responses and Web UI driver health cards.

## Deterministic Mock Mode (CI/Local)

Use `adapter = "mock"` with `mock_inputs` to run deterministic integration tests
without EtherCAT hardware.

Hardware transport notes:

- Non-mock adapters use EtherCrab transport and require a unix runtime target.
- Raw-socket access is required on the selected NIC.

## License and Trademark Compliance Checkpoint

Before public release/distribution of EtherCAT backend artifacts:

1. Keep third-party notices current (including EtherCAT-related dependencies such as EtherCrab when used in build artifacts).
2. Keep project dual-license notices (`MIT OR Apache-2.0`) intact in distributed packages.
3. Review EtherCAT trademark/logo/certification wording in docs and release notes to avoid implied certification claims.
