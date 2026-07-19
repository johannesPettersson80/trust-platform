# Specification Completeness Report

Generator: `spec-completeness v1`
Source revision: `3803c39a829cf4d771c8b621f48edff5d2500600`
Generated: `2026-07-19T20:10:00+02:00`
Platform: `linux-x86_64`
Generated JSON SHA-256: `d1f3d6afda6bed32728fbac04631e952456c2e982ecae2700b241d1437dd9009`
Input SHA-256: `sha256:8b04385970bf5f826008d2e1d166ec69c5088ca5601a748fbbe027e0f521e341`

`complete` means the committed metadata was exhaustively analyzed under the
declared scopes. It does not mean the specifications or tests are complete.

## Summary

- Invariants: 55
- Invariants without specified specs: 0
- Tests with expected results: 266
- Tests without oracle/spec/gap binding: 0
- Coverage cells: 71
- Coverage cells marked spec_gap: 0
- Bytecode pilot gaps: 0
- Registered public-claim sources: 4

## Invariants Without Specified Specs

| Invariant | Area | Risk | Invariant status | Spec status | Spec gaps |
| --- | --- | --- | --- | --- | --- |
| none | - | - | - | - | - |

## Expected-Result Tests Without Oracle Binding

| Test | Area | Class | Status | Missing bindings |
| --- | --- | --- | --- | --- |
| none | - | - | - | - |

## Spec-Gap Coverage Cells

| Invariant | Area | Risk | Cell | Dimension | Spec gap |
| --- | --- | --- | ---: | --- | --- |
| none | - | - | - | - | - |

## Bytecode/VM Pilot Gap Classification

Denominator: `open_spec_gaps_union_missing_required_runnable_test_classes`

- `test_gap`: 0
- `spec_gap`: 0
- `hardware_tool_blocked`: 0
- `not_applicable`: 0

| Gap | Classification | Source kind | Detail | Related records |
| --- | --- | --- | --- | --- |

## Registered Public-Claim Context

Basis: `registered_spec_sources_only`. Exhaustive public-doc scan: `no`.

| Source | Area | Status | Surface | Invariants | Oracles | Spec gaps |
| --- | --- | --- | --- | --- | --- | --- |
| `PUBLIC_CLAIM_BEHAVIOR_LOCKED_001` | `release` | `active` | `README.md` | `DEBUG_BEHAVIOR_LOCKED_001`, `RUNTIME_BEHAVIOR_LOCKED_001` | none | `SPEC_GAP_BEHAVIOR_LOCKED_PUBLIC_CLAIM_001`, `SPEC_GAP_CONFORMANCE_PUBLICATION_001` |
| `PUBLIC_CLAIM_RUNTIME_WIRE_001` | `protocols` | `active` | `README.md` | `REL_CLAIM_001`, `UI_STATUS_001` | none | `SPEC_GAP_ETHERCAT_UNAVAILABLE_RESOURCE_001`, `SPEC_GAP_HARDWARE_PUBLIC_CLAIM_001`, `SPEC_GAP_PROTOCOL_DISCOVERY_HANDSHAKE_001`, `SPEC_GAP_PROTOCOL_STATUS_MODEL_001`, `SPEC_GAP_PUBLIC_WIRE_CLAIM_001`, `SPEC_GAP_UI_STATUS_VOCABULARY_001` |
| `PUBLIC_CLAIM_SOURCE_BUILD_RUNTIME_001` | `release` | `active` | `docs/public/start/install-from-source.md#optional-openot-reference-checkout` | `RELEASE_SOURCE_BUILD_OPENOT_001` | none | `SPEC_GAP_ARTIFACT_PROVENANCE_001`, `SPEC_GAP_DEPENDENCY_AUDIT_POLICY_001`, `SPEC_GAP_RELEASE_VERSION_CHAIN_001`, `SPEC_GAP_SOURCE_BUILD_PUBLIC_CLAIM_001` |
| `PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001` | `release` | `active` | `README.md` | `PLAT_PATH_001`, `PLAT_VSCODE_001`, `RELEASE_PLATFORM_MATRIX_001`, `REL_CLAIM_001` | none | `SPEC_GAP_ARTIFACT_PROVENANCE_001`, `SPEC_GAP_HARDWARE_PUBLIC_CLAIM_001`, `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001`, `SPEC_GAP_RELEASE_VERSION_CHAIN_001` |

## Limitations

- The invariant, catalog, coverage-cell, and bytecode pilot sections are exhaustive only for committed verification metadata at the bound source revision.
- The bytecode pilot denominator is exactly the union of open bytecode_vm spec-gap records and required bytecode_vm test-class slots lacking an effectively runnable, non-ignored catalog row.
- The pilot does not infer hardware/tool-blocked or not-applicable entries; those classifications remain zero unless a future reviewed metadata source extends the denominator contract.
- A test is oracle-bound only by a non-empty oracle_ref, spec_ref, or spec_gap_ref field; names, paths, expected-result prose, and inferred references never create a binding.
- Public-claim rows in this report are registered-spec-source context only; the separate source audit now exhaustively dispositions every rendered public block and reports all substantive blocks without an invariant or oracle.
- verification/evidence-index.toml is live-validated but excluded from the input digest to avoid a report-evidence digest cycle; close-out evidence relationships are recomputed at rest.
- Report debt is visibility, not proof, spec-gap closure, test adequacy, or CI enforcement.
- Platform is historical provenance requiring evidence review; at-rest validation cannot rederive a prior host.
