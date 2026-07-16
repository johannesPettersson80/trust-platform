# Phase 6 Requirement and Oracle Audit

Generator: `requirement-oracle-audit v1`
Source revision: `16e332f091ff984b3e92ee343adbcb5b9ca23be1`
Generated: `2026-07-16T10:33:00+02:00`
Platform: `linux-x86_64`
Generated JSON SHA-256: `5c5df957e6b0fee3c2cdf3777702bb35d3fc0425c9c5df936941497931caebda`
Input SHA-256: `sha256:72c769547b6fd9a5827fa84fb19fa9c3cf6fb03fef8a3df0c9fee1548c19d2dd`

This is a report-only requirement/oracle association audit. It creates no
behavior proof, closes no specification gap, and enables no enforcement.
Its invariant denominator is all committed invariant records; public-claim
context is limited to the non-exhaustive registered source inventory.

## Summary

- Invariants: 53
- Phase 6 mapped invariants: 35
- Other-area invariants: 18
- Eligible oracles: 29
- Missing oracles: 24
- Future enforcement candidates: 17

## Mapping Groups

| Board row | Areas | Invariants | Eligible oracle | Spec-gap blocked |
| --- | --- | ---: | ---: | ---: |
| `VERIF-P6-001` | `compiler_iec` | 5 | 5 | 0 |
| `VERIF-P6-002` | `runtime_safety` | 11 | 11 | 0 |
| `VERIF-P6-003` | `protocols` | 7 | 1 | 6 |
| `VERIF-P6-004` | `editor_safety` | 6 | 5 | 1 |
| `VERIF-P6-005` | `control_security, supply_chain_platform` | 6 | 2 | 4 |

## Invariant Oracle Ledger

| Invariant | Area | Risk | Status | Oracle state | Oracle ref | Sources | Gaps |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `DEBUG_AUTH_001` | `control_security` | `security` | `implemented/G1` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_DEBUG_ADAPTER_001`, `SPEC_RUNTIME_ENGINE_001` | none |
| `DEBUG_BEHAVIOR_LOCKED_001` | `editor_safety` | `false_status` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_BEHAVIOR_LOCKED_PUBLIC_CLAIM_001` | `PUBLIC_CLAIM_BEHAVIOR_LOCKED_001`, `SPEC_DEBUG_ADAPTER_001` | `SPEC_GAP_BEHAVIOR_LOCKED_PUBLIC_CLAIM_001` |
| `DEBUG_PAUSE_001` | `editor_safety` | `safety_critical` | `implemented/G1` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_DEBUG_ADAPTER_001`, `SPEC_RUNTIME_ENGINE_001` | none |
| `DEV_COMMIT_SCOPE_001` | `plcopen_devtools` | `data_loss` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_DEV_COMMIT_SCOPE_001` | none | `SPEC_GAP_DEV_COMMIT_SCOPE_001` |
| `DEV_TEST_DISCOVERY_001` | `plcopen_devtools` | `false_status` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_DEV_TEST_DISCOVERY_CASE_001` | none | `SPEC_GAP_DEV_TEST_DISCOVERY_CASE_001` |
| `EDIT_DIAG_CANCEL_001` | `editor_safety` | `false_status` | `gap_open/S0` | `eligible_oracle` | `SPEC_LSP_CONTRACT_001` | `SPEC_LSP_CONTRACT_001` | none |
| `EDIT_LSP_POS_001` | `editor_safety` | `silent_corruption` | `gap_open/S0` | `eligible_oracle` | `SPEC_LSP_CONTRACT_001` | `SPEC_LSP_CONTRACT_001` | none |
| `EDIT_RENAME_001` | `editor_safety` | `silent_corruption` | `gap_open/S0` | `eligible_oracle` | `SPEC_LSP_CONTRACT_001` | `SPEC_LSP_CONTRACT_001` | none |
| `EDIT_RENAME_002` | `editor_safety` | `silent_corruption` | `gap_open/S0` | `eligible_oracle` | `SPEC_LSP_CONTRACT_001` | `SPEC_LSP_CONTRACT_001` | none |
| `IEC_PARSE_RECOVER_001` | `compiler_iec` | `silent_corruption` | `gap_open/S0` | `eligible_oracle` | `SPEC_IEC_DECISIONS_001` | `SPEC_IEC_DECISIONS_001` | none |
| `IEC_PREC_001` | `compiler_iec` | `wrong_result` | `gap_open/S0` | `eligible_oracle` | `SPEC_IEC_EXPRESSIONS_001` | `SPEC_IEC_EXPRESSIONS_001` | none |
| `IEC_STRING_001` | `compiler_iec` | `wrong_result` | `gap_open/S0` | `eligible_oracle` | `SPEC_IEC_DATA_TYPES_CANDIDATE_001` | `SPEC_IEC_DATA_TYPES_CANDIDATE_001`, `SPEC_IEC_DECISIONS_001` | none |
| `IEC_SUBRANGE_001` | `compiler_iec` | `wrong_result` | `gap_open/S0` | `eligible_oracle` | `SPEC_IEC_DECISIONS_001` | `SPEC_IEC_DECISIONS_001` | none |
| `IEC_TIMER_001` | `compiler_iec` | `safety_critical` | `implemented/G2` | `eligible_oracle` | `SPEC_IEC_STANDARD_FBS_CANDIDATE_001` | `SPEC_IEC_STANDARD_FBS_CANDIDATE_001`, `SPEC_IEC_DECISIONS_001` | none |
| `PLAT_PATH_001` | `supply_chain_platform` | `platform` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` | `PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001` | `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` |
| `PLAT_VSCODE_001` | `supply_chain_platform` | `compatibility` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` | `PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001` | `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` |
| `PLCO_IMPORT_001` | `plcopen_devtools` | `silent_corruption` | `gap_open/S0` | `eligible_oracle` | `SPEC_PLCOPEN_IMPORT_DECISION_001` | `SPEC_PLCOPEN_IMPORT_DECISION_001` | none |
| `PROTO_ADS_001` | `protocols` | `false_status` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_PROTOCOL_STATUS_MODEL_001` | `PUBLIC_CLAIM_RUNTIME_WIRE_001` | `SPEC_GAP_PROTOCOL_STATUS_MODEL_001` |
| `PROTO_DISCOVERY_TRUTH_001` | `protocols` | `false_status` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_PUBLIC_WIRE_CLAIM_001` | `PUBLIC_CLAIM_RUNTIME_WIRE_001` | `SPEC_GAP_PUBLIC_WIRE_CLAIM_001` |
| `PROTO_ETHERCAT_001` | `protocols` | `false_status` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_ETHERCAT_UNAVAILABLE_RESOURCE_001` | `PUBLIC_CLAIM_RUNTIME_WIRE_001` | `SPEC_GAP_ETHERCAT_UNAVAILABLE_RESOURCE_001` |
| `PROTO_MODBUS_001` | `protocols` | `false_status` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_PROTOCOL_DISCOVERY_HANDSHAKE_001` | `PUBLIC_CLAIM_RUNTIME_WIRE_001` | `SPEC_GAP_PROTOCOL_DISCOVERY_HANDSHAKE_001` |
| `PROTO_MQTT_001` | `protocols` | `false_status` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_PROTOCOL_DISCOVERY_HANDSHAKE_001` | `PUBLIC_CLAIM_RUNTIME_WIRE_001` | `SPEC_GAP_PROTOCOL_DISCOVERY_HANDSHAKE_001` |
| `PROTO_OPCUA_001` | `protocols` | `false_status` | `gap_open/S0` | `eligible_oracle` | `SPEC_OPCUA_CLIENT_LIFECYCLE_DECISION_001` | `SPEC_OPCUA_CLIENT_LIFECYCLE_DECISION_001` | none |
| `PROTO_STATUS_TRUTH_001` | `protocols` | `false_status` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_PUBLIC_WIRE_CLAIM_001` | `PUBLIC_CLAIM_RUNTIME_WIRE_001` | `SPEC_GAP_PUBLIC_WIRE_CLAIM_001` |
| `RELEASE_PLATFORM_MATRIX_001` | `release` | `compatibility` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` | `PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001` | `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` |
| `RELEASE_SOURCE_BUILD_OPENOT_001` | `release` | `compatibility` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_SOURCE_BUILD_PUBLIC_CLAIM_001` | `PUBLIC_CLAIM_SOURCE_BUILD_RUNTIME_001` | `SPEC_GAP_SOURCE_BUILD_PUBLIC_CLAIM_001` |
| `REL_CLAIM_001` | `release` | `false_status` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_HARDWARE_PUBLIC_CLAIM_001` | `PUBLIC_CLAIM_RUNTIME_WIRE_001`, `PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001` | `SPEC_GAP_HARDWARE_PUBLIC_CLAIM_001` |
| `REL_CONF_001` | `release` | `false_status` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_CONFORMANCE_PUBLICATION_001` | `PUBLIC_CLAIM_BEHAVIOR_LOCKED_001` | `SPEC_GAP_CONFORMANCE_PUBLICATION_001` |
| `REL_VERSION_001` | `release` | `false_status` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_RELEASE_VERSION_CHAIN_001` | `PUBLIC_CLAIM_SOURCE_BUILD_RUNTIME_001`, `PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001` | `SPEC_GAP_RELEASE_VERSION_CHAIN_001` |
| `RT_RELOAD_001` | `runtime_safety` | `silent_corruption` | `implemented/G2` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_DEBUG_ADAPTER_001` | none |
| `RT_SAFE_DEADLINE_001` | `runtime_safety` | `safety_critical` | `implemented/G2` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_RUNTIME_SAFETY_FAIL_CLOSED_BOARD_001` | none |
| `RT_SAFE_FORCE_001` | `runtime_safety` | `safety_critical` | `implemented/G1` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_DEBUG_ADAPTER_001` | none |
| `RT_SAFE_IO_001` | `runtime_safety` | `safety_critical` | `gap_open/S0` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001` | none |
| `RT_SAFE_IO_WORKER_001` | `runtime_safety` | `safety_critical` | `implemented/G2` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001` | none |
| `RT_SAFE_NAN_001` | `runtime_safety` | `safety_critical` | `implemented/G2` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_RUNTIME_SEMANTICS_001` | none |
| `RT_SAFE_PANIC_001` | `runtime_safety` | `safety_critical` | `implemented/G1` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_RUNTIME_SAFETY_FAIL_CLOSED_BOARD_001` | none |
| `RT_SAFE_RESTART_001` | `runtime_safety` | `wrong_result` | `gap_open/S0` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_RUNTIME_SEMANTICS_001` | none |
| `RT_SAFE_RESTART_TIME_002` | `runtime_safety` | `safety_critical` | `implemented/G2` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_RUNTIME_SEMANTICS_001`, `SPEC_IEC_DECISIONS_001` | none |
| `RT_SAFE_RETAIN_001` | `runtime_safety` | `data_loss` | `implemented/G2` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_RUNTIME_SEMANTICS_001` | none |
| `RT_SAFE_STOP_001` | `runtime_safety` | `safety_critical` | `implemented/G2` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001` | none |
| `RUNTIME_BEHAVIOR_LOCKED_001` | `release` | `false_status` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_BEHAVIOR_LOCKED_PUBLIC_CLAIM_001` | `PUBLIC_CLAIM_BEHAVIOR_LOCKED_001` | `SPEC_GAP_BEHAVIOR_LOCKED_PUBLIC_CLAIM_001` |
| `SEC_ARTIFACT_001` | `supply_chain_platform` | `supply_chain` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_ARTIFACT_PROVENANCE_001` | `PUBLIC_CLAIM_SOURCE_BUILD_RUNTIME_001`, `PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001` | `SPEC_GAP_ARTIFACT_PROVENANCE_001` |
| `SEC_AUTHZ_001` | `control_security` | `security` | `implemented/G1` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_DEBUG_ADAPTER_001` | none |
| `SEC_DEP_AUDIT_001` | `supply_chain_platform` | `supply_chain` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_DEPENDENCY_AUDIT_POLICY_001` | `PUBLIC_CLAIM_SOURCE_BUILD_RUNTIME_001` | `SPEC_GAP_DEPENDENCY_AUDIT_POLICY_001` |
| `UI_STATUS_001` | `hmi_ui` | `false_status` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_UI_STATUS_VOCABULARY_001` | `PUBLIC_CLAIM_RUNTIME_WIRE_001` | `SPEC_GAP_UI_STATUS_VOCABULARY_001` |
| `VM_SEAM_DECLARED_TYPE_001` | `bytecode_vm` | `wrong_result` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_VM_ERROR_MODEL_001` | `SPEC_VM_VALUE_SEMANTICS_001` | `SPEC_GAP_VM_ERROR_MODEL_001` |
| `VM_SEAM_DETERMINISM_LIMITS_001` | `bytecode_vm` | `wrong_result` | `implemented/G2` | `eligible_oracle` | `SPEC_BYTECODE_FORMAT_001` | `SPEC_BYTECODE_FORMAT_001`, `SPEC_RUNTIME_SEMANTICS_001` | none |
| `VM_SEAM_ENC_001` | `bytecode_vm` | `silent_corruption` | `gap_open/S0` | `eligible_oracle` | `SPEC_BYTECODE_FORMAT_001` | `SPEC_BYTECODE_FORMAT_001` | none |
| `VM_SEAM_OWNER_001` | `bytecode_vm` | `silent_corruption` | `gap_open/S0` | `eligible_oracle` | `SPEC_BYTECODE_FORMAT_001` | `SPEC_BYTECODE_FORMAT_001` | none |
| `VM_SEAM_REF_001` | `bytecode_vm` | `silent_corruption` | `gap_open/S0` | `eligible_oracle` | `SPEC_BYTECODE_FORMAT_001` | `SPEC_BYTECODE_FORMAT_001` | none |
| `VM_SEAM_STRING_BOUND_001` | `bytecode_vm` | `wrong_result` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_VM_ERROR_MODEL_001` | `SPEC_VM_VALUE_SEMANTICS_001` | `SPEC_GAP_VM_ERROR_MODEL_001` |
| `VM_SEAM_SUBRANGE_001` | `bytecode_vm` | `wrong_result` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_VM_ERROR_MODEL_001` | `SPEC_VM_VALUE_SEMANTICS_001` | `SPEC_GAP_VM_ERROR_MODEL_001` |
| `VM_SEAM_VALID_001` | `bytecode_vm` | `silent_corruption` | `spec_gap/S0` | `spec_gap_blocked` | `SPEC_GAP_VM_ERROR_MODEL_001` | `SPEC_BYTECODE_FORMAT_001` | `SPEC_GAP_VM_ERROR_MODEL_001` |

## Missing Oracles

| Invariant | Risk | Gap | Future enforcement candidate |
| --- | --- | --- | --- |
| `DEBUG_BEHAVIOR_LOCKED_001` | `false_status` | `SPEC_GAP_BEHAVIOR_LOCKED_PUBLIC_CLAIM_001` | `true` |
| `DEV_COMMIT_SCOPE_001` | `data_loss` | `SPEC_GAP_DEV_COMMIT_SCOPE_001` | `false` |
| `DEV_TEST_DISCOVERY_001` | `false_status` | `SPEC_GAP_DEV_TEST_DISCOVERY_CASE_001` | `true` |
| `PLAT_PATH_001` | `platform` | `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` | `false` |
| `PLAT_VSCODE_001` | `compatibility` | `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` | `false` |
| `PROTO_ADS_001` | `false_status` | `SPEC_GAP_PROTOCOL_STATUS_MODEL_001` | `true` |
| `PROTO_DISCOVERY_TRUTH_001` | `false_status` | `SPEC_GAP_PUBLIC_WIRE_CLAIM_001` | `true` |
| `PROTO_ETHERCAT_001` | `false_status` | `SPEC_GAP_ETHERCAT_UNAVAILABLE_RESOURCE_001` | `true` |
| `PROTO_MODBUS_001` | `false_status` | `SPEC_GAP_PROTOCOL_DISCOVERY_HANDSHAKE_001` | `true` |
| `PROTO_MQTT_001` | `false_status` | `SPEC_GAP_PROTOCOL_DISCOVERY_HANDSHAKE_001` | `true` |
| `PROTO_STATUS_TRUTH_001` | `false_status` | `SPEC_GAP_PUBLIC_WIRE_CLAIM_001` | `true` |
| `RELEASE_PLATFORM_MATRIX_001` | `compatibility` | `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` | `false` |
| `RELEASE_SOURCE_BUILD_OPENOT_001` | `compatibility` | `SPEC_GAP_SOURCE_BUILD_PUBLIC_CLAIM_001` | `false` |
| `REL_CLAIM_001` | `false_status` | `SPEC_GAP_HARDWARE_PUBLIC_CLAIM_001` | `true` |
| `REL_CONF_001` | `false_status` | `SPEC_GAP_CONFORMANCE_PUBLICATION_001` | `true` |
| `REL_VERSION_001` | `false_status` | `SPEC_GAP_RELEASE_VERSION_CHAIN_001` | `true` |
| `RUNTIME_BEHAVIOR_LOCKED_001` | `false_status` | `SPEC_GAP_BEHAVIOR_LOCKED_PUBLIC_CLAIM_001` | `true` |
| `SEC_ARTIFACT_001` | `supply_chain` | `SPEC_GAP_ARTIFACT_PROVENANCE_001` | `false` |
| `SEC_DEP_AUDIT_001` | `supply_chain` | `SPEC_GAP_DEPENDENCY_AUDIT_POLICY_001` | `false` |
| `UI_STATUS_001` | `false_status` | `SPEC_GAP_UI_STATUS_VOCABULARY_001` | `true` |
| `VM_SEAM_DECLARED_TYPE_001` | `wrong_result` | `SPEC_GAP_VM_ERROR_MODEL_001` | `true` |
| `VM_SEAM_STRING_BOUND_001` | `wrong_result` | `SPEC_GAP_VM_ERROR_MODEL_001` | `true` |
| `VM_SEAM_SUBRANGE_001` | `wrong_result` | `SPEC_GAP_VM_ERROR_MODEL_001` | `true` |
| `VM_SEAM_VALID_001` | `silent_corruption` | `SPEC_GAP_VM_ERROR_MODEL_001` | `true` |

## Boundaries

- `audit_creates_proof`: `false`
- `audit_closes_spec_gaps`: `false`
- `missing_oracle_enforcement_enabled`: `false`
- `public_claims_are_oracles`: `false`
- `public_docs_inventory_exhaustive`: `false`
- `forward_traceability_complete`: `false`
- `reverse_traceability_complete`: `false`
- `p4a_005_public_claim_inventory_remains_open`: `true`
- `p6_007_enforcement_remains_open`: `true`
- `p6_008_to_p6_010_remain_open`: `true`
- `p14_000_grace_rule_remains_open`: `true`

## Limitations

- Mappings come only from explicit invariant spec/oracle references; names, paths, and prose do not create mappings.
- An eligible source listed as context does not replace an invariant's explicit open spec-gap oracle.
- Public claims are provenance and proof obligations, never behavior oracles.
- Product contracts and reviewed IEC decisions or deviations are not external IEC conformance proof; the registered external IEC source remains non-oracle and its ignored local bytes are not provenance inputs.
- The separate specification-source audit completes mechanical document discovery; semantic classification and conflict review remain incomplete under VERIF-P1A-003 and VERIF-P1A-006.
- This report's public-claim view is registered-spec-sources-only; the separate audit has an exhaustive prose-block denominator, but semantic claim review remains incomplete while VERIF-P4A-005 is open.
- Invariant test, gate, and evidence IDs are copied explicit associations, not a completed forward trace; referenced metadata is live-validated at rest.
- verification/evidence-index.toml is excluded from the input digest to avoid a report-evidence digest cycle.
- Missing-oracle debt is report-only until VERIF-P14-000 defines the grace period required by VERIF-P6-007.
- Forward, reverse, and orphan traceability remain outside this slice under VERIF-P6-008 through VERIF-P6-010.
- The blocked-row posture is checked live from the implementation board, which is excluded from the digest because board and evidence closure follow report generation.
- The report creates no proof, closes no specification gap, and changes no runtime or product behavior.
