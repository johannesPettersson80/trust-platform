# Specification Completeness Report

Generator: `spec-completeness v1`
Source revision: `423e7407ad7e1ca3985c872b033fe42d786f6a82`
Generated: `2026-07-16T19:41:00+02:00`
Platform: `linux-x86_64`
Generated JSON SHA-256: `23ac8874ad069f3577b9c9b6866849566c35df1869ad8a9b64d6e20a89aae77f`
Input SHA-256: `sha256:9d4b6d47852bf5829f45d16670538bd68e685cf6a866a67a7d05e82610d5f0d7`

`complete` means the committed metadata was exhaustively analyzed under the
declared scopes. It does not mean the specifications or tests are complete.

## Summary

- Invariants: 53
- Invariants without specified specs: 14
- Tests with expected results: 193
- Tests without oracle/spec/gap binding: 3
- Coverage cells: 68
- Coverage cells marked spec_gap: 18
- Bytecode pilot gaps: 3
- Registered public-claim sources: 4

## Invariants Without Specified Specs

| Invariant | Area | Risk | Invariant status | Spec status | Spec gaps |
| --- | --- | --- | --- | --- | --- |
| `DEBUG_BEHAVIOR_LOCKED_001` | `editor_safety` | `false_status` | `spec_gap` | `ambiguous` | `SPEC_GAP_BEHAVIOR_LOCKED_PUBLIC_CLAIM_001` |
| `DEV_COMMIT_SCOPE_001` | `plcopen_devtools` | `data_loss` | `spec_gap` | `missing` | `SPEC_GAP_DEV_COMMIT_SCOPE_001` |
| `DEV_TEST_DISCOVERY_001` | `plcopen_devtools` | `false_status` | `spec_gap` | `missing` | `SPEC_GAP_DEV_TEST_DISCOVERY_CASE_001` |
| `PLAT_PATH_001` | `supply_chain_platform` | `platform` | `spec_gap` | `missing` | `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` |
| `PLAT_VSCODE_001` | `supply_chain_platform` | `compatibility` | `spec_gap` | `missing` | `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` |
| `RELEASE_PLATFORM_MATRIX_001` | `release` | `compatibility` | `spec_gap` | `ambiguous` | `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` |
| `RELEASE_SOURCE_BUILD_OPENOT_001` | `release` | `compatibility` | `spec_gap` | `ambiguous` | `SPEC_GAP_SOURCE_BUILD_PUBLIC_CLAIM_001` |
| `REL_CLAIM_001` | `release` | `false_status` | `spec_gap` | `missing` | `SPEC_GAP_HARDWARE_PUBLIC_CLAIM_001` |
| `REL_CONF_001` | `release` | `false_status` | `spec_gap` | `missing` | `SPEC_GAP_CONFORMANCE_PUBLICATION_001` |
| `REL_VERSION_001` | `release` | `false_status` | `spec_gap` | `missing` | `SPEC_GAP_RELEASE_VERSION_CHAIN_001` |
| `RUNTIME_BEHAVIOR_LOCKED_001` | `release` | `false_status` | `spec_gap` | `ambiguous` | `SPEC_GAP_BEHAVIOR_LOCKED_PUBLIC_CLAIM_001` |
| `SEC_ARTIFACT_001` | `supply_chain_platform` | `supply_chain` | `spec_gap` | `missing` | `SPEC_GAP_ARTIFACT_PROVENANCE_001` |
| `SEC_DEP_AUDIT_001` | `supply_chain_platform` | `supply_chain` | `spec_gap` | `missing` | `SPEC_GAP_DEPENDENCY_AUDIT_POLICY_001` |
| `UI_STATUS_001` | `hmi_ui` | `false_status` | `spec_gap` | `missing` | `SPEC_GAP_UI_STATUS_VOCABULARY_001` |

## Expected-Result Tests Without Oracle Binding

| Test | Area | Class | Status | Missing bindings |
| --- | --- | --- | --- | --- |
| `TEST_CASE_TABLE_VM_SEAM_DECLARED_TYPE_001` | `bytecode_vm` | `metadata_validation` | `planned` | `oracle_ref`, `spec_ref`, `spec_gap_ref` |
| `TEST_CASE_TABLE_VM_SEAM_STRING_BOUND_001` | `bytecode_vm` | `metadata_validation` | `planned` | `oracle_ref`, `spec_ref`, `spec_gap_ref` |
| `TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001` | `bytecode_vm` | `metadata_validation` | `planned` | `oracle_ref`, `spec_ref`, `spec_gap_ref` |

## Spec-Gap Coverage Cells

| Invariant | Area | Risk | Cell | Dimension | Spec gap |
| --- | --- | --- | ---: | --- | --- |
| `DEBUG_BEHAVIOR_LOCKED_001` | `editor_safety` | `false_status` | 0 | `happy_path` | `SPEC_GAP_BEHAVIOR_LOCKED_PUBLIC_CLAIM_001` |
| `DEV_COMMIT_SCOPE_001` | `plcopen_devtools` | `data_loss` | 0 | `duplicate_or_collision` | `SPEC_GAP_DEV_COMMIT_SCOPE_001` |
| `DEV_TEST_DISCOVERY_001` | `plcopen_devtools` | `false_status` | 0 | `platform_or_filesystem_variation` | `SPEC_GAP_DEV_TEST_DISCOVERY_CASE_001` |
| `PLAT_PATH_001` | `supply_chain_platform` | `platform` | 0 | `platform_or_filesystem_variation` | `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` |
| `PLAT_VSCODE_001` | `supply_chain_platform` | `compatibility` | 0 | `platform_or_filesystem_variation` | `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` |
| `RELEASE_PLATFORM_MATRIX_001` | `release` | `compatibility` | 0 | `happy_path` | `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` |
| `RELEASE_SOURCE_BUILD_OPENOT_001` | `release` | `compatibility` | 0 | `supply_chain_or_artifact_fault` | `SPEC_GAP_SOURCE_BUILD_PUBLIC_CLAIM_001` |
| `REL_CLAIM_001` | `release` | `false_status` | 0 | `hardware_or_network_fault` | `SPEC_GAP_HARDWARE_PUBLIC_CLAIM_001` |
| `REL_CONF_001` | `release` | `false_status` | 0 | `supply_chain_or_artifact_fault` | `SPEC_GAP_CONFORMANCE_PUBLICATION_001` |
| `REL_VERSION_001` | `release` | `false_status` | 0 | `supply_chain_or_artifact_fault` | `SPEC_GAP_RELEASE_VERSION_CHAIN_001` |
| `RUNTIME_BEHAVIOR_LOCKED_001` | `release` | `false_status` | 0 | `happy_path` | `SPEC_GAP_BEHAVIOR_LOCKED_PUBLIC_CLAIM_001` |
| `SEC_ARTIFACT_001` | `supply_chain_platform` | `supply_chain` | 0 | `supply_chain_or_artifact_fault` | `SPEC_GAP_ARTIFACT_PROVENANCE_001` |
| `SEC_DEP_AUDIT_001` | `supply_chain_platform` | `supply_chain` | 0 | `supply_chain_or_artifact_fault` | `SPEC_GAP_DEPENDENCY_AUDIT_POLICY_001` |
| `UI_STATUS_001` | `hmi_ui` | `false_status` | 0 | `ordering_or_lifecycle` | `SPEC_GAP_UI_STATUS_VOCABULARY_001` |
| `VM_SEAM_DECLARED_TYPE_001` | `bytecode_vm` | `wrong_result` | 1 | `wrong_type_or_shape` | `SPEC_GAP_VM_ERROR_MODEL_001` |
| `VM_SEAM_STRING_BOUND_001` | `bytecode_vm` | `wrong_result` | 2 | `wrong_type_or_shape` | `SPEC_GAP_VM_ERROR_MODEL_001` |
| `VM_SEAM_SUBRANGE_001` | `bytecode_vm` | `wrong_result` | 3 | `wrong_type_or_shape` | `SPEC_GAP_VM_ERROR_MODEL_001` |
| `VM_SEAM_VALID_001` | `bytecode_vm` | `silent_corruption` | 2 | `extra_or_unknown` | `SPEC_GAP_VM_ERROR_MODEL_001` |

## Bytecode/VM Pilot Gap Classification

Denominator: `open_spec_gaps_union_missing_required_runnable_test_classes`

- `test_gap`: 2
- `spec_gap`: 1
- `hardware_tool_blocked`: 0
- `not_applicable`: 0

| Gap | Classification | Source kind | Detail | Related records |
| --- | --- | --- | --- | --- |
| `SPEC_GAP_VM_ERROR_MODEL_001` | `spec_gap` | `spec_gap_record` | Which stable typed error identifiers must bytecode validation, runtime value conversion, and VM traps emit so tests do not match ad-hoc strings? | `VM_SEAM_DECLARED_TYPE_001`, `VM_SEAM_STRING_BOUND_001`, `VM_SEAM_SUBRANGE_001`, `VM_SEAM_VALID_001` |
| `TEST_CLASS_GAP:bytecode_vm:iec_conformance` | `test_gap` | `required_test_class_slot` | Required test class iec_conformance has no catalog row. | none |
| `TEST_CLASS_GAP:bytecode_vm:metadata_validation` | `test_gap` | `required_test_class_slot` | Required test class metadata_validation has catalog rows but none are effectively runnable. | `TEST_CASE_TABLE_VM_SEAM_DECLARED_TYPE_001`, `TEST_CASE_TABLE_VM_SEAM_STRING_BOUND_001`, `TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001`, `TEST_CASE_TABLE_VM_SEAM_VALID_001` |

## Registered Public-Claim Context

Basis: `registered_spec_sources_only`. Exhaustive public-doc scan: `no`.

| Source | Area | Status | Surface | Invariants | Oracles | Spec gaps |
| --- | --- | --- | --- | --- | --- | --- |
| `PUBLIC_CLAIM_BEHAVIOR_LOCKED_001` | `release` | `active` | `README.md` | `DEBUG_BEHAVIOR_LOCKED_001`, `REL_CONF_001`, `RUNTIME_BEHAVIOR_LOCKED_001` | none | `SPEC_GAP_BEHAVIOR_LOCKED_PUBLIC_CLAIM_001`, `SPEC_GAP_CONFORMANCE_PUBLICATION_001` |
| `PUBLIC_CLAIM_RUNTIME_WIRE_001` | `protocols` | `active` | `README.md` | `REL_CLAIM_001`, `UI_STATUS_001` | none | `SPEC_GAP_ETHERCAT_UNAVAILABLE_RESOURCE_001`, `SPEC_GAP_HARDWARE_PUBLIC_CLAIM_001`, `SPEC_GAP_PROTOCOL_DISCOVERY_HANDSHAKE_001`, `SPEC_GAP_PROTOCOL_STATUS_MODEL_001`, `SPEC_GAP_PUBLIC_WIRE_CLAIM_001`, `SPEC_GAP_UI_STATUS_VOCABULARY_001` |
| `PUBLIC_CLAIM_SOURCE_BUILD_RUNTIME_001` | `release` | `active` | `docs/public/start/install-from-source.md#optional-openot-reference-checkout` | `RELEASE_SOURCE_BUILD_OPENOT_001`, `REL_VERSION_001`, `SEC_ARTIFACT_001`, `SEC_DEP_AUDIT_001` | none | `SPEC_GAP_ARTIFACT_PROVENANCE_001`, `SPEC_GAP_DEPENDENCY_AUDIT_POLICY_001`, `SPEC_GAP_RELEASE_VERSION_CHAIN_001`, `SPEC_GAP_SOURCE_BUILD_PUBLIC_CLAIM_001` |
| `PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001` | `release` | `active` | `README.md` | `PLAT_PATH_001`, `PLAT_VSCODE_001`, `RELEASE_PLATFORM_MATRIX_001`, `REL_CLAIM_001`, `REL_VERSION_001`, `SEC_ARTIFACT_001` | none | `SPEC_GAP_ARTIFACT_PROVENANCE_001`, `SPEC_GAP_HARDWARE_PUBLIC_CLAIM_001`, `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001`, `SPEC_GAP_RELEASE_VERSION_CHAIN_001` |

## Limitations

- The invariant, catalog, coverage-cell, and bytecode pilot sections are exhaustive only for committed verification metadata at the bound source revision.
- The bytecode pilot denominator is exactly the union of open bytecode_vm spec-gap records and required bytecode_vm test-class slots lacking an effectively runnable, non-ignored catalog row.
- The pilot does not infer hardware/tool-blocked or not-applicable entries; those classifications remain zero unless a future reviewed metadata source extends the denominator contract.
- A test is oracle-bound only by a non-empty oracle_ref, spec_ref, or spec_gap_ref field; names, paths, expected-result prose, and inferred references never create a binding.
- Public-claim rows in this report are registered-spec-source context only; the separate source audit inventories all rendered public prose, but semantic claim dispositions remain incomplete and VERIF-P4A-005 stays open.
- verification/evidence-index.toml is live-validated but excluded from the input digest to avoid a report-evidence digest cycle; close-out evidence relationships are recomputed at rest.
- Report debt is visibility, not proof, spec-gap closure, test adequacy, or CI enforcement.
- Platform is historical provenance requiring evidence review; at-rest validation cannot rederive a prior host.
