# Phase 4 Invariant-Seed Audit

Generator: `invariant-seed-audit v2`
Source revision: `d6a0e75fa951131ce96df55894829df176253eb0`
Generated: `2026-07-16T09:38:00+02:00`
Platform: `linux-x86_64`
Generated JSON SHA-256: `c3f4e4cf60981de206e52f43fc3ff67b2b9d948b814b897e110afb7eaab8c69a`
Input SHA-256: `sha256:5a899a2eae6c895405654553bd65047b7cad80f1444053f2a2c86a69d6b5be98`

This is a registry-completeness report. It creates no behavior proof,
closes no specification gap, and changes no runtime behavior.

## Summary

- Written seeds: 44
- Canonical invariants: 43
- Authorized merged aliases: 1
- Newly introduced Phase 4 records: 36
- Pre-existing seed mappings: 8
- Baseline lifecycle records: 22
- Execution-ready lifecycle records: 22
- Gap-open records: 15
- Spec-gap records: 18
- Test-written records: 0
- Implemented records: 11
- Validated records: 0
- Imported P4-000 review risks: 5

| Board row | Seeds |
| --- | ---: |
| `VERIF-P4-001` | 5 |
| `VERIF-P4-002` | 6 |
| `VERIF-P4-003` | 9 |
| `VERIF-P4-004` | 6 |
| `VERIF-P4-005` | 8 |
| `VERIF-P4-006` | 1 |
| `VERIF-P4-007` | 3 |
| `VERIF-P4-008` | 6 |

## Seed Registry

| Seed | Canonical invariant | Area | Row | Origin | Lifecycle | Status | Oracle | P4-000 risk |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `RT_SAFE_PANIC_001` | `RT_SAFE_PANIC_001` | `runtime_safety` | `VERIF-P4-003` | `phase4` | `v1:execution_ready` | `implemented/G1` | `SPEC_RUNTIME_ENGINE_001` | `none` |
| `RT_SAFE_DEADLINE_001` | `RT_SAFE_DEADLINE_001` | `runtime_safety` | `VERIF-P4-003` | `phase4` | `v1:execution_ready` | `implemented/G2` | `SPEC_RUNTIME_ENGINE_001` | `none` |
| `RT_SAFE_STOP_001` | `RT_SAFE_STOP_001` | `runtime_safety` | `VERIF-P4-003` | `preexisting` | `v1:execution_ready` | `implemented/G2` | `SPEC_RUNTIME_ENGINE_001` | `none` |
| `RT_SAFE_IO_001` | `RT_SAFE_IO_001` | `runtime_safety` | `VERIF-P4-003` | `phase4` | `v1:execution_ready` | `gap_open/S0` | `SPEC_RUNTIME_ENGINE_001` | `none` |
| `RT_SAFE_RETAIN_001` | `RT_SAFE_RETAIN_001` | `runtime_safety` | `VERIF-P4-003` | `phase4` | `v1:execution_ready` | `implemented/G2` | `SPEC_RUNTIME_ENGINE_001` | `none` |
| `RT_SAFE_RESTART_001` | `RT_SAFE_RESTART_001` | `runtime_safety` | `VERIF-P4-003` | `phase4` | `v1:execution_ready` | `gap_open/S0` | `SPEC_RUNTIME_ENGINE_001` | `none` |
| `RT_SAFE_FORCE_001` | `RT_SAFE_FORCE_001` | `runtime_safety` | `VERIF-P4-003` | `phase4` | `v1:execution_ready` | `implemented/G1` | `SPEC_RUNTIME_ENGINE_001` | `none` |
| `RT_SAFE_NAN_001` | `RT_SAFE_NAN_001` | `runtime_safety` | `VERIF-P4-003` | `phase4` | `v1:execution_ready` | `implemented/G2` | `SPEC_RUNTIME_ENGINE_001` | `RISK_RUNTIME_NONFINITE_INGRESS_001` |
| `RT_RELOAD_001` | `RT_RELOAD_001` | `runtime_safety` | `VERIF-P4-003` | `phase4` | `v1:execution_ready` | `implemented/G2` | `SPEC_RUNTIME_ENGINE_001` | `RISK_RUNTIME_RELOAD_TRANSACTION_001` |
| `VM_SEAM_TYPE_001` | `VM_SEAM_DECLARED_TYPE_001` | `bytecode_vm` | `VERIF-P4-002` | `preexisting` | `v1:execution_ready` | `spec_gap/S0` | `SPEC_GAP_VM_ERROR_MODEL_001` | `none` |
| `VM_SEAM_TYPE_002` | `VM_SEAM_DECLARED_TYPE_001` | `bytecode_vm` | `VERIF-P4-002` | `preexisting` | `v1:execution_ready` | `spec_gap/S0` | `SPEC_GAP_VM_ERROR_MODEL_001` | `none` |
| `VM_SEAM_REF_001` | `VM_SEAM_REF_001` | `bytecode_vm` | `VERIF-P4-002` | `preexisting` | `v1:execution_ready` | `gap_open/S0` | `SPEC_BYTECODE_FORMAT_001` | `none` |
| `VM_SEAM_OWNER_001` | `VM_SEAM_OWNER_001` | `bytecode_vm` | `VERIF-P4-002` | `preexisting` | `v1:execution_ready` | `gap_open/S0` | `SPEC_BYTECODE_FORMAT_001` | `none` |
| `VM_SEAM_VALID_001` | `VM_SEAM_VALID_001` | `bytecode_vm` | `VERIF-P4-002` | `preexisting` | `v1:execution_ready` | `spec_gap/S0` | `SPEC_GAP_VM_ERROR_MODEL_001` | `none` |
| `VM_SEAM_ENC_001` | `VM_SEAM_ENC_001` | `bytecode_vm` | `VERIF-P4-002` | `preexisting` | `v1:execution_ready` | `gap_open/S0` | `SPEC_BYTECODE_FORMAT_001` | `none` |
| `IEC_PARSE_RECOVER_001` | `IEC_PARSE_RECOVER_001` | `compiler_iec` | `VERIF-P4-001` | `phase4` | `v1:execution_ready` | `gap_open/S0` | `SPEC_IEC_DECISIONS_001` | `none` |
| `IEC_PREC_001` | `IEC_PREC_001` | `compiler_iec` | `VERIF-P4-001` | `phase4` | `v1:baseline` | `gap_open/S0` | `SPEC_IEC_EXPRESSIONS_001` | `none` |
| `IEC_STRING_001` | `IEC_STRING_001` | `compiler_iec` | `VERIF-P4-001` | `phase4` | `v1:execution_ready` | `gap_open/S0` | `SPEC_IEC_DATA_TYPES_CANDIDATE_001` | `none` |
| `IEC_SUBRANGE_001` | `IEC_SUBRANGE_001` | `compiler_iec` | `VERIF-P4-001` | `phase4` | `v1:execution_ready` | `gap_open/S0` | `SPEC_IEC_DECISIONS_001` | `none` |
| `IEC_TIMER_001` | `IEC_TIMER_001` | `compiler_iec` | `VERIF-P4-001` | `phase4` | `v1:execution_ready` | `implemented/G2` | `SPEC_IEC_STANDARD_FBS_CANDIDATE_001` | `RISK_IEC_TIMER_SEMANTICS_001` |
| `PLCO_IMPORT_001` | `PLCO_IMPORT_001` | `plcopen_devtools` | `VERIF-P4-005` | `phase4` | `v1:baseline` | `gap_open/S0` | `SPEC_PLCOPEN_IMPORT_DECISION_001` | `none` |
| `DEV_TEST_DISCOVERY_001` | `DEV_TEST_DISCOVERY_001` | `plcopen_devtools` | `VERIF-P4-005` | `phase4` | `v1:baseline` | `spec_gap/S0` | `SPEC_GAP_DEV_TEST_DISCOVERY_CASE_001` | `none` |
| `DEV_COMMIT_SCOPE_001` | `DEV_COMMIT_SCOPE_001` | `plcopen_devtools` | `VERIF-P4-005` | `phase4` | `v1:baseline` | `spec_gap/S0` | `SPEC_GAP_DEV_COMMIT_SCOPE_001` | `none` |
| `PROTO_DISC_001` | `PROTO_DISCOVERY_TRUTH_001` | `protocols` | `VERIF-P4-004` | `preexisting` | `v1:baseline` | `spec_gap/S0` | `SPEC_GAP_PUBLIC_WIRE_CLAIM_001` | `none` |
| `PROTO_MODBUS_001` | `PROTO_MODBUS_001` | `protocols` | `VERIF-P4-004` | `phase4` | `v1:baseline` | `spec_gap/S0` | `SPEC_GAP_PROTOCOL_DISCOVERY_HANDSHAKE_001` | `none` |
| `PROTO_MQTT_001` | `PROTO_MQTT_001` | `protocols` | `VERIF-P4-004` | `phase4` | `v1:baseline` | `spec_gap/S0` | `SPEC_GAP_PROTOCOL_DISCOVERY_HANDSHAKE_001` | `none` |
| `PROTO_ETHERCAT_001` | `PROTO_ETHERCAT_001` | `protocols` | `VERIF-P4-004` | `phase4` | `v1:baseline` | `spec_gap/S0` | `SPEC_GAP_ETHERCAT_UNAVAILABLE_RESOURCE_001` | `none` |
| `PROTO_ADS_001` | `PROTO_ADS_001` | `protocols` | `VERIF-P4-004` | `phase4` | `v1:baseline` | `spec_gap/S0` | `SPEC_GAP_PROTOCOL_STATUS_MODEL_001` | `none` |
| `PROTO_OPCUA_001` | `PROTO_OPCUA_001` | `protocols` | `VERIF-P4-004` | `phase4` | `v1:baseline` | `gap_open/S0` | `SPEC_OPCUA_CLIENT_LIFECYCLE_DECISION_001` | `RISK_OPCUA_CLIENT_LIFECYCLE_001` |
| `EDIT_RENAME_001` | `EDIT_RENAME_001` | `editor_safety` | `VERIF-P4-005` | `phase4` | `v1:baseline` | `gap_open/S0` | `SPEC_LSP_CONTRACT_001` | `none` |
| `EDIT_RENAME_002` | `EDIT_RENAME_002` | `editor_safety` | `VERIF-P4-005` | `phase4` | `v1:baseline` | `gap_open/S0` | `SPEC_LSP_CONTRACT_001` | `none` |
| `EDIT_LSP_POS_001` | `EDIT_LSP_POS_001` | `editor_safety` | `VERIF-P4-005` | `phase4` | `v1:baseline` | `gap_open/S0` | `SPEC_LSP_CONTRACT_001` | `none` |
| `EDIT_DIAG_CANCEL_001` | `EDIT_DIAG_CANCEL_001` | `editor_safety` | `VERIF-P4-005` | `phase4` | `v1:baseline` | `gap_open/S0` | `SPEC_LSP_CONTRACT_001` | `none` |
| `UI_STATUS_001` | `UI_STATUS_001` | `hmi_ui` | `VERIF-P4-006` | `phase4` | `v1:baseline` | `spec_gap/S0` | `SPEC_GAP_UI_STATUS_VOCABULARY_001` | `none` |
| `DEBUG_AUTH_001` | `DEBUG_AUTH_001` | `control_security` | `VERIF-P4-008` | `phase4` | `v1:execution_ready` | `implemented/G1` | `SPEC_RUNTIME_ENGINE_001` | `none` |
| `DEBUG_PAUSE_001` | `DEBUG_PAUSE_001` | `editor_safety` | `VERIF-P4-005` | `phase4` | `v1:execution_ready` | `implemented/G1` | `SPEC_RUNTIME_ENGINE_001` | `none` |
| `SEC_DEP_AUDIT_001` | `SEC_DEP_AUDIT_001` | `supply_chain_platform` | `VERIF-P4-008` | `phase4` | `v1:baseline` | `spec_gap/S0` | `SPEC_GAP_DEPENDENCY_AUDIT_POLICY_001` | `none` |
| `SEC_AUTHZ_001` | `SEC_AUTHZ_001` | `control_security` | `VERIF-P4-008` | `phase4` | `v1:execution_ready` | `implemented/G1` | `SPEC_RUNTIME_ENGINE_001` | `RISK_RUNTIME_AUTHORIZATION_001` |
| `SEC_ARTIFACT_001` | `SEC_ARTIFACT_001` | `supply_chain_platform` | `VERIF-P4-008` | `phase4` | `v1:baseline` | `spec_gap/S0` | `SPEC_GAP_ARTIFACT_PROVENANCE_001` | `none` |
| `PLAT_PATH_001` | `PLAT_PATH_001` | `supply_chain_platform` | `VERIF-P4-008` | `phase4` | `v1:baseline` | `spec_gap/S0` | `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` | `none` |
| `PLAT_VSCODE_001` | `PLAT_VSCODE_001` | `supply_chain_platform` | `VERIF-P4-008` | `phase4` | `v1:baseline` | `spec_gap/S0` | `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` | `none` |
| `REL_CLAIM_001` | `REL_CLAIM_001` | `release` | `VERIF-P4-007` | `phase4` | `v1:baseline` | `spec_gap/S0` | `SPEC_GAP_HARDWARE_PUBLIC_CLAIM_001` | `none` |
| `REL_CONF_001` | `REL_CONF_001` | `release` | `VERIF-P4-007` | `phase4` | `v1:baseline` | `spec_gap/S0` | `SPEC_GAP_CONFORMANCE_PUBLICATION_001` | `none` |
| `REL_VERSION_001` | `REL_VERSION_001` | `release` | `VERIF-P4-007` | `phase4` | `v1:baseline` | `spec_gap/S0` | `SPEC_GAP_RELEASE_VERSION_CHAIN_001` | `none` |

## Limitations

- This audit proves registry completeness and metadata posture, not product behavior.
- Lifecycle authorization does not create proof, close an invariant, or close a specification gap.
- Only explicitly referenced live catalog and evidence records can support an execution-ready seed.
- verification/evidence-index.toml is live-validated but excluded from the input digest to avoid a report-evidence cycle.
