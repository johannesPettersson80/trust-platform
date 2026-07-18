# Phase 6 Requirement and Oracle Audit

Generator: `requirement-oracle-audit v1`
Source revision: `82c62abe3d16873c8a65d92cab099843d9dbc5a3`
Generated: `2026-07-18T22:30:00+02:00`
Platform: `linux-x86_64`
Generated JSON SHA-256: `6570eb2cbd583e2818827976a293dd624a04741051d314995ddf1ac1244bb0ff`
Input SHA-256: `sha256:358328ca005fae21f3e807e65e7eb04272bf0d76f4e7df93b62ae70cd2d64597`

This is a report-only requirement/oracle association audit. It creates no
behavior proof, closes no specification gap, and enables no enforcement.
Its invariant denominator is all committed invariant records; public-claim
context is limited to the non-exhaustive registered source inventory.

## Summary

- Invariants: 54
- Phase 6 mapped invariants: 36
- Other-area invariants: 18
- Eligible oracles: 54
- Missing oracles: 0
- Future enforcement candidates: 0

## Mapping Groups

| Board row | Areas | Invariants | Eligible oracle | Spec-gap blocked |
| --- | --- | ---: | ---: | ---: |
| `VERIF-P6-001` | `compiler_iec` | 5 | 5 | 0 |
| `VERIF-P6-002` | `runtime_safety` | 11 | 11 | 0 |
| `VERIF-P6-003` | `protocols` | 7 | 7 | 0 |
| `VERIF-P6-004` | `editor_safety` | 7 | 7 | 0 |
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
