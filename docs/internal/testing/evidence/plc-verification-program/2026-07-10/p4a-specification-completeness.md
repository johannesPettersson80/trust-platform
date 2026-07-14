# Specification Completeness Report

Generator: `spec-completeness v1`
Source revision: `c935e82b209b8dabab17f17e398b4dc5fc5ab5b6`
Generated: `2026-07-14T14:35:59+02:00`
Platform: `linux-x86_64`
Generated JSON SHA-256: `ccce74f6f34709bc24b5019615c512ac50e950561dbb3c50f6aadf8a10a111f1`
Input SHA-256: `sha256:e246e4cb4b4a5f4a39ec54d58444c84f52ae34fc51d35027e7ffbcc4ddc9e263`

`complete` means the committed metadata was exhaustively analyzed under the
declared scopes. It does not mean the specifications or tests are complete.

## Summary

- Invariants: 53
- Invariants without specified specs: 37
- Tests with expected results: 88
- Tests without oracle/spec/gap binding: 0
- Coverage cells: 65
- Coverage cells marked spec_gap: 44
- Bytecode pilot gaps: 7
- Registered public-claim sources: 4

## Invariants Without Specified Specs

| Invariant | Area | Risk | Invariant status | Spec status | Spec gaps |
| --- | --- | --- | --- | --- | --- |
| `DEBUG_AUTH_001` | `control_security` | `security` | `spec_gap` | `missing` | `SPEC_GAP_DEBUG_AUTHORIZATION_001` |
| `DEBUG_BEHAVIOR_LOCKED_001` | `editor_safety` | `false_status` | `spec_gap` | `ambiguous` | `SPEC_GAP_BEHAVIOR_LOCKED_PUBLIC_CLAIM_001` |
| `DEBUG_PAUSE_001` | `editor_safety` | `safety_critical` | `spec_gap` | `missing` | `SPEC_GAP_DEBUG_PAUSE_WATCHDOG_001` |
| `DEV_COMMIT_SCOPE_001` | `plcopen_devtools` | `data_loss` | `spec_gap` | `missing` | `SPEC_GAP_DEV_COMMIT_SCOPE_001` |
| `DEV_TEST_DISCOVERY_001` | `plcopen_devtools` | `false_status` | `spec_gap` | `missing` | `SPEC_GAP_DEV_TEST_DISCOVERY_CASE_001` |
| `EDIT_DIAG_CANCEL_001` | `editor_safety` | `false_status` | `spec_gap` | `ambiguous` | `SPEC_GAP_EDITOR_DIAGNOSTIC_CANCELLATION_001` |
| `EDIT_LSP_POS_001` | `editor_safety` | `silent_corruption` | `spec_gap` | `missing` | `SPEC_GAP_EDITOR_LSP_POSITION_ENCODING_001` |
| `EDIT_RENAME_001` | `editor_safety` | `silent_corruption` | `spec_gap` | `ambiguous` | `SPEC_GAP_EDITOR_RENAME_CONFLICT_001` |
| `EDIT_RENAME_002` | `editor_safety` | `silent_corruption` | `spec_gap` | `ambiguous` | `SPEC_GAP_EDITOR_RENAME_CONFLICT_001` |
| `IEC_PARSE_RECOVER_001` | `compiler_iec` | `silent_corruption` | `spec_gap` | `missing` | `SPEC_GAP_IEC_PARSER_RECOVERY_001` |
| `IEC_STRING_001` | `compiler_iec` | `wrong_result` | `spec_gap` | `ambiguous` | `SPEC_GAP_IEC_STRING_BINDING_BOUNDS_001` |
| `PLAT_PATH_001` | `supply_chain_platform` | `platform` | `spec_gap` | `missing` | `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` |
| `PLAT_VSCODE_001` | `supply_chain_platform` | `compatibility` | `spec_gap` | `missing` | `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` |
| `PROTO_ADS_001` | `protocols` | `false_status` | `spec_gap` | `missing` | `SPEC_GAP_PROTOCOL_STATUS_MODEL_001` |
| `PROTO_DISCOVERY_TRUTH_001` | `protocols` | `false_status` | `spec_gap` | `ambiguous` | `SPEC_GAP_PUBLIC_WIRE_CLAIM_001` |
| `PROTO_ETHERCAT_001` | `protocols` | `false_status` | `spec_gap` | `missing` | `SPEC_GAP_ETHERCAT_UNAVAILABLE_RESOURCE_001` |
| `PROTO_MODBUS_001` | `protocols` | `false_status` | `spec_gap` | `missing` | `SPEC_GAP_PROTOCOL_DISCOVERY_HANDSHAKE_001` |
| `PROTO_MQTT_001` | `protocols` | `false_status` | `spec_gap` | `missing` | `SPEC_GAP_PROTOCOL_DISCOVERY_HANDSHAKE_001` |
| `PROTO_STATUS_TRUTH_001` | `protocols` | `false_status` | `spec_gap` | `ambiguous` | `SPEC_GAP_PUBLIC_WIRE_CLAIM_001` |
| `RELEASE_PLATFORM_MATRIX_001` | `release` | `compatibility` | `spec_gap` | `ambiguous` | `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` |
| `RELEASE_SOURCE_BUILD_OPENOT_001` | `release` | `compatibility` | `spec_gap` | `ambiguous` | `SPEC_GAP_SOURCE_BUILD_PUBLIC_CLAIM_001` |
| `REL_CLAIM_001` | `release` | `false_status` | `spec_gap` | `missing` | `SPEC_GAP_HARDWARE_PUBLIC_CLAIM_001` |
| `REL_CONF_001` | `release` | `false_status` | `spec_gap` | `missing` | `SPEC_GAP_CONFORMANCE_PUBLICATION_001` |
| `REL_VERSION_001` | `release` | `false_status` | `spec_gap` | `missing` | `SPEC_GAP_RELEASE_VERSION_CHAIN_001` |
| `RUNTIME_BEHAVIOR_LOCKED_001` | `release` | `false_status` | `spec_gap` | `ambiguous` | `SPEC_GAP_BEHAVIOR_LOCKED_PUBLIC_CLAIM_001` |
| `SEC_ARTIFACT_001` | `supply_chain_platform` | `supply_chain` | `spec_gap` | `missing` | `SPEC_GAP_ARTIFACT_PROVENANCE_001` |
| `SEC_AUTHZ_001` | `control_security` | `security` | `spec_gap` | `missing` | `SPEC_GAP_CONTROL_AUTHORIZATION_MATRIX_001` |
| `SEC_DEP_AUDIT_001` | `supply_chain_platform` | `supply_chain` | `spec_gap` | `missing` | `SPEC_GAP_DEPENDENCY_AUDIT_POLICY_001` |
| `UI_STATUS_001` | `hmi_ui` | `false_status` | `spec_gap` | `missing` | `SPEC_GAP_UI_STATUS_VOCABULARY_001` |
| `VM_SEAM_DECLARED_TYPE_001` | `bytecode_vm` | `wrong_result` | `spec_gap` | `ambiguous` | `SPEC_GAP_VM_VALUE_SEMANTICS_001` |
| `VM_SEAM_DETERMINISM_LIMITS_001` | `bytecode_vm` | `wrong_result` | `spec_gap` | `missing` | `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001` |
| `VM_SEAM_ENC_001` | `bytecode_vm` | `silent_corruption` | `spec_gap` | `missing` | `SPEC_GAP_VM_ERROR_MODEL_001`, `SPEC_GAP_VM_LOWERING_FAIL_CLOSED_001` |
| `VM_SEAM_OWNER_001` | `bytecode_vm` | `silent_corruption` | `spec_gap` | `missing` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `VM_SEAM_REF_001` | `bytecode_vm` | `silent_corruption` | `spec_gap` | `missing` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `VM_SEAM_STRING_BOUND_001` | `bytecode_vm` | `wrong_result` | `spec_gap` | `ambiguous` | `SPEC_GAP_VM_VALUE_SEMANTICS_001` |
| `VM_SEAM_SUBRANGE_001` | `bytecode_vm` | `wrong_result` | `spec_gap` | `ambiguous` | `SPEC_GAP_VM_ERROR_MODEL_001`, `SPEC_GAP_VM_VALUE_SEMANTICS_001` |
| `VM_SEAM_VALID_001` | `bytecode_vm` | `silent_corruption` | `spec_gap` | `missing` | `SPEC_GAP_BYTECODE_VALIDATOR_001`, `SPEC_GAP_VM_ERROR_MODEL_001` |

## Expected-Result Tests Without Oracle Binding

| Test | Area | Class | Status | Missing bindings |
| --- | --- | --- | --- | --- |
| none | - | - | - | - |

## Spec-Gap Coverage Cells

| Invariant | Area | Risk | Cell | Dimension | Spec gap |
| --- | --- | --- | ---: | --- | --- |
| `DEBUG_AUTH_001` | `control_security` | `security` | 0 | `auth_or_permission` | `SPEC_GAP_DEBUG_AUTHORIZATION_001` |
| `DEBUG_BEHAVIOR_LOCKED_001` | `editor_safety` | `false_status` | 0 | `happy_path` | `SPEC_GAP_BEHAVIOR_LOCKED_PUBLIC_CLAIM_001` |
| `DEBUG_PAUSE_001` | `editor_safety` | `safety_critical` | 0 | `time_or_clock_fault` | `SPEC_GAP_DEBUG_PAUSE_WATCHDOG_001` |
| `DEV_COMMIT_SCOPE_001` | `plcopen_devtools` | `data_loss` | 0 | `duplicate_or_collision` | `SPEC_GAP_DEV_COMMIT_SCOPE_001` |
| `DEV_TEST_DISCOVERY_001` | `plcopen_devtools` | `false_status` | 0 | `platform_or_filesystem_variation` | `SPEC_GAP_DEV_TEST_DISCOVERY_CASE_001` |
| `EDIT_DIAG_CANCEL_001` | `editor_safety` | `false_status` | 0 | `concurrency_or_cancellation` | `SPEC_GAP_EDITOR_DIAGNOSTIC_CANCELLATION_001` |
| `EDIT_LSP_POS_001` | `editor_safety` | `silent_corruption` | 0 | `encoding_or_unicode` | `SPEC_GAP_EDITOR_LSP_POSITION_ENCODING_001` |
| `EDIT_RENAME_001` | `editor_safety` | `silent_corruption` | 0 | `duplicate_or_collision` | `SPEC_GAP_EDITOR_RENAME_CONFLICT_001` |
| `EDIT_RENAME_002` | `editor_safety` | `silent_corruption` | 0 | `duplicate_or_collision` | `SPEC_GAP_EDITOR_RENAME_CONFLICT_001` |
| `IEC_PARSE_RECOVER_001` | `compiler_iec` | `silent_corruption` | 0 | `wrong_type_or_shape` | `SPEC_GAP_IEC_PARSER_RECOVERY_001` |
| `IEC_STRING_001` | `compiler_iec` | `wrong_result` | 0 | `boundary_high` | `SPEC_GAP_IEC_STRING_BINDING_BOUNDS_001` |
| `PLAT_PATH_001` | `supply_chain_platform` | `platform` | 0 | `platform_or_filesystem_variation` | `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` |
| `PLAT_VSCODE_001` | `supply_chain_platform` | `compatibility` | 0 | `platform_or_filesystem_variation` | `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` |
| `PROTO_ADS_001` | `protocols` | `false_status` | 0 | `ordering_or_lifecycle` | `SPEC_GAP_PROTOCOL_STATUS_MODEL_001` |
| `PROTO_DISCOVERY_TRUTH_001` | `protocols` | `false_status` | 0 | `hardware_or_network_fault` | `SPEC_GAP_PUBLIC_WIRE_CLAIM_001` |
| `PROTO_ETHERCAT_001` | `protocols` | `false_status` | 0 | `resource_limit` | `SPEC_GAP_ETHERCAT_UNAVAILABLE_RESOURCE_001` |
| `PROTO_MODBUS_001` | `protocols` | `false_status` | 0 | `hardware_or_network_fault` | `SPEC_GAP_PROTOCOL_DISCOVERY_HANDSHAKE_001` |
| `PROTO_MQTT_001` | `protocols` | `false_status` | 0 | `ordering_or_lifecycle` | `SPEC_GAP_PROTOCOL_DISCOVERY_HANDSHAKE_001` |
| `PROTO_STATUS_TRUTH_001` | `protocols` | `false_status` | 0 | `hardware_or_network_fault` | `SPEC_GAP_PUBLIC_WIRE_CLAIM_001` |
| `RELEASE_PLATFORM_MATRIX_001` | `release` | `compatibility` | 0 | `happy_path` | `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` |
| `RELEASE_SOURCE_BUILD_OPENOT_001` | `release` | `compatibility` | 0 | `supply_chain_or_artifact_fault` | `SPEC_GAP_SOURCE_BUILD_PUBLIC_CLAIM_001` |
| `REL_CLAIM_001` | `release` | `false_status` | 0 | `hardware_or_network_fault` | `SPEC_GAP_HARDWARE_PUBLIC_CLAIM_001` |
| `REL_CONF_001` | `release` | `false_status` | 0 | `supply_chain_or_artifact_fault` | `SPEC_GAP_CONFORMANCE_PUBLICATION_001` |
| `REL_VERSION_001` | `release` | `false_status` | 0 | `supply_chain_or_artifact_fault` | `SPEC_GAP_RELEASE_VERSION_CHAIN_001` |
| `RT_SAFE_FORCE_001` | `runtime_safety` | `safety_critical` | 0 | `ordering_or_lifecycle` | `SPEC_GAP_RUNTIME_FORCE_LIFECYCLE_001` |
| `RUNTIME_BEHAVIOR_LOCKED_001` | `release` | `false_status` | 0 | `happy_path` | `SPEC_GAP_BEHAVIOR_LOCKED_PUBLIC_CLAIM_001` |
| `SEC_ARTIFACT_001` | `supply_chain_platform` | `supply_chain` | 0 | `supply_chain_or_artifact_fault` | `SPEC_GAP_ARTIFACT_PROVENANCE_001` |
| `SEC_AUTHZ_001` | `control_security` | `security` | 0 | `auth_or_permission` | `SPEC_GAP_CONTROL_AUTHORIZATION_MATRIX_001` |
| `SEC_DEP_AUDIT_001` | `supply_chain_platform` | `supply_chain` | 0 | `supply_chain_or_artifact_fault` | `SPEC_GAP_DEPENDENCY_AUDIT_POLICY_001` |
| `UI_STATUS_001` | `hmi_ui` | `false_status` | 0 | `ordering_or_lifecycle` | `SPEC_GAP_UI_STATUS_VOCABULARY_001` |
| `VM_SEAM_DECLARED_TYPE_001` | `bytecode_vm` | `wrong_result` | 0 | `happy_path` | `SPEC_GAP_VM_VALUE_SEMANTICS_001` |
| `VM_SEAM_DECLARED_TYPE_001` | `bytecode_vm` | `wrong_result` | 1 | `wrong_type_or_shape` | `SPEC_GAP_VM_VALUE_SEMANTICS_001` |
| `VM_SEAM_DETERMINISM_LIMITS_001` | `bytecode_vm` | `wrong_result` | 0 | `resource_limit` | `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001` |
| `VM_SEAM_DETERMINISM_LIMITS_001` | `bytecode_vm` | `wrong_result` | 1 | `time_or_clock_fault` | `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001` |
| `VM_SEAM_ENC_001` | `bytecode_vm` | `silent_corruption` | 0 | `extra_or_unknown` | `SPEC_GAP_VM_LOWERING_FAIL_CLOSED_001` |
| `VM_SEAM_OWNER_001` | `bytecode_vm` | `silent_corruption` | 0 | `wrong_type_or_shape` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `VM_SEAM_REF_001` | `bytecode_vm` | `silent_corruption` | 0 | `wrong_type_or_shape` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `VM_SEAM_STRING_BOUND_001` | `bytecode_vm` | `wrong_result` | 0 | `happy_path` | `SPEC_GAP_VM_VALUE_SEMANTICS_001` |
| `VM_SEAM_STRING_BOUND_001` | `bytecode_vm` | `wrong_result` | 1 | `above_max` | `SPEC_GAP_VM_VALUE_SEMANTICS_001` |
| `VM_SEAM_STRING_BOUND_001` | `bytecode_vm` | `wrong_result` | 2 | `wrong_type_or_shape` | `SPEC_GAP_VM_VALUE_SEMANTICS_001` |
| `VM_SEAM_SUBRANGE_001` | `bytecode_vm` | `wrong_result` | 3 | `wrong_type_or_shape` | `SPEC_GAP_VM_VALUE_SEMANTICS_001` |
| `VM_SEAM_VALID_001` | `bytecode_vm` | `silent_corruption` | 0 | `missing_required` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `VM_SEAM_VALID_001` | `bytecode_vm` | `silent_corruption` | 1 | `wrong_type_or_shape` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `VM_SEAM_VALID_001` | `bytecode_vm` | `silent_corruption` | 2 | `extra_or_unknown` | `SPEC_GAP_VM_ERROR_MODEL_001` |

## Bytecode/VM Pilot Gap Classification

Denominator: `open_spec_gaps_union_missing_required_runnable_test_classes`

- `test_gap`: 2
- `spec_gap`: 5
- `hardware_tool_blocked`: 0
- `not_applicable`: 0

| Gap | Classification | Source kind | Detail | Related records |
| --- | --- | --- | --- | --- |
| `SPEC_GAP_BYTECODE_VALIDATOR_001` | `spec_gap` | `spec_gap_record` | Which frontend and VM invariants must bytecode validation reject before a module can be applied? | `VM_SEAM_OWNER_001`, `VM_SEAM_REF_001`, `VM_SEAM_VALID_001` |
| `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001` | `spec_gap` | `spec_gap_record` | Which VM determinism, instruction, stack, local, reference, call-depth, and resource limits must be specified and tested independently of bytecode validator structure? | `VM_SEAM_DETERMINISM_LIMITS_001` |
| `SPEC_GAP_VM_ERROR_MODEL_001` | `spec_gap` | `spec_gap_record` | Which stable typed error identifiers must bytecode validation, runtime value conversion, and VM traps emit so tests do not match ad-hoc strings? | `VM_SEAM_DECLARED_TYPE_001`, `VM_SEAM_STRING_BOUND_001`, `VM_SEAM_SUBRANGE_001`, `VM_SEAM_VALID_001` |
| `SPEC_GAP_VM_LOWERING_FAIL_CLOSED_001` | `spec_gap` | `spec_gap_record` | Which HIR-clean but unencodable source constructs must fail compilation rather than lower to NOP or partial bytecode? | `VM_SEAM_ENC_001` |
| `SPEC_GAP_VM_VALUE_SEMANTICS_001` | `spec_gap` | `spec_gap_record` | What exact behavior applies to STRING[n] over-bound writes, unreviewed signed/unsigned and finite-range conversion edges, wrong-type subrange writes, and reference-store conversions? | `VM_SEAM_DECLARED_TYPE_001`, `VM_SEAM_STRING_BOUND_001`, `VM_SEAM_SUBRANGE_001` |
| `TEST_CLASS_GAP:bytecode_vm:iec_conformance` | `test_gap` | `required_test_class_slot` | Required test class iec_conformance has no catalog row. | none |
| `TEST_CLASS_GAP:bytecode_vm:metadata_validation` | `test_gap` | `required_test_class_slot` | Required test class metadata_validation has catalog rows but none are effectively runnable. | `TEST_CASE_TABLE_VM_SEAM_DECLARED_TYPE_001`, `TEST_CASE_TABLE_VM_SEAM_STRING_BOUND_001`, `TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001`, `TEST_CASE_TABLE_VM_SEAM_VALID_001` |

## Registered Public-Claim Context

Basis: `registered_spec_sources_only`. Exhaustive public-doc scan: `no`.

| Source | Area | Status | Surface | Invariants | Oracles | Spec gaps |
| --- | --- | --- | --- | --- | --- | --- |
| `PUBLIC_CLAIM_BEHAVIOR_LOCKED_001` | `release` | `active` | `README.md` | `DEBUG_BEHAVIOR_LOCKED_001`, `REL_CONF_001`, `RUNTIME_BEHAVIOR_LOCKED_001` | none | `SPEC_GAP_BEHAVIOR_LOCKED_PUBLIC_CLAIM_001`, `SPEC_GAP_CONFORMANCE_PUBLICATION_001` |
| `PUBLIC_CLAIM_RUNTIME_WIRE_001` | `protocols` | `active` | `README.md` | `PROTO_ADS_001`, `PROTO_DISCOVERY_TRUTH_001`, `PROTO_ETHERCAT_001`, `PROTO_MODBUS_001`, `PROTO_MQTT_001`, `PROTO_STATUS_TRUTH_001`, `REL_CLAIM_001`, `UI_STATUS_001` | none | `SPEC_GAP_ETHERCAT_UNAVAILABLE_RESOURCE_001`, `SPEC_GAP_HARDWARE_PUBLIC_CLAIM_001`, `SPEC_GAP_PROTOCOL_DISCOVERY_HANDSHAKE_001`, `SPEC_GAP_PROTOCOL_STATUS_MODEL_001`, `SPEC_GAP_PUBLIC_WIRE_CLAIM_001`, `SPEC_GAP_UI_STATUS_VOCABULARY_001` |
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
