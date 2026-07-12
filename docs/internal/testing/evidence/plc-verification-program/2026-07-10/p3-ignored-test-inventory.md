# Ignored-Test Inventory

Generator: `ignored-test-inventory v1`
Source revision: `dfd292c4f14e802810c2ec7a67aca92cb4d528f5`
Generated: `2026-07-12T19:45:00+02:00`
Platform: `linux-aarch64`
Generated JSON SHA-256: `d5890f005b59f5368103485fd82b4ffad24c8d5383bb5723c701a9a158967e2f`
Input SHA-256: `sha256:c0bb74a46860a5534c8395eb4a573e35fa62699b246d2c1f06ae9eb1cbd85701`

This report is a mechanical inventory. It does not classify an ignored test,
establish expected behavior, or count as product proof.

## Summary

- Records: 88
- Statically ignored: 86
- Conditional ignore observations: 2
- Diagnostics: 0
- Errors: 0
- Warnings: 0

| Source kind | Records |
| --- | ---: |
| `playwright_test` | 1 |
| `rust_integration_test` | 57 |
| `rust_unit_test` | 29 |
| `vscode_test` | 1 |

## Surface Coverage

| Surface | Scanned files | Records | Ignored | Conditional | Coverage |
| --- | ---: | ---: | ---: | ---: | --- |
| `conformance` | 21 | 0 | 0 | 0 | `limitation` |
| `node` | 47 | 1 | 0 | 1 | `mechanical` |
| `playwright` | 6 | 1 | 1 | 0 | `mechanical` |
| `rust` | 538 | 86 | 85 | 1 | `mechanical` |
| `shell` | 29 | 0 | 0 | 0 | `limitation` |

Surface notes:

- `conformance`: Runtime skipped results are not source ignore declarations.
- `node`: Modeled VS Code facts and fail-closed excluded Node sentinel files are included in the scanned-file count.
- `playwright`: Only same-line literal test.skip calls in tracked capture specs are inventory facts.
- `rust`: Modeled crate Rust facts and fail-closed xtask/fuzz sentinel files are included in the scanned-file count.
- `shell`: Shell source has no repository-wide static ignored-test identity convention.

## Inventory

| Discovery ID | State | Mechanism | Source | Path | Name | Reason |
| --- | --- | --- | --- | --- | --- | --- |
| `DISC_E45EDC8D2860AAECF144` | `ignored` | `playwright_literal_skip` | `playwright_test` | `scripts/captures/vscode/runtime-panel-command.spec.mjs:5` | `capture code-server runtime panel command palette` | literal test.skip declaration |
| `DISC_94100DB31F1CB2D16C0F` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs:476` | `test_ref_rejects_function_return_variable` | red test for runtime-safety Phase 11 SEAM-TEST-005 |
| `DISC_F1BFAEF8054FF79967E4` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-hir/tests/semantic_type_checking/types_and_references.rs:498` | `test_ref_rejects_method_return_variable` | red test for runtime-safety Phase 11 SEAM-TEST-005 |
| `DISC_F85382D0B4505D5984DB` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/coercion_proof.rs:55` | `assignment_widening_materializes_target_runtime_type` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_34DA98F67689CD13B551` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/coercion_proof.rs:6` | `function_input_parameter_widening_executes_as_runtime_target_type` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_266A60F3E75C685188EB` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/coercion_proof.rs:30` | `function_output_parameter_widening_executes_as_runtime_target_type` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_934C37AAF7B29C216166` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/coercion_proof.rs:75` | `initializer_widening_materializes_target_runtime_type` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_B7E97F9BAA11D0743107` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/coercion_proof.rs:113` | `inout_narrowing_is_rejected_instead_of_silent_writeback_loss` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_56F608E0FA7014D18845` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/coercion_proof.rs:146` | `narrowing_assignment_is_rejected_instead_of_silent_truncation` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_F103626E494B610C2A79` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/coercion_proof.rs:92` | `return_value_widening_executes_as_runtime_target_type` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_8CC0C32A02C75700E3A8` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/debug_control.rs:593` | `logpoint_sender_drop_buffers_log_in_debug_control` | red test for runtime-safety fail-closed Phase 8 |
| `DISC_409932463EB8BCB34BAF` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/debug_control.rs:571` | `runtime_event_sender_drop_buffers_event_in_debug_control` | red test for runtime-safety fail-closed Phase 8 |
| `DISC_FE262D7963202D5AFB9D` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/device_in_the_loop.rs:177` | `ads_lab_twincat_doctor_records_status_json` | requires configured lab TwinCAT/ADS target; see docs/internal/testing/runtime-device-in-the-loop.md |
| `DISC_26934332D29338103AFD` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/device_in_the_loop.rs:16` | `ethercat_lab_hardware_discovery_records_topology` | requires configured lab EtherCAT hardware; see docs/internal/testing/runtime-device-in-the-loop.md |
| `DISC_A007AD886E7ABB17EF37` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/device_in_the_loop.rs:77` | `ethercat_lab_pdu_storage_stress_records_artifact` | requires explicit lab EtherCAT storage stress opt-in; see docs/internal/testing/runtime-device-in-the-loop.md |
| `DISC_EC41FC62FBEF09EFDFE8` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/device_in_the_loop.rs:273` | `modbus_lab_target_confirms_protocol_probe` | requires configured lab Modbus target; see docs/internal/testing/runtime-device-in-the-loop.md |
| `DISC_75F421CF89F3935418D6` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/device_in_the_loop.rs:366` | `mqtt_lab_broker_records_auth_tls_reconnect_and_disconnect` | requires configured lab MQTT broker; see docs/internal/testing/runtime-device-in-the-loop.md |
| `DISC_6F665C0AE3F1C64063FF` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/ethercat_driver.rs:133` | `ethercat_image_size_mismatch_faults_under_warn_policy` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_B9ACE9B650FE5BFE0DC6` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/ethercat_driver.rs:243` | `ethercat_missing_adapter_post_allocation_failure_is_terminal_until_rebuild` | red test for runtime-safety EtherCAT bounded post-allocation retry policy |
| `DISC_083918C0A1695BEDB92E` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/ethercat_driver.rs:168` | `ethercat_missing_adapter_records_pdu_storage_retry_baseline` | runtime-safety EtherCAT PduStorage baseline; explicitly run for storage evidence |
| `DISC_5CB0738F4F5DD699DBEF` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/init_fail_closed.rs:68` | `debug_queued_global_write_unknown_target_fails` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_6EB69C59453ED035612E` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/init_fail_closed.rs:44` | `debug_queued_lvalue_write_failure_is_observable` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_37BD30C003E5713BEF32` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/init_fail_closed.rs:9` | `interface_param_default_failure_returns_init_failed` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_FE38938F6FB54A1B26E7` | `conditional` | `rust_cfg_attr` | `rust_integration_test` | `crates/trust-runtime/tests/io_multidriver_live.rs:410` | `runtime_composes_modbus_and_mqtt_drivers_live` | live MQTT broker handshake is network-timing flaky on non-Linux CI runners; \ run with `--ignored` locally to exercise. Linux runners are the source of truth \ for the driver-composition contract. |
| `DISC_A3BEFC6AEFD5123C849D` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/modbus_driver.rs:772` | `modbus_exception_is_not_reported_as_generic_transport` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_7F7F305082D2D125BF56` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/openot_capstone.rs:98` | `openot_capstone_consumer_process` | spawned by openot_capstone_fenced_cross_process |
| `DISC_2EEB083ACDA59C7173DC` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/openot_capstone.rs:89` | `openot_capstone_producer_process` | spawned by openot_capstone_fenced_cross_process |
| `DISC_076A01316BE505A43D6C` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/openot_capstone.rs:59` | `openot_capstone_unfenced_contrast` | diagnostic unfenced experiment; set OPENOT_CAPSTONE_RUN_UNFENCED=1 |
| `DISC_B0E363553D57310C9D63` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/oscat_oop_examples.rs:583` | `oscat_oop_example_st_unit_tests_pass` | expensive OSCAT gate runs all 98 paired example projects through trust-runtime CLI |
| `DISC_BDEAFF431FC2B9A44DFD` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase10_performance.rs:105` | `phase10_ads_opcua_publish_clone_partial_baseline` | Phase 10 benchmark; run explicitly with --ignored --nocapture |
| `DISC_8A83ABE26CDE3A14E938` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase10_performance.rs:44` | `phase10_debug_snapshot_overhead_baseline` | Phase 10 benchmark; run explicitly with --ignored --nocapture |
| `DISC_9DA8F1E2D659F4E0F5E7` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase10_performance.rs:76` | `phase10_retain_fsync_impact_baseline` | Phase 10 benchmark; run explicitly with --ignored --nocapture |
| `DISC_547AC119BF2B3BEFB960` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase11_seam_contract.rs:1166` | `computed_subrange_assignment_fails_visibly_without_committing_out_of_range_value` | red test for runtime-safety Phase 11 SEAM-TEST-012 |
| `DISC_265444021E11E0C2B452` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase11_seam_contract.rs:1193` | `computed_subrange_fb_input_binding_fails_visibly_without_committing_out_of_range_value` | red test for runtime-safety Phase 11 SEAM-TEST-012 |
| `DISC_D76D0BCB14E615250E0B` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase11_seam_contract.rs:1241` | `computed_subrange_ref_write_fails_visibly_without_committing_out_of_range_value` | red test for runtime-safety Phase 11 SEAM-TEST-012 |
| `DISC_53DB35C22AFA96FE355B` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase11_seam_contract.rs:1271` | `crafted_frame_local_ref_cannot_persist_to_global_storage` | red test for runtime-safety Phase 11 SEAM-TEST-007 |
| `DISC_BCA3C293212BE0D52B3D` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase11_seam_contract.rs:898` | `crafted_multi_owner_instance_refs_are_rejected_before_execution` | red test for runtime-safety Phase 11 SEAM-TEST-008 |
| `DISC_C1C1116CE99213C96040` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase11_seam_contract.rs:757` | `declared_dint_conversion_matches_iec_on_stack_register_and_tier1_paths` | red test for runtime-safety Phase 11 SEAM-TEST-004 |
| `DISC_043E03287D0BD498DBFE` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase11_seam_contract.rs:704` | `declared_dint_keeps_dint_width_after_int_assignment` | red test for runtime-safety Phase 11 SEAM-TEST-002 |
| `DISC_278EEBF061FD97B539D0` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase11_seam_contract.rs:730` | `declared_real_conversion_matches_iec_on_stack_register_and_tier1_paths` | red test for runtime-safety Phase 11 SEAM-TEST-004 |
| `DISC_1D261CB173DC0CF72297` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase11_seam_contract.rs:678` | `declared_real_keeps_real_semantics_after_integer_assignment` | red test for runtime-safety Phase 11 SEAM-TEST-001 |
| `DISC_71ACEE6895199128F92D` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase11_seam_contract.rs:1097` | `fb_string_parameter_binding_truncates_input_and_inout_fields` | red test for runtime-safety Phase 11 SEAM-TEST-011 |
| `DISC_8E59F13125886D732DA2` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase11_seam_contract.rs:784` | `parameter_copy_in_materializes_declared_numeric_widening` | red test for runtime-safety Phase 11 SEAM-IMPL-001B |
| `DISC_7DBEF68950F9AC3B5F29` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase11_seam_contract.rs:849` | `ref_return_name_is_rejected_before_runtime_lowering` | red test for runtime-safety Phase 11 SEAM-TEST-006 |
| `DISC_48945885EF6AAC63E41D` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase11_seam_contract.rs:1063` | `unsupported_array_initializer_assignment_fails_build_instead_of_nop` | red test for runtime-safety Phase 11 SEAM-TEST-010 |
| `DISC_1B5448C8DB96DF53A63C` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase11_seam_contract.rs:985` | `validator_rejects_bool_operands_for_arithmetic_opcode` | red test for runtime-safety Phase 11 SEAM-TEST-009C |
| `DISC_80833EFC8B17DF0C8754` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase11_seam_contract.rs:1001` | `validator_rejects_const_type_incompatible_with_store_ref_target` | red test for runtime-safety Phase 11 SEAM-TEST-009D |
| `DISC_E8D36E045CD657B29BDD` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase11_seam_contract.rs:1025` | `validator_rejects_inout_parameter_bound_to_literal_argument` | red test for runtime-safety Phase 11 SEAM-TEST-009E |
| `DISC_F850C3D482F1F1486738` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase11_seam_contract.rs:1011` | `validator_rejects_invalid_parameter_direction_metadata` | red test for runtime-safety Phase 11 SEAM-TEST-009E |
| `DISC_1685CC6D1F65B9539D13` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase11_seam_contract.rs:976` | `validator_rejects_leftover_stack_at_pou_return` | red test for runtime-safety Phase 11 SEAM-TEST-009C |
| `DISC_C160AB9422688E289F44` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase11_seam_contract.rs:1039` | `validator_rejects_legacy_call_opcode_even_when_target_exists` | red test for runtime-safety Phase 11 SEAM-TEST-009F |
| `DISC_EC610E3917E2D7BC4E2B` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase11_seam_contract.rs:947` | `validator_rejects_multi_owner_instance_ref_contract` | red test for runtime-safety Phase 11 SEAM-TEST-009B |
| `DISC_36947D824681068CB733` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase11_seam_contract.rs:1308` | `validator_rejects_persistent_frame_local_ref_escape` | red test for runtime-safety Phase 11 SEAM-TEST-009A |
| `DISC_0D216C5A37ABCA5C342D` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/phase11_seam_contract.rs:967` | `validator_rejects_stack_underflow_store_ref` | red test for runtime-safety Phase 11 SEAM-TEST-009C |
| `DISC_B25F48AA676D50A29BB2` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_safety_fail_closed.rs:666` | `retain_save_failure_prevents_output_commit_when_due` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_2D0B462DA31DE705F6EC` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_safety_fail_closed.rs:690` | `safe_state_write_failure_is_reported_without_losing_root_fault` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_8BAF8461F95A94A773A6` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/runtime_safety_fail_closed.rs:629` | `watchdog_deadline_breach_before_commit_prevents_output_write` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_20DF277B08348ED8A798` | `ignored` | `rust_attribute` | `rust_integration_test` | `crates/trust-runtime/tests/web_dispatch_hol_probe.rs:249` | `incomplete_body_route_blocks_unrelated_hmi_request_until_released` | explicit probe for the current single-threaded web dispatch HOL behavior |
| `DISC_6F5F5107DDE61D3C1E43` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs:456` | `perf_completion_budget` | ignore |
| `DISC_A5BFD4D2FEC4A5FE7C73` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs:759` | `perf_diagnostics_budget` | ignore |
| `DISC_F7D08DE71C821B2157CF` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs:668` | `perf_document_highlight_scaling_budget` | ignore |
| `DISC_08CEAF52EF3509019BFF` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs:813` | `perf_edit_loop_budget` | ignore |
| `DISC_B65B577CE97DDD0DC665` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs:416` | `perf_hover_budget` | ignore |
| `DISC_D5AB69F6A0EBB17DA479` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs:535` | `perf_large_workspace_index_budget` | ignore |
| `DISC_0D7E1103BA7F27B6CD0E` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs:500` | `perf_rename_budget` | ignore |
| `DISC_E0C98D6962CBF3D5E78F` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs:614` | `perf_semantic_tokens_scaling_budget` | ignore |
| `DISC_F4D12F76BE0C9EC8F0EF` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs:718` | `perf_workspace_navigation_scaling_budget` | ignore |
| `DISC_CE3D5672C7ABFFB9A685` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-lsp/src/perf.rs:572` | `perf_workspace_symbol_budget` | ignore |
| `DISC_85C1B6864B76D834F59A` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/audit_durability.rs:3` | `control_audit_send_failure_records_audit_dropped_event` | red test for runtime-safety fail-closed Phase 8 |
| `DISC_C2C7721DF77E1F95004E` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/audit_durability.rs:33` | `debug_feature_disabled_returns_structured_feature_disabled_response` | red test for runtime-safety fail-closed Phase 8 |
| `DISC_1257BED418738BD81458` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-runtime/src/control/tests/hmi_values_write.rs:668` | `hmi_write_rejects_out_of_range_subrange_value` | red test for runtime-safety Phase 11 SEAM-TEST-012 |
| `DISC_FDFF5424770C2D53EE41` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/errors.rs:93` | `evaluator_unknown_assignment_fails_without_creating_global` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_2DF32E01941660BF1A3C` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/pou_function.rs:58` | `function_input_default_failure_returns_init_failed` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_A06DF7FD3DD0AA8532B9` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/pou_function.rs:140` | `function_local_default_failure_returns_init_failed` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_E3F480EBCF048676B771` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-runtime/src/host/eval/tests/pou_function.rs:110` | `function_return_default_failure_returns_init_failed` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_30BF7A25E82D62C835F3` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-runtime/src/host/helper_eval/storage_lvalue.rs:350` | `unknown_name_write_fails_without_creating_global` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_17A3C97ED5EF6F1D342D` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-runtime/src/host/mesh/tests.rs:143` | `mesh_snapshot_timeout_is_not_a_successful_empty_snapshot` | red test for runtime-safety fail-closed Phase 8 |
| `DISC_AAC32F62FD46D1A39C78` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-runtime/src/io/gpio.rs:817` | `gpio_read_failure_updates_driver_health` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_E944C2D055EB2EDB0E79` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-runtime/src/io/gpio.rs:836` | `gpio_write_failure_updates_driver_health` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_2A6A13834385AB7C9A1D` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs:1204` | `fail_closed_connect_failure_is_observable` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_C3A91E2860C30B08BED4` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs:1134` | `fail_closed_disconnected_read_returns_freshness_error` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_A58F9A1B38CF9A02367C` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-runtime/src/io/mqtt/tests.rs:1170` | `fail_closed_publish_failure_returns_output_error` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_5BBB12B235856C1C45CC` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs:212` | `compile_session_surfaces_openot_validation_failure_instead_of_building_uninstrumented_bytecode` | red test for runtime-safety Phase 11 SEAM-TEST-015 |
| `DISC_1466DAFE96EF828CB3AF` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-runtime/src/openot_authoring/tests.rs:132` | `hir_and_runtime_authoring_report_explicit_sourceid_collisions_consistently` | red test for runtime-safety Phase 11 SEAM-TEST-014 |
| `DISC_C322445040BB29FD414A` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/local_init.rs:495` | `vm_local_default_failure_returns_init_failed` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_4375CD7D7381F6012287` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-runtime/src/runtime/vm/local_init.rs:474` | `vm_return_slot_default_failure_returns_init_failed` | red test for runtime-safety fail-closed Phase 1 |
| `DISC_72E65032998D488D9425` | `ignored` | `rust_attribute` | `rust_unit_test` | `crates/trust-runtime/src/web/runtime_cloud_state/config.rs:186` | `runtime_cloud_corrupt_config_state_does_not_reset_to_default` | red test for runtime-safety fail-closed Phase 8 |
| `DISC_78213ECD5BFD1CD1ACE3` | `conditional` | `vscode_runtime_skip` | `vscode_test` | `editors/vscode/src/test/suite/new-project.test.ts:339` | `generated ST parses cleanly and TOML is usable by build` | runtime this.skip() cannot be represented as a declared ignore attribute |

## Limitations

- Discovery is static and recognizes only the ignore mechanisms named by this report.
- Runtime this.skip() is accepted only when contained in exactly one literal VS Code test callback.
- Rust files under xtask, root fuzz, and crate-local fuzz are bound by a fail-closed ignore-marker sentinel until identity support is added.
- Modeled Node identities are limited to VS Code Mocha and tracked Playwright capture specs; other tracked test/spec files use a fail-closed skip sentinel.
- Shell source has no repository-wide static ignored-test identity convention.
- Conformance runtime skipped results are outcomes, not source ignore declarations.
- Ignore classes, owners, areas, unblock conditions, expected behavior, and proof are hand-owned metadata.
