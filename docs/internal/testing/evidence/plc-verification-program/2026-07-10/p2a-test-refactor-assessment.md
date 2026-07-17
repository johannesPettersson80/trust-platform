# Existing-Test Refactor Assessment

Generator: `test-refactor-assessment v1`
Source revision: `3a53c0c1b936464ed515300b591b541f2b4c1e01`
Generated: `2026-07-17T05:25:00+02:00`
Platform: `linux-x86_64`
Generated JSON SHA-256: `e1d7449d434327e491949f9350689ed8bb2bd988a84a3fcb2b75590bafd100c6`
Input SHA-256: `sha256:30808bc22219e43d9dad92b457265a83e1fe4be67b75b53e398d1e4970c51b15`

Size is a review signal, not a refactor decision.
Mechanical similarity is candidate evidence only; it never authorizes
a move, split, rename, fixture merge, or behavior change.

## Summary

- Scanner facts: 3990
- Fact-bearing files: 717
- Large-file candidates: 24
- Reviewed mapping-diversity candidates: 15
- Broad multi-invariant claim candidates: 9
- Exact fact-file duplicate groups: 0
- Whitespace-normalized fact-file duplicate groups: 0
- Exact case-input duplicate groups: 3
- Same-table structural case-input peer groups: 34
- Shared case-file reference groups: 7
- Malformed-class overlap groups: 8
- VS Code facts: 456
- VS Code files: 38
- VS Code registrations: 38
- Large registered VS Code files: 5
- Catalog records: 214
- Scanner facts with reviewed duration: 206
- Scanner facts without reviewed duration: 3784
- Catalog rows explicitly classified slow: 3
- Reviewed proposal decisions: 1
- Assessment-supported decisions: 1

## Large Or Mixed-Purpose Signals

| Path | Lines | Facts | Reviewed mappings | Signals |
| --- | ---: | ---: | ---: | --- |
| `crates/trust-ads-server/src/commands/tests.rs` | 1287 | 43 | 0 | `large_file` |
| `crates/trust-ads-server/src/listener.rs` | 1374 | 16 | 0 | `large_file` |
| `crates/trust-debug/src/adapter/tests_part_02.rs` | 1175 | 13 | 0 | `large_file` |
| `crates/trust-hir/src/openot_authoring.rs` | 2868 | 22 | 0 | `large_file` |
| `crates/trust-hir/src/symbols/table.rs` | 1136 | 3 | 0 | `large_file` |
| `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | 1295 | 69 | 0 | `large_file` |
| `crates/trust-hir/tests/semantic_type_checking/bounded_value_semantics.rs` | 190 | 6 | 6 | `reviewed_mapping_diversity` |
| `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | 1316 | 23 | 0 | `large_file` |
| `crates/trust-lsp/src/handlers/tests/core_part_01.rs` | 498 | 13 | 6 | `reviewed_mapping_diversity` |
| `crates/trust-lsp/src/handlers/tests/lsp_trace_cases.rs` | 431 | 3 | 3 | `reviewed_mapping_diversity` |
| `crates/trust-runtime/src/bin/trust-runtime/ads.rs` | 1562 | 2 | 0 | `large_file` |
| `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | 1543 | 49 | 0 | `large_file` |
| `crates/trust-runtime/src/config/tests.rs` | 1154 | 71 | 0 | `large_file` |
| `crates/trust-runtime/src/control/comm_handlers/browse_symbols.rs` | 1123 | 7 | 0 | `large_file` |
| `crates/trust-runtime/src/control/tests/core.rs` | 5785 | 71 | 0 | `large_file` |
| `crates/trust-runtime/src/control/tests/hmi_values_write.rs` | 884 | 14 | 4 | `reviewed_mapping_diversity` |
| `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | 1291 | 29 | 0 | `large_file` |
| `crates/trust-runtime/src/host/ads/tests.rs` | 1404 | 36 | 1 | `large_file` |
| `crates/trust-runtime/src/io/mqtt/tests.rs` | 1240 | 31 | 1 | `large_file` |
| `crates/trust-runtime/src/runtime/vm/type_policy/tests.rs` | 441 | 7 | 7 | `reviewed_mapping_diversity` |
| `crates/trust-runtime/tests/bounded_value_semantics.rs` | 222 | 6 | 6 | `reviewed_mapping_diversity` |
| `crates/trust-runtime/tests/bytecode_decode_resource_bounds.rs` | 302 | 7 | 7 | `reviewed_mapping_diversity` |
| `crates/trust-runtime/tests/bytecode_verification_cases.rs` | 315 | 2 | 2 | `reviewed_mapping_diversity` |
| `crates/trust-runtime/tests/bytecode_vm_core/ref_validation.rs` | 281 | 9 | 2 | `reviewed_mapping_diversity` |
| `crates/trust-runtime/tests/debug_pause_watchdog.rs` | 122 | 2 | 2 | `reviewed_mapping_diversity` |
| `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st` | 1748 | 19 | 0 | `large_file` |
| `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | 2869 | 50 | 0 | `large_file` |
| `crates/trust-runtime/tests/modbus_driver.rs` | 907 | 23 | 4 | `reviewed_mapping_diversity` |
| `crates/trust-runtime/tests/openot_telemetry.rs` | 3148 | 37 | 0 | `large_file` |
| `crates/trust-runtime/tests/phase11_seam_contract.rs` | 1261 | 22 | 21 | `large_file`, `reviewed_mapping_diversity` |
| `crates/trust-runtime/tests/retain_integrity.rs` | 539 | 13 | 3 | `reviewed_mapping_diversity` |
| `crates/trust-runtime/tests/runtime_safety_fail_closed.rs` | 800 | 17 | 5 | `reviewed_mapping_diversity` |
| `crates/trust-syntax/tests/parser_error_recovery.rs` | 218 | 12 | 6 | `reviewed_mapping_diversity` |
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
- `TEST_IEC_CONTEXT_LITERAL_CONVERSION_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_CONTEXT_LITERAL_PRECISION_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_CROSS_FAMILY_CONVERSION_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_IMPLICIT_CONVERSION_MATRIX_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_LOSSY_TYPED_FLOAT_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_SUBRANGE_INITIALIZER_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_STRING_INOUT_ALIAS_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_STRING_INOUT_DIAGNOSTIC_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_SUBRANGE_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_EDITOR_RENAME_CASE_INSENSITIVE_SCOPE_001` claims 1 invariants; result `single_invariant`.
- `TEST_EDITOR_RENAME_CASE_ONLY_001` claims 1 invariants; result `single_invariant`.
- `TEST_EDITOR_RENAME_CROSS_FILE_CAPTURE_001` claims 2 invariants; result `candidate_missing_coverage_dimensions`.
- `TEST_EDITOR_RENAME_FIELD_COLLISION_001` claims 1 invariants; result `single_invariant`.
- `TEST_EDITOR_RENAME_IMPORTED_ORIGIN_CONFLICT_001` claims 1 invariants; result `single_invariant`.
- `TEST_EDITOR_RENAME_PROJECT_POU_COLLISION_001` claims 1 invariants; result `single_invariant`.
- `TEST_EDITOR_RENAME_SHADOW_CAPTURE_001` claims 1 invariants; result `single_invariant`.
- `TEST_LSP_POSITION_ENCODING_LINE_ENDINGS_001` claims 1 invariants; result `single_invariant`.
- `TEST_LSP_POSITION_ENCODING_SYNC_001` claims 1 invariants; result `single_invariant`.
- `TEST_LSP_POSITION_ENCODING_BARE_CR_HOVER_001` claims 1 invariants; result `single_invariant`.
- `TEST_LSP_POSITION_ENCODING_HOVER_001` claims 1 invariants; result `single_invariant`.
- `TEST_LSP_PULL_DIAGNOSTIC_CANCELLATION_001` claims 1 invariants; result `single_invariant`.
- `TEST_LSP_PUSH_DIAGNOSTIC_CANCELLATION_001` claims 1 invariants; result `single_invariant`.
- `TEST_LSP_RENAME_CONFLICT_ERROR_001` claims 1 invariants; result `single_invariant`.
- `TEST_LSP_WORKSPACE_DIAGNOSTIC_CANCELLATION_001` claims 1 invariants; result `single_invariant`.
- `TEST_LSP_DOCUMENT_CLOSE_DISK_RELOAD_001` claims 1 invariants; result `single_invariant`.
- `TEST_LSP_POSITION_ENCODING_SEMANTIC_TOKENS_001` claims 1 invariants; result `single_invariant`.
- `TEST_LSP_DIAGNOSTIC_CANCELLATION_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_LSP_DOCUMENT_CLOSE_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_LSP_POSITION_ENCODING_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_EDITOR_RENAME_LOCAL_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_EDITOR_RENAME_PROJECT_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_LSP_POSITION_ENCODING_HANDSHAKE_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_STABLE_ERROR_CODE_MODEL_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_REAL_BINARY_OVERFLOW_UNIT_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_NORMALIZE_NON_WIDENING_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_TRAP_STABLE_ERROR_CODE_MODEL_001` claims 1 invariants; result `single_invariant`.
- `TEST_PROTOCOL_DISCOVERY_CONFIDENCE_TRACE_001` claims 3 invariants; result `candidate_missing_coverage_dimensions`.
- `TEST_CONTROL_AUTHORIZATION_FAILSAFE_001` claims 1 invariants; result `single_invariant`.
- `TEST_DEBUG_AUTHORIZATION_MATRIX_001` claims 2 invariants; result `candidate_missing_coverage_dimensions`.
- `TEST_CONTROL_AUTHORIZATION_TRACE_001` claims 2 invariants; result `candidate_missing_coverage_dimensions`.
- `TEST_RUNTIME_FORCE_AUTH_CHANGE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_HMI_NONFINITE_WRITE_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_HMI_STRING_CAPACITY_ERROR_CODE_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_HMI_TYPE_ERROR_CODE_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_SUBRANGE_HMI_WRITE_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_ADS_NONFINITE_SERVER_INGRESS_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_ADS_NONFINITE_CLIENT_INGRESS_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_MESH_NONFINITE_INGRESS_001` claims 1 invariants; result `single_invariant`.
- `TEST_OPCUA_CLIENT_LIFECYCLE_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_OPCUA_NONFINITE_INGRESS_001` claims 1 invariants; result `single_invariant`.
- `TEST_ETHERCAT_UNAVAILABLE_RESOURCE_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_TYPED_INPUT_NONFINITE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_TYPED_LREAL_OUTPUT_TRANSACTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_TYPED_REAL_OUTPUT_NONFINITE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_MODBUS_NONFINITE_PROCESS_IMAGE_READ_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_MODBUS_NONFINITE_WIRE_DECODE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_MQTT_NONFINITE_PROCESS_IMAGE_READ_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_MQTT_NONFINITE_TEXT_DECODE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_MQTT_NONFINITE_BATCH_TRANSACTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_SAFE_STATE_MQTT_PENDING_001` claims 2 invariants; result `candidate_missing_coverage_dimensions`.
- `TEST_VM_DECLARED_TYPE_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_PRIMITIVE_FLOAT_WIDENING_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_PRIMITIVE_TAG_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_STRING_BOUND_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_STRING_PRIMITIVE_POLICY_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_SUBRANGE_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_SUBRANGE_TYPE_POLICY_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_PANIC_CYCLE_GUARD_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_RESTART_AUTOMATIC_STORAGE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_RESTART_STORAGE_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_BOUNDED_FLOAT_MATERIALIZATION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_BOUNDED_TYPED_FLOAT_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_CALL_ARGUMENT_TYPE_MATERIALIZATION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_FOR_CONTROL_TYPE_MATERIALIZATION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_FUNCTION_RETURN_TYPE_MATERIALIZATION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_SUBRANGE_BOUNDS_TRANSACTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_CHECKSUM_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_CONTAINER_DUPLICATE_STANDARD_SECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_CONTAINER_INVALID_MAGIC` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_MISSING_OWNER_FIELD_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_SCHEMA_TAG_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_VERSION_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_FIXED_SECTION_COUNT_BOUND_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_NESTED_POU_COUNT_BOUND_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_NESTED_REFERENCE_COUNT_BOUND_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_NESTED_RESOURCE_COUNT_BOUND_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_NESTED_TYPE_COUNT_BOUND_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_STRING_TABLE_COUNT_BOUND_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_TYPE_TABLE_COUNT_BOUND_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_UNKNOWN_OPTIONAL_SECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_CALL_TARGET_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_JUMP_BOUNDARY_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_MISSING_SECTION_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_OWNER_SHARED_FRAME_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_TRANSFORM_SEED_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_VALIDATOR_CASES_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_LOWERING_SUPPORTED_CONTROL_FLOW_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_LOWERING_UNSUPPORTED_STATEMENT_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_LOCAL_REF_PATH_ACCEPTANCE_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_LOCAL_REF_RANGE_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_ENCODER_FAIL_CLOSED_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_OSCAT_BINARY_TYPE_PRESERVATION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_COERCION_ASSIGNMENT_WIDENING_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_COERCION_FUNCTION_INPUT_WIDENING_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_COERCION_FUNCTION_OUTPUT_WIDENING_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_COERCION_INITIALIZER_WIDENING_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_COERCION_INOUT_NARROWING_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_COERCION_NARROWING_ASSIGNMENT_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_COERCION_RETURN_WIDENING_001` claims 1 invariants; result `single_invariant`.
- `TEST_DEBUG_PAUSE_WATCHDOG_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_PAUSE_WATCHDOG_001` claims 1 invariants; result `single_invariant`.
- `TEST_DEBUG_PAUSE_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_REAL_BINARY_OVERFLOW_INTEGRATION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_REAL_NAMED_OVERFLOW_INTEGRATION_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_TIMER_TOF_DURATION_OVERFLOW_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_TIMER_TON_DURATION_OVERFLOW_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_TIMER_TP_DURATION_OVERFLOW_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_TIMER_RUNTIME_VARIANTS_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_FORCE_LIFECYCLE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_FORCE_DISCONNECT_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_FORCE_FAULT_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_FORCE_PAUSE_RESUME_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_FORCE_RELEASE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_FORCE_STOP_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_PRECEDENCE_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_STRING_BINDING_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_TIMER_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_MODBUS_NONFINITE_MAPPED_READ_TRANSACTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_MODBUS_SLOW_READ_BOUND_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_MODBUS_SLOW_WRITE_BOUND_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_SAFE_STATE_MODBUS_PENDING_001` claims 2 invariants; result `candidate_missing_coverage_dimensions`.
- `TEST_VM_OSCAT_OOP_CORE_REGRESSION_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_ARITHMETIC_TYPE_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_INOUT_LITERAL_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_LEGACY_CALL_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_OWNER_PRODUCT_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_OWNER_VALIDATOR_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_PARAMETER_DIRECTION_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_REF_ESCAPE_PRODUCT_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_REF_ESCAPE_VALIDATOR_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_STACK_EXIT_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_STACK_UNDERFLOW_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_STORE_TYPE_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_DECLARED_DINT_CONVERSION_PATHS_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_DECLARED_DINT_RUNTIME_TAG_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_DECLARED_LITERAL_CONTEXT_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_DECLARED_PARAMETER_COPY_IN_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_DECLARED_REAL_CONVERSION_PATHS_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_DECLARED_REAL_RUNTIME_TAG_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_LOWERING_UNSUPPORTED_EXPRESSION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_SUBRANGE_ASSIGNMENT_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_SUBRANGE_FB_INPUT_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_SUBRANGE_REF_WRITE_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_OWNER_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_REF_ESCAPE_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_PLCOPEN_IMPORT_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_CONNECTOR_STATUS_TRUTH_TRACE_001` claims 3 invariants; result `candidate_missing_coverage_dimensions`.
- `TEST_RUNTIME_RELOAD_TRANSACTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_RETAIN_FAILURE_ATOMICITY_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_RETAIN_NONFINITE_LOAD_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_RETAIN_NONFINITE_SAVE_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_SUBRANGE_RETAIN_RELOAD_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_IO_BOUND_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_PANIC_TRACE_001` claims 1 invariants; result `single_invariant`.
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
- `TEST_VM_RESOURCE_LIMIT_CASES_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_PARSER_DEEP_PAREN_BOUND_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_PARSER_DEEP_UNARY_BOUND_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_PARSER_MISSING_POU_TERMINATOR_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_PARSER_NESTED_TERMINATOR_RECOVERY_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_PARSER_REQUIRED_DELIMITERS_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_PARSER_UNKNOWN_TOKEN_PROGRESS_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_PARSER_UNCLOSED_CALL_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_PARSER_RECOVERY_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_VALIDATOR_MUTATION_SHARD_001` claims 1 invariants; result `single_invariant`.
- `TEST_CASE_TABLE_VM_SEAM_DECLARED_TYPE_001` claims 1 invariants; result `single_invariant`.
- `TEST_CASE_TABLE_VM_SEAM_ENC_001` claims 1 invariants; result `single_invariant`.
- `TEST_CASE_TABLE_VM_SEAM_OWNER_001` claims 1 invariants; result `single_invariant`.
- `TEST_CASE_TABLE_VM_SEAM_REF_001` claims 1 invariants; result `single_invariant`.
- `TEST_CASE_TABLE_VM_SEAM_STRING_BOUND_001` claims 1 invariants; result `single_invariant`.
- `TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001` claims 1 invariants; result `single_invariant`.
- `TEST_CASE_TABLE_VM_SEAM_VALID_001` claims 1 invariants; result `single_invariant`.

## Duplicate And Structural Signals

- Exact fact-file groups: 0
- Whitespace-normalized fact-file groups: 0
- Exact case-input groups: 3
- Same-table structural case-input peer groups: 34
- Shared case-file reference groups: 7
- Explicit malformed-class overlap groups: 8
- Free-form source-body similarity: `not_assessed`
- Exact case input `sha256:c2467975b7077a659ba0d19744fe27cee0569c411db242fb93a8f176f8eac6f8`: cases `SEC_AUTHZ_001_OPERATOR_FORCE_DENIED`, `SEC_AUTHZ_001_VIEWER_FORCE_DENIED`, `SEC_AUTHZ_001_VIEWER_RELEASE_DENIED`; files `verification/cases/control_security/SEC_AUTHZ_001.toml`.
- Exact case input `sha256:c670cbf91197596289caf54141e9ad5505065ef61ebd999029f415beb97a26be`: cases `RT_SAFE_IO_WORKER_001_FAILURE_DOES_NOT_SKIP_LATER_DRIVER`, `RT_SAFE_IO_WORKER_001_FAULTED_HANDOFF_FAULTS_STOP`, `RT_SAFE_STOP_001_DEGRADED_HANDOFF_FAULTS_STOP`; files `verification/cases/runtime_safety/RT_SAFE_STOP_001.toml`.
- Exact case input `sha256:d3907b16d05f6e748252246e5902055235d2da62e339dd08af0ad89041a6db31`: cases `SEC_AUTHZ_001_ADMIN_ACTIVATION_ALLOWED`, `SEC_AUTHZ_001_ENGINEER_FORCE_ALLOWED`, `SEC_AUTHZ_001_ENGINEER_RELEASE_ALLOWED`; files `verification/cases/control_security/SEC_AUTHZ_001.toml`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_DECLARED_TYPE_001.toml`: `VM_SEAM_DECLARED_TYPE_001_ACCURACY_PRESERVING_INTEGER_TO_FLOAT_1089AD45`, `VM_SEAM_DECLARED_TYPE_001_COMPATIBLE_LITERAL_INITIALIZER_WIDENING_D8DF321B`, `VM_SEAM_DECLARED_TYPE_001_COMPATIBLE_TYPED_ASSIGNMENT_WIDENING_A6856BB4`, `VM_SEAM_DECLARED_TYPE_001_FUNCTION_RESULT_WIDENING_E52C15B0`, `VM_SEAM_DECLARED_TYPE_001_INT_EXPRESSION_TO_DINT_SLOT_9BF228AA`, `VM_SEAM_DECLARED_TYPE_001_INT_LITERAL_TO_REAL_SLOT_04979927`, `VM_SEAM_DECLARED_TYPE_001_INT_VARIABLE_TO_REAL_SLOT_C3821866`, `VM_SEAM_DECLARED_TYPE_001_POU_OUTPUT_WIDENING_8D957490`; shape `sha256:1be9655b754ca9d884c27bd43460558fb86cb55d995475a1d19b276b5de60ecc`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_DECLARED_TYPE_001.toml`: `VM_SEAM_DECLARED_TYPE_001_WRONG_TYPE_1A1BA936`, `VM_SEAM_DECLARED_TYPE_001_WRONG_TYPE_33057268`, `VM_SEAM_DECLARED_TYPE_001_WRONG_TYPE_3D6F1197`, `VM_SEAM_DECLARED_TYPE_001_WRONG_TYPE_6346A636`, `VM_SEAM_DECLARED_TYPE_001_WRONG_TYPE_77B93993`, `VM_SEAM_DECLARED_TYPE_001_WRONG_TYPE_D8E1DB83`; shape `sha256:0d790842b7e2d96191704c9f07776e47e75c71b2d7dec4c06ff4ba9f0187a606`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_DETERMINISM_LIMITS_001.toml`: `VM_SEAM_DETERMINISM_LIMITS_001_CALL_DEPTH_ABOVE_MAX`, `VM_SEAM_DETERMINISM_LIMITS_001_CONTAINER_ABOVE_MAX`, `VM_SEAM_DETERMINISM_LIMITS_001_EXECUTION_ABOVE_MAX`, `VM_SEAM_DETERMINISM_LIMITS_001_INSTRUCTIONS_ABOVE_MAX`, `VM_SEAM_DETERMINISM_LIMITS_001_LOCALS_ABOVE_MAX`, `VM_SEAM_DETERMINISM_LIMITS_001_OSCAT_WITHIN_LIMITS`, `VM_SEAM_DETERMINISM_LIMITS_001_PARAMETERS_ABOVE_MAX`, `VM_SEAM_DETERMINISM_LIMITS_001_REFERENCES_ABOVE_MAX`, `VM_SEAM_DETERMINISM_LIMITS_001_STACK_ABOVE_MAX`; shape `sha256:5583f769954ed3a1265abd03edaee615985ac4cd8dffbd88b1b246bdc76df8e2`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_ENC_001.toml`: `VM_SEAM_ENC_001_SUPPORTED_LOOP_CONTROL_D29CD0E7`, `VM_SEAM_ENC_001_UNSUPPORTED_EXPRESSION_AFTER_VALID_PREFIX_8588F066`, `VM_SEAM_ENC_001_UNSUPPORTED_STATEMENT_695F0675`; shape `sha256:1be9655b754ca9d884c27bd43460558fb86cb55d995475a1d19b276b5de60ecc`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_OWNER_001.toml`: `VM_SEAM_OWNER_001_INSTRUCTION_MIXES_INSTANCE_OWNERS_961CC2A4`, `VM_SEAM_OWNER_001_LOCAL_RANGES_SHARE_FRAME_OWNER_05903843`; shape `sha256:1be9655b754ca9d884c27bd43460558fb86cb55d995475a1d19b276b5de60ecc`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_REF_001.toml`: `VM_SEAM_REF_001_FRAME_LOCAL_REFERENCE_ESCAPES_B4EE64E0`, `VM_SEAM_REF_001_LOCAL_REFERENCE_OUTSIDE_POU_RANGE_8578E25F`; shape `sha256:1be9655b754ca9d884c27bd43460558fb86cb55d995475a1d19b276b5de60ecc`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_STRING_BOUND_001.toml`: `VM_SEAM_STRING_BOUND_001_MAX_D165B4CE`, `VM_SEAM_STRING_BOUND_001_MIN_D165B4CE`; shape `sha256:3940b6625c5f575bfcae24332dd917448996f46e736d442993370d110b01778a`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_SUBRANGE_001.toml`: `VM_SEAM_SUBRANGE_001_MAX_AE132E71`, `VM_SEAM_SUBRANGE_001_MIN_AE132E71`; shape `sha256:74118ebe11ebf1204131d61e0250805ebee19ed88606029843da8d8741cc79ad`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml`: `VM_SEAM_VALID_001_JUMP_TARGET_POU_BODY_JMP_OPERAND_100_DF169D85`, `VM_SEAM_VALID_001_JUMP_TARGET_POU_BODY_JMP_OPERAND__100_4555B79F`; shape `sha256:4c2dbca3a9792afb543b0301d33ce1a8bd127b78dee5e254a7c0f3350539b70f`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml`: `VM_SEAM_VALID_001_TRUNCATE_BEFORE_POU_BODIES_4DDD6D2B`, `VM_SEAM_VALID_001_TRUNCATE_BEFORE_SECTION_TABLE_0E24E3AC`; shape `sha256:e0bd21a1e4c5110f2132f018441b9faa042eff4a8587ab2ce394077f907edf8d`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml`: `VM_SEAM_VALID_001_UNKNOWN_OPCODE_POU_BODY_FIRST_OPCODE_80_36D182BA`, `VM_SEAM_VALID_001_UNKNOWN_OPCODE_POU_BODY_FIRST_OPCODE_FF_3A193633`; shape `sha256:7afbe67384583995479cd4b26ae4dfb1e78bf262fb626d55f38a7b05699ab8e3`.
- Structural peers in `verification/cases/compiler_iec/IEC_PARSE_RECOVER_001.toml`: `IEC_PARSE_RECOVER_001_CASE_LABEL_MISSING_COLON_D638EDB8`, `IEC_PARSE_RECOVER_001_CASE_MISSING_OF_234E8F21`, `IEC_PARSE_RECOVER_001_DEEP_PAREN_NESTING_33E1EE76`, `IEC_PARSE_RECOVER_001_DEEP_UNARY_NESTING_F46F309F`, `IEC_PARSE_RECOVER_001_ELSIF_MISSING_THEN_1BE5365E`, `IEC_PARSE_RECOVER_001_FOR_MISSING_ASSIGNMENT_4CBAB1BD`, `IEC_PARSE_RECOVER_001_FOR_MISSING_CONTROL_VARIABLE_9FD4FFBB`, `IEC_PARSE_RECOVER_001_FOR_MISSING_DO_7365CE24`, `IEC_PARSE_RECOVER_001_FOR_MISSING_TO_709376F0`, `IEC_PARSE_RECOVER_001_MISSING_INNER_TERMINATOR_AT_OUTER_BOUNDARY_D2AE7DC9`, `IEC_PARSE_RECOVER_001_MISSING_POU_TERMINATOR_AT_EOF_CF2D3C17`, `IEC_PARSE_RECOVER_001_REPEAT_MISSING_UNTIL_0B026343`, `IEC_PARSE_RECOVER_001_UNCLOSED_CALL_DELIMITER_378966DA`, `IEC_PARSE_RECOVER_001_UNKNOWN_TOKEN_98BED96C`, `IEC_PARSE_RECOVER_001_WHILE_MISSING_DO_E4A72A43`; shape `sha256:1be9655b754ca9d884c27bd43460558fb86cb55d995475a1d19b276b5de60ecc`.
- Structural peers in `verification/cases/compiler_iec/IEC_PREC_001.toml`: `IEC_PREC_001_ADDITIVE_LEFT_ASSOCIATIVE_780E3108`, `IEC_PREC_001_AND_BEFORE_XOR_BEFORE_OR_6AC7B6D0`, `IEC_PREC_001_COMPARISON_BEFORE_BOOLEAN_AND_5391BC0E`, `IEC_PREC_001_EXPONENTIATION_LEFT_ASSOCIATIVE_21D9554E`, `IEC_PREC_001_MULTIPLICATIVE_BEFORE_ADDITIVE_9A3C35EC`, `IEC_PREC_001_MULTIPLICATIVE_LEFT_ASSOCIATIVE_55CA8AD0`, `IEC_PREC_001_PARENTHESES_OVERRIDE_PRECEDENCE_ED1DEEBA`, `IEC_PREC_001_UNARY_BEFORE_EXPONENTIATION_584586B7`; shape `sha256:1be9655b754ca9d884c27bd43460558fb86cb55d995475a1d19b276b5de60ecc`.
- Structural peers in `verification/cases/compiler_iec/IEC_STRING_001.toml`: `IEC_STRING_001_FB_INOUT_MISMATCH_STRING_AC942A20`, `IEC_STRING_001_FB_INOUT_MISMATCH_WSTRING_F5BDDBEB`, `IEC_STRING_001_FB_INPUT_EQUAL_INOUT_5B247E55`, `IEC_STRING_001_FB_OUTPUT_COPYBACK_STRING_9E0FC54F`, `IEC_STRING_001_FB_OUTPUT_COPYBACK_WSTRING_756A092E`, `IEC_STRING_001_FUNCTION_INOUT_MISMATCH_6DD1624F`, `IEC_STRING_001_FUNCTION_INPUT_EQUAL_INOUT_9D7C4411`, `IEC_STRING_001_FUNCTION_OUTPUT_COPYBACK_STRING_6843B725`, `IEC_STRING_001_FUNCTION_RECEIVER_BOUND_B927E5EF`, `IEC_STRING_001_FUNCTION_RESULT_BOUND_E6D2E79D`, `IEC_STRING_001_HIR_INOUT_EQUAL_ALIAS_DD817223`, `IEC_STRING_001_HIR_INOUT_MISMATCH_1E9D1FD2`, `IEC_STRING_001_IMPORTED_CAPACITY_EQUAL_65EC0D8A`, `IEC_STRING_001_IMPORTED_CAPACITY_MISMATCH_59881371`, `IEC_STRING_001_LOCAL_ALIAS_COPYBACK_WSTRING_AC67D589`, `IEC_STRING_001_LOCAL_OUTPUT_COPYBACK_STRING_8FA2BA21`, `IEC_STRING_001_NESTED_FIELD_COPYBACK_FFDD0C4B`, `IEC_STRING_001_STRING_WSTRING_CROSS_FAMILY_6C05FADD`, `IEC_STRING_001_UNICODE_ASSIGNMENT_BOUND_FA58C3C5`, `IEC_STRING_001_UNICODE_LITERAL_BOUND_8C17CCD9`; shape `sha256:1be9655b754ca9d884c27bd43460558fb86cb55d995475a1d19b276b5de60ecc`.
- Structural peers in `verification/cases/compiler_iec/IEC_SUBRANGE_001.toml`: `IEC_SUBRANGE_001_ABOVE_UPPER_BOUND_INITIALIZER_82099BC8`, `IEC_SUBRANGE_001_BELOW_LOWER_BOUND_INITIALIZER_C22D03A7`, `IEC_SUBRANGE_001_REAL_TO_INT_SUBRANGE_E0D5BF63`; shape `sha256:1be9655b754ca9d884c27bd43460558fb86cb55d995475a1d19b276b5de60ecc`.
- Structural peers in `verification/cases/compiler_iec/IEC_TIMER_001.toml`: `IEC_TIMER_001_TOF_LTIME_POST_EXPIRY_HOLD`, `IEC_TIMER_001_TOF_TIME_POST_EXPIRY_HOLD`, `IEC_TIMER_001_TON_TIME_BASIC_DELAY`, `IEC_TIMER_001_TP_TIME_BASIC_PULSE`; shape `sha256:5583f769954ed3a1265abd03edaee615985ac4cd8dffbd88b1b246bdc76df8e2`.
- Structural peers in `verification/cases/control_security/SEC_AUTHZ_001.toml`: `SEC_AUTHZ_001_ADMIN_ACTIVATION_ALLOWED`, `SEC_AUTHZ_001_ENGINEER_ACTIVATION_DENIED`, `SEC_AUTHZ_001_ENGINEER_FORCE_ALLOWED`, `SEC_AUTHZ_001_ENGINEER_RELEASE_ALLOWED`, `SEC_AUTHZ_001_OPERATOR_FORCE_DENIED`, `SEC_AUTHZ_001_UNCLASSIFIED_ENGINEER_DENIED`, `SEC_AUTHZ_001_VIEWER_FORCE_DENIED`, `SEC_AUTHZ_001_VIEWER_RELEASE_DENIED`; shape `sha256:5583f769954ed3a1265abd03edaee615985ac4cd8dffbd88b1b246bdc76df8e2`.
- Structural peers in `verification/cases/editor_safety/DEBUG_PAUSE_001.toml`: `DEBUG_PAUSE_001_BETWEEN_CYCLES_STARTS_NO_SCAN`, `DEBUG_PAUSE_001_STATEMENT_DWELL_EXCLUDED`; shape `sha256:5583f769954ed3a1265abd03edaee615985ac4cd8dffbd88b1b246bdc76df8e2`.
- Structural peers in `verification/cases/editor_safety/EDIT_DIAG_CANCEL_001.toml`: `EDIT_DIAG_CANCEL_001_CANCELLED_PUSH`, `EDIT_DIAG_CANCEL_001_CANCELLED_TEXT_PULL`, `EDIT_DIAG_CANCEL_001_CANCELLED_WORKSPACE_PULL`, `EDIT_DIAG_CANCEL_001_COMPLETED_PUSH`, `EDIT_DIAG_CANCEL_001_COMPLETED_WORKSPACE_PULL`; shape `sha256:1be9655b754ca9d884c27bd43460558fb86cb55d995475a1d19b276b5de60ecc`.
- Structural peers in `verification/cases/editor_safety/EDIT_DOC_CLOSE_001.toml`: `EDIT_DOC_CLOSE_001_CACHE_INVALIDATION`, `EDIT_DOC_CLOSE_001_DEPENDENT_RECOMPUTE`, `EDIT_DOC_CLOSE_001_PULL_DIAGNOSTIC_RECOMPUTE`, `EDIT_DOC_CLOSE_001_TRACKED_FILE_RELOAD`, `EDIT_DOC_CLOSE_001_UNREADABLE_OR_NON_FILE_REMOVE`; shape `sha256:1be9655b754ca9d884c27bd43460558fb86cb55d995475a1d19b276b5de60ecc`.
- Structural peers in `verification/cases/editor_safety/EDIT_LSP_POS_001.toml`: `EDIT_LSP_POS_001_BARE_CR_LINE_MAPPING`, `EDIT_LSP_POS_001_CRLF_LINE_MAPPING`, `EDIT_LSP_POS_001_EOF_AFTER_TRAILING_NEWLINE`, `EDIT_LSP_POS_001_HOVER_RANGE_AFTER_EMOJI`, `EDIT_LSP_POS_001_INCREMENTAL_EDIT_AFTER_EMOJI`, `EDIT_LSP_POS_001_INITIALIZE_UTF16`, `EDIT_LSP_POS_001_LF_LINE_MAPPING`, `EDIT_LSP_POS_001_OVERLONG_COLUMN_CLAMP`, `EDIT_LSP_POS_001_SEMANTIC_TOKEN_AFTER_EMOJI`; shape `sha256:1be9655b754ca9d884c27bd43460558fb86cb55d995475a1d19b276b5de60ecc`.
- Structural peers in `verification/cases/editor_safety/EDIT_RENAME_001.toml`: `EDIT_RENAME_001_CASE_INSENSITIVE_SCOPE_COLLISION`, `EDIT_RENAME_001_CASE_ONLY_SELF_RENAME`, `EDIT_RENAME_001_DECLARING_SCOPE_COLLISION`, `EDIT_RENAME_001_INVALID_IDENTIFIER`, `EDIT_RENAME_001_RESERVED_KEYWORD`, `EDIT_RENAME_001_STRUCT_FIELD_COLLISION`; shape `sha256:1be9655b754ca9d884c27bd43460558fb86cb55d995475a1d19b276b5de60ecc`.
- Structural peers in `verification/cases/editor_safety/EDIT_RENAME_002.toml`: `EDIT_RENAME_002_CROSS_FILE_REFERENCE_CAPTURE`, `EDIT_RENAME_002_IMPORTED_ORIGIN_COLLISION`, `EDIT_RENAME_002_PROJECT_POU_COLLISION`, `EDIT_RENAME_002_SAFE_CROSS_FILE_RENAME`; shape `sha256:1be9655b754ca9d884c27bd43460558fb86cb55d995475a1d19b276b5de60ecc`.
- Structural peers in `verification/cases/plcopen_devtools/PLCO_IMPORT_001.toml`: `PLCO_IMPORT_001_FBD_BODY_REJECTED_C22BAB6E`, `PLCO_IMPORT_001_LD_BODY_REJECTED_04D6E1B5`, `PLCO_IMPORT_001_MIXED_ST_AND_UNSUPPORTED_BODY_564D002E`, `PLCO_IMPORT_001_SFC_BODY_REJECTED_646387CA`, `PLCO_IMPORT_001_ST_BODY_IMPORT_A352DEE4`, `PLCO_IMPORT_001_ST_BODY_WITH_BENIGN_METADATA_19049E61`, `PLCO_IMPORT_001_UNKNOWN_EXECUTABLE_BODY_REJECTED_3AD1C269`; shape `sha256:1be9655b754ca9d884c27bd43460558fb86cb55d995475a1d19b276b5de60ecc`.
- Structural peers in `verification/cases/protocols/PROTO_DISCOVERY_TRUTH_001.toml`: `PROTO_DISCOVERY_TRUTH_001_MODBUS_DEVICE_ID_CONFIRMED`, `PROTO_DISCOVERY_TRUTH_001_MODBUS_SAFE_READ_CONFIRMED`, `PROTO_DISCOVERY_TRUTH_001_MODBUS_TCP_ONLY`, `PROTO_DISCOVERY_TRUTH_001_MQTT_ACCEPTED_CONNACK`, `PROTO_DISCOVERY_TRUTH_001_MQTT_AUTH_REJECTED`, `PROTO_DISCOVERY_TRUTH_001_MQTT_GENERIC_REJECTED`, `PROTO_DISCOVERY_TRUTH_001_MQTT_TCP_ONLY`; shape `sha256:1be9655b754ca9d884c27bd43460558fb86cb55d995475a1d19b276b5de60ecc`.
- Structural peers in `verification/cases/protocols/PROTO_ETHERCAT_001.toml`: `PROTO_ETHERCAT_001_MISSING_ADAPTER_TERMINAL`, `PROTO_ETHERCAT_001_MOCK_OPERATIONAL_NON_HARDWARE`; shape `sha256:1be9655b754ca9d884c27bd43460558fb86cb55d995475a1d19b276b5de60ecc`.
- Structural peers in `verification/cases/protocols/PROTO_OPCUA_001.toml`: `PROTO_OPCUA_001_PERSISTENT_SESSION`, `PROTO_OPCUA_001_REJECTED_WRITE_POINT_FAULT_ISOLATION`, `PROTO_OPCUA_001_SESSION_LOSS_BACKOFF`, `PROTO_OPCUA_001_STALE_VALUE_HOLD_AND_RECOVERY`, `PROTO_OPCUA_001_SUBSCRIPTION_RECREATED_AFTER_RECONNECT`, `PROTO_OPCUA_001_SUBSCRIPTION_TRANSFER_RECOVERY`; shape `sha256:1be9655b754ca9d884c27bd43460558fb86cb55d995475a1d19b276b5de60ecc`.
- Structural peers in `verification/cases/protocols/PROTO_STATUS_TRUTH_001.toml`: `PROTO_STATUS_TRUTH_001_ADS_STATUS_LIFECYCLE`, `PROTO_STATUS_TRUTH_001_OPCUA_STATUS_LIFECYCLE`, `PROTO_STATUS_TRUTH_001_POINT_STALE_SEPARATION`; shape `sha256:1be9655b754ca9d884c27bd43460558fb86cb55d995475a1d19b276b5de60ecc`.
- Structural peers in `verification/cases/runtime_safety/RT_SAFE_FORCE_001.toml`: `RT_SAFE_FORCE_001_RESTART_CLEARS_DEBUG_MUTATIONS`, `RT_SAFE_FORCE_001_SAFE_STATE_CLEARS_IO_FORCE`; shape `sha256:5583f769954ed3a1265abd03edaee615985ac4cd8dffbd88b1b246bdc76df8e2`.
- Structural peers in `verification/cases/runtime_safety/RT_SAFE_IO_001.toml`: `RT_SAFE_IO_001_DELAYED_MODBUS_READ`, `RT_SAFE_IO_001_DELAYED_MODBUS_WRITE`; shape `sha256:5583f769954ed3a1265abd03edaee615985ac4cd8dffbd88b1b246bdc76df8e2`.
- Structural peers in `verification/cases/runtime_safety/RT_SAFE_PANIC_001.toml`: `RT_SAFE_PANIC_001_INPUT_DRIVER_VISIBLE`, `RT_SAFE_PANIC_001_OUTER_THREAD_VISIBLE`, `RT_SAFE_PANIC_001_RESTART_POLICY_DOES_NOT_RETRY`, `RT_SAFE_PANIC_001_SAFE_OUTPUT`; shape `sha256:5583f769954ed3a1265abd03edaee615985ac4cd8dffbd88b1b246bdc76df8e2`.
- Structural peers in `verification/cases/runtime_safety/RT_SAFE_RESTART_001.toml`: `RT_SAFE_RESTART_001_AUTOMATIC_FAULT_STORAGE`, `RT_SAFE_RESTART_001_COLD_STORAGE`, `RT_SAFE_RESTART_001_WARM_STORAGE`; shape `sha256:5583f769954ed3a1265abd03edaee615985ac4cd8dffbd88b1b246bdc76df8e2`.
- Structural peers in `verification/cases/runtime_safety/RT_SAFE_RESTART_TIME_002.toml`: `RT_SAFE_RESTART_TIME_002_COLD_PRESERVES_TIME_AND_REINITIALIZES_STATE`, `RT_SAFE_RESTART_TIME_002_WARM_PRESERVES_TIME_AND_REINITIALIZES_STATE`; shape `sha256:5583f769954ed3a1265abd03edaee615985ac4cd8dffbd88b1b246bdc76df8e2`.
- Structural peers in `verification/cases/runtime_safety/RT_SAFE_STOP_001.toml`: `RT_SAFE_IO_WORKER_001_FAILURE_DOES_NOT_SKIP_LATER_DRIVER`, `RT_SAFE_IO_WORKER_001_FAULTED_HANDOFF_FAULTS_STOP`, `RT_SAFE_STOP_001_DEGRADED_HANDOFF_FAULTS_STOP`; shape `sha256:5583f769954ed3a1265abd03edaee615985ac4cd8dffbd88b1b246bdc76df8e2`.
- Shared case file `verification/cases/bytecode_vm/VM_SEAM_DECLARED_TYPE_001.toml`: tests `TEST_CASE_TABLE_VM_SEAM_DECLARED_TYPE_001`, `TEST_VM_DECLARED_TYPE_TRACE_001`; record paths `crates/trust-runtime/src/runtime/vm/type_policy/tests.rs`, `verification/cases/bytecode_vm/VM_SEAM_DECLARED_TYPE_001.toml`.
- Shared case file `verification/cases/bytecode_vm/VM_SEAM_ENC_001.toml`: tests `TEST_CASE_TABLE_VM_SEAM_ENC_001`, `TEST_VM_ENCODER_FAIL_CLOSED_TRACE_001`; record paths `crates/trust-runtime/tests/bytecode_vm_core/seam_case_runners.rs`, `verification/cases/bytecode_vm/VM_SEAM_ENC_001.toml`.
- Shared case file `verification/cases/bytecode_vm/VM_SEAM_OWNER_001.toml`: tests `TEST_CASE_TABLE_VM_SEAM_OWNER_001`, `TEST_VM_OWNER_TRACE_001`; record paths `crates/trust-runtime/tests/phase11_seam_contract/vm_seam_case_support.rs`, `verification/cases/bytecode_vm/VM_SEAM_OWNER_001.toml`.
- Shared case file `verification/cases/bytecode_vm/VM_SEAM_REF_001.toml`: tests `TEST_CASE_TABLE_VM_SEAM_REF_001`, `TEST_VM_REF_ESCAPE_TRACE_001`; record paths `crates/trust-runtime/tests/phase11_seam_contract/vm_seam_case_support.rs`, `verification/cases/bytecode_vm/VM_SEAM_REF_001.toml`.
- Shared case file `verification/cases/bytecode_vm/VM_SEAM_STRING_BOUND_001.toml`: tests `TEST_CASE_TABLE_VM_SEAM_STRING_BOUND_001`, `TEST_VM_STRING_BOUND_TRACE_001`; record paths `crates/trust-runtime/src/runtime/vm/type_policy/tests.rs`, `verification/cases/bytecode_vm/VM_SEAM_STRING_BOUND_001.toml`.
- Shared case file `verification/cases/bytecode_vm/VM_SEAM_SUBRANGE_001.toml`: tests `TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001`, `TEST_VM_SUBRANGE_TRACE_001`; record paths `crates/trust-runtime/src/runtime/vm/type_policy/tests.rs`, `verification/cases/bytecode_vm/VM_SEAM_SUBRANGE_001.toml`.
- Shared case file `verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml`: tests `TEST_BYTECODE_VALIDATOR_CASES_001`, `TEST_BYTECODE_VALIDATOR_MUTATION_SHARD_001`, `TEST_CASE_TABLE_VM_SEAM_VALID_001`; record paths `crates/trust-runtime/tests/bytecode_verification_cases.rs`, `scripts/bytecode_validator_mutation.py`, `verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml`.
- Malformed class `ambiguous_instance_owner`: tests `TEST_BYTECODE_OWNER_PRODUCT_REJECTION_001`, `TEST_BYTECODE_OWNER_SHARED_FRAME_REJECTION_001`, `TEST_BYTECODE_OWNER_VALIDATOR_REJECTION_001`; paths `crates/trust-runtime/tests/bytecode_validation.rs`, `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Malformed class `local_frame_reference_persistence`: tests `TEST_BYTECODE_REF_ESCAPE_PRODUCT_REJECTION_001`, `TEST_BYTECODE_REF_ESCAPE_VALIDATOR_REJECTION_001`; paths `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Malformed class `parameter_direction_mismatch`: tests `TEST_BYTECODE_INOUT_LITERAL_REJECTION_001`, `TEST_BYTECODE_PARAMETER_DIRECTION_REJECTION_001`; paths `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Malformed class `reference_escape`: tests `TEST_BYTECODE_REF_ESCAPE_PRODUCT_REJECTION_001`, `TEST_BYTECODE_REF_ESCAPE_VALIDATOR_REJECTION_001`; paths `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Malformed class `stack_type_mismatch`: tests `TEST_BYTECODE_ARITHMETIC_TYPE_REJECTION_001`, `TEST_BYTECODE_STORE_TYPE_REJECTION_001`; paths `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Malformed class `stack_underflow`: tests `TEST_BYTECODE_STACK_UNDERFLOW_REJECTION_001`, `TEST_BYTECODE_VALIDATOR_CASES_001`; paths `crates/trust-runtime/tests/bytecode_verification_cases.rs`, `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Malformed class `stale_instance_owner`: tests `TEST_BYTECODE_OWNER_PRODUCT_REJECTION_001`, `TEST_BYTECODE_OWNER_VALIDATOR_REJECTION_001`; paths `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Malformed class `unknown_opcode`: tests `TEST_BYTECODE_LEGACY_CALL_REJECTION_001`, `TEST_BYTECODE_VALIDATOR_CASES_001`; paths `crates/trust-runtime/tests/bytecode_verification_cases.rs`, `crates/trust-runtime/tests/phase11_seam_contract.rs`.

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

- Scanner facts listed: 3990
- Artifact catalog rows listed separately: 8
- Ignored, nightly, hardware, and name signals never infer duration.
- Scanner `DISC_011E6BF892E89DBE3DBD` / `TEST_VM_ENCODER_FAIL_CLOSED_TRACE_001`: `fast` at `crates/trust-runtime/tests/bytecode_vm_core/seam_case_runners.rs`.
- Scanner `DISC_043E03287D0BD498DBFE` / `TEST_VM_DECLARED_DINT_RUNTIME_TAG_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_05908CB5750AA05E1CDC` / `TEST_IEC_IMPLICIT_CONVERSION_MATRIX_001`: `fast` at `crates/trust-hir/tests/semantic_type_checking/bounded_value_semantics.rs`.
- Scanner `DISC_06CD7DEDEF34CE12046F` / `TEST_IEC_PRECEDENCE_TRACE_001`: `fast` at `crates/trust-runtime/tests/iec_precedence_trace_cases.rs`.
- Scanner `DISC_093B7EAE0DCB979D4540` / `TEST_VM_SUBRANGE_RETAIN_RELOAD_REJECTION_001`: `fast` at `crates/trust-runtime/tests/retain_integrity.rs`.
- Scanner `DISC_09809EC05A9886BFB1A0` / `TEST_IEC_PARSER_DEEP_UNARY_BOUND_001`: `fast` at `crates/trust-syntax/tests/parser_error_recovery.rs`.
- Scanner `DISC_0A44B72796DEA543EA65` / `TEST_IEC_STRING_BINDING_TRACE_001`: `fast` at `crates/trust-runtime/tests/iec_string_binding_trace_cases.rs`.
- Scanner `DISC_0A4A33F9FA0BEF44D267` / `TEST_IEC_PARSER_UNCLOSED_CALL_001`: `fast` at `crates/trust-syntax/tests/parser_expressions.rs`.
- Scanner `DISC_0B4C0B6E0201F0AC32CB` / `TEST_LSP_POSITION_ENCODING_HANDSHAKE_001`: `fast` at `crates/trust-lsp/src/main.rs`.
- Scanner `DISC_0D216C5A37ABCA5C342D` / `TEST_BYTECODE_STACK_UNDERFLOW_REJECTION_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_0D2E60ED0E3666A207F8` / `TEST_VM_REF_ESCAPE_TRACE_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract/vm_seam_case_support.rs`.
- Scanner `DISC_0F2A9C178711CAFD31F3` / `TEST_OPCUA_CLIENT_LIFECYCLE_TRACE_001`: `fast` at `crates/trust-runtime/src/host/opcua/lifecycle_cases.rs`.
- Scanner `DISC_0FA8796302DAFA85B5B5` / `TEST_VM_OWNER_TRACE_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract/vm_seam_case_support.rs`.
- Scanner `DISC_0FEC9C149B9A25229595` / `TEST_LSP_POSITION_ENCODING_TRACE_001`: `fast` at `crates/trust-lsp/src/handlers/tests/lsp_trace_cases.rs`.
- Scanner `DISC_110C6B5B49703576EB1E` / `TEST_IEC_STRING_FUNCTION_INPUT_INOUT_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_1257BED418738BD81458` / `TEST_VM_SUBRANGE_HMI_WRITE_REJECTION_001`: `fast` at `crates/trust-runtime/src/control/tests/hmi_values_write.rs`.
- Scanner `DISC_12A4F21EF84C13A91D28` / `TEST_RUNTIME_RETAIN_FAILURE_ATOMICITY_001`: `fast` at `crates/trust-runtime/tests/retain_failure_trace_cases.rs`.
- Scanner `DISC_1685CC6D1F65B9539D13` / `TEST_BYTECODE_STACK_EXIT_REJECTION_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_1A431BB2B9B087DD4D22` / `TEST_LSP_POSITION_ENCODING_BARE_CR_HOVER_001`: `fast` at `crates/trust-lsp/src/handlers/tests/core_part_01.rs`.
- Scanner `DISC_1A7FBD5D70961E458BE7` / `TEST_RUNTIME_MODBUS_NONFINITE_MAPPED_READ_TRANSACTION_001`: `fast` at `crates/trust-runtime/tests/modbus_driver.rs`.
- Scanner `DISC_1B5448C8DB96DF53A63C` / `TEST_BYTECODE_ARITHMETIC_TYPE_REJECTION_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_1C084A31C00020864EAB` / `TEST_VM_SUBRANGE_TYPE_POLICY_001`: `fast` at `crates/trust-runtime/src/runtime/vm/type_policy/tests.rs`.
- Scanner `DISC_1C1CE2E896E2228C0C79` / `TEST_VM_FUNCTION_RETURN_TYPE_MATERIALIZATION_001`: `fast` at `crates/trust-runtime/tests/bounded_value_semantics.rs`.
- Scanner `DISC_1D261CB173DC0CF72297` / `TEST_VM_DECLARED_REAL_RUNTIME_TAG_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_1E139E65D6EA58F838A8` / `TEST_CONNECTOR_STATUS_TRUTH_TRACE_001`: `fast` at `crates/trust-runtime/tests/protocol_status_trace_cases.rs`.
- Scanner `DISC_1E970F116643481D0DB2` / `TEST_IEC_SUBRANGE_INITIALIZER_REJECTION_001`: `fast` at `crates/trust-hir/tests/semantic_type_checking/bounded_value_semantics.rs`.
- Scanner `DISC_1F2DC8CB9F07388EBECB` / `TEST_BYTECODE_CONTAINER_DUPLICATE_STANDARD_SECTION_001`: `fast` at `crates/trust-runtime/tests/bytecode_container.rs`.
- Scanner `DISC_1F649F7F3E3BCE2021D2` / `TEST_LSP_POSITION_ENCODING_LINE_ENDINGS_001`: `fast` at `crates/trust-lsp/src/handlers/lsp_utils.rs`.
- Scanner `DISC_200665EACFA567BA9AB5` / `TEST_DEBUG_MANAGED_REAL_NONFINITE_001`: `fast` at `crates/trust-debug/src/adapter/tests_part_01.rs`.
- Scanner `DISC_203211A3F8CF0EFC7FC1` / `TEST_VM_LOWERING_SUPPORTED_CONTROL_FLOW_001`: `fast` at `crates/trust-runtime/tests/bytecode_vm_core/lowering_and_constants.rs`.
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
- Scanner `DISC_2A4BDACD2DFE7087A51C` / `TEST_IEC_SUBRANGE_TRACE_001`: `fast` at `crates/trust-hir/tests/subrange_trace_cases.rs`.
- Scanner `DISC_2BD9E868A194F5EFEB80` / `TEST_VM_PRIMITIVE_FLOAT_WIDENING_001`: `fast` at `crates/trust-runtime/src/runtime/vm/type_policy/tests.rs`.
- Scanner `DISC_2DB5D553A2151F589572` / `TEST_IEC_STRING_INOUT_DIAGNOSTIC_001`: `fast` at `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs`.
- Scanner `DISC_2EABAFF06472CDDE49C9` / `TEST_LSP_DOCUMENT_CLOSE_TRACE_001`: `fast` at `crates/trust-lsp/src/handlers/tests/lsp_trace_cases.rs`.
- Scanner `DISC_2F259133A9069DCE6CDC` / `TEST_IEC_WSTRING_FB_INOUT_REJECTION_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_30C382889325B64C5854` / `TEST_RUNTIME_PANIC_IO_DRIVER_001`: `fast` at `crates/trust-runtime/tests/runtime_safety_fail_closed.rs`.
- Scanner `DISC_320D1EE366A7EDAC74A1` / `TEST_PROTOCOL_DISCOVERY_CONFIDENCE_TRACE_001`: `fast` at `crates/trust-runtime/src/control/comm_handlers/discovery_probe/trace_cases.rs`.
- Scanner `DISC_34DA98F67689CD13B551` / `TEST_VM_COERCION_FUNCTION_INPUT_WIDENING_001`: `fast` at `crates/trust-runtime/tests/coercion_proof.rs`.
- Scanner `DISC_35503A169374F5E03594` / `TEST_VM_DECLARED_TYPE_TRACE_001`: `fast` at `crates/trust-runtime/src/runtime/vm/type_policy/tests.rs`.
- Scanner `DISC_356CDEB0445067A48271` / `TEST_BYTECODE_NESTED_REFERENCE_COUNT_BOUND_001`: `fast` at `crates/trust-runtime/tests/bytecode_decode_resource_bounds.rs`.
- Scanner `DISC_36947D824681068CB733` / `TEST_BYTECODE_REF_ESCAPE_VALIDATOR_REJECTION_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_381CA01068E8B914A411` / `TEST_LSP_POSITION_ENCODING_SYNC_001`: `fast` at `crates/trust-lsp/src/handlers/sync.rs`.
- Scanner `DISC_39BF089F7544B5199036` / `TEST_LSP_PUSH_DIAGNOSTIC_CANCELLATION_001`: `fast` at `crates/trust-lsp/src/handlers/tests/core_part_01.rs`.
- Scanner `DISC_3E6CBEABE26D9C3AE875` / `TEST_IEC_PARSER_REQUIRED_DELIMITERS_001`: `fast` at `crates/trust-syntax/tests/parser_error_recovery.rs`.
- Scanner `DISC_40D602971AB9F58ADC84` / `TEST_IEC_STRING_FB_INPUT_INOUT_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_40F1AE3A1CEE7BA3BA4C` / `TEST_IEC_STRING_FUNCTION_INOUT_REJECTION_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_41562587AD279081DFC3` / `TEST_BYTECODE_CALL_TARGET_REJECTION_001`: `fast` at `crates/trust-runtime/tests/bytecode_validation.rs`.
- Scanner `DISC_44B26FACB93E0329E289` / `TEST_BYTECODE_MISSING_SECTION_REJECTION_001`: `fast` at `crates/trust-runtime/tests/bytecode_validation.rs`.
- Scanner `DISC_44D0C7B1DA8B0B5C0B8B` / `TEST_IEC_CONTEXT_LITERAL_PRECISION_REJECTION_001`: `fast` at `crates/trust-hir/tests/semantic_type_checking/bounded_value_semantics.rs`.
- Scanner `DISC_44E3BDF8E0CC841EDB60` / `TEST_BYTECODE_UNKNOWN_OPTIONAL_SECTION_001`: `fast` at `crates/trust-runtime/tests/bytecode_optional_sections.rs`.
- Scanner `DISC_45B9440A281BFB62649B` / `TEST_RUNTIME_RELOAD_TRANSACTION_001`: `fast` at `crates/trust-runtime/tests/reload_transaction_trace_cases.rs`.
- Scanner `DISC_475B227B49C510855C19` / `TEST_VM_DECLARED_LITERAL_CONTEXT_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_485733554D25202B3795` / `TEST_IEC_STRING_NESTED_FIELD_COPYBACK_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_4875D2BB57E417083144` / `TEST_BYTECODE_JUMP_BOUNDARY_REJECTION_001`: `fast` at `crates/trust-runtime/tests/bytecode_validation.rs`.
- Scanner `DISC_48945885EF6AAC63E41D` / `TEST_VM_LOWERING_UNSUPPORTED_EXPRESSION_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_49D96694751D91832F85` / `TEST_RUNTIME_FORCE_LIFECYCLE_001`: `fast` at `crates/trust-runtime/tests/force_lifecycle.rs`.
- Scanner `DISC_4C14198404E587FE4384` / `TEST_VM_OSCAT_OOP_CORE_REGRESSION_001`: `slow` at `crates/trust-runtime/tests/oscat_oop_library.rs`.
- Scanner `DISC_4D65BEF6CD15A409C1E1` / `TEST_BYTECODE_FIXED_SECTION_COUNT_BOUND_001`: `fast` at `crates/trust-runtime/tests/bytecode_decode_resource_bounds.rs`.
- Scanner `DISC_4E0EFE2F6CE2913C5C99` / `TEST_RUNTIME_WATCHDOG_BEFORE_OUTPUT_COMMIT_001`: `fast` at `crates/trust-runtime/tests/runtime_watchdog_output_case.rs`.
- Scanner `DISC_4E9CC7F9F0911410DC2E` / `TEST_IEC_STRING_FUNCTION_RECEIVER_BOUND_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_4F07551DC5298B929A70` / `TEST_VM_REAL_BINARY_OVERFLOW_INTEGRATION_001`: `fast` at `crates/trust-runtime/tests/errors_policy.rs`.
- Scanner `DISC_503BF56E15C30663AC1A` / `TEST_LSP_RENAME_CONFLICT_ERROR_001`: `fast` at `crates/trust-lsp/src/handlers/tests/core_part_01.rs`.
- Scanner `DISC_509AD0A5B81568F80699` / `TEST_VM_HMI_TYPE_ERROR_CODE_001`: `fast` at `crates/trust-runtime/src/control/tests/hmi_values_write.rs`.
- Scanner `DISC_5210F54B3E0F89CA2DE0` / `TEST_RUNTIME_RETAIN_NONFINITE_SAVE_001`: `fast` at `crates/trust-runtime/tests/retain_integrity.rs`.
- Scanner `DISC_53DB35C22AFA96FE355B` / `TEST_BYTECODE_REF_ESCAPE_PRODUCT_REJECTION_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_5416CCD3FD48F9F31374` / `TEST_RUNTIME_MQTT_NONFINITE_PROCESS_IMAGE_READ_001`: `fast` at `crates/trust-runtime/src/io/mqtt/point_map.rs`.
- Scanner `DISC_547AC119BF2B3BEFB960` / `TEST_VM_SUBRANGE_ASSIGNMENT_REJECTION_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_55DE77B00C6511880E2E` / `TEST_IEC_STRING_IMPORTED_CAPACITY_001`: `fast` at `crates/trust-hir/src/db/queries/database/database_tests_part_01.rs`.
- Scanner `DISC_55E928EED7B9D9120424` / `TEST_IEC_STRING_LOCAL_OUTPUT_COPYBACK_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_56F608E0FA7014D18845` / `TEST_VM_COERCION_NARROWING_ASSIGNMENT_REJECTION_001`: `fast` at `crates/trust-runtime/tests/coercion_proof.rs`.
- Scanner `DISC_57F2CE87DC1F410512A6` / `TEST_EDITOR_RENAME_CROSS_FILE_CAPTURE_001`: `fast` at `crates/trust-ide/tests/ide_features/ide_features_part_02.rs`.
- Scanner `DISC_586DFB900FD4BB4178DD` / `TEST_RUNTIME_FORCE_RELEASE_001`: `fast` at `crates/trust-runtime/tests/force_lifecycle_boundaries.rs`.
- Scanner `DISC_5CA02219B3E4D1B01B31` / `TEST_RUNTIME_MQTT_NONFINITE_BATCH_TRANSACTION_001`: `fast` at `crates/trust-runtime/src/io/mqtt/tests.rs`.
- Scanner `DISC_5D4CC58AB74F35B9E2A9` / `TEST_RUNTIME_MQTT_NONFINITE_TEXT_DECODE_001`: `fast` at `crates/trust-runtime/src/io/mqtt/point_map.rs`.
- Scanner `DISC_5F7310588B70E8224104` / `TEST_VM_OSCAT_BINARY_TYPE_PRESERVATION_001`: `fast` at `crates/trust-runtime/tests/bytecode_vm_differential.rs`.
- Scanner `DISC_5FEB03086FCBE0310A3B` / `TEST_LSP_PULL_DIAGNOSTIC_CANCELLATION_001`: `fast` at `crates/trust-lsp/src/handlers/tests/core_part_01.rs`.
- Scanner `DISC_5FF8968BF2E1CC6926BC` / `TEST_VM_REAL_NAMED_OVERFLOW_DIRECT_001`: `fast` at `crates/trust-runtime/tests/stdlib_numeric.rs`.
- Scanner `DISC_636CB8D8E6B5C60C383F` / `TEST_CONTROL_AUTHORIZATION_TRACE_001`: `fast` at `crates/trust-runtime/src/control/tests/authorization_trace.rs`.
- Scanner `DISC_6C10B5D7ACB21ECE8E9D` / `TEST_RUNTIME_MODBUS_SLOW_WRITE_BOUND_001`: `fast` at `crates/trust-runtime/tests/modbus_driver.rs`.
- Scanner `DISC_6CE89ED4FAD369DC5B1D` / `TEST_VM_BOUNDED_TYPED_FLOAT_REJECTION_001`: `fast` at `crates/trust-runtime/tests/bounded_value_semantics.rs`.
- Scanner `DISC_6D4BCBF011BA193AD9AB` / `TEST_RUNTIME_RETAIN_NONFINITE_LOAD_001`: `fast` at `crates/trust-runtime/tests/retain_integrity.rs`.
- Scanner `DISC_6E9FF1D629FBEFC55BDE` / `TEST_RUNTIME_OPCUA_NONFINITE_INGRESS_001`: `fast` at `crates/trust-runtime/src/host/opcua/tests.rs`.
- Scanner `DISC_6EDF6218DDBF01A4060E` / `TEST_BYTECODE_TYPE_TABLE_COUNT_BOUND_001`: `fast` at `crates/trust-runtime/tests/bytecode_decode_resource_bounds.rs`.
- Scanner `DISC_6F61BF50F054D14B12B8` / `TEST_RUNTIME_TYPED_REAL_OUTPUT_NONFINITE_001`: `fast` at `crates/trust-runtime/src/io/interface.rs`.
- Scanner `DISC_6FCA577549BA14A407BB` / `TEST_RUNTIME_SIMULATION_THRESHOLD_NONFINITE_001`: `fast` at `crates/trust-runtime/tests/simulation_workflow.rs`.
- Scanner `DISC_6FE11E79A06AEA138657` / `TEST_BYTECODE_VERSION_REJECTION_001`: `fast` at `crates/trust-runtime/tests/bytecode_container.rs`.
- Scanner `DISC_7159C3BA77CC33C8F48C` / `TEST_RUNTIME_MODBUS_SLOW_READ_BOUND_001`: `fast` at `crates/trust-runtime/tests/modbus_driver.rs`.
- Scanner `DISC_716FA81FC2506FBA1592` / `TEST_DEBUG_MANAGED_LREAL_NONFINITE_001`: `fast` at `crates/trust-debug/src/adapter/tests_part_01.rs`.
- Scanner `DISC_71DE8E05EE3BC6181277` / `TEST_EDITOR_RENAME_PROJECT_TRACE_001`: `fast` at `crates/trust-lsp/src/handlers/tests/rename_trace_cases.rs`.
- Scanner `DISC_725D6C1E9787F7B1D630` / `TEST_IEC_WSTRING_LOCAL_ALIAS_COPYBACK_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_726176040625B846A7AD` / `TEST_RUNTIME_FORCE_DISCONNECT_001`: `fast` at `crates/trust-runtime/tests/force_lifecycle_boundaries.rs`.
- Scanner `DISC_733F49BAD3FF00FA4D54` / `TEST_RUNTIME_TYPED_INPUT_NONFINITE_001`: `fast` at `crates/trust-runtime/src/io/interface.rs`.
- Scanner `DISC_7B56C0BED850B5D2ECF1` / `TEST_BYTECODE_NESTED_RESOURCE_COUNT_BOUND_001`: `fast` at `crates/trust-runtime/tests/bytecode_decode_resource_bounds.rs`.
- Scanner `DISC_7D6DBE32ECF7F31D1748` / `TEST_BYTECODE_LOCAL_REF_PATH_ACCEPTANCE_001`: `fast` at `crates/trust-runtime/tests/bytecode_vm_core/ref_validation.rs`.
- Scanner `DISC_80833EFC8B17DF0C8754` / `TEST_BYTECODE_STORE_TYPE_REJECTION_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_82037BF366E0CC07D92C` / `TEST_IEC_TIMER_RUNTIME_VARIANTS_001`: `fast` at `crates/trust-runtime/tests/fb_timers_full.rs`.
- Scanner `DISC_82DF586FECB97A31B50C` / `TEST_VM_TRAP_STABLE_ERROR_CODE_MODEL_001`: `fast` at `crates/trust-runtime-core/src/vm/errors.rs`.
- Scanner `DISC_83FE96819CB25F21CA77` / `TEST_RUNTIME_RESTART_WARM_STORAGE_001`: `fast` at `crates/trust-runtime/tests/runtime_restart.rs`.
- Scanner `DISC_846E4BECA070B0C77CDA` / `TEST_RUNTIME_ADS_NONFINITE_SCALAR_DECODE_001`: `fast` at `crates/trust-ads-core/src/mapping.rs`.
- Scanner `DISC_852E381BD310A63FCE71` / `TEST_RUNTIME_FORCE_FAULT_001`: `fast` at `crates/trust-runtime/tests/force_lifecycle_boundaries.rs`.
- Scanner `DISC_85497C69357D10323712` / `TEST_IEC_PARSER_NESTED_TERMINATOR_RECOVERY_001`: `fast` at `crates/trust-syntax/tests/parser_error_recovery.rs`.
- Scanner `DISC_8664E7408C2C3D60D5C5` / `TEST_DEBUG_AUTHORIZATION_MATRIX_001`: `fast` at `crates/trust-runtime/src/control/tests/authorization_matrix.rs`.
- Scanner `DISC_88F921D24D3708CEF3E1` / `TEST_BYTECODE_CONTAINER_INVALID_MAGIC`: `fast` at `crates/trust-runtime/tests/bytecode_container.rs`.
- Scanner `DISC_8925B3C0E6A786597E4B` / `TEST_RUNTIME_TYPED_OUTPUT_DRIVER_COMMIT_001`: `fast` at `crates/trust-runtime/tests/runtime_safety_fail_closed.rs`.
- Scanner `DISC_8CD53CDE1E92A31A96D1` / `TEST_BYTECODE_STABLE_ERROR_CODE_MODEL_001`: `fast` at `crates/trust-runtime-core/src/bytecode/mod.rs`.
- Scanner `DISC_8D254A3AADEDD4121B5F` / `TEST_RUNTIME_SAFE_STATE_MQTT_PENDING_001`: `fast` at `crates/trust-runtime/src/io/mqtt/tests/safe_state.rs`.
- Scanner `DISC_8E59F13125886D732DA2` / `TEST_VM_DECLARED_PARAMETER_COPY_IN_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_8F0C898E72D907AA065A` / `TEST_RUNTIME_ADS_NONFINITE_CLIENT_INGRESS_001`: `fast` at `crates/trust-runtime/src/host/ads/tests.rs`.
- Scanner `DISC_8F2DBAEEFDD35FE74605` / `TEST_LSP_DIAGNOSTIC_CANCELLATION_TRACE_001`: `fast` at `crates/trust-lsp/src/handlers/tests/lsp_trace_cases.rs`.
- Scanner `DISC_92FCECA7163F38786723` / `TEST_RUNTIME_PANIC_TRACE_001`: `fast` at `crates/trust-runtime/tests/runtime_panic_trace_cases.rs`.
- Scanner `DISC_934C37AAF7B29C216166` / `TEST_VM_COERCION_INITIALIZER_WIDENING_001`: `fast` at `crates/trust-runtime/tests/coercion_proof.rs`.
- Scanner `DISC_93D5E86E7CF67A1A5AAE` / `TEST_IEC_TIMER_TON_DURATION_OVERFLOW_001`: `fast` at `crates/trust-runtime/tests/fb_timers.rs`.
- Scanner `DISC_95796224C9ECEDD5BFA6` / `TEST_VM_SUBRANGE_TRACE_001`: `fast` at `crates/trust-runtime/src/runtime/vm/type_policy/tests.rs`.
- Scanner `DISC_96A8793BF2C3E6F579B4` / `TEST_RUNTIME_PANIC_CYCLE_GUARD_001`: `fast` at `crates/trust-runtime/src/scheduler/runner_loop.rs`.
- Scanner `DISC_98E387D5D8C3B2E06B11` / `TEST_BYTECODE_NESTED_POU_COUNT_BOUND_001`: `fast` at `crates/trust-runtime/tests/bytecode_decode_resource_bounds.rs`.
- Scanner `DISC_9957C9AF2898026C2E6A` / `TEST_IEC_STRING_FUNCTION_RESULT_BOUND_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_99710F5C15A625CF2D08` / `TEST_RUNTIME_FORCE_PAUSE_RESUME_001`: `fast` at `crates/trust-runtime/tests/force_lifecycle_boundaries.rs`.
- Scanner `DISC_997B38E1827A6BA3FD56` / `TEST_IEC_STRING_UNICODE_ASSIGNMENT_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_9B5074A2F4ECFB141D84` / `TEST_EDITOR_RENAME_SHADOW_CAPTURE_001`: `fast` at `crates/trust-ide/tests/ide_features/ide_features_part_02.rs`.
- Scanner `DISC_9DB502BC879C74994C6A` / `TEST_RUNTIME_TYPED_LREAL_OUTPUT_TRANSACTION_001`: `fast` at `crates/trust-runtime/src/io/interface.rs`.
- Scanner `DISC_9DC62C547895CF5C3B8E` / `TEST_LSP_DOCUMENT_CLOSE_DISK_RELOAD_001`: `fast` at `crates/trust-lsp/src/handlers/tests/core_part_05.rs`.
- Scanner `DISC_9FD52F2E664756D5B3A7` / `TEST_IEC_STRING_FUNCTION_OUTPUT_COPYBACK_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_A25BA8D22EAE6F745364` / `TEST_VM_LOWERING_UNSUPPORTED_STATEMENT_001`: `fast` at `crates/trust-runtime/tests/bytecode_vm_core/lowering_and_constants.rs`.
- Scanner `DISC_A2698249E8827008FE24` / `TEST_IEC_STRING_FB_INOUT_REJECTION_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_A324C9FF622A5F835C5A` / `TEST_BYTECODE_VALIDATOR_CASES_001`: `fast` at `crates/trust-runtime/tests/bytecode_verification_cases.rs`.
- Scanner `DISC_A5D3D4D2770B1A5B5B0E` / `TEST_IEC_CROSS_FAMILY_CONVERSION_REJECTION_001`: `fast` at `crates/trust-hir/tests/semantic_type_checking/bounded_value_semantics.rs`.
- Scanner `DISC_A5D80AC55562CF612615` / `TEST_VM_BOUNDED_FLOAT_MATERIALIZATION_001`: `fast` at `crates/trust-runtime/tests/bounded_value_semantics.rs`.
- Scanner `DISC_A5E84A20B9805DE4F3BA` / `TEST_LSP_POSITION_ENCODING_SEMANTIC_TOKENS_001`: `fast` at `crates/trust-lsp/src/handlers/tests/core_part_06.rs`.
- Scanner `DISC_A66997F12A4CE2CE2018` / `TEST_RUNTIME_HMI_NONFINITE_WRITE_001`: `fast` at `crates/trust-runtime/src/control/tests/hmi_values_write.rs`.
- Scanner `DISC_A778FE822F9A808E700F` / `TEST_VM_REAL_BINARY_OVERFLOW_UNIT_001`: `fast` at `crates/trust-runtime-core/src/program_model/ops.rs`.
- Scanner `DISC_A7A560AF6BF54C758EAA` / `TEST_IEC_PARSER_DEEP_PAREN_BOUND_001`: `fast` at `crates/trust-syntax/tests/parser_error_recovery.rs`.
- Scanner `DISC_AC6B130967B7B99212D8` / `TEST_DEBUG_PAUSE_TRACE_001`: `fast` at `crates/trust-runtime/tests/debug_pause_watchdog_trace_cases.rs`.
- Scanner `DISC_AE46B3ECD59B91B3AB4B` / `TEST_IEC_PARSER_RECOVERY_TRACE_001`: `fast` at `crates/trust-syntax/tests/parser_recovery_trace_cases.rs`.
- Scanner `DISC_AEDAB26C7EA182A1DA20` / `TEST_VM_REAL_NAMED_OVERFLOW_INTEGRATION_001`: `fast` at `crates/trust-runtime/tests/errors_policy.rs`.
- Scanner `DISC_B13D641C6F6871D47022` / `TEST_IEC_PARSER_MISSING_POU_TERMINATOR_001`: `fast` at `crates/trust-syntax/tests/parser_error_recovery.rs`.
- Scanner `DISC_B14A6D7CC4920B6B7DA3` / `TEST_VM_STRING_PRIMITIVE_POLICY_001`: `fast` at `crates/trust-runtime/src/runtime/vm/type_policy/tests.rs`.
- Scanner `DISC_B27FCCAE211DE6352273` / `TEST_VM_CALL_ARGUMENT_TYPE_MATERIALIZATION_001`: `fast` at `crates/trust-runtime/tests/bounded_value_semantics.rs`.
- Scanner `DISC_B2AEF59399732DF0D7A7` / `TEST_BYTECODE_SCHEMA_TAG_REJECTION_001`: `fast` at `crates/trust-runtime/tests/bytecode_container.rs`.
- Scanner `DISC_B2E279776F3EC1A8D02A` / `TEST_EDITOR_RENAME_FIELD_COLLISION_001`: `fast` at `crates/trust-ide/tests/ide_features/ide_features_part_02.rs`.
- Scanner `DISC_B3D707B941701C75F0A3` / `TEST_EDITOR_RENAME_IMPORTED_ORIGIN_CONFLICT_001`: `fast` at `crates/trust-ide/tests/ide_features/ide_features_part_02.rs`.
- Scanner `DISC_B4572A94890D2D4AAD66` / `TEST_RUNTIME_IO_BOUND_TRACE_001`: `fast` at `crates/trust-runtime/tests/runtime_io_bound_trace_cases.rs`.
- Scanner `DISC_B4F19BDADF413705E697` / `TEST_RUNTIME_ADS_NONFINITE_SERVER_INGRESS_001`: `fast` at `crates/trust-runtime/src/host/ads/server/tests.rs`.
- Scanner `DISC_B7CAF85079658EF2FB1B` / `TEST_EDITOR_RENAME_CASE_ONLY_001`: `fast` at `crates/trust-ide/tests/ide_features/ide_features_part_02.rs`.
- Scanner `DISC_B7E97F9BAA11D0743107` / `TEST_VM_COERCION_INOUT_NARROWING_REJECTION_001`: `fast` at `crates/trust-runtime/tests/coercion_proof.rs`.
- Scanner `DISC_BCA3C293212BE0D52B3D` / `TEST_BYTECODE_OWNER_PRODUCT_REJECTION_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_BD8E8257D5B1250AA650` / `TEST_EDITOR_RENAME_CASE_INSENSITIVE_SCOPE_001`: `fast` at `crates/trust-ide/tests/ide_features/ide_features_part_02.rs`.
- Scanner `DISC_BDB3FF86B2273DD827C1` / `TEST_BYTECODE_CHECKSUM_REJECTION_001`: `fast` at `crates/trust-runtime/tests/bytecode_container.rs`.
- Scanner `DISC_BFC7BB3FA588D9EC14C1` / `TEST_RUNTIME_RESTART_STORAGE_TRACE_001`: `fast` at `crates/trust-runtime/src/scheduler/runner_loop/restart_storage_trace_cases.rs`.
- Scanner `DISC_C160AB9422688E289F44` / `TEST_BYTECODE_LEGACY_CALL_REJECTION_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_C1C1116CE99213C96040` / `TEST_VM_DECLARED_DINT_CONVERSION_PATHS_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_C4571F678034F8B13121` / `TEST_VM_NORMALIZE_NON_WIDENING_001`: `fast` at `crates/trust-runtime-core/src/value/types/tests.rs`.
- Scanner `DISC_C46D4AB834A6E974B42E` / `TEST_RUNTIME_MESH_NONFINITE_INGRESS_001`: `fast` at `crates/trust-runtime/src/host/mesh/tests.rs`.
- Scanner `DISC_C5A04B37E39DDBE237C5` / `TEST_IEC_TIMER_TRACE_001`: `fast` at `crates/trust-runtime/tests/iec_timer_trace_cases.rs`.
- Scanner `DISC_C74FB437C18E65242C35` / `TEST_BYTECODE_LOCAL_REF_RANGE_REJECTION_001`: `fast` at `crates/trust-runtime/tests/bytecode_vm_core/ref_validation.rs`.
- Scanner `DISC_CA34AD06400345CAADED` / `TEST_RUNTIME_FORCE_AUTH_CHANGE_001`: `fast` at `crates/trust-runtime/src/control/tests/debug_mutation_lifecycle.rs`.
- Scanner `DISC_CA616B8FA0E5BB982CB5` / `TEST_ETHERCAT_UNAVAILABLE_RESOURCE_TRACE_001`: `fast` at `crates/trust-runtime/src/io/ethercat/trace_cases.rs`.
- Scanner `DISC_CF07A66B6BF18051C670` / `TEST_BYTECODE_NESTED_TYPE_COUNT_BOUND_001`: `fast` at `crates/trust-runtime/tests/bytecode_decode_resource_bounds.rs`.
- Scanner `DISC_D5C4C1D59854BC4EA981` / `TEST_RUNTIME_SAFE_STATE_HANDOFF_001`: `fast` at `crates/trust-runtime/tests/safe_state_handoff_trace_cases.rs`.
- Scanner `DISC_D5D85CCF0C4152A13CCE` / `TEST_EDITOR_RENAME_LOCAL_TRACE_001`: `fast` at `crates/trust-lsp/src/handlers/tests/rename_trace_cases.rs`.
- Scanner `DISC_D76D0BCB14E615250E0B` / `TEST_VM_SUBRANGE_REF_WRITE_REJECTION_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_DA4DF0247AD9823BF96F` / `TEST_IEC_TIMER_TOF_DURATION_OVERFLOW_001`: `fast` at `crates/trust-runtime/tests/fb_timers.rs`.
- Scanner `DISC_DBCE837999092CDE3372` / `TEST_LSP_WORKSPACE_DIAGNOSTIC_CANCELLATION_001`: `fast` at `crates/trust-lsp/src/handlers/tests/core_part_01.rs`.
- Scanner `DISC_DF6BE740DA85943739A5` / `TEST_RUNTIME_SAFE_STATE_MODBUS_PENDING_001`: `fast` at `crates/trust-runtime/tests/modbus_driver.rs`.
- Scanner `DISC_E4521EB1299D2E86F016` / `TEST_BYTECODE_OWNER_SHARED_FRAME_REJECTION_001`: `fast` at `crates/trust-runtime/tests/bytecode_validation.rs`.
- Scanner `DISC_E48865AC746B285034BC` / `TEST_VM_FOR_CONTROL_TYPE_MATERIALIZATION_001`: `fast` at `crates/trust-runtime/tests/bounded_value_semantics.rs`.
- Scanner `DISC_E4A27307965153DFADA9` / `TEST_VM_SUBRANGE_BOUNDS_TRANSACTION_001`: `fast` at `crates/trust-runtime/tests/bounded_value_semantics.rs`.
- Scanner `DISC_E6963439801BFF301755` / `TEST_RUNTIME_PANIC_SAFE_OUTPUT_001`: `fast` at `crates/trust-runtime/tests/runtime_safety_fail_closed.rs`.
- Scanner `DISC_E6ED69DD8B97B244A411` / `TEST_IEC_STRING_INOUT_ALIAS_001`: `fast` at `crates/trust-hir/tests/semantic_type_checking/control_flow_and_calls.rs`.
- Scanner `DISC_E88335AA584F723ECF04` / `TEST_PLCOPEN_IMPORT_TRACE_001`: `fast` at `crates/trust-runtime/tests/plcopen_import_trace_cases.rs`.
- Scanner `DISC_E8D36E045CD657B29BDD` / `TEST_BYTECODE_INOUT_LITERAL_REJECTION_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_E92555663D7FB7B4C866` / `TEST_VM_STRING_BOUND_TRACE_001`: `fast` at `crates/trust-runtime/src/runtime/vm/type_policy/tests.rs`.
- Scanner `DISC_EB73AB6889251287113D` / `TEST_IEC_LOSSY_TYPED_FLOAT_REJECTION_001`: `fast` at `crates/trust-hir/tests/semantic_type_checking/bounded_value_semantics.rs`.
- Scanner `DISC_EB9C1ED2E879B48CDDDD` / `TEST_EDITOR_RENAME_PROJECT_POU_COLLISION_001`: `fast` at `crates/trust-ide/tests/ide_features/ide_features_part_02.rs`.
- Scanner `DISC_EC610E3917E2D7BC4E2B` / `TEST_BYTECODE_OWNER_VALIDATOR_REJECTION_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_EE29217C8E428AA1539E` / `TEST_RUNTIME_MODBUS_NONFINITE_PROCESS_IMAGE_READ_001`: `fast` at `crates/trust-runtime/src/io/modbus/point_map.rs`.
- Scanner `DISC_EE9408494F62E80474E3` / `TEST_RUNTIME_ADS_NONFINITE_ARRAY_DECODE_001`: `fast` at `crates/trust-ads-core/src/mapping.rs`.
- Scanner `DISC_EEF14FDB800A7590C8F3` / `TEST_RUNTIME_PANIC_THREAD_GUARD_001`: `fast` at `crates/trust-runtime/tests/runtime_safety_fail_closed.rs`.
- Scanner `DISC_EFCA6D0FAFC2EB9C01AA` / `TEST_IEC_WSTRING_FB_OUTPUT_COPYBACK_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Scanner `DISC_F103626E494B610C2A79` / `TEST_VM_COERCION_RETURN_WIDENING_001`: `fast` at `crates/trust-runtime/tests/coercion_proof.rs`.
- Scanner `DISC_F13122338B6C009DFE6D` / `TEST_RUNTIME_SAFE_STATE_STOP_DEGRADED_001`: `fast` at `crates/trust-runtime/tests/runtime_safety_fail_closed.rs`.
- Scanner `DISC_F249C0ED30C6D840D514` / `TEST_VM_RESOURCE_LIMIT_CASES_001`: `slow` at `crates/trust-runtime/tests/vm_resource_limit_cases.rs`.
- Scanner `DISC_F266CBDB89F6324BB3DE` / `TEST_VM_PRIMITIVE_TAG_REJECTION_001`: `fast` at `crates/trust-runtime/src/runtime/vm/type_policy/tests.rs`.
- Scanner `DISC_F2E62E25E1A58826EE7D` / `TEST_RUNTIME_RESTART_AUTOMATIC_STORAGE_001`: `fast` at `crates/trust-runtime/src/scheduler/runner_loop.rs`.
- Scanner `DISC_F4845D4DC06C49AF59D6` / `TEST_RUNTIME_PAUSE_WATCHDOG_001`: `fast` at `crates/trust-runtime/tests/debug_pause_watchdog.rs`.
- Scanner `DISC_F4B46894F7E22B37EC5F` / `TEST_IEC_CONTEXT_LITERAL_CONVERSION_001`: `fast` at `crates/trust-hir/tests/semantic_type_checking/bounded_value_semantics.rs`.
- Scanner `DISC_F7316A527BB51A15C20F` / `TEST_IEC_STRING_IMPORTED_MISMATCH_001`: `fast` at `crates/trust-hir/src/db/queries/database/database_tests_part_01.rs`.
- Scanner `DISC_F850C3D482F1F1486738` / `TEST_BYTECODE_PARAMETER_DIRECTION_REJECTION_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_F85382D0B4505D5984DB` / `TEST_VM_COERCION_ASSIGNMENT_WIDENING_001`: `fast` at `crates/trust-runtime/tests/coercion_proof.rs`.
- Scanner `DISC_F99DDE5C72C87AAEC469` / `TEST_VM_HMI_STRING_CAPACITY_ERROR_CODE_001`: `fast` at `crates/trust-runtime/src/control/tests/hmi_values_write.rs`.
- Scanner `DISC_FAEEFF50E8BC4B81FB79` / `TEST_IEC_PARSER_UNKNOWN_TOKEN_PROGRESS_001`: `fast` at `crates/trust-syntax/tests/parser_error_recovery.rs`.
- Scanner `DISC_FCA63C2A07B9C48D971E` / `TEST_BYTECODE_TRANSFORM_SEED_001`: `fast` at `crates/trust-runtime/tests/bytecode_verification_cases.rs`.
- Scanner `DISC_FD5D2B3F7B42E18220E6` / `TEST_LSP_POSITION_ENCODING_HOVER_001`: `fast` at `crates/trust-lsp/src/handlers/tests/core_part_01.rs`.
- Scanner `DISC_FE14D8FC44DB0AC97510` / `TEST_BYTECODE_MISSING_OWNER_FIELD_REJECTION_001`: `fast` at `crates/trust-runtime/tests/bytecode_container.rs`.
- Scanner `DISC_FE665C5DDCC65815D217` / `TEST_BYTECODE_STRING_TABLE_COUNT_BOUND_001`: `fast` at `crates/trust-runtime/tests/bytecode_decode_resource_bounds.rs`.
- Scanner `DISC_FF77A462527037C6EEA8` / `TEST_IEC_TIMER_TP_DURATION_OVERFLOW_001`: `fast` at `crates/trust-runtime/tests/fb_timers.rs`.
- Scanner `DISC_FFED59DF8ADBF82880D8` / `TEST_IEC_STRING_UNICODE_LITERAL_BOUND_001`: `fast` at `crates/trust-runtime/tests/string_binding_bounds.rs`.
- Artifact `TEST_BYTECODE_VALIDATOR_MUTATION_SHARD_001`: `slow` `mutation_shard_runner` at `scripts/bytecode_validator_mutation.py`; suites `nightly`.
- Artifact `TEST_CASE_TABLE_VM_SEAM_DECLARED_TYPE_001`: `fast` `case_table_artifact` at `verification/cases/bytecode_vm/VM_SEAM_DECLARED_TYPE_001.toml`; suites `veryquick`.
- Artifact `TEST_CASE_TABLE_VM_SEAM_ENC_001`: `fast` `case_table_artifact` at `verification/cases/bytecode_vm/VM_SEAM_ENC_001.toml`; suites `veryquick`.
- Artifact `TEST_CASE_TABLE_VM_SEAM_OWNER_001`: `fast` `case_table_artifact` at `verification/cases/bytecode_vm/VM_SEAM_OWNER_001.toml`; suites `veryquick`.
- Artifact `TEST_CASE_TABLE_VM_SEAM_REF_001`: `fast` `case_table_artifact` at `verification/cases/bytecode_vm/VM_SEAM_REF_001.toml`; suites `veryquick`.
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
