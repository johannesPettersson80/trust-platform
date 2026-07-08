# Phase 7 Device-In-The-Loop Gates

Date: 2026-07-05

Validation copy: `trust-builder:/home/johannes/projects/trust-platform-rtconn-validation`

Shared target directory:
`trust-builder:/home/johannes/.cache/codex-targets/trust-platform-rtconn-validation`

## Implementation

Phase 7 adds a dedicated ignored device-in-the-loop suite:

- `crates/trust-runtime/tests/device_in_the_loop.rs`
- `scripts/runtime_device_in_loop_gate.sh`
- `.github/workflows/protocol-device-in-loop.yml`
- `docs/internal/testing/runtime-device-in-the-loop.md`

The suite has one ignored test each for EtherCAT, ADS, Modbus TCP, and MQTT.
When lab variables are absent, each test writes an explicit JSON skip artifact.
When `TRUST_DIT_REQUIRE_HARDWARE=1`, missing prerequisites become hard failures.

Workflow behavior:

- Scheduled/manual workflow defaults to hosted Linux and produces skip artifacts
  when lab variables are absent.
- Repository variable `TRUST_DIT_RUNNER` can redirect the workflow to a lab
  self-hosted runner.
- Manual `require_hardware=true` runs fail instead of silently passing missing
  hardware.

## Disk Preflight

Command:

```bash
ssh trust-builder 'df -hT /home/johannes /tmp && du -xhd1 "$HOME/projects" 2>/dev/null | sort -h | tail -20 && du -xhd1 "$HOME/.cache" 2>/dev/null | sort -h | tail -20'
```

Result before Phase 7 remote gates:

- `/home/johannes`: 111G free.
- `/tmp`: 6.7G free.

The isolated validation copy was trimmed before sync by removing copied
heavyweight generated/evidence directories only. After trimming, it was 534M.
The final post-gate disk check showed:

- `/home/johannes`: 97G free.
- `/tmp`: 6.7G free.
- Validation copy: 534M.
- Shared target directory: 15G.

## Local Proof

Commands:

```bash
cargo fmt --all -- --check
git diff --check
cargo test -p trust-runtime --test device_in_the_loop --no-run
OUT_DIR=target/gate-artifacts/device-in-the-loop-local scripts/runtime_device_in_loop_gate.sh
```

Results:

- `cargo fmt --all -- --check`: pass after rustfmt.
- `git diff --check`: pass.
- `cargo test -p trust-runtime --test device_in_the_loop --no-run`: pass.
- Local skip-mode gate: pass; wrote four JSON artifacts:
  - `ads-doctor.json`
  - `ethercat-discovery.json`
  - `modbus-discovery.json`
  - `mqtt-interop.json`

## Remote Device-In-The-Loop Gate

Command:

```bash
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" OUT_DIR="$HOME/projects/trust-platform-rtconn-validation/gate-artifacts/phase7-device-in-loop" scripts/runtime_device_in_loop_gate.sh'
```

Result: pass in skip mode because the builder has no lab protocol targets
configured.

Artifacts:

- `gate-artifacts/phase7-device-in-loop/ads-doctor.json`:
  `status=skipped`, missing `TRUST_DIT_ADS_TARGET`.
- `gate-artifacts/phase7-device-in-loop/ethercat-discovery.json`:
  `status=skipped`, missing `TRUST_DIT_ETHERCAT_ADAPTER`.
- `gate-artifacts/phase7-device-in-loop/modbus-discovery.json`:
  `status=skipped`, missing `TRUST_DIT_MODBUS_HOST`.
- `gate-artifacts/phase7-device-in-loop/mqtt-interop.json`:
  `status=skipped`, missing `TRUST_DIT_MQTT_BROKER`.

This satisfies the Phase 7 alternate completion path: no lab hardware was
available on `trust-builder`, and the skip evidence names the missing device
configuration explicitly.

## Communication-Specific Gate Bundle

Commands:

```bash
cargo test -p trust-runtime --test opcua_integration
cargo test -p trust-runtime --test opcua_client_runtime
cargo test -p trust-runtime --test ads_cli_command
cargo test -p trust-runtime --test ads_web_api
cargo test -p trust-runtime --test modbus_driver
cargo test -p trust-runtime --test ethercat_driver
cargo test -p trust-runtime --test io_multidriver_live
OUT_DIR="$HOME/projects/trust-platform-rtconn-validation/gate-artifacts/phase7-runtime-comms-conformance" ./scripts/runtime_comms_conformance_gate.sh
```

Results:

- `opcua_integration`: 4 passed.
- `opcua_client_runtime`: 2 passed.
- `ads_cli_command`: 9 passed.
- `ads_web_api`: 6 passed.
- `modbus_driver`: 8 passed, 2 ignored.
- `ethercat_driver`: 3 passed, 2 ignored.
- `io_multidriver_live`: 2 passed.
- `runtime_comms_conformance_gate.sh`: PASS.

Runtime comms conformance artifacts:
`gate-artifacts/phase7-runtime-comms-conformance/`

Summary JSON result:

```json
{"result":"pass"}
```

Suites covered by the summary:

- `t0-shm`
- `zenoh-mesh`
- `gateway-bridge`
- `config-rollout`
- `audit-ha`

## Common Gate

Commands:

```bash
just fmt
just clippy
just test
```

Results on `trust-builder`:

- `just fmt`: pass.
- `just clippy`: pass.
- `just test`: pass; `cargo-nextest` was absent, so the repo fallback ran
  `cargo test -p trust-runtime --lib` with 838 passed and 16 ignored.

## Networking Gates

Commands:

```bash
./scripts/runtime_mesh_tls_stability_gate.sh --iterations 8
RUSTFLAGS=-Dwarnings cargo check -p trust-runtime --all-targets
```

Results:

- `runtime_mesh_tls_stability_gate.sh --iterations 8`: PASS, all 8 runs passed
  on first attempt.
- `RUSTFLAGS=-Dwarnings cargo check -p trust-runtime --all-targets`: pass in
  2m34s.

## Notes

No real lab EtherCAT, ADS, Modbus, or MQTT target was configured on
`trust-builder` during this run. The new suite and workflow are therefore
proven for skip and artifact behavior, while real hardware acceptance remains a
workflow dispatch or lab-runner operation using the documented environment
contract.
