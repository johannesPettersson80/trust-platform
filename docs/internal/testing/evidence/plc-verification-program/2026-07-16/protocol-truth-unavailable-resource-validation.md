# Protocol truth and unavailable-resource batch validation

Date: 2026-07-16

Product, proof, and metadata checkpoint:
`8cc3199e155b54bfb8f167091ea90eb3483da947`

Report source checkpoint:
`423e7407ad7e1ca3985c872b033fe42d786f6a82`

Report binding checkpoint:
`8cc3199e155b54bfb8f167091ea90eb3483da947`

## Outcome

This batch closed four written specification gaps and promoted six invariants
from S0 to G1 using producer-authentic red/green proof or current-contract
behavior locks:

- `SPEC_GAP_PUBLIC_WIRE_CLAIM_001`;
- `SPEC_GAP_PROTOCOL_DISCOVERY_HANDSHAKE_001`;
- `SPEC_GAP_ETHERCAT_UNAVAILABLE_RESOURCE_001`; and
- `SPEC_GAP_PROTOCOL_STATUS_MODEL_001`.

The promoted invariants are `PROTO_ADS_001`, `PROTO_MODBUS_001`,
`PROTO_MQTT_001`, `PROTO_ETHERCAT_001`, `PROTO_DISCOVERY_TRUTH_001`, and
`PROTO_STATUS_TRUTH_001`. `PROTO_OPCUA_001` was already G1 and participates in
the status trace without receiving a new promotion. No broad gate was used as
causal promotion evidence, so this batch stops honestly at G1.

## Product defect and fix

The discovery-confidence trace produced a genuine red result at
`60e0394d`: an MQTT authentication-rejected CONNACK was classified as
`confirmed`, although the written contract reserves that confidence for a
protocol session that was accepted. The other six discovery cases passed.

Commit `ae6d37f4` changes only that outcome to `likely`, while preserving the
authentication-required flag, warning, clean-session CONNECT, and DISCONNECT.
The paired green proof passes all seven cases with the same case-file and
execution-contract digests.

Connector status projection and EtherCAT unavailable-resource behavior already
matched the written contracts. Their producer-authentic baseline/compare pairs
record current behavior locks; no failing result was manufactured for either
surface.

## Tests and contracts

The batch adds three hand-authored, cataloged runtime trace runners with 12
total cases:

- `TEST_CONNECTOR_STATUS_TRUTH_TRACE_001`: three lifecycle and staleness cases;
- `TEST_PROTOCOL_DISCOVERY_CONFIDENCE_TRACE_001`: seven Modbus and MQTT probe
  cases; and
- `TEST_ETHERCAT_UNAVAILABLE_RESOURCE_TRACE_001`: two absent-adapter and mock
  operation cases.

All runners consume committed case files through the shared
`verification-cases` artifact contract. The product fix remains in the
discovery-probe module; status projection and EtherCAT traces stay in their
own focused test modules. No new verification validator, schema, lifecycle,
suite binding, approved proof producer, or CI enforcement was added.

## Focused verification validation

At the clean final checkpoint on `trust-builder`:

- `python3 scripts/run_verification_focused_tests.py`: 808/808 passed in
  480.972 seconds;
- `python3 scripts/validate_verification_metadata.py`: 581 records before this
  batch-validation row was indexed;
- `scripts/verification_metadata_gate.sh`: passed, including the Phase 16
  report-only product fence and generated-case checks; and
- `git diff --check`: passed.

The first complete focused run exposed seven stale verification tripwires
caused by the three new Rust facts, catalog mappings, closed coverage cells,
and newly eligible protocol oracles. No product test failed in that run.
Commit `423e7407` refreshes only those measured expectations and retargets one
tooling self-test from the now-G1 MQTT invariant to an S0 UI invariant while
preserving its assigned catcher. The focused repair subset passed 79/79 before
the complete 808-test rerun.

## Generated report refresh

All 15 report pairs were generated one at a time from pristine detached
worktrees at `423e7407ad7e1ca3985c872b033fe42d786f6a82` with timestamp
`2026-07-16T19:41:00+02:00`. Every generator and production at-rest validator
exited zero, every Markdown report matched the imported bytes, and each
worktree was reset to pristine before generating the next report.

| Report | JSON SHA-256 |
| --- | --- |
| Test-class completeness | `96656be68c650d3c4b11f4c1dc8e64922cd6be901935befda486ff856792370f` |
| Coverage-matrix gaps | `4dbf8ef447de640e3c86dac9aeda62eea42fb5a7087c27fbbc24ab35f40752d3` |
| Malformed-input coverage | `8c8fab966db5dbc86c5bc9be0140c87750d1f4a23a8e70401e8e3aead869b92c` |
| Unmapped-test debt | `630242e763c790fded6ae112d2643fd1c75d7e5b40d9727a355058dcf8daffd6` |
| Test-refactor assessment | `a921a2d38c74e80f06fef20e925b22a6eedefe1ab56b33c56acf1f75d7ceaa0a` |
| Ignored-test inventory | `cdfefea2af78d0e0e5b512e08ec4c39fba749b1fbefa0b3d6c4ae9f6780258dc` |
| Phase 5 suite audit | `d9374adce888da2150868aac68b27c4afc9b091daf0c1e9d6bdde3134842bcb3` |
| Invariant-seed audit | `5723daa7ad5835382949cd3e22889eede74bf7c1bc725c8746d9c4c5f27b3eb3` |
| Specification completeness | `23ac8874ad069f3577b9c9b6866849566c35df1869ad8a9b64d6e20a89aae77f` |
| Requirement/oracle audit | `e7b2e03fce5304ada8f1b399e2c96541303bad1a8003d8a89b8c6572f5fd891c` |
| Conformance alignment | `27ca6ca1f232d81d809abc574a10a3aa33a26f0ca2170be0968952772d5b319c` |
| Runtime-anomaly audit | `60757f81cba8b5d935d5a3845b866b01ee5430660fd699bee6bb0fc686a9e823` |
| Fuzz-program audit | `8d01f61ca65f01b63d9f0c3bb889929f7d4737156c8d215ecd03295f655f05ce` |
| Mutation program | `82c984e60dd4a43e96765f597e5b3ee15ad8e97f88e46b9ea7748929c3cc8cc3` |
| Specification-source audit | `1164eb8e9cb1e9f6a947341926188c0f8ebe117bde1ab2859cb6f6f75a362c66` |

## Remote product validation

The final heavy gates ran once after the implementation and report binding on
the clean `trust-builder` worktree at
`8cc3199e155b54bfb8f167091ea90eb3483da947`, using the shared warmed target:

- `just fmt`: passed;
- `just clippy`: passed in 1 minute 53 seconds;
- `just test-all`: passed with an aggregate 3,207 passed, 0 failed, and 27
  ignored results across its emitted test-result lines;
- `cargo test -p trust-runtime --test api_smoke`: 3/3 passed;
- `cargo test -p trust-runtime --test debug_control`: 20/20 passed;
- `cargo test -p trust-runtime --test complete_program`: 1/1 passed;
- `cargo test -p trust-runtime --test runtime_reliability`: 4/4 passed;
- `scripts/runtime_comms_conformance_gate.sh`: passed;
- `scripts/runtime_mesh_tls_stability_gate.sh --iterations 8`: passed 8/8 on
  the first attempt; and
- `RUSTFLAGS=-Dwarnings cargo check -p trust-runtime --all-targets`: passed in
  5 minutes 21 seconds.

The first `just test-all` attempt stopped during linking because the builder
ran out of disk space. It produced no test assertion failure. Following the
repository disk-failure procedure, remaining compiler processes were checked,
disk usage was audited, only inactive generated target output was removed, and
the gate was rerun from a clean target. The successful retry above is the
accepted result. After final cleanup, the builder retained 72 GiB free on the
home filesystem and 3.5 GiB free on `/tmp`.

## Honest remaining posture

- The gap register contains 22 closed, 11 open, and 1 `spec_updated` record.
- The invariant register contains 53 records: 30 at S0, 14 at G1, and 9 at G2.
- The hand-owned catalog contains 193 records against 3,973 scanner facts; 188
  records represent generated tests and most existing-test debt remains open.
- This batch supplies loopback and deterministic-mock proof, not physical PLC,
  field-device, broker-policy, or EtherCAT-topology proof.
- No invariant was promoted to G2 or validated, no hardware result was
  inferred, and no CI, workflow, suite, or approved-proof-producer change was
  made.
- Version metadata is synchronized at 0.24.51; tagging and public release are
  deferred until the change reaches `main`.
