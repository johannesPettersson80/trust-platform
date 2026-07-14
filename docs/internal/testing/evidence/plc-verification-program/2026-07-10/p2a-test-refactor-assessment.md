# Existing-Test Refactor Assessment

Generator: `test-refactor-assessment v1`
Source revision: `92f68757a2b72c9b209e711d8b8729d15bb40075`
Generated: `2026-07-14T18:13:00+02:00`
Platform: `linux-x86_64`
Generated JSON SHA-256: `364007b023f51d8c3533648e20809a5beda4e8740127f9b2c76faf0426a63890`
Input SHA-256: `sha256:f19517e9a7891c5adc3bd266d63e5b99aa6736a47021b5e69af41403786b18c7`

Size is a review signal, not a refactor decision.
Mechanical similarity is candidate evidence only; it never authorizes
a move, split, rename, fixture merge, or behavior change.

## Summary

- Scanner facts: 3896
- Fact-bearing files: 685
- Large-file candidates: 24
- Reviewed mapping-diversity candidates: 6
- Broad multi-invariant claim candidates: 5
- Exact fact-file duplicate groups: 0
- Whitespace-normalized fact-file duplicate groups: 0
- Exact case-input duplicate groups: 1
- Same-table structural case-input peer groups: 11
- Shared case-file reference groups: 1
- Malformed-class overlap groups: 0
- VS Code facts: 456
- VS Code files: 38
- VS Code registrations: 38
- Large registered VS Code files: 5
- Catalog records: 98
- Scanner facts with reviewed duration: 93
- Scanner facts without reviewed duration: 3803
- Catalog rows explicitly classified slow: 1
- Reviewed proposal decisions: 1
- Assessment-supported decisions: 1

## Large Or Mixed-Purpose Signals

| Path | Lines | Facts | Reviewed mappings | Signals |
| --- | ---: | ---: | ---: | --- |
| `crates/trust-ads-server/src/commands/tests.rs` | 1287 | 43 | 0 | `large_file` |
| `crates/trust-ads-server/src/listener.rs` | 1374 | 16 | 0 | `large_file` |
| `crates/trust-debug/src/adapter/tests_part_02.rs` | 1175 | 13 | 0 | `large_file` |
| `crates/trust-hir/src/db/queries/database/database_tests_part_01.rs` | 591 | 17 | 2 | `reviewed_mapping_diversity` |
| `crates/trust-hir/src/openot_authoring.rs` | 2868 | 22 | 0 | `large_file` |
| `crates/trust-hir/src/symbols/table.rs` | 1136 | 3 | 0 | `large_file` |
| `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | 1295 | 69 | 0 | `large_file` |
| `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | 1316 | 23 | 0 | `large_file` |
| `crates/trust-runtime/src/bin/trust-runtime/ads.rs` | 1562 | 2 | 0 | `large_file` |
| `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | 1543 | 49 | 0 | `large_file` |
| `crates/trust-runtime/src/config/tests.rs` | 1154 | 71 | 0 | `large_file` |
| `crates/trust-runtime/src/control/comm_handlers/browse_symbols.rs` | 1123 | 7 | 0 | `large_file` |
| `crates/trust-runtime/src/control/tests/core.rs` | 5785 | 71 | 0 | `large_file` |
| `crates/trust-runtime/src/control/tests/hmi_values_write.rs` | 868 | 14 | 2 | `reviewed_mapping_diversity` |
| `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | 1291 | 29 | 0 | `large_file` |
| `crates/trust-runtime/src/host/ads/tests.rs` | 1404 | 36 | 1 | `large_file` |
| `crates/trust-runtime/src/io/mqtt/tests.rs` | 1240 | 31 | 1 | `large_file` |
| `crates/trust-runtime/tests/debug_pause_watchdog.rs` | 122 | 2 | 2 | `reviewed_mapping_diversity` |
| `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st` | 1748 | 19 | 0 | `large_file` |
| `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | 2869 | 50 | 0 | `large_file` |
| `crates/trust-runtime/tests/modbus_driver.rs` | 907 | 23 | 4 | `reviewed_mapping_diversity` |
| `crates/trust-runtime/tests/openot_telemetry.rs` | 3148 | 37 | 0 | `large_file` |
| `crates/trust-runtime/tests/phase11_seam_contract.rs` | 1275 | 22 | 9 | `large_file` |
| `crates/trust-runtime/tests/retain_integrity.rs` | 510 | 12 | 3 | `reviewed_mapping_diversity` |
| `crates/trust-runtime/tests/runtime_safety_fail_closed.rs` | 800 | 17 | 5 | `reviewed_mapping_diversity` |
| `editors/vscode/src/test/suite/hmi.integration.test.ts` | 1445 | 14 | 0 | `large_file` |
| `editors/vscode/src/test/suite/ladder-engine.test.ts` | 1093 | 14 | 0 | `large_file` |
| `editors/vscode/src/test/suite/network-canvas.test.ts` | 2109 | 56 | 0 | `large_file` |
| `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | 1245 | 47 | 0 | `large_file` |
| `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | 4378 | 158 | 0 | `large_file` |

## Broad Invariant Claims

- `TEST_RUNTIME_ADS_NONFINITE_ARRAY_DECODE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_ADS_NONFINITE_SCALAR_DECODE_001` claims 1 invariants; result `single_invariant`.
- `TEST_DEBUG_MANAGED_LREAL_NONFINITE_001` claims 1 invariants; result `single_invariant`.
- `TEST_DEBUG_MANAGED_REAL_NONFINITE_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_STRING_IMPORTED_CAPACITY_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_STRING_IMPORTED_MISMATCH_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_STRING_INOUT_ALIAS_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_STRING_INOUT_DIAGNOSTIC_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_REAL_BINARY_OVERFLOW_UNIT_001` claims 1 invariants; result `single_invariant`.
- `TEST_CONTROL_AUTHORIZATION_FAILSAFE_001` claims 1 invariants; result `single_invariant`.
- `TEST_DEBUG_AUTHORIZATION_MATRIX_001` claims 2 invariants; result `candidate_missing_coverage_dimensions`.
- `TEST_RUNTIME_FORCE_AUTH_CHANGE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_HMI_NONFINITE_WRITE_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_SUBRANGE_HMI_WRITE_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_ADS_NONFINITE_SERVER_INGRESS_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_ADS_NONFINITE_CLIENT_INGRESS_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_MESH_NONFINITE_INGRESS_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_OPCUA_NONFINITE_INGRESS_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_TYPED_INPUT_NONFINITE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_TYPED_LREAL_OUTPUT_TRANSACTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_TYPED_REAL_OUTPUT_NONFINITE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_MODBUS_NONFINITE_PROCESS_IMAGE_READ_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_MODBUS_NONFINITE_WIRE_DECODE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_MQTT_NONFINITE_PROCESS_IMAGE_READ_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_MQTT_NONFINITE_TEXT_DECODE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_MQTT_NONFINITE_BATCH_TRANSACTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_SAFE_STATE_MQTT_PENDING_001` claims 2 invariants; result `candidate_missing_coverage_dimensions`.
- `TEST_RUNTIME_PANIC_CYCLE_GUARD_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_RESTART_AUTOMATIC_STORAGE_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_CONTAINER_INVALID_MAGIC` claims 1 invariants; result `single_invariant`.
- `TEST_VM_COERCION_ASSIGNMENT_WIDENING_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_COERCION_FUNCTION_INPUT_WIDENING_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_COERCION_FUNCTION_OUTPUT_WIDENING_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_COERCION_INITIALIZER_WIDENING_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_COERCION_INOUT_NARROWING_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_COERCION_NARROWING_ASSIGNMENT_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_COERCION_RETURN_WIDENING_001` claims 1 invariants; result `single_invariant`.
- `TEST_DEBUG_PAUSE_WATCHDOG_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_PAUSE_WATCHDOG_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_REAL_BINARY_OVERFLOW_INTEGRATION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_REAL_NAMED_OVERFLOW_INTEGRATION_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_FORCE_LIFECYCLE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_FORCE_DISCONNECT_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_FORCE_FAULT_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_FORCE_PAUSE_RESUME_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_FORCE_RELEASE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_FORCE_STOP_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_TIMER_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_MODBUS_NONFINITE_MAPPED_READ_TRANSACTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_MODBUS_SLOW_READ_BOUND_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_MODBUS_SLOW_WRITE_BOUND_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_SAFE_STATE_MODBUS_PENDING_001` claims 2 invariants; result `candidate_missing_coverage_dimensions`.
- `TEST_VM_DECLARED_DINT_CONVERSION_PATHS_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_DECLARED_DINT_RUNTIME_TAG_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_DECLARED_LITERAL_CONTEXT_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_DECLARED_PARAMETER_COPY_IN_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_DECLARED_REAL_CONVERSION_PATHS_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_DECLARED_REAL_RUNTIME_TAG_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_SUBRANGE_ASSIGNMENT_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_SUBRANGE_FB_INPUT_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_SUBRANGE_REF_WRITE_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_RELOAD_TRANSACTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_RETAIN_FAILURE_ATOMICITY_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_RETAIN_NONFINITE_LOAD_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_RETAIN_NONFINITE_SAVE_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_SUBRANGE_RETAIN_RELOAD_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_RESTART_COLD_STORAGE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_RESTART_WARM_STORAGE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_RESTART_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_PANIC_IO_DRIVER_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_PANIC_SAFE_OUTPUT_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_PANIC_THREAD_GUARD_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_SAFE_STATE_STOP_DEGRADED_001` claims 2 invariants; result `candidate_missing_coverage_dimensions`.
- `TEST_RUNTIME_TYPED_OUTPUT_DRIVER_COMMIT_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_WATCHDOG_BEFORE_OUTPUT_COMMIT_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_SAFE_STATE_HANDOFF_001` claims 2 invariants; result `candidate_missing_coverage_dimensions`.
- `TEST_RUNTIME_SIMULATION_THRESHOLD_NONFINITE_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_REAL_NAMED_OVERFLOW_DIRECT_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_STRING_FB_INOUT_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_STRING_FB_INPUT_INOUT_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_STRING_FB_OUTPUT_COPYBACK_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_STRING_FUNCTION_INOUT_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_STRING_FUNCTION_INPUT_INOUT_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_STRING_FUNCTION_OUTPUT_COPYBACK_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_STRING_FUNCTION_RECEIVER_BOUND_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_STRING_FUNCTION_RESULT_BOUND_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_STRING_LOCAL_OUTPUT_COPYBACK_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_STRING_NESTED_FIELD_COPYBACK_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_STRING_UNICODE_ASSIGNMENT_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_STRING_UNICODE_LITERAL_BOUND_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_WSTRING_FB_INOUT_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_WSTRING_FB_OUTPUT_COPYBACK_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_WSTRING_LOCAL_ALIAS_COPYBACK_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_VALIDATOR_MUTATION_SHARD_001` claims 1 invariants; result `single_invariant`.
- `TEST_CASE_TABLE_VM_SEAM_DECLARED_TYPE_001` claims 1 invariants; result `single_invariant`.
- `TEST_CASE_TABLE_VM_SEAM_STRING_BOUND_001` claims 1 invariants; result `single_invariant`.
- `TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001` claims 1 invariants; result `single_invariant`.
- `TEST_CASE_TABLE_VM_SEAM_VALID_001` claims 1 invariants; result `single_invariant`.

## Duplicate And Structural Signals

- Exact fact-file groups: 0
- Whitespace-normalized fact-file groups: 0
- Exact case-input groups: 1
- Same-table structural case-input peer groups: 11
- Shared case-file reference groups: 1
- Explicit malformed-class overlap groups: 0
- Free-form source-body similarity: `not_assessed`
- Exact case input `sha256:c670cbf91197596289caf54141e9ad5505065ef61ebd999029f415beb97a26be`: cases `RT_SAFE_IO_WORKER_001_FAILURE_DOES_NOT_SKIP_LATER_DRIVER`, `RT_SAFE_IO_WORKER_001_FAULTED_HANDOFF_FAULTS_STOP`, `RT_SAFE_STOP_001_DEGRADED_HANDOFF_FAULTS_STOP`; files `verification/cases/runtime_safety/RT_SAFE_STOP_001.toml`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_DECLARED_TYPE_001.toml`: `VM_SEAM_DECLARED_TYPE_001_COMPATIBLE_LITERAL_INITIALIZER_WIDENING_D8DF321B`, `VM_SEAM_DECLARED_TYPE_001_COMPATIBLE_TYPED_ASSIGNMENT_WIDENING_A6856BB4`, `VM_SEAM_DECLARED_TYPE_001_FUNCTION_RESULT_WIDENING_E52C15B0`, `VM_SEAM_DECLARED_TYPE_001_INT_EXPRESSION_TO_DINT_SLOT_9BF228AA`, `VM_SEAM_DECLARED_TYPE_001_INT_LITERAL_TO_REAL_SLOT_04979927`, `VM_SEAM_DECLARED_TYPE_001_INT_VARIABLE_TO_REAL_SLOT_C3821866`, `VM_SEAM_DECLARED_TYPE_001_POU_OUTPUT_WIDENING_8D957490`; shape `sha256:1be9655b754ca9d884c27bd43460558fb86cb55d995475a1d19b276b5de60ecc`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_DECLARED_TYPE_001.toml`: `VM_SEAM_DECLARED_TYPE_001_WRONG_TYPE_1A1BA936`, `VM_SEAM_DECLARED_TYPE_001_WRONG_TYPE_77B93993`, `VM_SEAM_DECLARED_TYPE_001_WRONG_TYPE_D8E1DB83`; shape `sha256:0d790842b7e2d96191704c9f07776e47e75c71b2d7dec4c06ff4ba9f0187a606`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_STRING_BOUND_001.toml`: `VM_SEAM_STRING_BOUND_001_MAX_D165B4CE`, `VM_SEAM_STRING_BOUND_001_MIN_D165B4CE`; shape `sha256:3940b6625c5f575bfcae24332dd917448996f46e736d442993370d110b01778a`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_SUBRANGE_001.toml`: `VM_SEAM_SUBRANGE_001_MAX_AE132E71`, `VM_SEAM_SUBRANGE_001_MIN_AE132E71`; shape `sha256:74118ebe11ebf1204131d61e0250805ebee19ed88606029843da8d8741cc79ad`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml`: `VM_SEAM_VALID_001_JUMP_TARGET_POU_BODY_JMP_OPERAND_100_6DD115EE`, `VM_SEAM_VALID_001_JUMP_TARGET_POU_BODY_JMP_OPERAND__100_09FC189F`; shape `sha256:4c2dbca3a9792afb543b0301d33ce1a8bd127b78dee5e254a7c0f3350539b70f`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml`: `VM_SEAM_VALID_001_TRUNCATE_BEFORE_POU_BODIES_D6833A8D`, `VM_SEAM_VALID_001_TRUNCATE_BEFORE_SECTION_TABLE_58B11C2B`; shape `sha256:e0bd21a1e4c5110f2132f018441b9faa042eff4a8587ab2ce394077f907edf8d`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml`: `VM_SEAM_VALID_001_UNKNOWN_OPCODE_POU_BODY_FIRST_OPCODE_80_CA909A71`, `VM_SEAM_VALID_001_UNKNOWN_OPCODE_POU_BODY_FIRST_OPCODE_FF_32935955`; shape `sha256:7afbe67384583995479cd4b26ae4dfb1e78bf262fb626d55f38a7b05699ab8e3`.
- Structural peers in `verification/cases/compiler_iec/IEC_TIMER_001.toml`: `IEC_TIMER_001_TOF_LTIME_POST_EXPIRY_HOLD`, `IEC_TIMER_001_TOF_TIME_POST_EXPIRY_HOLD`, `IEC_TIMER_001_TON_TIME_BASIC_DELAY`, `IEC_TIMER_001_TP_TIME_BASIC_PULSE`; shape `sha256:5583f769954ed3a1265abd03edaee615985ac4cd8dffbd88b1b246bdc76df8e2`.
- Structural peers in `verification/cases/runtime_safety/RT_SAFE_FORCE_001.toml`: `RT_SAFE_FORCE_001_RESTART_CLEARS_DEBUG_MUTATIONS`, `RT_SAFE_FORCE_001_SAFE_STATE_CLEARS_IO_FORCE`; shape `sha256:5583f769954ed3a1265abd03edaee615985ac4cd8dffbd88b1b246bdc76df8e2`.
- Structural peers in `verification/cases/runtime_safety/RT_SAFE_RESTART_TIME_002.toml`: `RT_SAFE_RESTART_TIME_002_COLD_PRESERVES_TIME_AND_REINITIALIZES_STATE`, `RT_SAFE_RESTART_TIME_002_WARM_PRESERVES_TIME_AND_REINITIALIZES_STATE`; shape `sha256:5583f769954ed3a1265abd03edaee615985ac4cd8dffbd88b1b246bdc76df8e2`.
- Structural peers in `verification/cases/runtime_safety/RT_SAFE_STOP_001.toml`: `RT_SAFE_IO_WORKER_001_FAILURE_DOES_NOT_SKIP_LATER_DRIVER`, `RT_SAFE_IO_WORKER_001_FAULTED_HANDOFF_FAULTS_STOP`, `RT_SAFE_STOP_001_DEGRADED_HANDOFF_FAULTS_STOP`; shape `sha256:5583f769954ed3a1265abd03edaee615985ac4cd8dffbd88b1b246bdc76df8e2`.
- Shared case file `verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml`: tests `TEST_BYTECODE_VALIDATOR_MUTATION_SHARD_001`, `TEST_CASE_TABLE_VM_SEAM_VALID_001`; record paths `scripts/bytecode_validator_mutation.py`, `verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml`.

## VS Code Registration

- Discovered facts: 456
- Test files: 38
- Literal registrations: 38
- Diagnostics: 0
- `editors/vscode/src/test/suite/hmi.integration.test.ts`: 1445 lines, 14 facts.
- `editors/vscode/src/test/suite/ladder-engine.test.ts`: 1093 lines, 14 facts.
- `editors/vscode/src/test/suite/network-canvas.test.ts`: 2109 lines, 56 facts.
- `editors/vscode/src/test/suite/runtime-controls-contract.test.ts`: 1245 lines, 47 facts.
- `editors/vscode/src/test/suite/ux-shell-contract.test.ts`: 4378 lines, 158 facts.

## Duration Classification

- Scanner facts listed: 3896
- Artifact catalog rows listed separately: 5
- Ignored, nightly, hardware, and name signals never infer duration.
- Scanner `DISC_043E03287D0BD498DBFE` / `TEST_VM_DECLARED_DINT_RUNTIME_TAG_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_093B7EAE0DCB979D4540` / `TEST_VM_SUBRANGE_RETAIN_RELOAD_REJECTION_001`: `fast` at `crates/trust-runtime/tests/retain_integrity.rs`.
- Scanner `DISC_110C6B5B49703576EB1E` / `TEST_IEC_STRING_FUNCTION_INPUT_INOUT_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_1257BED418738BD81458` / `TEST_VM_SUBRANGE_HMI_WRITE_REJECTION_001`: `fast` at `crates/trust-runtime/src/control/tests/hmi_values_write.rs`.
- Scanner `DISC_12A4F21EF84C13A91D28` / `TEST_RUNTIME_RETAIN_FAILURE_ATOMICITY_001`: `fast` at `crates/trust-runtime/tests/retain_failure_trace_cases.rs`.
- Scanner `DISC_1A7FBD5D70961E458BE7` / `TEST_RUNTIME_MODBUS_NONFINITE_MAPPED_READ_TRANSACTION_001`: `fast` at `crates/trust-runtime/tests/modbus_driver.rs`.
- Scanner `DISC_1D261CB173DC0CF72297` / `TEST_VM_DECLARED_REAL_RUNTIME_TAG_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_200665EACFA567BA9AB5` / `TEST_DEBUG_MANAGED_REAL_NONFINITE_001`: `fast` at `crates/trust-debug/src/adapter/tests_part_01.rs`.
- Scanner `DISC_21006DA7606FD83EC00A` / `TEST_RUNTIME_MODBUS_NONFINITE_WIRE_DECODE_001`: `fast` at `crates/trust-runtime/src/io/modbus/point_map.rs`.
- Scanner `DISC_213D1264041EB1793C79` / `TEST_RUNTIME_RESTART_COLD_STORAGE_001`: `fast` at `crates/trust-runtime/tests/runtime_restart.rs`.
- Scanner `DISC_21561AEEDB07941017B8` / `TEST_RUNTIME_RESTART_TRACE_001`: `fast` at `crates/trust-runtime/tests/runtime_restart_trace_cases.rs`.
- Scanner `DISC_2186AC96174B5EBBFCF2` / `TEST_RUNTIME_FORCE_STOP_001`: `fast` at `crates/trust-runtime/tests/force_lifecycle_boundaries.rs`.
- Scanner `DISC_234FF0395E492391D0E0` / `TEST_DEBUG_PAUSE_WATCHDOG_001`: `fast` at `crates/trust-runtime/tests/debug_pause_watchdog.rs`.
- Scanner `DISC_23588E738D07E62BA79E` / `TEST_IEC_STRING_FB_OUTPUT_COPYBACK_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_265444021E11E0C2B452` / `TEST_VM_SUBRANGE_FB_INPUT_REJECTION_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_266A60F3E75C685188EB` / `TEST_VM_COERCION_FUNCTION_OUTPUT_WIDENING_001`: `fast` at `crates/trust-runtime/tests/coercion_proof.rs`.
- Scanner `DISC_269219F3ACC34E73387B` / `TEST_CONTROL_AUTHORIZATION_FAILSAFE_001`: `fast` at `crates/trust-runtime/src/control/policy.rs`.
- Scanner `DISC_278EEBF061FD97B539D0` / `TEST_VM_DECLARED_REAL_CONVERSION_PATHS_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_2DB5D553A2151F589572` / `TEST_IEC_STRING_INOUT_DIAGNOSTIC_001`: `fast` at `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs`.
- Scanner `DISC_2F259133A9069DCE6CDC` / `TEST_IEC_WSTRING_FB_INOUT_REJECTION_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_30C382889325B64C5854` / `TEST_RUNTIME_PANIC_IO_DRIVER_001`: `fast` at `crates/trust-runtime/tests/runtime_safety_fail_closed.rs`.
- Scanner `DISC_34DA98F67689CD13B551` / `TEST_VM_COERCION_FUNCTION_INPUT_WIDENING_001`: `fast` at `crates/trust-runtime/tests/coercion_proof.rs`.
- Scanner `DISC_40D602971AB9F58ADC84` / `TEST_IEC_STRING_FB_INPUT_INOUT_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_40F1AE3A1CEE7BA3BA4C` / `TEST_IEC_STRING_FUNCTION_INOUT_REJECTION_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_45B9440A281BFB62649B` / `TEST_RUNTIME_RELOAD_TRANSACTION_001`: `fast` at `crates/trust-runtime/tests/reload_transaction_trace_cases.rs`.
- Scanner `DISC_475B227B49C510855C19` / `TEST_VM_DECLARED_LITERAL_CONTEXT_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_485733554D25202B3795` / `TEST_IEC_STRING_NESTED_FIELD_COPYBACK_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_49D96694751D91832F85` / `TEST_RUNTIME_FORCE_LIFECYCLE_001`: `fast` at `crates/trust-runtime/tests/force_lifecycle.rs`.
- Scanner `DISC_4E0EFE2F6CE2913C5C99` / `TEST_RUNTIME_WATCHDOG_BEFORE_OUTPUT_COMMIT_001`: `fast` at `crates/trust-runtime/tests/runtime_watchdog_output_case.rs`.
- Scanner `DISC_4E9CC7F9F0911410DC2E` / `TEST_IEC_STRING_FUNCTION_RECEIVER_BOUND_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_4F07551DC5298B929A70` / `TEST_VM_REAL_BINARY_OVERFLOW_INTEGRATION_001`: `fast` at `crates/trust-runtime/tests/errors_policy.rs`.
- Scanner `DISC_5210F54B3E0F89CA2DE0` / `TEST_RUNTIME_RETAIN_NONFINITE_SAVE_001`: `fast` at `crates/trust-runtime/tests/retain_integrity.rs`.
- Scanner `DISC_5416CCD3FD48F9F31374` / `TEST_RUNTIME_MQTT_NONFINITE_PROCESS_IMAGE_READ_001`: `fast` at `crates/trust-runtime/src/io/mqtt/point_map.rs`.
- Scanner `DISC_547AC119BF2B3BEFB960` / `TEST_VM_SUBRANGE_ASSIGNMENT_REJECTION_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_55DE77B00C6511880E2E` / `TEST_IEC_STRING_IMPORTED_CAPACITY_001`: `fast` at `crates/trust-hir/src/db/queries/database/database_tests_part_01.rs`.
- Scanner `DISC_55E928EED7B9D9120424` / `TEST_IEC_STRING_LOCAL_OUTPUT_COPYBACK_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_56F608E0FA7014D18845` / `TEST_VM_COERCION_NARROWING_ASSIGNMENT_REJECTION_001`: `fast` at `crates/trust-runtime/tests/coercion_proof.rs`.
- Scanner `DISC_586DFB900FD4BB4178DD` / `TEST_RUNTIME_FORCE_RELEASE_001`: `fast` at `crates/trust-runtime/tests/force_lifecycle_boundaries.rs`.
- Scanner `DISC_5CA02219B3E4D1B01B31` / `TEST_RUNTIME_MQTT_NONFINITE_BATCH_TRANSACTION_001`: `fast` at `crates/trust-runtime/src/io/mqtt/tests.rs`.
- Scanner `DISC_5D4CC58AB74F35B9E2A9` / `TEST_RUNTIME_MQTT_NONFINITE_TEXT_DECODE_001`: `fast` at `crates/trust-runtime/src/io/mqtt/point_map.rs`.
- Scanner `DISC_5FF8968BF2E1CC6926BC` / `TEST_VM_REAL_NAMED_OVERFLOW_DIRECT_001`: `fast` at `crates/trust-runtime/tests/stdlib_numeric.rs`.
- Scanner `DISC_6C10B5D7ACB21ECE8E9D` / `TEST_RUNTIME_MODBUS_SLOW_WRITE_BOUND_001`: `fast` at `crates/trust-runtime/tests/modbus_driver.rs`.
- Scanner `DISC_6D4BCBF011BA193AD9AB` / `TEST_RUNTIME_RETAIN_NONFINITE_LOAD_001`: `fast` at `crates/trust-runtime/tests/retain_integrity.rs`.
- Scanner `DISC_6E9FF1D629FBEFC55BDE` / `TEST_RUNTIME_OPCUA_NONFINITE_INGRESS_001`: `fast` at `crates/trust-runtime/src/host/opcua/tests.rs`.
- Scanner `DISC_6F61BF50F054D14B12B8` / `TEST_RUNTIME_TYPED_REAL_OUTPUT_NONFINITE_001`: `fast` at `crates/trust-runtime/src/io/interface.rs`.
- Scanner `DISC_6FCA577549BA14A407BB` / `TEST_RUNTIME_SIMULATION_THRESHOLD_NONFINITE_001`: `fast` at `crates/trust-runtime/tests/simulation_workflow.rs`.
- Scanner `DISC_7159C3BA77CC33C8F48C` / `TEST_RUNTIME_MODBUS_SLOW_READ_BOUND_001`: `fast` at `crates/trust-runtime/tests/modbus_driver.rs`.
- Scanner `DISC_716FA81FC2506FBA1592` / `TEST_DEBUG_MANAGED_LREAL_NONFINITE_001`: `fast` at `crates/trust-debug/src/adapter/tests_part_01.rs`.
- Scanner `DISC_725D6C1E9787F7B1D630` / `TEST_IEC_WSTRING_LOCAL_ALIAS_COPYBACK_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_726176040625B846A7AD` / `TEST_RUNTIME_FORCE_DISCONNECT_001`: `fast` at `crates/trust-runtime/tests/force_lifecycle_boundaries.rs`.
- Scanner `DISC_733F49BAD3FF00FA4D54` / `TEST_RUNTIME_TYPED_INPUT_NONFINITE_001`: `fast` at `crates/trust-runtime/src/io/interface.rs`.
- Scanner `DISC_83FE96819CB25F21CA77` / `TEST_RUNTIME_RESTART_WARM_STORAGE_001`: `fast` at `crates/trust-runtime/tests/runtime_restart.rs`.
- Scanner `DISC_846E4BECA070B0C77CDA` / `TEST_RUNTIME_ADS_NONFINITE_SCALAR_DECODE_001`: `fast` at `crates/trust-ads-core/src/mapping.rs`.
- Scanner `DISC_852E381BD310A63FCE71` / `TEST_RUNTIME_FORCE_FAULT_001`: `fast` at `crates/trust-runtime/tests/force_lifecycle_boundaries.rs`.
- Scanner `DISC_8664E7408C2C3D60D5C5` / `TEST_DEBUG_AUTHORIZATION_MATRIX_001`: `fast` at `crates/trust-runtime/src/control/tests/authorization_matrix.rs`.
- Scanner `DISC_88F921D24D3708CEF3E1` / `TEST_BYTECODE_CONTAINER_INVALID_MAGIC`: `fast` at `crates/trust-runtime/tests/bytecode_container.rs`.
- Scanner `DISC_8925B3C0E6A786597E4B` / `TEST_RUNTIME_TYPED_OUTPUT_DRIVER_COMMIT_001`: `fast` at `crates/trust-runtime/tests/runtime_safety_fail_closed.rs`.
- Scanner `DISC_8D254A3AADEDD4121B5F` / `TEST_RUNTIME_SAFE_STATE_MQTT_PENDING_001`: `fast` at `crates/trust-runtime/src/io/mqtt/tests/safe_state.rs`.
- Scanner `DISC_8E59F13125886D732DA2` / `TEST_VM_DECLARED_PARAMETER_COPY_IN_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_8F0C898E72D907AA065A` / `TEST_RUNTIME_ADS_NONFINITE_CLIENT_INGRESS_001`: `fast` at `crates/trust-runtime/src/host/ads/tests.rs`.
- Scanner `DISC_934C37AAF7B29C216166` / `TEST_VM_COERCION_INITIALIZER_WIDENING_001`: `fast` at `crates/trust-runtime/tests/coercion_proof.rs`.
- Scanner `DISC_96A8793BF2C3E6F579B4` / `TEST_RUNTIME_PANIC_CYCLE_GUARD_001`: `fast` at `crates/trust-runtime/src/scheduler/runner_loop.rs`.
- Scanner `DISC_9957C9AF2898026C2E6A` / `TEST_IEC_STRING_FUNCTION_RESULT_BOUND_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_99710F5C15A625CF2D08` / `TEST_RUNTIME_FORCE_PAUSE_RESUME_001`: `fast` at `crates/trust-runtime/tests/force_lifecycle_boundaries.rs`.
- Scanner `DISC_997B38E1827A6BA3FD56` / `TEST_IEC_STRING_UNICODE_ASSIGNMENT_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_9DB502BC879C74994C6A` / `TEST_RUNTIME_TYPED_LREAL_OUTPUT_TRANSACTION_001`: `fast` at `crates/trust-runtime/src/io/interface.rs`.
- Scanner `DISC_9FD52F2E664756D5B3A7` / `TEST_IEC_STRING_FUNCTION_OUTPUT_COPYBACK_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_A2698249E8827008FE24` / `TEST_IEC_STRING_FB_INOUT_REJECTION_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_A66997F12A4CE2CE2018` / `TEST_RUNTIME_HMI_NONFINITE_WRITE_001`: `fast` at `crates/trust-runtime/src/control/tests/hmi_values_write.rs`.
- Scanner `DISC_A778FE822F9A808E700F` / `TEST_VM_REAL_BINARY_OVERFLOW_UNIT_001`: `fast` at `crates/trust-runtime-core/src/program_model/ops.rs`.
- Scanner `DISC_AEDAB26C7EA182A1DA20` / `TEST_VM_REAL_NAMED_OVERFLOW_INTEGRATION_001`: `fast` at `crates/trust-runtime/tests/errors_policy.rs`.
- Scanner `DISC_B4F19BDADF413705E697` / `TEST_RUNTIME_ADS_NONFINITE_SERVER_INGRESS_001`: `fast` at `crates/trust-runtime/src/host/ads/server/tests.rs`.
- Scanner `DISC_B7E97F9BAA11D0743107` / `TEST_VM_COERCION_INOUT_NARROWING_REJECTION_001`: `fast` at `crates/trust-runtime/tests/coercion_proof.rs`.
- Scanner `DISC_C1C1116CE99213C96040` / `TEST_VM_DECLARED_DINT_CONVERSION_PATHS_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_C46D4AB834A6E974B42E` / `TEST_RUNTIME_MESH_NONFINITE_INGRESS_001`: `fast` at `crates/trust-runtime/src/host/mesh/tests.rs`.
- Scanner `DISC_C5A04B37E39DDBE237C5` / `TEST_IEC_TIMER_TRACE_001`: `fast` at `crates/trust-runtime/tests/iec_timer_trace_cases.rs`.
- Scanner `DISC_CA34AD06400345CAADED` / `TEST_RUNTIME_FORCE_AUTH_CHANGE_001`: `fast` at `crates/trust-runtime/src/control/tests/debug_mutation_lifecycle.rs`.
- Scanner `DISC_D5C4C1D59854BC4EA981` / `TEST_RUNTIME_SAFE_STATE_HANDOFF_001`: `fast` at `crates/trust-runtime/tests/safe_state_handoff_trace_cases.rs`.
- Scanner `DISC_D76D0BCB14E615250E0B` / `TEST_VM_SUBRANGE_REF_WRITE_REJECTION_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_DF6BE740DA85943739A5` / `TEST_RUNTIME_SAFE_STATE_MODBUS_PENDING_001`: `fast` at `crates/trust-runtime/tests/modbus_driver.rs`.
- Scanner `DISC_E6963439801BFF301755` / `TEST_RUNTIME_PANIC_SAFE_OUTPUT_001`: `fast` at `crates/trust-runtime/tests/runtime_safety_fail_closed.rs`.
- Scanner `DISC_E6ED69DD8B97B244A411` / `TEST_IEC_STRING_INOUT_ALIAS_001`: `fast` at `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs`.
- Scanner `DISC_EE29217C8E428AA1539E` / `TEST_RUNTIME_MODBUS_NONFINITE_PROCESS_IMAGE_READ_001`: `fast` at `crates/trust-runtime/src/io/modbus/point_map.rs`.
- Scanner `DISC_EE9408494F62E80474E3` / `TEST_RUNTIME_ADS_NONFINITE_ARRAY_DECODE_001`: `fast` at `crates/trust-ads-core/src/mapping.rs`.
- Scanner `DISC_EEF14FDB800A7590C8F3` / `TEST_RUNTIME_PANIC_THREAD_GUARD_001`: `fast` at `crates/trust-runtime/tests/runtime_safety_fail_closed.rs`.
- Scanner `DISC_EFCA6D0FAFC2EB9C01AA` / `TEST_IEC_WSTRING_FB_OUTPUT_COPYBACK_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_F103626E494B610C2A79` / `TEST_VM_COERCION_RETURN_WIDENING_001`: `fast` at `crates/trust-runtime/tests/coercion_proof.rs`.
- Scanner `DISC_F13122338B6C009DFE6D` / `TEST_RUNTIME_SAFE_STATE_STOP_DEGRADED_001`: `fast` at `crates/trust-runtime/tests/runtime_safety_fail_closed.rs`.
- Scanner `DISC_F2E62E25E1A58826EE7D` / `TEST_RUNTIME_RESTART_AUTOMATIC_STORAGE_001`: `fast` at `crates/trust-runtime/src/scheduler/runner_loop.rs`.
- Scanner `DISC_F4845D4DC06C49AF59D6` / `TEST_RUNTIME_PAUSE_WATCHDOG_001`: `fast` at `crates/trust-runtime/tests/debug_pause_watchdog.rs`.
- Scanner `DISC_F7316A527BB51A15C20F` / `TEST_IEC_STRING_IMPORTED_MISMATCH_001`: `fast` at `crates/trust-hir/src/db/queries/database/database_tests_part_01.rs`.
- Scanner `DISC_F85382D0B4505D5984DB` / `TEST_VM_COERCION_ASSIGNMENT_WIDENING_001`: `fast` at `crates/trust-runtime/tests/coercion_proof.rs`.
- Scanner `DISC_FFED59DF8ADBF82880D8` / `TEST_IEC_STRING_UNICODE_LITERAL_BOUND_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Artifact `TEST_BYTECODE_VALIDATOR_MUTATION_SHARD_001`: `slow` `mutation_shard_runner` at `scripts/bytecode_validator_mutation.py`; suites `nightly`.
- Artifact `TEST_CASE_TABLE_VM_SEAM_DECLARED_TYPE_001`: `fast` `case_table_artifact` at `verification/cases/bytecode_vm/VM_SEAM_DECLARED_TYPE_001.toml`; suites `veryquick`.
- Artifact `TEST_CASE_TABLE_VM_SEAM_STRING_BOUND_001`: `fast` `case_table_artifact` at `verification/cases/bytecode_vm/VM_SEAM_STRING_BOUND_001.toml`; suites `veryquick`.
- Artifact `TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001`: `fast` `case_table_artifact` at `verification/cases/bytecode_vm/VM_SEAM_SUBRANGE_001.toml`; suites `veryquick`.
- Artifact `TEST_CASE_TABLE_VM_SEAM_VALID_001`: `fast` `case_table_artifact` at `verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml`; suites `veryquick`.
- Commandless suites: `supporting_local`
- Placeholder suites: none
- Catalog rows without suite tiers: `TEST_BYTECODE_CONTAINER_INVALID_MAGIC`
- Unknown assigned suites: none

## Reviewed Proposal Decisions

- `TEST_REFACTOR_BYTECODE_CONTAINER_INVALID_MAGIC_001`: disposition `no_refactor_needed`, supported `yes`, sources `crates/trust-runtime/tests/bytecode_container.rs`, observed signals none.

## Limitations

- Large-file findings are mechanical line counts at the reviewed inclusive threshold.
- Mixed-purpose findings require multiple reviewed catalog areas or test classes; names and source text never establish purpose.
- Broad-claim findings require multiple catalog invariants; catalog v2 has no authorized coverage-dimension field.
- Duplicate findings compare committed whole-file bytes and whitespace-normalized whole-file text; they do not infer semantic similarity.
- Fixture helper functions and helper-only files are not assessed as duplicate fixtures in this slice.
- Malformed-input overlap comes only from explicit malformed_input_class_ids in reviewed catalog rows.
- Duration classifications come only from hand-owned catalog metadata; unclassified scanner facts receive no inferred duration.
- A supported proposal means its disposition agrees with visible assessment signals; it does not authorize a move, split, or rename.
- Mechanical signals never authorize a move, rename, or split; change dispositions remain unsupported in this v1 assessment.
- The single-identity proposal model refuses split rather than under-modeling multiple targets.
- Completed moves and renames require case-file-bound lock proof; catalog rows without that binding remain blocked.
- The mutable evidence index is globally validated but excluded from the report digest closure to avoid self-reference.
- Platform is historical generation provenance; at-rest validation cannot rederive a prior host platform.
