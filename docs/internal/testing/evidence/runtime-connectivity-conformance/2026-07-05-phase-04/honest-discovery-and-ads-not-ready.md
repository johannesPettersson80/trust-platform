# Phase 4 Remote Proof - Honest Discovery And ADS NotReady

Date: 2026-07-05 local, 2026-07-04T23:10:09Z remote log start

Remote host: `trust-builder`

Remote validation copy: `/home/johannes/projects/trust-platform-rtconn-validation`

Remote target dir: `/home/johannes/.cache/codex-targets/trust-platform-rtconn-validation`

Remote log dir: `/home/johannes/projects/trust-platform-rtconn-validation/.phase4-logs-20260704T231009Z`

The validation copy was refreshed from the local checkout with the isolated
`rsync` command before running these gates. The main remote checkout was not
used or overwritten.

## Source Coverage

- Modbus and MQTT discovery probes live in
  `crates/trust-runtime/src/control/comm_handlers/discovery_probe.rs`.
- Discovery orchestration and confidence projection live in
  `crates/trust-runtime/src/control/comm_handlers/discover.rs`.
- Discovery regressions live in
  `crates/trust-runtime/src/control/comm_handlers/discover_tests.rs`.
- ADS server cold-start degradation lives in
  `crates/trust-runtime/src/host/ads/server/lifecycle.rs`,
  `crates/trust-runtime/src/host/ads/server/symbols.rs`, and
  `crates/trust-runtime/src/host/ads/server/doctor.rs`.
- ADS `not_ready` connector mapping is covered by
  `crates/trust-runtime/src/connectors/adapters/ads.rs`,
  `crates/trust-runtime/src/connectors/mapping.rs`, and
  `crates/trust-runtime/tests/connectors_status.rs`.
- CLI discovery probe flags and help are covered by
  `crates/trust-runtime/src/bin/trust-runtime/cli/comm.rs`,
  `crates/trust-runtime/src/bin/trust-runtime/comm.rs`, and
  `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs`.
- Public discovery confidence docs were updated in
  `docs/public/connect/protocol-matrix.md`.

File size check after the discovery split:

- `discover.rs`: 861 lines.
- `discover_tests.rs`: 244 lines.
- `discovery_probe.rs`: 484 lines.
- `host/ads/server/lifecycle.rs`: 287 lines.

## Intentional Golden Updates

The Phase 0 discovery goldens changed because Phase 4 intentionally fixes the
old honesty bug where a random TCP listener could look like a confirmed
protocol endpoint.

- `crates/trust-runtime/tests/fixtures/connectors/phase0/discovery/modbus_tcp_listener_observed.json`
  changed the TCP-only Modbus listener from confirmed protocol discovery to
  `confidence: "port_reachable"`, `source: "tcp_connect"`, with
  `probe_source`, `probe_detail`, and a warning telling operators to configure
  a safe read probe when FC43/14 device identification is unavailable.
- `crates/trust-runtime/tests/fixtures/connectors/phase0/discovery/mqtt_tcp_listener_observed.json`
  changed the TCP-only MQTT listener from confirmed protocol discovery to
  `confidence: "port_reachable"`, `source: "tcp_connect"`, with
  `probe_detail`, `auth_required: false`, and a warning that no MQTT CONNACK
  was received.

## Remote Validation

Disk preflight:

```text
Filesystem     Type   Size  Used Avail Use% Mounted on
/dev/sda1      ext4   301G  179G  110G  63% /
tmpfs          tmpfs  7.7G  921M  6.8G  12% /tmp
```

Focused Phase 4 tests:

- `cargo test -p trust-runtime discovery_probe --lib`: passed, 4 tests.
- `cargo test -p trust-runtime comm_handlers::discover --lib`: passed, 12 tests.
- `cargo test -p trust-runtime lifecycle_starts_not_ready --lib`: passed, 1 test.
- `cargo test -p trust-runtime --test connectors_status`: passed, 12 tests.
- `cargo test -p trust-runtime parse_comm_discover_command`: passed, named
  parser test passed.
- `cargo test -p trust-runtime phase0_discovery_matches_current_goldens --lib`:
  passed, 1 test.

Protocol and communication gates:

- `cargo test -p trust-runtime --test modbus_driver`: passed, 2 passed and 2 ignored.
- `cargo test -p trust-runtime --test opcua_integration`: passed, 4 tests.
- `cargo test -p trust-runtime --test opcua_client_runtime`: passed, 2 tests.
- `cargo test -p trust-runtime --test ads_cli_command`: passed, 9 tests.
- `cargo test -p trust-runtime --test ads_web_api`: passed, 6 tests.
- `cargo test -p trust-runtime --test ethercat_driver`: passed, 3 passed and 2 ignored.
- `cargo test -p trust-runtime --test io_multidriver_live`: passed, 2 tests.
- `./scripts/runtime_comms_conformance_gate.sh`: passed, final marker
  `[conformance-gate] PASS`.

Common and boundary gates:

- `just fmt`: passed.
- `just clippy`: passed in 90 seconds.
- `just test`: passed, 831 passed and 16 ignored.
- `just test-all`: passed in 578 seconds.

Networking gates:

- `./scripts/runtime_mesh_tls_stability_gate.sh --iterations 8`: passed, final
  marker `[mesh-gate] PASS`.
- `RUSTFLAGS=-Dwarnings cargo check -p trust-runtime --all-targets`: passed in
  155 seconds.

## Result

Phase 4 is validated on `trust-builder`. Modbus and MQTT discovery now
distinguish wire-proven protocol discovery from TCP reachability, MQTT probes
use clean-session CONNECT/CONNACK and immediate DISCONNECT, auth-required MQTT
brokers are classified separately, and ADS server cold start without a runtime
snapshot degrades to `not_ready` instead of hard-failing.
