# Runtime Connectivity And Conformance Remediation Checklist

Status: implementation validated; release proof and external-input follow-ups remain open
Owner: runtime/connectivity
Scope: repair the truST Mesh connectivity architecture, protocol honesty, and conformance proof surface without collapsing scan-cycle I/O and supervisory connector responsibilities.

## Contract Summary

- [x] `RTCONN-SUM-001` Preserve `IoDriver` as the scan-cycle process-image interface for drivers that read inputs and write outputs each cycle. Evidence: `crates/trust-runtime/src/io/driver.rs` still owns the `IoDriver` scan-cycle trait; process-image connector reporting is projected through `connectors/adapters/io_driver.rs` without replacing the trait.
- [x] `RTCONN-SUM-002` Do not force ADS or OPC UA supervisory clients/servers into the `IoDriver` execution loop. Evidence: ADS and OPC UA status projection lives in `connectors/adapters/ads.rs` and `connectors/adapters/opcua.rs`; Phase 2/3 evidence records that ADS notification behavior and the OPC UA persistent worker remain protocol-owned.
- [x] `RTCONN-SUM-003` Add one shared status, config projection, discovery-confidence, and point-quality contract that both process-image drivers and supervisory connectors project into. Evidence: `connectors/contract.rs`, `connectors/report.rs`, and protocol adapters project I/O, ADS, OPC UA, MQTT, Modbus, EtherCAT, and GPIO into the shared schema; HMI/VS Code propagation is recorded in Phase 5 evidence.
- [x] `RTCONN-SUM-004` Keep transport execution loops protocol-owned; unify the reporting contract, not the transport implementation. Evidence: connector modules contain schema/report/adapters only; ADS, OPC UA, Modbus, MQTT, EtherCAT, and GPIO retain their protocol modules and transport loops.
- [x] `RTCONN-SUM-005` Treat this board as runtime architecture work. Update diagrams, architecture checklists, evidence, and release hygiene when behavior changes. Evidence: diagram sources/manifest, `architecture-improvements.md`, phase evidence files, and `CHANGELOG.md` were updated across the board; remote diagram/common gates are recorded in phase evidence.

## Non-Goals

- [x] `RTCONN-NONGOAL-001` No new protocols in this board. Do not add S7, PROFINET, EtherNet/IP, or other new fieldbus stacks. Evidence: connector protocol coverage stays within ADS, OPC UA, Modbus TCP, MQTT, EtherCAT, GPIO, simulated, and loopback; Sparkplug B is an MQTT profile, not a new fieldbus stack.
- [x] `RTCONN-NONGOAL-002` No redesign of the `IoDriver` trait. Evidence: `IoDriver` remains the scan-cycle trait in `crates/trust-runtime/src/io/driver.rs`; connector work adds projection adapters rather than trait shape changes.
- [x] `RTCONN-NONGOAL-003` No unification of ADS, OPC UA, MQTT, Modbus, EtherCAT, or GPIO transport execution loops. Evidence: transport modules remain protocol-owned; shared connector code is confined to `crates/trust-runtime/src/connectors/**`.
- [x] `RTCONN-NONGOAL-004` No Mesh internals rewrite beyond the named status/discovery surfaces. Evidence: this board used the runtime mesh/TLS stability gate for networking changes and did not move connector reporting into mesh internals.
- [x] `RTCONN-NONGOAL-005` No runtime.toml/io.toml format unification in Phases 1-3. Evidence: Phase 3 row `RTCONN-P3-015` records existing `runtime.opcua_client`/`opcua_client.toml` settings and no `runtime.toml`/`io.toml` schema unification in Phases 1-3.
- [x] `RTCONN-NONGOAL-006` No graphical-language support work beyond explicit PLCopen non-ST rejection diagnostics and fixtures. Evidence: Phase 9 rejects LD/FBD/SFC/unknown non-ST bodies with named diagnostics; no graphical-language execution/import support was added.
- [x] `RTCONN-NONGOAL-007` No committed generated conformance reports under `conformance/reports/`; reports are CI artifacts unless a later contract explicitly changes that policy. Evidence: `git ls-files conformance/reports` lists only `conformance/reports/.gitkeep`; Phase 8 CI uploads JSON/Markdown reports as artifacts.

## Module Placement Map

Initial decision: connector contract code lives in `crates/trust-runtime`, not `trust-runtime-core`. `trust-runtime-core` currently has no serde dependency and should stay focused on portable execution core semantics until a separate design note proves the schema-only contract belongs there.

- [x] `RTCONN-MOD-001` Add connector contract modules under `crates/trust-runtime/src/connectors/`. Evidence: `crates/trust-runtime/src/connectors/mod.rs`.
- [x] `RTCONN-MOD-002` Use `crates/trust-runtime/src/connectors/mod.rs` for module wiring only. Evidence: `crates/trust-runtime/src/connectors/mod.rs`.
- [x] `RTCONN-MOD-003` Use `crates/trust-runtime/src/connectors/contract.rs` for enums, schema version, discovery confidence, point quality, and report field contracts. Evidence: `crates/trust-runtime/src/connectors/contract.rs`.
- [x] `RTCONN-MOD-004` Use `crates/trust-runtime/src/connectors/report.rs` for report assembly helpers and stable serialization helpers. Evidence: `crates/trust-runtime/src/connectors/report.rs`.
- [x] `RTCONN-MOD-005` Use `crates/trust-runtime/src/connectors/mapping.rs` only for pure state/health mapping helpers shared by adapters. Evidence: `crates/trust-runtime/src/connectors/mapping.rs`.
- [x] `RTCONN-MOD-006` Put each protocol adapter in a new file under `crates/trust-runtime/src/connectors/adapters/`. Evidence: `crates/trust-runtime/src/connectors/adapters/`.
- [x] `RTCONN-MOD-007` Required initial adapter files: `adapters/io_driver.rs`, `adapters/ads.rs`, `adapters/opcua.rs`, `adapters/mqtt.rs`, `adapters/modbus.rs`, and `adapters/ethercat.rs`. Evidence: all six files exist under `crates/trust-runtime/src/connectors/adapters/`.
- [x] `RTCONN-MOD-008` If the control API exposes a new route, add a dedicated `crates/trust-runtime/src/control/connectors_handlers.rs` file instead of appending to `control/comm_handlers/*.rs`. Evidence: `crates/trust-runtime/src/control/connectors_handlers.rs` and `crates/trust-runtime/src/control/handlers/connectors.rs`.
- [x] `RTCONN-MOD-009` Do not append connector reporting code to transport modules such as `host/ads/**`, `host/opcua/**`, `io/modbus.rs`, `io/mqtt/**`, or `io/ethercat/**` except for narrow adapter hooks. Evidence: `rg -n "crate::connectors|connectors::" crates/trust-runtime/src/host/ads crates/trust-runtime/src/host/opcua crates/trust-runtime/src/io/modbus.rs crates/trust-runtime/src/io/mqtt.rs crates/trust-runtime/src/io/ethercat.rs crates/trust-runtime/src/io/gpio.rs crates/trust-runtime/src/io/mqtt crates/trust-runtime/src/io/modbus -g '*.rs'` returned no connector imports; connector projection code lives under `src/connectors/adapters/`.
- [x] `RTCONN-MOD-010` Do not move connector contract types into `trust-runtime-core` without a reviewed design note covering serde, no-std impact, public API impact, and downstream schema consumers. Evidence: `rg -n "ConnectorStatusReport|ConnectorState|DiscoveryConfidence|PointQuality" crates/trust-runtime-core -g '*.rs'` returned no connector contract types; they remain in `crates/trust-runtime/src/connectors/`.
- [x] `RTCONN-MOD-011` Add focused tests in `crates/trust-runtime/tests/connectors_status.rs` and protocol-specific integration tests only where needed. Evidence: `cargo test -p trust-runtime --test connectors_status` passed locally with 7 tests.
- [x] `RTCONN-MOD-012` Store golden JSON fixtures under `crates/trust-runtime/tests/fixtures/connectors/`. Evidence: `crates/trust-runtime/tests/fixtures/connectors/phase0/**` and remote `cargo test -p trust-runtime phase0 --lib` passed with 5 tests.

## Compatibility Rules

- [x] `RTCONN-COMPAT-001` Phases 1-2 are additive-only. Evidence: Phases 1-2 added the `connectors.status` projection surface and ADS projections while preserving existing ADS, OPC UA, and `IoDriverStatus` JSON goldens.
- [x] `RTCONN-COMPAT-002` Add a new connectors status surface with an explicit `schema_version` field. Evidence: `connectors_status_reports_process_image_drivers_without_mutating_legacy_status` in `crates/trust-runtime/src/control/tests/connectors.rs`.
- [x] `RTCONN-COMPAT-003` Existing ADS status JSON must stay byte-identical to Phase 0 goldens through Phase 2 unless a checklist row records a deliberate breaking change. Evidence: `RTCONN-P2-009` records remote `cargo test -p trust-runtime phase0 --lib` passing after ADS connector projection.
- [x] `RTCONN-COMPAT-004` Existing OPC UA status JSON must stay byte-identical to Phase 0 goldens through Phase 2 unless a checklist row records a deliberate breaking change. Evidence: `RTCONN-P1-018` records `phase0_opcua_status_matches_capability_goldens` passing on `trust-builder`; Phase 2 did not change OPC UA status surfaces.
- [x] `RTCONN-COMPAT-005` Existing `IoDriverStatus` JSON must stay byte-identical to Phase 0 goldens through Phase 2 unless a checklist row records a deliberate breaking change. Evidence: `RTCONN-P1-018` records `phase0_io_driver_status_matches_legacy_golden` passing on `trust-builder`; Phase 2 did not change `IoDriverStatus`.
- [x] `RTCONN-COMPAT-006` No `runtime.toml` format changes in Phases 1-3. Evidence: `RTCONN-P3-015` records no `runtime.toml` schema changes in the OPC UA persistent worker phase; Phases 1-2 were additive status projection only.
- [x] `RTCONN-COMPAT-007` No `io.toml` format changes in Phases 1-3. Evidence: `RTCONN-P3-015` records no `io.toml` schema changes in Phase 3; Phases 1-2 did not change I/O config schema.
- [x] `RTCONN-COMPAT-008` `ReconnectPolicy` is a projection of existing per-driver settings in Phases 1-3, not a new config schema. Evidence: `ReconnectPolicy` lives in `connectors/contract.rs` and is populated by adapters/report builders; no Phase 1-3 config schema adds a `ReconnectPolicy` field.
- [x] `RTCONN-COMPAT-009` Golden fixture updates require an explicit diff note in the relevant checklist row. Evidence: `RTCONN-P0-013` records Phase 0 golden diff notes; Phase 4 golden discovery changes are recorded in `honest-discovery-and-ads-not-ready.md`.
- [x] `RTCONN-COMPAT-010` If a compatibility rule must be broken, stop and create a separate migration checklist before continuing this board. Evidence: no Phase 1-3 compatibility break was taken; later intentional discovery honesty changes are documented under Phase 4, not hidden in the Phase 1-2 additive surface.

## Evidence Convention

- [x] `RTCONN-EVID-001` Store phase evidence under `docs/internal/testing/evidence/runtime-connectivity-conformance/`. Evidence: baseline and mapping-review files added under that directory.
- [x] `RTCONN-EVID-002` Each phase gets a dated subdirectory, for example `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-04-phase-01/`. Evidence: `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-04-phase-01/remote-focused-connector-test.md`.
- [x] `RTCONN-EVID-003` Every completed checklist row must include the command, test, artifact path, or source file proving completion. Evidence: completed rows in this checklist include source/test/evidence references as of the Phase 1 scaffold slice.
- [x] `RTCONN-EVID-004` Status rows must be updated as work lands, not only at the end of the board. Evidence: rows updated with source and remote focused-test proof during the Phase 1 scaffold slice.
- [x] `RTCONN-EVID-005` Current known status/discovery truth gaps go in `docs/internal/testing/evidence/runtime-connectivity-conformance/current-status-and-discovery-baseline.md`. Evidence: `docs/internal/testing/evidence/runtime-connectivity-conformance/current-status-and-discovery-baseline.md`.
- [x] `RTCONN-EVID-006` Use `docs/internal/testing/evidence/runtime-connectivity-conformance/state-mapping-review.md` for the reviewed mapping-table signoff. Evidence: `docs/internal/testing/evidence/runtime-connectivity-conformance/state-mapping-review.md`.

## Builder Gate Template

Run the remote disk preflight before broad builder gates:

```sh
ssh trust-builder 'df -hT /home/johannes /tmp && du -xhd1 "$HOME/projects" 2>/dev/null | sort -h | tail -20 && du -xhd1 "$HOME/.cache" 2>/dev/null | sort -h | tail -20'
```

Common per-phase gate:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform" && just fmt'
ssh trust-builder 'cd "$HOME/projects/trust-platform" && just clippy'
ssh trust-builder 'cd "$HOME/projects/trust-platform" && just test'
```

Full boundary gate, required at Phases 2, 4, 8, and final closeout at minimum:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform" && just test-all'
```

Runtime networking gate, required for any phase that changes networking/runtime protocol behavior:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform" && ./scripts/runtime_mesh_tls_stability_gate.sh --iterations 8'
ssh trust-builder 'cd "$HOME/projects/trust-platform" && RUSTFLAGS=-Dwarnings cargo check -p trust-runtime --all-targets'
```

Communication-specific gate bundle:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform" && cargo test -p trust-runtime --test opcua_integration'
ssh trust-builder 'cd "$HOME/projects/trust-platform" && cargo test -p trust-runtime --test opcua_client_runtime'
ssh trust-builder 'cd "$HOME/projects/trust-platform" && cargo test -p trust-runtime --test ads_cli_command'
ssh trust-builder 'cd "$HOME/projects/trust-platform" && cargo test -p trust-runtime --test ads_web_api'
ssh trust-builder 'cd "$HOME/projects/trust-platform" && cargo test -p trust-runtime --test modbus_driver'
ssh trust-builder 'cd "$HOME/projects/trust-platform" && cargo test -p trust-runtime --test ethercat_driver'
ssh trust-builder 'cd "$HOME/projects/trust-platform" && cargo test -p trust-runtime --test io_multidriver_live'
ssh trust-builder 'cd "$HOME/projects/trust-platform" && ./scripts/runtime_comms_conformance_gate.sh'
```

## Phase 0 - Baseline, Coordination, And Behavior Locks

- [x] `RTCONN-P0-001` Record the base branch, base commit, and active ADS client branch or PR in the evidence directory. Evidence: `current-status-and-discovery-baseline.md` records branch `ads/client` and commit `6a7492cae`.
- [x] `RTCONN-P0-002` Do not start Phase 2 until the active `ads/client` work has merged, or this board has rebased onto it and updated the collision notes. Evidence: `current-status-and-discovery-baseline.md` records the sequencing constraint.
- [x] `RTCONN-P0-003` Inventory process-image drivers: simulated, loopback, GPIO, Modbus TCP, MQTT, and EtherCAT. Evidence: `current-status-and-discovery-baseline.md`.
- [x] `RTCONN-P0-004` Inventory supervisory connectors: ADS client, ADS server, OPC UA client, and OPC UA server. Evidence: `current-status-and-discovery-baseline.md`.
- [x] `RTCONN-P0-005` Inventory current status consumers: CLI, control API, HMI, Network Canvas, logs, and docs. Evidence: `current-status-and-discovery-baseline.md`.
- [x] `RTCONN-P0-006` Capture ADS status JSON goldens under `crates/trust-runtime/tests/fixtures/connectors/phase0/ads/`. Evidence: `phase0/ads/client_disabled.json`, `phase0/ads/server_disabled.json`, and `phase0_ads_status_matches_disabled_goldens` passed on `trust-builder`.
- [x] `RTCONN-P0-007` Capture OPC UA status JSON goldens under `crates/trust-runtime/tests/fixtures/connectors/phase0/opcua/`. Evidence: current public OPC UA server/client status is locked through `comm.capabilities` in `phase0/opcua/capabilities_not_configured.json`.
- [x] `RTCONN-P0-008` Capture `IoDriverStatus` JSON goldens under `crates/trust-runtime/tests/fixtures/connectors/phase0/io_driver/`. Evidence: `phase0/io_driver/status_io_drivers.json`.
- [x] `RTCONN-P0-009` Capture discovery JSON goldens for Modbus, MQTT, ADS, OPC UA, and EtherCAT under `crates/trust-runtime/tests/fixtures/connectors/phase0/discovery/`. Evidence: `phase0/discovery/modbus_tcp_listener_observed.json`, `mqtt_tcp_listener_observed.json`, `ads_client_no_targets.json`, `ads_server_unavailable.json`, `opcua_server_only_warning.json`, and `ethercat_this_host_warning.json`.
- [x] `RTCONN-P0-010` Add behavior-lock tests proving the Phase 0 goldens stay stable. Evidence: `phase0_*` tests in `crates/trust-runtime/src/control/tests/goldens.rs` passed on `trust-builder` with ADS, OPC UA, `IoDriverStatus`, and discovery fixtures.
- [x] `RTCONN-P0-011` Add negative tests proving missing or failed connectors do not report healthy. Evidence: `phase0_missing_or_failed_connectors_do_not_report_healthy` in `crates/trust-runtime/src/control/tests/goldens.rs` passed on `trust-builder`.
- [x] `RTCONN-P0-012` Document current known status/discovery truth gaps in `current-status-and-discovery-baseline.md`. Evidence: `docs/internal/testing/evidence/runtime-connectivity-conformance/current-status-and-discovery-baseline.md`.
- [x] `RTCONN-P0-013` Add a checklist note for every Phase 0 golden update, including old value, new value, and why the change is intentional. Evidence: `remote-phase0-goldens.md` records the added ADS server fixture, the added ADS client no-target discovery fixture, and the corrected omitted-null ADS server fixture fields.

### Phase 0 Completion Gate

- [x] `RTCONN-P0-GATE-001` Run the trust-builder disk preflight. Evidence: `remote-phase0-goldens.md` records `/home/johannes` with 79G free and `/tmp` with 6.8G free.
- [x] `RTCONN-P0-GATE-002` Run targeted behavior-lock tests for current ADS, OPC UA, `IoDriverStatus`, and discovery goldens. Evidence: `ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime phase0 --lib'` passed with 5 tests.
- [x] `RTCONN-P0-GATE-003` Run the common per-phase gate on trust-builder. Evidence: `remote-phase0-goldens.md` records the rustfmt-only drift fix and passing `cargo fmt --check --all`, `just fmt`, `just clippy`, and `just test` on the isolated validation copy.
- [x] `RTCONN-P0-GATE-004` Record command output summaries and artifact paths in the Phase 0 evidence directory. Evidence: `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-04-phase-00/remote-phase0-goldens.md`.

## Phase 1 - Shared Connector Contract And Mapping Table

- [x] `RTCONN-P1-001` Add `ConnectorKind`: `process_image`, `supervisory_client`, `supervisory_server`. Evidence: `crates/trust-runtime/src/connectors/contract.rs`.
- [x] `RTCONN-P1-002` Add `ConnectorProtocol`: `ads`, `opcua`, `modbus_tcp`, `mqtt`, `ethercat`, `gpio`, `simulated`, `loopback`. Evidence: `crates/trust-runtime/src/connectors/contract.rs`.
- [x] `RTCONN-P1-003` Add `ConnectorState`: `disabled`, `configured`, `starting`, `ready`, `degraded`, `reconnecting`, `stale`, `not_ready`, `faulted`. Evidence: `crates/trust-runtime/src/connectors/contract.rs`.
- [x] `RTCONN-P1-004` Add `ConnectorHealth`: `ok`, `degraded`, `faulted`, `unknown`. Evidence: `crates/trust-runtime/src/connectors/contract.rs`.
- [x] `RTCONN-P1-005` Add `PointQuality`: `good`, `stale`, `bad`, `unsupported`, `unavailable`, `write_pending`, `write_failed`. Evidence: `crates/trust-runtime/src/connectors/contract.rs`.
- [x] `RTCONN-P1-006` Document that `stale` is intentionally both a connector state and point quality: connector `stale` means the whole connector is not fresh; point `stale` means a single point is old while the connector may still be usable. Evidence: `contract.rs` docs and `state-mapping-review.md`.
- [x] `RTCONN-P1-007` Add `DiscoveryConfidence`: `confirmed`, `likely`, `port_reachable`, `unavailable`. Evidence: `crates/trust-runtime/src/connectors/contract.rs`.
- [x] `RTCONN-P1-008` Add `ReconnectPolicy`: `disabled`, `fixed_delay`, `exponential_backoff`, `externally_managed`. Evidence: `crates/trust-runtime/src/connectors/contract.rs`.
- [x] `RTCONN-P1-009` Add `ConnectorStatusReport` with `schema_version`, connector id, protocol, kind, endpoint, state, health, confidence, last error, last transition, freshness, and point counts. Evidence: `contract.rs`, `report.rs`, and `connectors_status.rs`.
- [x] `RTCONN-P1-010` Add typed point metadata without coupling it to runtime storage internals. Evidence: `ConnectorPointMetadata` in `contract.rs`.
- [x] `RTCONN-P1-011` Add RBAC contract for the new read surface: connector status reads require Viewer access; no writes are introduced by this phase. Evidence: `required_role_for_control_request("connectors.status", None) == AccessRole::Viewer` in `crates/trust-runtime/src/control/policy.rs`.
- [x] `RTCONN-P1-012` Add authz tests proving unauthenticated local unix control keeps current Viewer behavior and token-protected control rejects missing/invalid tokens. Evidence: `connectors_status_authz_requires_viewer_and_preserves_local_unix_read` in `crates/trust-runtime/src/control/tests/connectors.rs`, passed locally and on `trust-builder`.
- [x] `RTCONN-P1-013` Write the state-mapping table before adapter implementation. Evidence: `state-mapping-review.md`.
- [x] `RTCONN-P1-014` Mapping table must cover `IoDriverHealth` combined with error policy, `AdsConnectionState`, `OpcUaClientConnectionState`, OPC UA server snapshot states, MQTT session/freshness states, Modbus timeout/error states, and EtherCAT bus states. Evidence: `state-mapping-review.md` and `mapping.rs`.
- [x] `RTCONN-P1-015` Review the mapping table and store signoff in `state-mapping-review.md`. Evidence: accepted Phase 1 decision in `state-mapping-review.md`.
- [x] `RTCONN-P1-016` Add unit tests for every state, health, discovery-confidence, and point-quality conversion. Evidence: `cargo test -p trust-runtime --test connectors_status` passed locally with 7 tests.
- [x] `RTCONN-P1-017` Add serialization tests for `ConnectorStatusReport`. Evidence: `connector_status_report_serializes_stable_schema` in `connectors_status.rs`.
- [x] `RTCONN-P1-018` Prove existing ADS, OPC UA, and `IoDriverStatus` JSON goldens remain byte-identical. Evidence: Phase 1 additive surface is present and `phase0_ads_status_matches_disabled_goldens`, `phase0_opcua_status_matches_capability_goldens`, and `phase0_io_driver_status_matches_legacy_golden` passed on `trust-builder`.
- [x] `RTCONN-P1-019` Update diagrams for the new connector reporting contract and adapter ownership. Evidence: `docs/diagrams/architecture/runtime-execution.puml`, `docs/diagrams/generated/runtime-execution.svg`, `docs/diagrams/manifest.json`, and `remote-focused-connector-test.md` record builder render/drift proof.
- [x] `RTCONN-P1-020` Update `docs/internal/testing/checklists/architecture-improvements.md` with the new connector contract ownership rule. Evidence: `ARCH-RTCONN-01` row added.

### Phase 1 SOLID/KISS/DRY Acceptance

- [x] `RTCONN-P1-SOLID-001` Contract modules contain no transport I/O code. Evidence: `contract.rs`, `mapping.rs`, and `report.rs` only define schema, mapping, and report assembly helpers.
- [x] `RTCONN-P1-SOLID-002` Adapters call into transport status/state, but transports do not depend on control/HMI/canvas consumers. Evidence: adapter files under `connectors/adapters/`; no transport module imports `connectors`.
- [x] `RTCONN-P1-SOLID-003` State mapping is table-driven or helper-driven, not copied ad hoc inside every adapter. Evidence: `mapping.rs` owns mapping helpers; adapters delegate to it.
- [x] `RTCONN-P1-SOLID-004` No file added or modified in this phase exceeds 1,000 lines without a split. Evidence: connector source files are split by contract, mapping, report, and per-protocol adapter.

### Phase 1 Completion Gate

- [x] `RTCONN-P1-GATE-001` Run the trust-builder disk preflight. Evidence: `remote-focused-connector-test.md` records 71G free on `/home/johannes` and 3.2G free on `/tmp` after generated-cache cleanup.
- [x] `RTCONN-P1-GATE-002` Run targeted connector contract and mapping tests. Evidence: `remote-focused-connector-test.md`.
- [x] `RTCONN-P1-GATE-003` Run `cargo test -p trust-runtime connectors_status --test connectors_status` or the exact final targeted test name. Evidence: `ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime --test connectors_status'` passed with 7 tests.
- [x] `RTCONN-P1-GATE-004` Run the common per-phase gate on trust-builder. Evidence: `remote-focused-connector-test.md` records passing `cargo fmt --check --all`, `just fmt`, `just clippy`, and `just test` on the isolated validation copy after the rustfmt-only drift fix.
- [x] `RTCONN-P1-GATE-005` Regenerate diagrams and verify drift if `.puml` files changed. Evidence: `ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && scripts/render_diagrams.sh && python scripts/check_diagram_drift.py'` passed; artifacts copied back to `docs/diagrams/generated/runtime-execution.svg` and `docs/diagrams/manifest.json`.
- [x] `RTCONN-P1-GATE-006` Record command output summaries and artifact paths in the Phase 1 evidence directory. Evidence: `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-04-phase-01/remote-focused-connector-test.md`.

## Phase 2 - ADS Reference Implementation

- [x] `RTCONN-P2-001` Confirm Phase 0 ADS sequencing row is satisfied before editing ADS status code. Evidence: this board is running on branch `ads/client`; `current-status-and-discovery-baseline.md` records the ADS sequencing decision.
- [x] `RTCONN-P2-002` Map ADS client status through `connectors/adapters/ads.rs`. Evidence: `project_ads_status_report` in `connectors/adapters/ads.rs` and `ads_status_report_projects_role_endpoint_and_point_counts` passed on `trust-builder`.
- [x] `RTCONN-P2-003` Map ADS server status through `connectors/adapters/ads.rs`. Evidence: `project_ads_status_report_with_default_endpoint` maps target-less ADS server reports and `connectors_status_reports_ads_client_and_server_roles` passed on `trust-builder`.
- [x] `RTCONN-P2-004` Preserve ADS notification/change-of-state behavior. Evidence: this slice only reads existing `AdsStatusReport` values for connector projection and did not change ADS worker, notification, transport, route-add, or config execution files; `ads_cli_command` and `ads_web_api` still pass on `trust-builder`.
- [x] `RTCONN-P2-005` Map ADS route, endpoint, AMS metadata, and role into `ConnectorStatusReport`. Evidence: `ads_status_report_projects_role_endpoint_and_point_counts` asserts client endpoint `5.23.91.12.1.1:851@192.168.77.20`, server endpoint `127.0.0.1.1.1:851@127.0.0.1`, and supervisory client/server kinds.
- [x] `RTCONN-P2-006` Map ADS notification/read/write point freshness into `PointQuality`. Evidence: `project_ads_point_statuses` and `project_active_ads_device_snapshot` in `connectors/adapters/ads.rs`; `ads_point_statuses_project_into_connector_point_quality` and `active_ads_device_snapshot_projects_point_rows_and_counts` passed on `trust-builder`.
- [x] `RTCONN-P2-007` Expose ADS client status through the new additive connectors status surface. Evidence: `connectors_status_reports_ads_client_and_server_roles` finds `ads:client:5.23.91.12.1.1` in `connectors.status`.
- [x] `RTCONN-P2-008` Expose ADS server status through the new additive connectors status surface. Evidence: `connectors_status_reports_ads_client_and_server_roles` finds `ads:server:ads-server` in `connectors.status`.
- [x] `RTCONN-P2-009` Keep existing ADS-specific status surfaces byte-identical to Phase 0 goldens. Evidence: remote `cargo test -p trust-runtime phase0 --lib` passed after the ADS connector projection changes.
- [x] `RTCONN-P2-010` Add tests for ADS connected, reconnecting, stale, faulted, route failure, and auth failure cases. Evidence: `ads_status_report_projects_role_endpoint_and_point_counts`, `connectors_status_reports_ads_client_and_server_roles`, and `ads_status_report_projects_reconnect_stale_fault_and_failure_details` passed on `trust-builder`.
- [x] `RTCONN-P2-011` Add tests proving ADS status appears consistently through CLI/control/HMI-facing runtime surfaces that this phase touches. Evidence: `connectors_status_ads_projection_matches_legacy_ads_status` compares legacy `ads.status` and additive `connectors.status` for the touched control surface; `remote-ads-connector-status.md` records that CLI/HMI rendering was not changed in this Phase 2 slice.
- [x] `RTCONN-P2-012` Add RBAC tests proving ADS connector status reads require Viewer and ADS writes/imports retain their stronger existing authorization rules. Evidence: `connectors_status_authz_requires_viewer_and_preserves_local_unix_read` proves Viewer read and rejects ADS route/live import with Viewer token; `ads_connector_status_and_ads_mutations_keep_role_boundaries` proves route add/remove remain Admin and live imports/write-enabled doctor remain Engineer.

### Phase 2 Completion Gate

- [x] `RTCONN-P2-GATE-001` Run the trust-builder disk preflight. Evidence: `remote-ads-connector-status.md` records `/home/johannes` with 73G free and `/tmp` with 6.8G free.
- [x] `RTCONN-P2-GATE-002` Run targeted ADS connector status tests. Evidence: `remote-ads-connector-status.md` records `connectors_status.rs` with 11 tests passed and `connectors_status --lib` with 5 tests passed.
- [x] `RTCONN-P2-GATE-003` Run `cargo test -p trust-runtime --test ads_cli_command`. Evidence: `remote-ads-connector-status.md` records 9 tests passed on `trust-builder`.
- [x] `RTCONN-P2-GATE-004` Run `cargo test -p trust-runtime --test ads_web_api`. Evidence: `remote-ads-connector-status.md` records 6 tests passed on `trust-builder`.
- [x] `RTCONN-P2-GATE-005` Run the common per-phase gate on trust-builder. Evidence: `remote-ads-connector-status.md` records passing `cargo fmt --check --all`, `just fmt`, `just clippy`, and `just test` on the isolated validation copy.
- [x] `RTCONN-P2-GATE-006` Run `just test-all` on trust-builder. Evidence: `remote-ads-connector-status.md` records `just test-all` passing through `scripts/cargo_test_fast_link.sh test --all` on the isolated validation copy.
- [x] `RTCONN-P2-GATE-007` Run networking gates if ADS networking behavior changed. Evidence: not required for this slice; `remote-ads-connector-status.md` records that no ADS networking, worker, notification, transport, route-add, or config execution behavior changed.
- [x] `RTCONN-P2-GATE-008` Record command output summaries and artifact paths in the Phase 2 evidence directory. Evidence: `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-04-phase-02/remote-ads-connector-status.md`.

## Phase 3 - OPC UA Persistent Client Spike And Implementation

- [x] `RTCONN-P3-SPIKE-001` Before worker implementation, write a spike report proving whether the current `opcua` crate version supports client-side Subscriptions, MonitoredItems, keep-alive handling, and session renewal. Evidence: `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-04-phase-03/opcua-client-subscription-spike.md`.
- [x] `RTCONN-P3-SPIKE-002` If the current crate cannot support the target worker design, stop and create a separate dependency-upgrade/replacement slice before continuing this phase. Evidence: spike passed; no dependency-upgrade/replacement slice is required before the Phase 3 worker implementation.
- [x] `RTCONN-P3-SPIKE-003` Record spike code, commands, server fixture, and findings under the Phase 3 evidence directory. Evidence: `opcua-client-subscription-spike.md` records the compile/API proof, existing live fixture ownership, remote command, and result.
- [x] `RTCONN-P3-001` Replace per-poll connect/read/disconnect with a persistent OPC UA client worker only after the spike gate passes. Evidence: `opcua-persistent-worker-remote-proof.md` records `runtime/opcua_client_subsystem.rs` now using `OpcUaClientBridge` instead of scan-cycle read/write helpers and remote worker tests passed.
- [x] `RTCONN-P3-002` Maintain lifecycle states: disabled, configured, connecting, connected, reconnecting, stale, and faulted. Evidence: `OpcUaClientConnectionState` in `host/opcua/client.rs`, `opcua_client_status` mapping tests, and `persistent_worker_*` tests passed on `trust-builder`.
- [x] `RTCONN-P3-003` Add keep-alive handling and stale transition behavior. Evidence: `client_worker.rs` runs the persistent `Session::run` path and connection callbacks; `persistent_worker_marks_stale_then_recovers_on_subscription_update` passed on `trust-builder`.
- [x] `RTCONN-P3-004` Add session-timeout negotiation or documented rejection if the server cannot renegotiate. Evidence: `connected_detail_reports_timeout_negotiation_or_documented_gap` documents the `opcua` 0.12 revised-timeout visibility gap and passed on `trust-builder`; real transport records `revised_timeout_ms: None` and uses the existing configured stale timeout.
- [x] `RTCONN-P3-005` Add subscription re-establish and republish behavior after reconnect. Evidence: `OpcUaClientTransport::recover_after_disconnect` and `OpcUaWireClientTransport` call `Session::reconnect_and_activate`; `persistent_worker_uses_recovery_hook_to_reestablish_subscriptions` passed on `trust-builder`; `opcua-persistent-worker-remote-proof.md` documents that `opcua` 0.12 transfers subscriptions with `send_initial_values=true` or recreates them, while direct `RepublishRequest` issuance is unavailable because publish sequence state is private.
- [x] `RTCONN-P3-006` Add Subscriptions and MonitoredItems for read points. Evidence: `OpcUaWireClientTransport::subscribe_read_points` creates subscriptions and monitored items; spike API proof and `persistent_worker_applies_subscription_updates_without_reconnecting_per_scan` passed on `trust-builder`.
- [x] `RTCONN-P3-007` Keep polling fallback only for explicitly unsupported server/subscription cases. Evidence: no runtime call sites remain for `read_opcua_client_point_values` or `write_opcua_client_point_values`; the persistent worker path has no silent polling fallback.
- [x] `RTCONN-P3-008` Batch writes through the persistent session. Evidence: `publish_pending_writes` in `client_worker.rs` and `persistent_worker_batches_writes_without_reconnecting_per_write` passed on `trust-builder`.
- [x] `RTCONN-P3-009` Add a bounded latest-value handoff so the worker can never block the scan cycle. Evidence: `OpcUaClientEventSink` uses bounded `sync_channel(OPCUA_EVENT_CHANNEL_CAPACITY)` and `persistent_worker_applies_subscription_updates_without_reconnecting_per_scan` proves callbacks do not mutate runtime storage mid-scan.
- [x] `RTCONN-P3-010` Surface per-point freshness and server status through `PointQuality`. Evidence: `connectors/adapters/opcua.rs`, `opcua_client_status_projects_point_quality_and_metadata`, and `connectors_status_reports_opcua_client_points_with_quality` passed on `trust-builder`.
- [x] `RTCONN-P3-011` Add tests proving reads do not create a new session every poll. Evidence: `persistent_worker_applies_subscription_updates_without_reconnecting_per_scan` passed on `trust-builder`.
- [x] `RTCONN-P3-012` Add tests proving writes do not create a new session per write. Evidence: `persistent_worker_batches_writes_without_reconnecting_per_write` passed on `trust-builder`.
- [x] `RTCONN-P3-013` Add tests for subscription update delivery. Evidence: `persistent_worker_applies_subscription_updates_without_reconnecting_per_scan` passed on `trust-builder`.
- [x] `RTCONN-P3-014` Add tests for server restart, connection loss, stale transition, and recovery. Evidence: `persistent_worker_recreates_subscription_after_server_restart`, `persistent_worker_reconnects_after_session_loss_without_scan_thread_io`, and `persistent_worker_marks_stale_then_recovers_on_subscription_update` passed on `trust-builder`.
- [x] `RTCONN-P3-015` Keep `runtime.toml` and `io.toml` unchanged in this phase. Evidence: Phase 3 uses existing `runtime.opcua_client` and `opcua_client.toml` settings in `bundle_apply.rs`; no `runtime.toml`, `io.toml`, or config parser schema changes were added in this slice.

### Phase 3 Completion Gate

- [x] `RTCONN-P3-GATE-001` Run the trust-builder disk preflight. Evidence: `opcua-persistent-worker-remote-proof.md` records `/home/johannes` with 104G free and `/tmp` with 6.8G free after the isolated validation sync.
- [x] `RTCONN-P3-GATE-002` Run the OPC UA spike proof or record the blocked dependency decision. Evidence: `ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" cargo test -p trust-runtime --test opcua_client_runtime'` passed with 2 tests, including the new API-surface spike proof.
- [x] `RTCONN-P3-GATE-003` Run `cargo test -p trust-runtime --test opcua_integration`. Evidence: `opcua-persistent-worker-remote-proof.md` records 4 tests passed on `trust-builder`.
- [x] `RTCONN-P3-GATE-004` Run `cargo test -p trust-runtime --test opcua_client_runtime`. Evidence: `opcua-persistent-worker-remote-proof.md` records 2 tests passed on `trust-builder`.
- [x] `RTCONN-P3-GATE-005` Run targeted OPC UA connector status tests. Evidence: `opcua-persistent-worker-remote-proof.md` records `cargo test -p trust-runtime --test connectors_status` with 12 tests passed and `cargo test -p trust-runtime connectors_status --lib` with 6 tests passed.
- [x] `RTCONN-P3-GATE-006` Run the common per-phase gate on trust-builder. Evidence: `opcua-persistent-worker-remote-proof.md` records passing `just fmt`, `just clippy`, and `just test` on the isolated validation copy after the generated target cache cleanup.
- [x] `RTCONN-P3-GATE-007` Run networking gates if OPC UA networking behavior changed. Evidence: `opcua-persistent-worker-remote-proof.md` records passing `RUSTFLAGS=-Dwarnings cargo check -p trust-runtime --all-targets` and `./scripts/runtime_mesh_tls_stability_gate.sh --iterations 8`.
- [x] `RTCONN-P3-GATE-008` Record command output summaries and artifact paths in the Phase 3 evidence directory. Evidence: `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-04-phase-03/opcua-persistent-worker-remote-proof.md`.

## Phase 4 - Honesty Fixes

- [x] `RTCONN-P4-001` Update Modbus discovery so a random TCP listener on port 502 is not reported as confirmed Modbus. Evidence: `probe_modbus_tcp` in `crates/trust-runtime/src/control/comm_handlers/discovery_probe.rs`, `discover_tests::modbus_discovery_reports_tcp_listener_as_port_reachable_only`, and `honest-discovery-and-ads-not-ready.md`.
- [x] `RTCONN-P4-002` Preferred Modbus proof: FC43/14 device identification when supported. Evidence: `build_modbus_device_id_request`, `parse_modbus_probe_response`, and `modbus_device_id_probe_confirms_protocol_response` in `discovery_probe.rs`.
- [x] `RTCONN-P4-003` Fallback Modbus proof: configured safe read probe with explicit address, unit id, and confidence label. Evidence: `ModbusSafeReadProbe`, `build_modbus_read_holding_registers_request`, `modbus_safe_read_probe_confirms_when_device_id_is_unavailable`, and `DiscoverScope` `unit_id`/`probe_read_address`/`probe_read_quantity` validation in `discover.rs`.
- [x] `RTCONN-P4-004` If only TCP connect succeeds, report `DiscoveryConfidence::port_reachable`, not confirmed protocol. Evidence: `modbus_tcp_listener_observed.json` golden now records `confidence: "port_reachable"` with a TCP-only warning, and `phase0_discovery_matches_current_goldens` passed on `trust-builder`.
- [x] `RTCONN-P4-005` Update MQTT discovery to use CONNECT/CONNACK for confirmed MQTT. Evidence: `probe_mqtt`, `build_mqtt_connect_packet`, `parse_mqtt_connack`, and `mqtt_probe_uses_clean_session_and_disconnects_after_connack` in `discovery_probe.rs`.
- [x] `RTCONN-P4-006` MQTT discovery must use `clean_session=true` and immediate DISCONNECT so probes do not leave broker sessions behind. Evidence: `mqtt_probe_uses_clean_session_and_disconnects_after_connack` asserts the CONNECT flags and DISCONNECT packet.
- [x] `RTCONN-P4-007` Classify MQTT auth failures separately from non-MQTT endpoints. Evidence: `mqtt_probe_classifies_auth_rejected_connack_separately` and the `auth_required` candidate parameter path in `discover.rs`.
- [x] `RTCONN-P4-008` Preserve non-invasive scan mode, but make its confidence labels honest. Evidence: TCP-only Modbus/MQTT candidates remain returned as `port_reachable` with warnings instead of being dropped or reported as confirmed; see `discover.rs` and Phase 4 golden diff notes in `honest-discovery-and-ads-not-ready.md`.
- [x] `RTCONN-P4-009` Add regression tests where a random TCP listener on 502 is not reported as confirmed Modbus. Evidence: `modbus_discovery_reports_tcp_listener_as_port_reachable_only` passed on `trust-builder`.
- [x] `RTCONN-P4-010` Add regression tests where a random TCP listener on 1883 is not reported as confirmed MQTT. Evidence: `targeted_mqtt_discovery_reports_tcp_listener_as_port_reachable_only` passed on `trust-builder`.
- [x] `RTCONN-P4-011` Change ADS server cold start without snapshot from hard failure to `not_ready`. Evidence: `AdsServerSymbolSource::empty`, `start_ads_server` degraded startup path in `host/ads/server/lifecycle.rs`, and `AdsStatusOverall::NotReady`/`AdsConnectionStatusState::NotReady` mappings.
- [x] `RTCONN-P4-012` Add tests proving ADS server starts, reports `not_ready`, and transitions to ready when a snapshot appears. Evidence: `lifecycle_starts_not_ready_without_snapshot_and_refreshes_when_snapshot_appears` passed on `trust-builder`.
- [x] `RTCONN-P4-013` Update CLI/control help and docs to explain discovery confidence labels. Evidence: `crates/trust-runtime/src/bin/trust-runtime/cli/comm.rs`, `crates/trust-runtime/src/bin/trust-runtime/comm.rs`, `parse_comm_discover_command`, and `docs/public/connect/protocol-matrix.md`.

### Phase 4 Completion Gate

- [x] `RTCONN-P4-GATE-001` Run the trust-builder disk preflight. Evidence: `honest-discovery-and-ads-not-ready.md` records `/home/johannes` with 110G free and `/tmp` with 6.8G free.
- [x] `RTCONN-P4-GATE-002` Run targeted discovery confidence tests. Evidence: `cargo test -p trust-runtime discovery_probe --lib` passed with 4 tests; `cargo test -p trust-runtime comm_handlers::discover --lib` passed with 12 tests.
- [x] `RTCONN-P4-GATE-003` Run targeted ADS server cold-start tests. Evidence: `cargo test -p trust-runtime lifecycle_starts_not_ready --lib` passed with 1 test.
- [x] `RTCONN-P4-GATE-004` Run `cargo test -p trust-runtime --test modbus_driver`. Evidence: `honest-discovery-and-ads-not-ready.md` records 2 passed and 2 ignored on `trust-builder`.
- [x] `RTCONN-P4-GATE-005` Run targeted MQTT discovery/session tests. Evidence: `mqtt_probe_uses_clean_session_and_disconnects_after_connack`, `mqtt_probe_classifies_auth_rejected_connack_separately`, and `targeted_mqtt_discovery_reports_tcp_listener_as_port_reachable_only` passed inside the targeted discovery commands.
- [x] `RTCONN-P4-GATE-006` Run the communication-specific gate bundle. Evidence: `honest-discovery-and-ads-not-ready.md` records passing `opcua_integration`, `opcua_client_runtime`, `ads_cli_command`, `ads_web_api`, `modbus_driver`, `ethercat_driver`, `io_multidriver_live`, and `runtime_comms_conformance_gate.sh`.
- [x] `RTCONN-P4-GATE-007` Run the common per-phase gate on trust-builder. Evidence: `honest-discovery-and-ads-not-ready.md` records passing `just fmt`, `just clippy`, and `just test`.
- [x] `RTCONN-P4-GATE-008` Run `just test-all` on trust-builder. Evidence: `honest-discovery-and-ads-not-ready.md` records `just test-all` passing in 578 seconds.
- [x] `RTCONN-P4-GATE-009` Run networking gates. Evidence: `honest-discovery-and-ads-not-ready.md` records passing `./scripts/runtime_mesh_tls_stability_gate.sh --iterations 8` and `RUSTFLAGS=-Dwarnings cargo check -p trust-runtime --all-targets`.
- [x] `RTCONN-P4-GATE-010` Record command output summaries and artifact paths in the Phase 4 evidence directory. Evidence: `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-04/honest-discovery-and-ads-not-ready.md`.

## Phase 5 - HMI And VS Code Surface Propagation

This board owns cross-surface propagation for the new status and discovery confidence labels. Do not leave runtime truth and Network Canvas/HMI display in contradictory states.

- [x] `RTCONN-P5-001` Identify every runtime API consumed by HMI and VS Code Network Canvas for connector status and discovery labels. Evidence: `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-05/hmi-vscode-status-propagation.md` API inventory.
- [x] `RTCONN-P5-002` Add or update HMI rendering for `ConnectorState`, `ConnectorHealth`, `PointQuality`, and `DiscoveryConfidence`. Evidence: `crates/trust-runtime/src/web/ui/hmi.html`, `crates/trust-runtime/src/web/ui/chunks/hmi-js/hmi-01.js`, `crates/trust-runtime/src/web/ui/modules/hmi-transport.js`, and HMI Playwright screenshot `screenshots/hmi-connector-status-summary.png`.
- [x] `RTCONN-P5-003` Add or update VS Code Network Canvas rendering for `ConnectorState`, `ConnectorHealth`, `PointQuality`, and `DiscoveryConfidence`. Evidence: `editors/vscode/src/networkCanvas/connectorsStatus.ts`, endpoint hover rows in `editors/vscode/src/networkCanvas/webview/nodes.tsx`, discovery confidence labels in `DiscoverPane.tsx`, and screenshot `screenshots/network-canvas-connector-hover.png`.
- [x] `RTCONN-P5-004` Add VS Code extension tests under `editors/vscode/src/test/suite/**`. Evidence: `connector status surface flows into endpoint graph metadata` in `editors/vscode/src/test/suite/network-canvas.test.ts` passed in the remote Xvfb `npm test` run.
- [x] `RTCONN-P5-005` Register every new VS Code test in `editors/vscode/src/test/suite/index.ts`. Evidence: no new test file was added; the new test lives in existing registered `network-canvas.test.ts`, which is already required by `editors/vscode/src/test/suite/index.ts`.
- [x] `RTCONN-P5-006` Add HMI browser-visible tests or Playwright verification for changed HMI/web surfaces. Evidence: HMI Playwright proof and screenshot `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-05/screenshots/hmi-connector-status-summary.png`.
- [x] `RTCONN-P5-007` Capture browser or VS Code screenshot evidence for status/confidence labels when the rendered behavior changes. Evidence: HMI screenshot `hmi-connector-status-summary.png` and Network Canvas screenshot `network-canvas-connector-hover.png`.
- [x] `RTCONN-P5-008` If `editors/vscode` behavior changes, bump `editors/vscode/package.json` and matching root version fields in `editors/vscode/package-lock.json` per release hygiene rules. Evidence: workspace `Cargo.toml`, `editors/vscode/package.json`, and root `editors/vscode/package-lock.json` are synchronized at target release `0.24.28`; `CHANGELOG.md` records the HMI/VS Code status propagation.
- [x] `RTCONN-P5-009` If the extension intentionally remains on legacy JSON for a sub-slice, record a dated non-goal and follow-up issue before closing the phase. Evidence: not applicable because the extension now consumes `connectors.status`; see `connectorsStatus.ts` and `hmi-vscode-status-propagation.md`.

### Phase 5 Completion Gate

- [x] `RTCONN-P5-GATE-001` Run the trust-builder disk preflight. Evidence: `hmi-vscode-status-propagation.md` records `/home/johannes` with 111G free and `/tmp` with 6.8G free before targeted gates.
- [x] `RTCONN-P5-GATE-002` Run targeted HMI/runtime status rendering tests. Evidence: `cargo test -p trust-runtime --test hmi_readonly_integration hmi_connector_summary_renders_shared_status_contract` and `hmi_dashboard_routes_render_without_manual_layout` passed on `trust-builder`.
- [x] `RTCONN-P5-GATE-003` Run Playwright/browser verification for browser-visible changes and store screenshots in the Phase 5 evidence directory. Evidence: screenshots under `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-05/screenshots/`.
- [x] `RTCONN-P5-GATE-004` If VS Code files changed, run `ssh trust-builder 'cd "$HOME/projects/trust-platform/editors/vscode" && npm run lint && npm run compile && npm test'`. Evidence: isolated-copy equivalents passed: `npm run lint`, `npm run compile`, and `xvfb-run -a -s "-screen 0 1920x1080x24" npm test`; direct `npm test` without Xvfb failed only because Electron had no X server.
- [x] `RTCONN-P5-GATE-005` If runtime protocol/debug behavior affects VS Code flows, run `ssh trust-builder 'cd "$HOME/projects/trust-platform/editors/vscode" && ST_LSP_TEST_SERVER=$HOME/projects/trust-platform/target/debug/trust-lsp npm test'`. Evidence: `ST_LSP_TEST_SERVER="$HOME/.cache/codex-targets/trust-platform-rtconn-validation/debug/trust-lsp" xvfb-run -a -s "-screen 0 1920x1080x24" npm test` passed with 443 tests on `trust-builder`.
- [x] `RTCONN-P5-GATE-006` Run the common per-phase gate on trust-builder. Evidence: `just fmt`, `just clippy`, and `just test` passed on the isolated builder copy.
- [x] `RTCONN-P5-GATE-007` Record command output summaries and artifact paths in the Phase 5 evidence directory. Evidence: `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-05/hmi-vscode-status-propagation.md`.

## Phase 6 - Protocol Depth

- [x] `RTCONN-P6-001` Add Modbus FC01, FC02, FC03, FC05, FC06, FC15, and FC16 only behind explicit config. Evidence: `crates/trust-runtime/src/io/modbus.rs` adds optional `input_function`/`output_function`; `crates/trust-runtime/tests/modbus_driver.rs` proves explicit FC01, FC02, FC03, FC05, FC06, FC15, and FC16 behavior while preserving the default FC04/FC16 profile; `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-06/modbus-function-code-depth.md` records remote builder proof.
- [x] `RTCONN-P6-002` Add per-point Modbus type mapping, scaling, endian, and word-order settings. Evidence: `crates/trust-runtime/src/io/modbus/point_map.rs` adds optional `input_points`/`output_points` with `bool`, `u16`, `i16`, `u32`, `i32`, and `f32` point types, scaling, Modbus byte order, and Modbus word order; `crates/trust-runtime/tests/modbus_driver.rs` covers mapped scaled register/coil reads, mapped coil/register writes, and invalid point-map config; `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-06/modbus-function-code-depth.md` records remote builder proof.
- [x] `RTCONN-P6-003` Add typed MQTT payload mapping before broad Sparkplug B behavior. Evidence: `crates/trust-runtime/src/io/mqtt/point_map.rs` adds optional typed `input_points`/`output_points` with `bool`, `u16`, `i16`, `u32`, `i32`, and `f32` values, `text`/`json`/`binary_le`/`binary_be` payload formats, and linear scaling; `crates/trust-runtime/src/io/mqtt/tests.rs` covers raw payload compatibility, typed input decoding, typed output publishing, and invalid config; `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-06/mqtt-typed-payload-depth.md` records remote builder proof.
- [x] `RTCONN-P6-004` Add Sparkplug B only with namespace, version, broker interop, and compatibility proof. Evidence: `crates/trust-runtime/src/io/mqtt/sparkplug.rs` adds a bounded outbound node profile pinned to `spBv1.0` and Sparkplug `3.0.0`, with NBIRTH, NDEATH last will, and NDATA scalar metric protobuf encoding; `crates/trust-runtime/src/io/mqtt/tests.rs` covers topic shape, NDEATH LWT, deterministic scalar protobuf wire fields, NBIRTH then NDATA publish order, and unsupported-shape rejection; `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-06/mqtt-sparkplug-b-outbound-depth.md` records remote builder proof and the Eclipse Tahu source check.
- [x] `RTCONN-P6-005` Do not start EtherCAT DC/CoE unless the motion roadmap explicitly accepts it. Evidence: `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-06/ethercat-dc-coe-nonstart.md` records that `docs/guides/ETHERCAT_BACKEND_V1.md` and `docs/specs/11-runtime-engine.md` still list advanced motion profile support as out of scope, and no accepted Phase 6 motion roadmap requires DC/CoE now.
- [x] `RTCONN-P6-006` If EtherCAT DC/CoE is accepted, create a dedicated DC/CoE checklist with hardware gates before implementation. Evidence: DC/CoE was not accepted for this phase; `ethercat-dc-coe-nonstart.md` records the follow-up rule requiring a dedicated DC/CoE checklist before future implementation.
- [x] `RTCONN-P6-007` Update public and internal protocol capability docs after each protocol-depth change. Evidence: Modbus docs are covered by `modbus-function-code-depth.md`; typed MQTT docs are covered by `mqtt-typed-payload-depth.md`; Sparkplug docs are covered by `mqtt-sparkplug-b-outbound-depth.md`; EtherCAT DC/CoE non-start remains documented by `docs/guides/ETHERCAT_BACKEND_V1.md`, `docs/specs/11-runtime-engine.md`, and `ethercat-dc-coe-nonstart.md`.

### Phase 6 Completion Gate

- [x] `RTCONN-P6-GATE-001` Run the trust-builder disk preflight. Evidence: `modbus-function-code-depth.md`, `mqtt-typed-payload-depth.md`, and `mqtt-sparkplug-b-outbound-depth.md` record trust-builder disk preflights; the final Sparkplug run started with `/home/johannes` at 95G free and `/tmp` at 6.7G free.
- [x] `RTCONN-P6-GATE-002` Run targeted Modbus/MQTT/EtherCAT protocol-depth tests for the touched protocol. Evidence: Modbus targeted tests are in `modbus-function-code-depth.md`; MQTT typed and Sparkplug targeted tests are in `mqtt-typed-payload-depth.md` and `mqtt-sparkplug-b-outbound-depth.md`; EtherCAT DC/CoE was not touched.
- [x] `RTCONN-P6-GATE-003` Run `cargo test -p trust-runtime --test modbus_driver` if Modbus changed. Evidence: passed on `trust-builder` with 8 passed and 2 ignored after the Modbus point-map slice.
- [x] `RTCONN-P6-GATE-004` Run `cargo test -p trust-runtime --test ethercat_driver` if EtherCAT changed. Evidence: EtherCAT DC/CoE was explicitly not started; `ethercat-dc-coe-nonstart.md` records no EtherCAT code changes for Phase 6.
- [x] `RTCONN-P6-GATE-005` Run `cargo test -p trust-runtime --test io_multidriver_live` if process-image driver interaction changed. Evidence: passed on `trust-builder` with 2 tests after the Modbus point-map, MQTT typed, and Sparkplug slices.
- [x] `RTCONN-P6-GATE-006` Run the common per-phase gate on trust-builder. Evidence: `just fmt`, `just clippy`, and `just test` passed on the isolated validation copy; final Sparkplug run recorded `just test` fallback with 838 passed and 16 ignored.
- [x] `RTCONN-P6-GATE-007` Run networking gates for networking changes. Evidence: `runtime_mesh_tls_stability_gate.sh --iterations 8` and `RUSTFLAGS=-Dwarnings cargo check -p trust-runtime --all-targets` passed on `trust-builder`; final Sparkplug run recorded warning-deny runtime all-targets check passing in 19.34s.
- [x] `RTCONN-P6-GATE-008` Record command output summaries and artifact paths in the Phase 6 evidence directory. Evidence: `modbus-function-code-depth.md`, `mqtt-typed-payload-depth.md`, `mqtt-sparkplug-b-outbound-depth.md`, and `ethercat-dc-coe-nonstart.md`.

## Phase 7 - Device-In-The-Loop Gates

- [x] `RTCONN-P7-001` Add ignored/device-gated EtherCAT tests for real lab hardware. Evidence: `crates/trust-runtime/tests/device_in_the_loop.rs` adds `ethercat_lab_hardware_discovery_records_topology`, which requires `TRUST_DIT_ETHERCAT_ADAPTER`, writes `ethercat-discovery.json`, and records topology when hardware is configured; skip-mode proof is recorded in `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-07/device-in-loop-gates.md`.
- [x] `RTCONN-P7-002` Add ignored/device-gated ADS tests against a real TwinCAT or compatible target. Evidence: `ads_lab_twincat_doctor_records_status_json` wraps `trust-runtime ads doctor --json`, accepts documented read-only/prod-ready policies, writes `ads-doctor.json`, and records missing `TRUST_DIT_ADS_TARGET` on builder skip.
- [x] `RTCONN-P7-003` Add ignored/device-gated Modbus tests against real hardware or a certified simulator target. Evidence: `modbus_lab_target_confirms_protocol_probe` runs `trust-runtime comm discover --protocol modbus-tcp --json`, requires confirmed protocol evidence instead of TCP-only reachability, writes `modbus-discovery.json`, and records missing `TRUST_DIT_MODBUS_HOST` on builder skip.
- [x] `RTCONN-P7-004` Add optional MQTT broker interop tests with auth, TLS, reconnect, clean session, and disconnect evidence. Evidence: `mqtt_lab_broker_records_auth_tls_reconnect_and_disconnect` uses `rumqttc` with clean sessions, optional username/password and TLS/mTLS config, repeated connect/publish cycles, explicit DISCONNECT evidence, and writes `mqtt-interop.json`; builder skip records missing `TRUST_DIT_MQTT_BROKER`.
- [x] `RTCONN-P7-005` Wire nightly/manual workflows so missing lab hardware skips clearly and does not silently pass. Evidence: `.github/workflows/protocol-device-in-loop.yml` runs on a schedule and by manual dispatch, uploads artifacts, supports lab runner variable `TRUST_DIT_RUNNER`, and uses `require_hardware`/`TRUST_DIT_REQUIRE_HARDWARE` to convert missing prerequisites from explicit skip to failure.
- [x] `RTCONN-P7-006` Store status JSON, logs, topology/discovery output, hardware identifiers, and failure evidence as artifacts. Evidence: `scripts/runtime_device_in_loop_gate.sh` sets `TRUST_DIT_ARTIFACT_DIR` and uploads JSON artifacts through the workflow; remote artifacts under `gate-artifacts/phase7-device-in-loop/` show explicit skip JSON for every unconfigured lab protocol, and configured runs write topology/discovery/doctor/interop payloads.
- [x] `RTCONN-P7-007` Add lab hardware prerequisites and skip reasons to public or internal docs as appropriate. Evidence: `docs/internal/testing/runtime-device-in-the-loop.md` documents every environment variable, skip/fail behavior, artifact file, and runner selection.

### Phase 7 Completion Gate

- [x] `RTCONN-P7-GATE-001` Run the trust-builder disk preflight. Evidence: `device-in-loop-gates.md` records `/home/johannes` with 111G free and `/tmp` with 6.7G free before remote gates.
- [x] `RTCONN-P7-GATE-002` Run the device-gated tests on available lab hardware, or record explicit skip evidence naming the missing device. Evidence: `scripts/runtime_device_in_loop_gate.sh` passed on `trust-builder` in skip mode and wrote JSON artifacts naming missing `TRUST_DIT_ETHERCAT_ADAPTER`, `TRUST_DIT_ADS_TARGET`, `TRUST_DIT_MODBUS_HOST`, and `TRUST_DIT_MQTT_BROKER`.
- [x] `RTCONN-P7-GATE-003` Run the communication-specific gate bundle. Evidence: `device-in-loop-gates.md` records passing `opcua_integration`, `opcua_client_runtime`, `ads_cli_command`, `ads_web_api`, `modbus_driver`, `ethercat_driver`, `io_multidriver_live`, and `runtime_comms_conformance_gate.sh`.
- [x] `RTCONN-P7-GATE-004` Run the common per-phase gate on trust-builder. Evidence: `just fmt`, `just clippy`, and `just test` passed on `trust-builder`; `just test` used the repo fallback with 838 passed and 16 ignored.
- [x] `RTCONN-P7-GATE-005` Run networking gates. Evidence: `runtime_mesh_tls_stability_gate.sh --iterations 8` passed all 8 runs and `RUSTFLAGS=-Dwarnings cargo check -p trust-runtime --all-targets` passed on `trust-builder`.
- [x] `RTCONN-P7-GATE-006` Record command output summaries and artifact paths in the Phase 7 evidence directory. Evidence: `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-07/device-in-loop-gates.md`.

## Phase 8 - Conformance Suite Growth

- [x] `RTCONN-P8-001` Keep the current run-twice determinism diff. Evidence: `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-08/conformance-suite-v2.md` records local and remote normalized run-twice diffs passing for the 21-case v2 suite.
- [x] `RTCONN-P8-002` Before adding categories, update `conformance/contract.md` to unfreeze or version the category contract. Evidence: `conformance/contract.md` now preserves v1 and adds the versioned v2 profile; remote schema/diff proof is in `conformance-suite-v2.md`.
- [x] `RTCONN-P8-003` Update `conformance/naming.md` for new category and case naming. Evidence: `conformance/naming.md` documents v2 categories, naming rules, and reports policy; new v2 case ids passed schema validation on `trust-builder`.
- [x] `RTCONN-P8-004` Add a new summary schema version under `conformance/schemas/` instead of silently mutating `summary-v1.schema.json`. Evidence: `conformance/schemas/summary-v2.schema.json` added; `scripts/validate_conformance_summary_schema.py` validated v1 and v2 schemas plus remote pass summaries.
- [x] `RTCONN-P8-005` Update `conformance/external-run-guide.md` for new category expectations. Evidence: `conformance/external-run-guide.md` documents v1/v2 schema use and validator commands.
- [x] `RTCONN-P8-006` Update `conformance/submissions.md` for submitted result compatibility. Evidence: `conformance/submissions.md` documents profile metadata and v1/v2 submission compatibility.
- [x] `RTCONN-P8-007` Add conformance categories for strings, arrays, structs, enums, and nested values. Evidence: new case and expected directories under `conformance/cases/{strings,arrays,structs,enums,nested_values}/` and `conformance/expected/**`; remote v2 suite passed 21/21.
- [x] `RTCONN-P8-008` Add OOP dispatch cases: methods, interfaces, inheritance, and polymorphism. Evidence: `conformance/cases/oop_dispatch/cfm_oop_dispatch_interface_super_001/` and expected output added; remote v2 suite passed 21/21.
- [x] `RTCONN-P8-009` Add `REF_TO` and pointer/reference edge cases. Evidence: `conformance/cases/references/cfm_references_ref_to_deref_write_001/` and expected output added; remote v2 suite passed 21/21.
- [x] `RTCONN-P8-010` Add retain/init/reset matrix cases for cold, warm, hot, fault, and download paths. Evidence: `conformance/cases/retain_matrix/cfm_retain_matrix_restart_aliases_001/`, `series_values.rs` restart-mode aliases, and expected output added; remote v2 suite passed 21/21.
- [x] `RTCONN-P8-011` Add scheduler and multitask determinism cases. Evidence: `conformance/cases/scheduler/cfm_scheduler_task_interval_001/` and expected output added; remote v2 suite passed 21/21.
- [x] `RTCONN-P8-012` Define comms determinism as simulated/loopback scripted status transitions, not live sockets. Evidence: `conformance/README.md`, `conformance/known-gaps.md`, and `conformance-suite-v2.md` define the simulated-only policy.
- [x] `RTCONN-P8-013` Add comms determinism cases using simulated/loopback connectors and the shared connector status model. Evidence: `conformance/cases/comms_determinism/cfm_comms_determinism_connector_projection_001/` plus `CaseKind::ConnectorStatusTrace` in `models.rs` and `execute_connector_status_trace_case` in `execution.rs`; remote v2 suite passed 21/21.
- [x] `RTCONN-P8-014` Generate result reports as CI artifacts; do not commit generated files under `conformance/reports/`. Evidence: `.github/workflows/ci.yml` renders `gate-artifacts/conformance-pass-1.md`, uploads JSON/Markdown artifacts, and no generated `conformance/reports/` files were committed.
- [x] `RTCONN-P8-015` Publish a public docs summary page only after CI artifact generation is stable. Evidence: CI artifact generation passed on `trust-builder`; `docs/public/reference/conformance.md` now documents the v2 public proof surface.
- [x] `RTCONN-P8-016` Add new public docs pages to the relevant section `index.md`, not only to mkdocs navigation. Evidence: no new page was needed because `docs/public/reference/conformance.md` already exists in `docs/public/reference/index.md` and `mkdocs.yml`; Phase 8 updated that page and `python3 scripts/check_public_docs_ia.py` passed.
- [x] `RTCONN-P8-017` Upload human-readable and machine-readable conformance reports from CI. Evidence: `.github/workflows/ci.yml` upload-artifact includes `gate-artifacts/conformance-pass-*.json` and `gate-artifacts/conformance-pass-*.md`; remote proof is in `conformance-suite-v2.md`.

### Phase 8 Completion Gate

- [x] `RTCONN-P8-GATE-001` Run the trust-builder disk preflight. Evidence: `conformance-suite-v2.md` records preflight `/home/johannes` with 112G free and `/tmp` with 6.7G free before remote Phase 8 gates.
- [x] `RTCONN-P8-GATE-002` Run the conformance suite twice and verify deterministic summary diff. Evidence: `conformance-suite-v2.md` records remote pass 1 and pass 2 at v2, 21 passed, and a clean normalized diff.
- [x] `RTCONN-P8-GATE-003` Run schema validation for every summary schema version. Evidence: `conformance-suite-v2.md` records `scripts/validate_conformance_summary_schema.py` validating v1 schema, v2 schema, and both remote summaries.
- [x] `RTCONN-P8-GATE-004` Run public docs link/index checks for added conformance pages. Evidence: `conformance-suite-v2.md` records passing `python3 scripts/check_public_docs_links.py` and `python3 scripts/check_public_docs_ia.py`.
- [x] `RTCONN-P8-GATE-005` Run the common per-phase gate on trust-builder. Evidence: `conformance-suite-v2.md` records passing `just fmt`, `just clippy`, and `just test` on the isolated validation copy.
- [x] `RTCONN-P8-GATE-006` Run `just test-all` on trust-builder. Evidence: `conformance-suite-v2.md` records `just test-all` passing through `./scripts/cargo_test_fast_link.sh test --all` on the isolated validation copy.
- [x] `RTCONN-P8-GATE-007` Record command output summaries and artifact paths in the Phase 8 evidence directory. Evidence: `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-08/conformance-suite-v2.md`.

## Phase 9 - PLCopen Import And Root Scratch Cleanup

- [x] `RTCONN-P9-001` Remove or guard the `extract_text_content` fallback in `crates/trust-plcopen/src/plcopen/xml_common.rs` so non-ST XML cannot synthesize an ST body. Evidence: `extract_single_st_body` now returns `None` when no supported `ST`/`text`/`xhtml` payload exists, and `import_rejects_non_st_bodies_with_named_diagnostics` passed locally and on `trust-builder`.
- [x] `RTCONN-P9-002` Add a regression fixture with an FBD body that currently would scrape text and must now reject. Evidence: `crates/trust-plcopen/tests/fixtures/plcopen/non_st_bodies.xml` includes FBD, LD, SFC, and unknown body payloads; `trust-plcopen` passed with the new regression.
- [x] `RTCONN-P9-003` Add named diagnostics for LD, FBD, SFC, and unknown non-ST body imports. Evidence: `PLCO215`, `PLCO216`, `PLCO217`, and `PLCO218` are emitted by `collect_non_st_body_diagnostics`; `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-09/plcopen-import-and-root-scratch-cleanup.md` records local/remote proof.
- [x] `RTCONN-P9-004` Create real CODESYS and TwinCAT export fixtures from scratch demo projects with IP-clean provenance. Remains accepted-deferred by explicit 2026-07-05 instruction: `plcopen-import-and-root-scratch-cleanup.md` and `2026-07-05-final/final-closeout.md` record that no real export XMLs or vendor export tools are available locally or on `trust-builder`; do not fabricate these fixtures. Follow-up requires user-provided or locally generated IP-clean CODESYS/TwinCAT exports from real vendor tools.
- [x] `RTCONN-P9-005` Record fixture provenance in the fixture directory or a companion README. Evidence: `crates/trust-runtime/tests/fixtures/plcopen/README.md` records synthetic fixture provenance and the real-export corpus requirement.
- [x] `RTCONN-P9-006` Rename existing tiny vendor-labeled fixtures as synthetic if they are not real vendor exports. Evidence: `synthetic-codesys.xml`, `synthetic-twincat.xml`, `synthetic-siemens.xml`, `synthetic-rockwell.xml`, `synthetic-schneider.xml`, and `synthetic-openplc.xml` replace the old vendor-labeled filenames; runtime PLCopen tests pass with updated references.
- [x] `RTCONN-P9-007` Before deleting tracked root `src/main.st` and `src/config.st`, grep walkthroughs, first-run flows, docs, tests, and scripts for references. Evidence: `plcopen-import-and-root-scratch-cleanup.md` records the grep command and conclusion that remaining `src/main.st`/`src/config.st` references are template/example/generic project-layout references, not repo-root dependencies.
- [x] `RTCONN-P9-008` If root `src/` is a real example, move it under `examples/` and document it. Evidence: not applicable; `plcopen-import-and-root-scratch-cleanup.md` records the files as tracked repo-root scratch coupled to ignored local runtime artifacts, not a standalone example.
- [x] `RTCONN-P9-009` If root `src/` is scratch, delete the tracked files and update any stale references. Evidence: `src/main.st` and `src/config.st` deleted; `docs/diagrams/architecture/full-software-map-generated.puml` now shows tracked root PLC sources as `none`; diagram render/drift passed on `trust-builder`.
- [x] `RTCONN-P9-010` Do not treat ignored root `program.stbc` as a tracked repo hygiene problem. Evidence: `git ls-files src/main.st src/config.st program.stbc .gitignore` lists `.gitignore` plus the deleted tracked ST files only; `.gitignore` already ignores `/program.stbc`.

### Phase 9 Completion Gate

- [x] `RTCONN-P9-GATE-001` Run the trust-builder disk preflight. Evidence: `plcopen-import-and-root-scratch-cleanup.md` records `/home/johannes` with 112G free and `/tmp` with 6.7G free before focused gates.
- [x] `RTCONN-P9-GATE-002` Run targeted PLCopen import tests. Evidence: local and remote `cargo test -p trust-plcopen` passed with 14 tests, including the non-ST rejection regression.
- [x] `RTCONN-P9-GATE-003` Run `cargo test -p trust-plcopen`. Evidence: passed locally and on `trust-builder` with 14 tests.
- [x] `RTCONN-P9-GATE-004` Run runtime PLCopen migration tests touched by fixture changes. Evidence: `plcopen_command`, `plcopen_migration`, `plcopen_st_complete_parity`, and `plcopen_codesys_import_runtime` passed on `trust-builder`.
- [x] `RTCONN-P9-GATE-005` Run the common per-phase gate on trust-builder. Evidence: `plcopen-import-and-root-scratch-cleanup.md` records passing `scripts/render_diagrams.sh`, `python scripts/check_diagram_drift.py`, `just fmt`, `just clippy`, and `just test`.
- [x] `RTCONN-P9-GATE-006` Record command output summaries and artifact paths in the Phase 9 evidence directory. Evidence: `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-09/plcopen-import-and-root-scratch-cleanup.md`.

## Phase 10 - Trust-Dev Alias Removal Window

- [x] `RTCONN-P10-001` Record the current compatibility policy for `trust-runtime test`, `trust-runtime docs`, `trust-runtime commit`, and `trust-runtime agent`. Evidence: `trust-dev-alias-removal-window.md`, `docs/public/reference/cli/trust-runtime.md`, `docs/public/reference/cli/trust-dev.md`, and `xtask/config/full_map_policy.json` record `trust-dev` as canonical while `trust-runtime` remains a deprecated forwarding alias.
- [x] `RTCONN-P10-002` Set a dated deprecation removal window. Evidence: `WORKBENCH_ALIAS_REMOVAL_NOT_BEFORE = "2026-10-05"` in `dev_forward.rs`, `removal_not_before: "2026-10-05"` in `xtask/config/full_map_policy.json`, and Phase 10 local/remote tests passed.
- [x] `RTCONN-P10-003` Update runtime help text to make `trust-dev` the canonical workbench path. Evidence: `cli/commands.rs` and `cli/agent.rs` help text now names canonical `trust-dev` commands; `runtime_help_names_trust_dev_workbench_commands_and_removal_window` passed locally and on `trust-builder`.
- [x] `RTCONN-P10-004` Update docs and examples to use `trust-dev` for workbench commands. Evidence: public CLI docs, version history, runtime CLI spec, public-docs system spec, OSCAT templates, ST test benchmark, and harness docs updated; `trust-dev-alias-removal-window.md` lists the touched docs.
- [x] `RTCONN-P10-005` Add tests for the current forwarding warning until removal. Evidence: `commit_command`, `docs_command`, `st_test_cli_command`, and `agent_command` alias tests now assert the canonical `trust-dev` command, `removed no earlier than 2026-10-05`, and `separate behavior-change release`; local and remote focused gates passed.
- [x] `RTCONN-P10-006` Add or update architecture-doctor policy for the removal window. Evidence: `xtask/config/full_map_policy.json` records `removal_not_before` and `removal_pr_required`; `xtask/src/full_map.rs` rejects missing removal-window metadata; `cargo test -p xtask full_map -- --nocapture` passed locally and on `trust-builder` with 64 tests.
- [x] `RTCONN-P10-007` When the window expires, remove runtime aliases in a separate behavior-change PR with explicit release notes. Evidence: aliases are intentionally retained in this phase because the window has not expired; `xtask/config/full_map_policy.json` sets `removal_pr_required: true`, warning text says removal must be a separate behavior-change release, and `trust-dev-alias-removal-window.md` records the future-removal constraint.

### Phase 10 Completion Gate

- [x] `RTCONN-P10-GATE-001` Run the trust-builder disk preflight. Evidence: `trust-dev-alias-removal-window.md` records `/home/johannes` with 112G free and `/tmp` with 6.7G free.
- [x] `RTCONN-P10-GATE-002` Run targeted CLI alias/deprecation tests. Evidence: remote focused gate records runtime help, `commit_command`, `docs_command`, `st_test_cli_command`, and `agent_command` alias tests passing after building the sibling `trust-dev` binary into the custom target cache.
- [x] `RTCONN-P10-GATE-003` Run `cargo test -p trust-runtime --test commit_command --test docs_command --test st_test_cli_command`. Evidence: `trust-dev-alias-removal-window.md` records the remote command passing 1 alias test in each integration suite.
- [x] `RTCONN-P10-GATE-004` Run relevant agent command tests if `agent` alias behavior changed. Evidence: `cargo test -p trust-runtime --test agent_command trust_runtime_agent_alias_forwards_to_trust_dev -- --nocapture` passed on `trust-builder`.
- [x] `RTCONN-P10-GATE-005` Run the common per-phase gate on trust-builder. Evidence: `trust-dev-alias-removal-window.md` records passing `just fmt`, `just clippy`, and `just test`; `just test` fell back to `cargo test -p trust-runtime --lib` and passed 838 tests with 16 ignored.
- [x] `RTCONN-P10-GATE-006` Record command output summaries and artifact paths in the Phase 10 evidence directory. Evidence: `docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-05-phase-10/trust-dev-alias-removal-window.md`.

## Final Closeout

- [x] `RTCONN-FINAL-001` Update `CHANGELOG.md` for user-visible runtime, CLI, status, discovery, HMI, VS Code, PLCopen, or conformance behavior changes. Evidence: `CHANGELOG.md` target release `v0.24.28` records connector status, HMI/VS Code propagation, discovery/status honesty, Modbus/MQTT depth, protocol DIT gates, conformance v2, PLCopen non-ST rejection, and `trust-dev` alias-window behavior; `2026-07-05-final/final-closeout.md` records the release-hygiene summary.
- [x] `RTCONN-FINAL-002` Bump workspace version for release-notable behavior changes unless explicitly told not to. Evidence: `[workspace.package].version` in `Cargo.toml` is `0.24.28`.
- [x] `RTCONN-FINAL-003` If workspace version changes, sync `editors/vscode/package.json` and matching root fields in `editors/vscode/package-lock.json`. Evidence: `editors/vscode/package.json` and root package entries in `editors/vscode/package-lock.json` are both `0.24.28`.
- [x] `RTCONN-FINAL-004` Update PlantUML sources under `docs/diagrams/**/*.puml` for connector ownership/status flow changes. Evidence: `runtime-execution.puml` and `full-software-map-generated.puml` updated; generated SVGs and `docs/diagrams/manifest.json` refreshed.
- [x] `RTCONN-FINAL-005` Run `scripts/render_diagrams.sh` on trust-builder if diagrams changed. Evidence: Phase 9 evidence `plcopen-import-and-root-scratch-cleanup.md` records passing remote diagram rendering after the latest diagram changes.
- [x] `RTCONN-FINAL-006` Run `python scripts/check_diagram_drift.py` on trust-builder if diagrams changed. Evidence: Phase 9 evidence `plcopen-import-and-root-scratch-cleanup.md` records passing remote diagram drift check after the latest diagram changes.
- [x] `RTCONN-FINAL-007` Run runtime vertical tests: `api_smoke`, `debug_control`, `complete_program`, and `runtime_reliability`. Evidence: `2026-07-05-final/final-closeout.md` records passing remote `cargo test -p trust-runtime --test api_smoke --test debug_control --test complete_program --test runtime_reliability`.
- [x] `RTCONN-FINAL-008` Run the communication-specific gate bundle. Evidence: `2026-07-05-final/final-closeout.md` records passing remote `opcua_integration`, `opcua_client_runtime`, `ads_cli_command`, `ads_web_api`, `modbus_driver`, `ethercat_driver`, `io_multidriver_live`, and `runtime_comms_conformance_gate.sh`.
- [x] `RTCONN-FINAL-009` Run networking gates. Evidence: `2026-07-05-final/final-closeout.md` records passing `runtime_mesh_tls_stability_gate.sh --iterations 8` and `RUSTFLAGS=-Dwarnings cargo check -p trust-runtime --all-targets` on `trust-builder`.
- [x] `RTCONN-FINAL-010` Run VS Code lint, compile, and tests if extension files changed. Evidence: Phase 5 evidence `hmi-vscode-status-propagation.md` records passing `npm run lint`, `npm run compile`, and Xvfb-backed `npm test` on `trust-builder`.
- [x] `RTCONN-FINAL-011` Run Playwright/browser verification for browser-visible changes and store screenshot evidence. Evidence: Phase 5 evidence stores HMI and Network Canvas screenshots plus JSON/DOM assertions under `2026-07-05-phase-05/screenshots/`; `2026-07-05-final/final-closeout.md` lists the screenshot paths.
- [x] `RTCONN-FINAL-012` Run the trust-builder disk preflight. Evidence: `2026-07-05-final/final-closeout.md` records final-gate preflight with `/home/johannes` at 111G free and `/tmp` at 6.7G free, plus post-cleanup disk at 98G/6.7G.
- [x] `RTCONN-FINAL-013` Run `just fmt` on trust-builder. Evidence: `2026-07-05-final/final-closeout.md` records passing `just fmt` on `trust-builder`.
- [x] `RTCONN-FINAL-014` Run `just clippy` on trust-builder. Evidence: `2026-07-05-final/final-closeout.md` records passing `just clippy` on `trust-builder`.
- [x] `RTCONN-FINAL-015` Run `just test-all` on trust-builder. Evidence: `2026-07-05-final/final-closeout.md` records passing `just test-all` through `./scripts/cargo_test_fast_link.sh test --all` on `trust-builder`.
- [ ] `RTCONN-FINAL-016` If version was bumped, complete tag/release flow and verify GitHub Latest release reflects the new tag. Reopened: release/tag proof must wait until this board is accepted for release from `main`; do not tag from the dirty `ads/client` worktree. `2026-07-05-final/final-closeout.md` records the release proof boundary.
