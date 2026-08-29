# Communication SOLID and Specification Ownership Audit

Status: In progress for the MQTT issue #111 / PR #115 release candidate.

Purpose: verify every runtime communication surface against explicit
single-responsibility, dependency-inversion, and specification-ownership
boundaries before the MQTT candidate is frozen. This audit does not treat a
shared status vocabulary or a shared process-image port as protocol mixing.
It does treat protocol-specific wire, configuration, security, discovery, and
interoperability rules in the runtime-engine specification as mixed ownership.

## Acceptance rules

- [x] `COMMSOLID-RULE-001` Protocol transport loops remain protocol-owned.
  Shared code may depend on narrow ports and normalized status types, but may
  not implement broker, fieldbus, or vendor wire behavior.
- [x] `COMMSOLID-RULE-002` The PLC executor and scheduler own scan execution,
  not MQTT, Modbus, OPC UA, ADS, EtherCAT, GPIO, mesh, or cloud transport work.
- [x] `COMMSOLID-RULE-003` The shared process-image interface owns bounded
  PLC-facing values and bindings. Protocol workers do not resolve program
  symbols or access program storage.
- [x] `COMMSOLID-RULE-004` `docs/specs/11-runtime-engine.md` may specify the
  runtime integration port and bounded scan interaction, but not detailed
  protocol configuration, wire, security, mapping, discovery, or
  interoperability semantics.
- [ ] `COMMSOLID-RULE-005` Each communication protocol has one authoritative
  product specification. Shared connector status remains authoritative in
  `docs/specs/23-connector-status.md`; protocol documents reference it rather
  than copying its vocabulary.
- [ ] `COMMSOLID-RULE-006` Every behavior-changing correction has a written
  protocol requirement and a closest-boundary native executable test. A real
  implementation is required where the contract claims interoperability.
- [x] `COMMSOLID-RULE-007` This audit distinguishes release-blocking behavior
  defects from pre-existing structural debt. It does not hide a second protocol
  implementation refactor inside the MQTT bug fix.

## Source-derived ownership inventory

| Surface | Production owner | Shared dependency | Current specification owner | Audit result |
|---|---|---|---|---|
| MQTT broker I/O | `io/mqtt/**`; startup-only symbolic lowering in `io/mqtt_tag_mapping.rs` | `IoDriver`, process image, connector-status adapter | Detailed rules are mixed into `11-runtime-engine.md` | Protocol code is separated; specification ownership must be split before candidate freeze. |
| Modbus TCP I/O | `io/modbus.rs`, `io/modbus/**` | `IoDriver`, process image, connector-status adapter | Detailed rules are mixed into `11-runtime-engine.md` | Protocol worker/point map are separated; specification ownership is pre-existing debt. |
| EtherCAT I/O | `io/ethercat.rs`, `io/ethercat/**` | `IoDriver`, process image, connector-status adapter | Detailed rules are mixed into `11-runtime-engine.md` and a guide | Bus/config/driver seams are separated; authoritative product-spec ownership is missing. |
| GPIO I/O | `io/gpio.rs` | `IoDriver`, process image | Detailed rules are mixed into `11-runtime-engine.md` and a guide | Local I/O is not a network protocol, but it still needs an I/O-adapter specification owner. File is 990 lines and must not grow without a split. |
| ADS client/server | `host/ads/**`; control adapters in `control/ads_handlers/**` | runtime subsystem and control ports; connector-status adapter | Extensive ADS rules are mixed into `11-runtime-engine.md` | Transport, onboarding, server, diagnostics, and control adapters are separated; specification ownership must be split. |
| OPC UA client/server | `host/opcua/**`; server/control integration outside that module | runtime subsystem and control ports; connector-status adapter | Detailed rules are mixed into `11-runtime-engine.md` | Client transport/cache/worker seams are separated; specification ownership must be split. |
| Connector status | `connectors/contract.rs`, `mapping.rs`, `report.rs`, tiny adapters | protocol status snapshots only | `23-connector-status.md` | Correct shared abstraction. It does not own protocol transport or configuration. |
| Communication authoring | `control/comm_handlers/{schema,apply,discover,probe,browse_symbols}.rs` | protocol-specific service functions | Detailed rules are mixed into `11-runtime-engine.md` | Command responsibilities are split, but protocol registration is repeated switchboard logic. This is OCP/KISS debt, not the cause of issue #111. |
| Discovery | `control/comm_handlers/discover.rs` and `discovery_probe.rs`; ADS discovery remains ADS-owned | normalized discovery response | Mixed into `11-runtime-engine.md` | Shared orchestration is valid; protocol probes must remain adapters. The near-1k orchestrator needs a later registry split before adding another protocol. |
| Realtime T0 and mesh | `host/realtime/**`, `host/mesh/**` | runtime boundary and connector/status surfaces | Detailed rules are mixed into `11-runtime-engine.md` | Runtime integration is separated, but transport specifications need dedicated owners. |
| Runtime Cloud | `runtime_cloud/**` and runtime subsystem adapter | control/runtime ports | Detailed rules are mixed into `11-runtime-engine.md` | Domain modules are separated; product protocol specification ownership needs a dedicated document. |
| OpenOT | OpenOT runtime and producer modules | runtime ports | `27-openot-authoring.md` plus integration text | Dedicated authoring specification already exists; retain only runtime integration summaries in `11-runtime-engine.md`. |

## Release-candidate findings

- [x] `COMMSOLID-FIND-001` Issue #111 is not a scheduler ownership failure.
  The PLC scan continues while the MQTT worker owns broker activity. The defect
  was configuration/tag-mapping support missing from the released runtime.
- [x] `COMMSOLID-FIND-002` PR #115 keeps symbolic tag resolution outside the
  MQTT worker. Startup lowers a fully qualified PLC scalar through the shared
  process-image boundary before constructing the worker.
- [x] `COMMSOLID-FIND-003` PR #115 does not add MQTT knowledge to the scheduler,
  executor, connector-status contract, or control serialization layer.
- [x] `COMMSOLID-FIND-004` Automated review follow-ups remain in their narrow
  owners: mapped/raw mode selection in MQTT configuration, enum identity in
  the process-image codec/snapshot boundary, capture timing in its test
  harness, and cleanup classification in release tooling.
- [x] `COMMSOLID-FIND-005` Move the MQTT protocol contract from
  `11-runtime-engine.md` into a dedicated MQTT specification. Retain only tag
  lowering, process-image, bounded scan handoff, and fault projection in the
  runtime-engine specification.
- [ ] `COMMSOLID-FIND-006` Establish dedicated authoritative specifications for
  the remaining communication protocols, then replace detailed protocol prose
  in `11-runtime-engine.md` with integration references. Do not merely copy the
  prose and leave two authorities.
- [ ] `COMMSOLID-FIND-007` Add a specification ownership/drift test that rejects
  protocol-detail headings in the runtime-engine document and requires every
  registered communication surface to name an authoritative specification.
- [x] `COMMSOLID-FIND-008` Enforce the release-relevant first owner route: an
  MQTT implementation change paired only with the generic runtime-engine spec
  fails the direct-change validator. Evidence: focused expected-red/green
  governance test and `docs/specs/30-verification-inventory.md`.

## Code debt disposition

- [ ] `COMMSOLID-DEBT-001` Refactor repeated protocol registration in
  `comm_handlers` into protocol descriptors before adding another protocol.
  Preserve the existing schema, apply, discover, probe, and capability JSON
  with native behavior-lock tests first. This is a separate architecture slice,
  not a prerequisite to proving the MQTT mapping fix.
- [ ] `COMMSOLID-DEBT-002` Split `io/gpio.rs` before it exceeds 1,000 lines.
  Keep configuration, discovery, driver behavior, and tests in separate modules.
- [ ] `COMMSOLID-DEBT-003` Continue splitting protocol test aggregators where a
  single test file obscures behavior ownership. `io/mqtt/tests.rs` is currently
  1,467 lines even after dedicated tag-mapping, safe-state, non-finite, and
  point-map cases were extracted.

## Required proof before candidate freeze

- [ ] `COMMSOLID-GATE-001` Run the closest native MQTT mapping and enum tests.
- [ ] `COMMSOLID-GATE-002` Run the shipped traffic-light project with the real
  runtime, Mosquitto, and `mosquitto_sub`.
- [ ] `COMMSOLID-GATE-003` Run the runtime vertical suite and bounded MQTT worker
  tests.
- [ ] `COMMSOLID-GATE-004` Run the full-map architecture doctor and confirm the
  source-derived communication ownership facts match the diagrams.
- [ ] `COMMSOLID-GATE-005` Render diagrams and pass diagram drift checks.
- [ ] `COMMSOLID-GATE-006` Run exact-SHA GitHub-job parity on `trust-builder`
  before the guarded push.
