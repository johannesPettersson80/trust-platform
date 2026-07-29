# Phase 18 debug-adapter wire-schema conformance

Date: 2026-07-30

## Outcome

This batch added one normative debug-adapter wire contract, six missing
conformance tests, and exact specification authority for all 60 production
DAP protocol types in `crates/trust-debug/src/protocol.rs`.

The six new tests pass on the clean `trust-builder` checkout. No product defect
was found. The focused catalog runner did have a closed-path routing gap for
the new test module; that harness gap was fixed test-first.

## Specification written

`docs/specs/13-debug-adapter.md#dap-json-wire-schema` now defines:

- request, response, and event envelope fields and correlation;
- canonical `request_seq` output and the documented `requestSeq` input alias;
- lower-camel-case standard DAP fields;
- required fields, optional omission, defaults, and explicit zero retention;
- truST I/O state, variable state, write, force, and release payloads;
- breakpoint, location, and reload payloads;
- fail-closed rejection of missing required fields, wrong shapes, and invalid
  message, action, or scope discriminators.

`DEBUG_DAP_WIRE_SCHEMA_001` partitions that contract into six independently
reviewed behaviors.

## Tests added and run

`crates/trust-debug/src/protocol/wire_schema_tests.rs` adds:

1. `dap_envelopes_use_canonical_keys_and_legacy_input_only`;
2. `custom_state_payloads_preserve_force_scan_frame_and_instance_identity`;
3. `custom_mutation_payloads_use_required_fields_and_lowercase_discriminators`;
4. `standard_session_inspection_and_mutation_payloads_keep_dap_field_names`;
5. `breakpoint_and_reload_payloads_preserve_positions_and_optional_inputs`;
6. `missing_required_fields_and_invalid_discriminators_fail_closed`.

The controlled focused runner passed 6/6 new tests. The broader protocol-only
selection also passed those six plus the pre-existing
`response_uses_request_seq_field` regression, for 7/7.

The accepted assertion review contains 52 extracted assertions:

- 39 direct product-oracle assertions;
- 13 serialization/deserialization setup or precondition assertions;
- zero non-oracle observations;
- zero unresolved assertion helpers.

The focused coverage trace observed all 6/6 tests. Five tests primarily execute
compiler-generated Serde implementations, which do not expose ordinary
source-line code identities. The breakpoint/reload test additionally reached
two explicit production functions. Exact JSON comparisons, roundtrip equality,
default checks, and the 21-case rejection matrix provide the assertion oracle.

## Harness defect and test-first repair

The first controlled-runner attempt rejected
`crates/trust-debug/src/protocol/wire_schema_tests.rs` as outside the closed
`debug-remaining` path set. A focused unit test was added first and failed with
that exact rejection. After the new module was assigned the exact
`protocol::wire_schema_tests::` prefix, the same unit test passed and the
controlled 6/6 execution succeeded.

This was a verification-harness routing defect, not a product defect.

## Code-authority review

The post-catalog authority proposal produced one singular group containing all
60 public types and helper constructors in `crates/trust-debug/src/protocol.rs`.
The group was accepted only for `DEBUG_DAP_WIRE_SCHEMA_001`; all 21 unrelated
candidate groups were withheld with their prior semantic decisions.

The accepted code/spec census contains 17,229 live and mapped facts, zero
findings, 2,090 `specified`, and 15,104 remaining `spec_gap` facts. No residual
authority proposal remains for `crates/trust-debug/src/protocol.rs`.

Retained machine-readable evidence:

- focused execution:
  `target/gate-artifacts/verification/p18-dap-wire-6-execution/results/wire-schema.json`
  (`sha256:5fdaa4e78be19aed1cfd67d2d252137127f5935345cb841f03649bf9ed25c116`);
- focused reachability:
  `target/gate-artifacts/verification/p18-dap-wire-6-coverage.json`
  (`sha256:90f50a6ab58afd4480b54cbe98911e3e2511034c9b501f720e5844d369bf6f96`);
- authority application:
  `target/gate-artifacts/code-spec-authority-reuse/p18-dap-wire-536379bd-application.json`
  (`sha256:8f397ed279f08466fc4d25872d9b90eebe400ecce36afbae82abf6bba5e7be73`);
- accepted code/spec report:
  `target/gate-artifacts/p18-dap-wire-5f2f334a/code-spec-coverage.json`
  (`sha256:2983055bdf871e15fbbf2fc76341c55b368520950158267d73bc5aed40492ea5`);
- residual proposal:
  `target/gate-artifacts/code-spec-authority-reuse/p18-dap-wire-5f2f334a-residual.json`
  (`sha256:7ae45ed14da754ffc9e61fe7006aeb8a7e2c22accd184c9e8603203bdfd48a5a`).

## Boundary

This closes the DAP wire-schema specification, catalog, focused execution,
assertion-review, reachability, and production code-authority slice. It does
not close the remaining 15,104 specification-gap facts, the ten newly
discovered PLCopen test mappings, mutation or adequacy campaigns, the final
broad remote gates, or any global Phase 18 row.

The repository-wide staleness validator remains red on the known Phase 18
metadata and proof backlog. No broad `just fmt`, `just clippy`, `just test`, or
`just test-all` gate ran.
