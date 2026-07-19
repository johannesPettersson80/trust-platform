# Phase 6 Requirement and Oracle Audit

Generator: `requirement-oracle-audit v2`
Source revision: `22fab65fe3ab7b31bb610d88daa5561f2642cee7`
Generated: `2026-07-19T08:00:00+02:00`
Platform: `linux-x86_64`
Generated JSON SHA-256: `ef906d5d587b6dc5febcc5e9eda1c68592ac781ab7199da362010a0de7e8c96c`
Input SHA-256: `sha256:1f9ee71b72f21b93d6909be56490536322ff1f4339a6a0206bb34dbd9ea71b94`

This is a requirement/oracle and explicit traceability audit. It creates no
behavior proof and closes no specification gap. Its invariant denominator is
all committed invariant records; its public-claim denominator is the complete
registered claim inventory, not all public prose.

## Summary

- Invariants: 55
- Phase 6 mapped invariants: 37
- Other-area invariants: 18
- Eligible oracles: 55
- Missing oracles: 0
- Future enforcement candidates: 0
- Complete forward chains to evidence: 55/55
- Linked registered public claims: 4/4

## Mapping Groups

| Board row | Areas | Invariants | Eligible oracle | Spec-gap blocked |
| --- | --- | ---: | ---: | ---: |
| `VERIF-P6-001` | `compiler_iec` | 5 | 5 | 0 |
| `VERIF-P6-002` | `runtime_safety` | 11 | 11 | 0 |
| `VERIF-P6-003` | `protocols` | 7 | 7 | 0 |
| `VERIF-P6-004` | `editor_safety` | 8 | 8 | 0 |
| `VERIF-P6-005` | `control_security, supply_chain_platform` | 6 | 6 | 0 |

## Invariant Oracle Ledger

| Invariant | Area | Risk | Status | Oracle state | Oracle ref | Sources | Gaps |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `DEBUG_AUTH_001` | `control_security` | `security` | `implemented/G1` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_DEBUG_ADAPTER_001`, `SPEC_RUNTIME_ENGINE_001` | none |
| `DEBUG_BEHAVIOR_LOCKED_001` | `editor_safety` | `false_status` | `validated/G1` | `eligible_oracle` | `SPEC_RELEASE_EVIDENCE_001` | `SPEC_RELEASE_EVIDENCE_001`, `PUBLIC_CLAIM_BEHAVIOR_LOCKED_001`, `SPEC_DEBUG_ADAPTER_001` | none |
| `DEBUG_PAUSE_001` | `editor_safety` | `safety_critical` | `implemented/G1` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_DEBUG_ADAPTER_001`, `SPEC_RUNTIME_ENGINE_001` | none |
| `DEV_COMMIT_SCOPE_001` | `plcopen_devtools` | `data_loss` | `implemented/G1` | `eligible_oracle` | `SPEC_DEVELOPER_WORKFLOWS_001` | `SPEC_DEVELOPER_WORKFLOWS_001` | none |
| `DEV_TEST_DISCOVERY_001` | `plcopen_devtools` | `false_status` | `implemented/G1` | `eligible_oracle` | `SPEC_DEVELOPER_WORKFLOWS_001` | `SPEC_DEVELOPER_WORKFLOWS_001` | none |
| `EDIT_DIAG_CANCEL_001` | `editor_safety` | `false_status` | `implemented/G1` | `eligible_oracle` | `SPEC_LSP_CONTRACT_001` | `SPEC_LSP_CONTRACT_001` | none |
| `EDIT_DOC_CLOSE_001` | `editor_safety` | `silent_corruption` | `implemented/G1` | `eligible_oracle` | `SPEC_LSP_CONTRACT_001` | `SPEC_LSP_CONTRACT_001` | none |
| `EDIT_LSP_DELIVERY_001` | `editor_safety` | `false_status` | `implemented/G1` | `eligible_oracle` | `SPEC_LSP_CONTRACT_001#progressive-results-and-interactive-latency` | `SPEC_LSP_CONTRACT_001` | none |
| `EDIT_LSP_POS_001` | `editor_safety` | `silent_corruption` | `implemented/G1` | `eligible_oracle` | `SPEC_LSP_CONTRACT_001` | `SPEC_LSP_CONTRACT_001` | none |
| `EDIT_RENAME_001` | `editor_safety` | `silent_corruption` | `implemented/G1` | `eligible_oracle` | `SPEC_LSP_CONTRACT_001` | `SPEC_LSP_CONTRACT_001` | none |
| `EDIT_RENAME_002` | `editor_safety` | `silent_corruption` | `implemented/G1` | `eligible_oracle` | `SPEC_LSP_CONTRACT_001` | `SPEC_LSP_CONTRACT_001` | none |
| `IEC_PARSE_RECOVER_001` | `compiler_iec` | `silent_corruption` | `implemented/G1` | `eligible_oracle` | `SPEC_IEC_DECISIONS_001` | `SPEC_IEC_DECISIONS_001` | none |
| `IEC_PREC_001` | `compiler_iec` | `wrong_result` | `implemented/G1` | `eligible_oracle` | `SPEC_IEC_EXPRESSIONS_001` | `SPEC_IEC_EXPRESSIONS_001` | none |
| `IEC_STRING_001` | `compiler_iec` | `wrong_result` | `implemented/G1` | `eligible_oracle` | `SPEC_IEC_DATA_TYPES_CANDIDATE_001` | `SPEC_IEC_DATA_TYPES_CANDIDATE_001`, `SPEC_IEC_DECISIONS_001` | none |
| `IEC_SUBRANGE_001` | `compiler_iec` | `wrong_result` | `implemented/G1` | `eligible_oracle` | `SPEC_IEC_DECISIONS_001` | `SPEC_IEC_DECISIONS_001` | none |
| `IEC_TIMER_001` | `compiler_iec` | `safety_critical` | `implemented/G2` | `eligible_oracle` | `SPEC_IEC_STANDARD_FBS_CANDIDATE_001` | `SPEC_IEC_STANDARD_FBS_CANDIDATE_001`, `SPEC_IEC_DECISIONS_001` | none |
| `PLAT_PATH_001` | `supply_chain_platform` | `platform` | `implemented/G1` | `eligible_oracle` | `SPEC_RELEASE_EVIDENCE_001` | `SPEC_RELEASE_EVIDENCE_001`, `PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001` | none |
| `PLAT_VSCODE_001` | `supply_chain_platform` | `compatibility` | `implemented/G1` | `eligible_oracle` | `SPEC_RELEASE_EVIDENCE_001` | `SPEC_RELEASE_EVIDENCE_001`, `PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001` | none |
| `PLCO_IMPORT_001` | `plcopen_devtools` | `silent_corruption` | `implemented/G1` | `eligible_oracle` | `SPEC_PLCOPEN_IMPORT_DECISION_001` | `SPEC_PLCOPEN_IMPORT_DECISION_001` | none |
| `PROTO_ADS_001` | `protocols` | `false_status` | `implemented/G1` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001` | none |
| `PROTO_DISCOVERY_TRUTH_001` | `protocols` | `false_status` | `implemented/G1` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001` | none |
| `PROTO_ETHERCAT_001` | `protocols` | `false_status` | `implemented/G1` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001` | none |
| `PROTO_MODBUS_001` | `protocols` | `false_status` | `implemented/G1` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001` | none |
| `PROTO_MQTT_001` | `protocols` | `false_status` | `implemented/G1` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001` | none |
| `PROTO_OPCUA_001` | `protocols` | `false_status` | `implemented/G1` | `eligible_oracle` | `SPEC_OPCUA_CLIENT_LIFECYCLE_DECISION_001` | `SPEC_OPCUA_CLIENT_LIFECYCLE_DECISION_001` | none |
| `PROTO_STATUS_TRUTH_001` | `protocols` | `false_status` | `implemented/G1` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001` | none |
| `RELEASE_PLATFORM_MATRIX_001` | `release` | `compatibility` | `validated/G1` | `eligible_oracle` | `SPEC_RELEASE_EVIDENCE_001` | `SPEC_RELEASE_EVIDENCE_001`, `PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001` | none |
| `RELEASE_SOURCE_BUILD_OPENOT_001` | `release` | `compatibility` | `validated/G1` | `eligible_oracle` | `SPEC_RELEASE_EVIDENCE_001` | `SPEC_RELEASE_EVIDENCE_001`, `PUBLIC_CLAIM_SOURCE_BUILD_RUNTIME_001` | none |
| `REL_CLAIM_001` | `release` | `false_status` | `validated/G1` | `eligible_oracle` | `SPEC_RELEASE_EVIDENCE_001` | `SPEC_RELEASE_EVIDENCE_001`, `PUBLIC_CLAIM_RUNTIME_WIRE_001`, `PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001` | none |
| `REL_CONF_001` | `release` | `false_status` | `implemented/G1` | `eligible_oracle` | `SPEC_RELEASE_EVIDENCE_001` | `SPEC_RELEASE_EVIDENCE_001` | none |
| `REL_VERSION_001` | `release` | `false_status` | `implemented/G1` | `eligible_oracle` | `SPEC_RELEASE_EVIDENCE_001` | `SPEC_RELEASE_EVIDENCE_001` | none |
| `RT_RELOAD_001` | `runtime_safety` | `silent_corruption` | `implemented/G2` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_DEBUG_ADAPTER_001` | none |
| `RT_SAFE_DEADLINE_001` | `runtime_safety` | `safety_critical` | `implemented/G1` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_RUNTIME_SAFETY_FAIL_CLOSED_BOARD_001` | none |
| `RT_SAFE_FORCE_001` | `runtime_safety` | `safety_critical` | `implemented/G1` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_DEBUG_ADAPTER_001` | none |
| `RT_SAFE_IO_001` | `runtime_safety` | `safety_critical` | `implemented/G1` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001` | none |
| `RT_SAFE_IO_WORKER_001` | `runtime_safety` | `safety_critical` | `implemented/G2` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001` | none |
| `RT_SAFE_NAN_001` | `runtime_safety` | `safety_critical` | `implemented/G1` | `eligible_oracle` | `SPEC_RUNTIME_SEMANTICS_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_RUNTIME_SEMANTICS_001` | none |
| `RT_SAFE_PANIC_001` | `runtime_safety` | `safety_critical` | `implemented/G1` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_RUNTIME_SAFETY_FAIL_CLOSED_BOARD_001` | none |
| `RT_SAFE_RESTART_001` | `runtime_safety` | `wrong_result` | `implemented/G1` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_RUNTIME_SEMANTICS_001` | none |
| `RT_SAFE_RESTART_TIME_002` | `runtime_safety` | `safety_critical` | `implemented/G2` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_RUNTIME_SEMANTICS_001`, `SPEC_IEC_DECISIONS_001` | none |
| `RT_SAFE_RETAIN_001` | `runtime_safety` | `data_loss` | `implemented/G2` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_RUNTIME_SEMANTICS_001` | none |
| `RT_SAFE_STOP_001` | `runtime_safety` | `safety_critical` | `implemented/G2` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001` | none |
| `RUNTIME_BEHAVIOR_LOCKED_001` | `release` | `false_status` | `validated/G1` | `eligible_oracle` | `SPEC_RELEASE_EVIDENCE_001` | `SPEC_RELEASE_EVIDENCE_001`, `PUBLIC_CLAIM_BEHAVIOR_LOCKED_001` | none |
| `SEC_ARTIFACT_001` | `supply_chain_platform` | `supply_chain` | `implemented/G1` | `eligible_oracle` | `SPEC_RELEASE_EVIDENCE_001` | `SPEC_RELEASE_EVIDENCE_001` | none |
| `SEC_AUTHZ_001` | `control_security` | `security` | `implemented/G1` | `eligible_oracle` | `SPEC_RUNTIME_ENGINE_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_DEBUG_ADAPTER_001` | none |
| `SEC_DEP_AUDIT_001` | `supply_chain_platform` | `supply_chain` | `implemented/G1` | `eligible_oracle` | `SPEC_RELEASE_EVIDENCE_001` | `SPEC_RELEASE_EVIDENCE_001` | none |
| `UI_STATUS_001` | `hmi_ui` | `false_status` | `implemented/G1` | `eligible_oracle` | `SPEC_CONNECTOR_STATUS_001` | `SPEC_CONNECTOR_STATUS_001`, `PUBLIC_CLAIM_RUNTIME_WIRE_001` | none |
| `VM_SEAM_DECLARED_TYPE_001` | `bytecode_vm` | `wrong_result` | `implemented/G1` | `eligible_oracle` | `SPEC_VM_VALUE_SEMANTICS_001` | `SPEC_VM_VALUE_SEMANTICS_001` | none |
| `VM_SEAM_DETERMINISM_LIMITS_001` | `bytecode_vm` | `wrong_result` | `implemented/G2` | `eligible_oracle` | `SPEC_BYTECODE_FORMAT_001` | `SPEC_BYTECODE_FORMAT_001`, `SPEC_RUNTIME_SEMANTICS_001` | none |
| `VM_SEAM_ENC_001` | `bytecode_vm` | `silent_corruption` | `implemented/G1` | `eligible_oracle` | `SPEC_BYTECODE_FORMAT_001` | `SPEC_BYTECODE_FORMAT_001` | none |
| `VM_SEAM_OWNER_001` | `bytecode_vm` | `silent_corruption` | `implemented/G1` | `eligible_oracle` | `SPEC_BYTECODE_FORMAT_001` | `SPEC_BYTECODE_FORMAT_001` | none |
| `VM_SEAM_REF_001` | `bytecode_vm` | `silent_corruption` | `implemented/G1` | `eligible_oracle` | `SPEC_BYTECODE_FORMAT_001` | `SPEC_BYTECODE_FORMAT_001` | none |
| `VM_SEAM_STRING_BOUND_001` | `bytecode_vm` | `wrong_result` | `implemented/G1` | `eligible_oracle` | `SPEC_VM_VALUE_SEMANTICS_001` | `SPEC_VM_VALUE_SEMANTICS_001` | none |
| `VM_SEAM_SUBRANGE_001` | `bytecode_vm` | `wrong_result` | `implemented/G1` | `eligible_oracle` | `SPEC_VM_VALUE_SEMANTICS_001` | `SPEC_VM_VALUE_SEMANTICS_001` | none |
| `VM_SEAM_VALID_001` | `bytecode_vm` | `silent_corruption` | `implemented/G1` | `eligible_oracle` | `SPEC_BYTECODE_FORMAT_001` | `SPEC_BYTECODE_FORMAT_001` | none |

## Missing Oracles

| Invariant | Risk | Gap | Future enforcement candidate |
| --- | --- | --- | --- |

## Forward Traceability

| Invariant | Sources | Tests | Suites | Evidence | Public claims | Missing links |
| --- | --- | ---: | --- | ---: | --- | --- |
| `DEBUG_AUTH_001` | `SPEC_DEBUG_ADAPTER_001`, `SPEC_RUNTIME_ENGINE_001` | 2 | `pr` | 9 | none | none |
| `DEBUG_BEHAVIOR_LOCKED_001` | `SPEC_RELEASE_EVIDENCE_001`, `PUBLIC_CLAIM_BEHAVIOR_LOCKED_001`, `SPEC_DEBUG_ADAPTER_001` | 1 | `pr` | 6 | `PUBLIC_CLAIM_BEHAVIOR_LOCKED_001` | none |
| `DEBUG_PAUSE_001` | `SPEC_DEBUG_ADAPTER_001`, `SPEC_RUNTIME_ENGINE_001` | 3 | `pr` | 8 | none | none |
| `DEV_COMMIT_SCOPE_001` | `SPEC_DEVELOPER_WORKFLOWS_001` | 3 | `pr` | 5 | none | none |
| `DEV_TEST_DISCOVERY_001` | `SPEC_DEVELOPER_WORKFLOWS_001` | 1 | `pr` | 4 | none | none |
| `EDIT_DIAG_CANCEL_001` | `SPEC_LSP_CONTRACT_001` | 4 | `pr` | 5 | none | none |
| `EDIT_DOC_CLOSE_001` | `SPEC_LSP_CONTRACT_001` | 3 | `pr` | 7 | none | none |
| `EDIT_LSP_DELIVERY_001` | `SPEC_LSP_CONTRACT_001` | 4 | `pr` | 2 | none | none |
| `EDIT_LSP_POS_001` | `SPEC_LSP_CONTRACT_001` | 9 | `pr` | 6 | none | none |
| `EDIT_RENAME_001` | `SPEC_LSP_CONTRACT_001` | 7 | `pr` | 4 | none | none |
| `EDIT_RENAME_002` | `SPEC_LSP_CONTRACT_001` | 5 | `pr` | 5 | none | none |
| `IEC_PARSE_RECOVER_001` | `SPEC_IEC_DECISIONS_001` | 8 | `veryquick`, `pr` | 4 | none | none |
| `IEC_PREC_001` | `SPEC_IEC_EXPRESSIONS_001` | 1 | `pr` | 5 | none | none |
| `IEC_STRING_001` | `SPEC_IEC_DATA_TYPES_CANDIDATE_001`, `SPEC_IEC_DECISIONS_001` | 21 | `pr` | 5 | none | none |
| `IEC_SUBRANGE_001` | `SPEC_IEC_DECISIONS_001` | 2 | `pr` | 4 | none | none |
| `IEC_TIMER_001` | `SPEC_IEC_STANDARD_FBS_CANDIDATE_001`, `SPEC_IEC_DECISIONS_001` | 8 | `pr` | 9 | none | none |
| `PLAT_PATH_001` | `SPEC_RELEASE_EVIDENCE_001`, `PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001` | 1 | `pr` | 5 | `PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001` | none |
| `PLAT_VSCODE_001` | `SPEC_RELEASE_EVIDENCE_001`, `PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001` | 1 | `pr` | 5 | `PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001` | none |
| `PLCO_IMPORT_001` | `SPEC_PLCOPEN_IMPORT_DECISION_001` | 1 | `pr` | 5 | none | none |
| `PROTO_ADS_001` | `SPEC_RUNTIME_ENGINE_001` | 1 | `pr` | 7 | none | none |
| `PROTO_DISCOVERY_TRUTH_001` | `SPEC_RUNTIME_ENGINE_001` | 1 | `pr`, `hardware_lab` | 7 | none | none |
| `PROTO_ETHERCAT_001` | `SPEC_RUNTIME_ENGINE_001` | 1 | `pr` | 6 | none | none |
| `PROTO_MODBUS_001` | `SPEC_RUNTIME_ENGINE_001` | 1 | `pr` | 7 | none | none |
| `PROTO_MQTT_001` | `SPEC_RUNTIME_ENGINE_001` | 1 | `pr` | 7 | none | none |
| `PROTO_OPCUA_001` | `SPEC_OPCUA_CLIENT_LIFECYCLE_DECISION_001` | 3 | `pr` | 11 | none | none |
| `PROTO_STATUS_TRUTH_001` | `SPEC_RUNTIME_ENGINE_001` | 2 | `pr`, `hardware_lab` | 7 | none | none |
| `RELEASE_PLATFORM_MATRIX_001` | `SPEC_RELEASE_EVIDENCE_001`, `PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001` | 1 | `pr` | 5 | `PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001` | none |
| `RELEASE_SOURCE_BUILD_OPENOT_001` | `SPEC_RELEASE_EVIDENCE_001`, `PUBLIC_CLAIM_SOURCE_BUILD_RUNTIME_001` | 1 | `pr` | 7 | `PUBLIC_CLAIM_SOURCE_BUILD_RUNTIME_001` | none |
| `REL_CLAIM_001` | `SPEC_RELEASE_EVIDENCE_001`, `PUBLIC_CLAIM_RUNTIME_WIRE_001`, `PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001` | 1 | `pr` | 5 | `PUBLIC_CLAIM_RUNTIME_WIRE_001`, `PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001` | none |
| `REL_CONF_001` | `SPEC_RELEASE_EVIDENCE_001` | 1 | `pr` | 4 | none | none |
| `REL_VERSION_001` | `SPEC_RELEASE_EVIDENCE_001` | 1 | `pr` | 4 | none | none |
| `RT_RELOAD_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_DEBUG_ADAPTER_001` | 1 | `pr` | 5 | none | none |
| `RT_SAFE_DEADLINE_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_RUNTIME_SAFETY_FAIL_CLOSED_BOARD_001` | 3 | `pr` | 5 | none | none |
| `RT_SAFE_FORCE_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_DEBUG_ADAPTER_001` | 7 | `pr` | 10 | none | none |
| `RT_SAFE_IO_001` | `SPEC_RUNTIME_ENGINE_001` | 5 | `nightly`, `pr` | 4 | none | none |
| `RT_SAFE_IO_WORKER_001` | `SPEC_RUNTIME_ENGINE_001` | 4 | `pr` | 5 | none | none |
| `RT_SAFE_NAN_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_RUNTIME_SEMANTICS_001` | 23 | `pr` | 9 | none | none |
| `RT_SAFE_PANIC_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_RUNTIME_SAFETY_FAIL_CLOSED_BOARD_001` | 5 | `pr` | 4 | none | none |
| `RT_SAFE_RESTART_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_RUNTIME_SEMANTICS_001` | 6 | `pr` | 4 | none | none |
| `RT_SAFE_RESTART_TIME_002` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_RUNTIME_SEMANTICS_001`, `SPEC_IEC_DECISIONS_001` | 1 | `pr` | 3 | none | none |
| `RT_SAFE_RETAIN_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_RUNTIME_SEMANTICS_001` | 1 | `pr` | 6 | none | none |
| `RT_SAFE_STOP_001` | `SPEC_RUNTIME_ENGINE_001` | 4 | `pr` | 5 | none | none |
| `RUNTIME_BEHAVIOR_LOCKED_001` | `SPEC_RELEASE_EVIDENCE_001`, `PUBLIC_CLAIM_BEHAVIOR_LOCKED_001` | 1 | `pr` | 6 | `PUBLIC_CLAIM_BEHAVIOR_LOCKED_001` | none |
| `SEC_ARTIFACT_001` | `SPEC_RELEASE_EVIDENCE_001` | 1 | `pr` | 4 | none | none |
| `SEC_AUTHZ_001` | `SPEC_RUNTIME_ENGINE_001`, `SPEC_DEBUG_ADAPTER_001` | 3 | `pr` | 9 | none | none |
| `SEC_DEP_AUDIT_001` | `SPEC_RELEASE_EVIDENCE_001` | 1 | `pr` | 4 | none | none |
| `UI_STATUS_001` | `SPEC_CONNECTOR_STATUS_001`, `PUBLIC_CLAIM_RUNTIME_WIRE_001` | 4 | `pr` | 5 | `PUBLIC_CLAIM_RUNTIME_WIRE_001` | none |
| `VM_SEAM_DECLARED_TYPE_001` | `SPEC_VM_VALUE_SEMANTICS_001` | 43 | `veryquick`, `pr` | 13 | none | none |
| `VM_SEAM_DETERMINISM_LIMITS_001` | `SPEC_BYTECODE_FORMAT_001`, `SPEC_RUNTIME_SEMANTICS_001` | 11 | `veryquick`, `pr` | 9 | none | none |
| `VM_SEAM_ENC_001` | `SPEC_BYTECODE_FORMAT_001` | 5 | `veryquick`, `pr` | 4 | none | none |
| `VM_SEAM_OWNER_001` | `SPEC_BYTECODE_FORMAT_001` | 5 | `veryquick`, `pr` | 5 | none | none |
| `VM_SEAM_REF_001` | `SPEC_BYTECODE_FORMAT_001` | 7 | `veryquick`, `pr` | 5 | none | none |
| `VM_SEAM_STRING_BOUND_001` | `SPEC_VM_VALUE_SEMANTICS_001` | 4 | `veryquick`, `pr` | 12 | none | none |
| `VM_SEAM_SUBRANGE_001` | `SPEC_VM_VALUE_SEMANTICS_001` | 9 | `veryquick`, `pr` | 12 | none | none |
| `VM_SEAM_VALID_001` | `SPEC_BYTECODE_FORMAT_001` | 23 | `veryquick`, `pr`, `nightly` | 13 | none | none |

## Reverse Public-Claim Traceability

| Public claim | State | Invariants | Tests | Suites | Evidence |
| --- | --- | ---: | ---: | --- | ---: |
| `PUBLIC_CLAIM_BEHAVIOR_LOCKED_001` | `linked` | 2 | 2 | `pr` | 8 |
| `PUBLIC_CLAIM_RUNTIME_WIRE_001` | `linked` | 2 | 5 | `pr` | 9 |
| `PUBLIC_CLAIM_SOURCE_BUILD_RUNTIME_001` | `linked` | 1 | 1 | `pr` | 7 |
| `PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001` | `linked` | 4 | 4 | `pr` | 13 |

## Orphans

- Spec sources: `SPEC_CONFORMANCE_CONTRACT_001`, `SPEC_EXTERNAL_REVIEW_V08_001`, `SPEC_IEC_61131_3_ED3_EXTERNAL_001`, `SPEC_IEC_DEVIATIONS_001`, `SPEC_PUBLIC_WORKFLOW_027D30E7601F_001`, `SPEC_PUBLIC_WORKFLOW_06F0B50B276C_001`, `SPEC_PUBLIC_WORKFLOW_0DFBFE040B69_001`, `SPEC_PUBLIC_WORKFLOW_0F343DE358D0_001`, `SPEC_PUBLIC_WORKFLOW_11CE18940EA5_001`, `SPEC_PUBLIC_WORKFLOW_15A2F0DD6613_001`, `SPEC_PUBLIC_WORKFLOW_28057E7E1763_001`, `SPEC_PUBLIC_WORKFLOW_3C0DDF559EC2_001`, `SPEC_PUBLIC_WORKFLOW_456FA568AFC2_001`, `SPEC_PUBLIC_WORKFLOW_556BCA7D07FB_001`, `SPEC_PUBLIC_WORKFLOW_7E9C84093FC5_001`, `SPEC_PUBLIC_WORKFLOW_7EEF7481947C_001`, `SPEC_PUBLIC_WORKFLOW_812A3212DF07_001`, `SPEC_PUBLIC_WORKFLOW_81374E35F805_001`, `SPEC_PUBLIC_WORKFLOW_819D4CA4AFE8_001`, `SPEC_PUBLIC_WORKFLOW_87AF7E445561_001`, `SPEC_PUBLIC_WORKFLOW_92F337788E11_001`, `SPEC_PUBLIC_WORKFLOW_94D293455A69_001`, `SPEC_PUBLIC_WORKFLOW_A22E00BD1F13_001`, `SPEC_PUBLIC_WORKFLOW_A9AF3F21E41E_001`, `SPEC_PUBLIC_WORKFLOW_ACCF4969E132_001`, `SPEC_PUBLIC_WORKFLOW_B856ED5CDDB4_001`, `SPEC_PUBLIC_WORKFLOW_BB46EA910511_001`, `SPEC_PUBLIC_WORKFLOW_C8E790E48310_001`, `SPEC_PUBLIC_WORKFLOW_CC557DA84056_001`, `SPEC_PUBLIC_WORKFLOW_CDC07CFF8A91_001`, `SPEC_PUBLIC_WORKFLOW_D24FCC998C80_001`, `SPEC_PUBLIC_WORKFLOW_D3D2C2908244_001`, `SPEC_PUBLIC_WORKFLOW_D7844FFDB0CC_001`, `SPEC_PUBLIC_WORKFLOW_E15DCFA0D264_001`, `SPEC_PUBLIC_WORKFLOW_F08D90359C6B_001`, `SPEC_PUBLIC_WORKFLOW_F16D362FF623_001`, `SPEC_PUBLIC_WORKFLOW_F495BAA5B93D_001`, `SPEC_VERIFICATION_PROGRAM_CONTRACT_001`
- Tests: none
- Invariants: none
- Public claims: none
- Evidence: `EVID_BOUNDED_FUZZ_CAMPAIGN_20260718`, `EVID_COMPLETE_MAPPED_TEST_EXECUTION_20260718`, `EVID_NON_CATALOG_LEDGER_BATCH_VALIDATION_20260717`, `EVID_P10_MUTATION_PROGRAM_CLOSURE_20260712`, `EVID_P10_MUTATION_PROGRAM_REPORT_20260712`, `EVID_P10_SOURCE_MUTATION_EXECUTION_VALIDATION_20260716`, `EVID_P12_WORKFLOW_UI_AUDIT_20260719`, `EVID_P13_RELEASE_EVIDENCE_AUDIT_20260719`, `EVID_P14_GOVERNANCE_CLOSEOUT_20260719`, `EVID_P16_EIGHTEENTH_REVIEW_FINAL_REPORT_REBIND_20260718`, `EVID_P16_EIGHTEENTH_REVIEW_FINAL_VALIDATION_20260718`, `EVID_P16_EIGHTEENTH_REVIEW_REPORT_REBIND_20260718`, `EVID_P16_ENFORCEMENT_CLOSEOUT_20260718`, `EVID_P16_EXECUTABLE_CLOSURE_FINAL_VALIDATION_20260718`, `EVID_P16_EXECUTABLE_CLOSURE_REPORT_REBIND_20260718`, `EVID_P16_EXECUTION_READINESS_ACCEPTANCE_20260712`, `EVID_P16_EXECUTION_READINESS_ACCEPTANCE_CLOSURE_20260712`, `EVID_P16_EXECUTION_READINESS_CLOSURE_20260712`, `EVID_P16_PUBLIC_CLAIM_FINAL_VALIDATION_20260718`, `EVID_P16_PUBLIC_CLAIM_REPORT_REBIND_20260718`, `EVID_P16_TEST_CATALOG_DENOMINATOR_CLOSURE_20260718`, `EVID_P1A_P4A_SOURCE_REVIEW_CLOSEOUT_20260719`, `EVID_P1A_SPEC_SOURCE_AUDIT_20260713`, `EVID_P1A_SPEC_SOURCE_AUDIT_CLOSURE_20260713`, `EVID_P1B_ADVERSARIAL_SELFTESTS_20260709`, `EVID_P1B_ARTIFACT_STAMPING_FOUNDATION_20260709`, `EVID_P1B_ARTIFACT_STAMPING_REVIEW_FIXES_20260709`, `EVID_P1B_PLANNING_PILOT_20260709`, `EVID_P1B_PLANNING_PILOT_REVIEW_FIXES_20260709`, `EVID_P1B_PROVE_CONTRACT_DESIGN_20260709`, `EVID_P1B_PROVE_CONTRACT_REVIEW_FIXES_20260709`, `EVID_P1B_PROVE_GREEN_IMPLEMENTATION_20260709`, `EVID_P1B_PROVE_GREEN_REVIEW_FIXES_20260709`, `EVID_P1B_PROVE_LOCK_IMPLEMENTATION_20260709`, `EVID_P1B_PROVE_LOCK_REVIEW_FIXES_20260709`, `EVID_P1B_PROVE_RED_IMPLEMENTATION_20260709`, `EVID_P1B_PROVE_RED_REVIEW_FIXES_20260709`, `EVID_P1B_VERIFICATION_CASES_HELPER_20260709`, `EVID_P1B_VERIFICATION_CASES_HELPER_REVIEW_FIXES_20260709`, `EVID_P1B_VERIFICATION_GATE_REPORT_ONLY_20260709`, `EVID_P2A_REFACTOR_ASSESSMENT_CLOSURE_20260710`, `EVID_P2A_REVIEW_FIXES_20260710`, `EVID_P2A_TEST_REFACTOR_ASSESSMENT_20260710`, `EVID_P2_CATALOG_BINDING_REGISTRATION_20260710`, `EVID_P2_COVERAGE_DEBT_CLOSURE_VALIDATION_20260710`, `EVID_P2_COVERAGE_MATRIX_GAPS_20260710`, `EVID_P2_EXISTING_TEST_CATALOG_20260709`, `EVID_P2_MALFORMED_INPUT_COVERAGE_20260710`, `EVID_P2_REPORT_REPLAY_REVIEW_FIXES_20260710`, `EVID_P2_TEST_CLASS_COMPLETENESS_20260710`, `EVID_P2_UNMAPPED_TEST_DEBT_20260710`, `EVID_P3_IGNORED_TEST_INVENTORY_20260710`, `EVID_P3_IGNORED_TEST_REGISTER_CLOSURE_20260710`, `EVID_P4A_SPECIFICATION_COMPLETENESS_20260710`, `EVID_P4_CONFIRMED_FINDINGS_SOURCE_REVIEW_20260710`, `EVID_P4_INVARIANT_SEED_AUDIT_20260710`, `EVID_P4_INVARIANT_SPEC_AUDIT_CLOSURE_20260710`, `EVID_P5_SUITE_GATE_ROUTING_AUDIT_20260710`, `EVID_P5_SUITE_GATE_ROUTING_CLOSURE_20260710`, `EVID_P6A_TOOLING_SELFTESTS_20260711`, `EVID_P6A_TOOLING_SELFTESTS_CLOSURE_20260711`, `EVID_P6_REQUIREMENT_ORACLE_AUDIT_20260711`, `EVID_P6_REQUIREMENT_ORACLE_CLOSURE_20260711`, `EVID_P7_CONFORMANCE_ALIGNMENT_20260711`, `EVID_P7_CONFORMANCE_ALIGNMENT_CLOSURE_20260711`, `EVID_P8_RUNTIME_ANOMALY_AUDIT_20260711`, `EVID_P8_RUNTIME_ANOMALY_CLOSURE_20260711`, `EVID_P8_RUNTIME_ANOMALY_DENOMINATOR_CLOSURE_20260718`, `EVID_P8_RUNTIME_ANOMALY_REPORT_REBIND_20260718`, `EVID_P9_FUZZ_PROGRAM_AUDIT_20260711`, `EVID_P9_FUZZ_PROGRAM_CLOSURE_20260711`, `EVID_PUBLIC_DOCS_TRUTH_SCAN_20260708_001`, `EVID_VERIFICATION_CONTROL_PLANE_REVIEW_FIXES_20260709`, `EVID_VERIFICATION_CONTROL_PLANE_SKELETON_20260708`

## Boundaries

- `audit_creates_proof`: `false`
- `audit_closes_spec_gaps`: `false`
- `missing_oracle_enforcement_enabled`: `true`
- `public_claims_are_oracles`: `false`
- `public_docs_inventory_exhaustive`: `false`
- `forward_traceability_complete`: `true`
- `reverse_traceability_complete`: `true`
- `orphan_traceability_complete`: `true`
- `p4a_005_public_claim_inventory_remains_open`: `true`
- `p6_007_enforcement_remains_open`: `false`
- `p6_008_to_p6_010_remain_open`: `false`
- `p14_000_grace_rule_remains_open`: `false`

## Limitations

- Mappings come only from explicit invariant spec/oracle references; names, paths, and prose do not create mappings.
- An eligible source listed as context does not replace an invariant's explicit open spec-gap oracle.
- Public claims are provenance and proof obligations, never behavior oracles.
- Product contracts and reviewed IEC decisions or deviations are not external IEC conformance proof; the registered external IEC source remains non-oracle and its ignored local bytes are not provenance inputs.
- The separate specification-source audit exhaustively classifies its tracked-document denominator and records conflict, checklist-staleness, and removed-behavior dispositions without creating proof.
- This report's public-claim view is registered-spec-sources-only; the separate audit exhaustively dispositions rendered prose and conservatively reports every substantive unbound block without an invariant or oracle.
- Forward and reverse paths use only explicit source, invariant, test, suite, evidence, gap, and public-claim identifiers; names, paths, and prose create no edges.
- verification/evidence-index.toml is excluded from the input digest to avoid a report-evidence digest cycle.
- The committed governance contract enforces overdue high-risk missing-oracle debt after its reviewed grace period; this report itself remains non-proof.
- An orphan is a registered record without the explicit links named by the report contract; an orphan finding does not infer that the underlying document, test, or evidence is useless.
- The blocked-row posture is checked live from the implementation board, which is excluded from the digest because board and evidence closure follow report generation.
- The report creates no proof, closes no specification gap, and changes no runtime or product behavior.
