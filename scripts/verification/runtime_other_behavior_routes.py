"""Reviewed exact-test routes for the Phase 18 runtime-other behavior tranche."""

from __future__ import annotations

from collections.abc import Mapping


BEHAVIOR_TEST_NAMES: Mapping[str, tuple[str, ...]] = {
    "EDIT_RENAME_001#behavior:3": (
        "test_rename_refuses_cross_file_reference_capture",
        "test_rename_refuses_shadow_capture_with_named_reason",
    ),
    "OSCAT_CLASSIC_LIBRARY_PROFILE_001#behavior:1": (
        "oscat_replaced_upstream_name_remains_absent",
    ),
    "RT_BENCH_PROJECT_INIT_001#behavior:1": (
        "project_workload_rejects_missing_project_folder",
        "project_workload_rejects_zero_samples",
    ),
    "RT_BENCH_STATISTICS_OUTPUT_001#behavior:0": (
        "summarize_ns_computes_quantiles",
        "histogram_includes_overflow_bucket",
    ),
    "RT_BENCH_STATISTICS_OUTPUT_001#behavior:1": (
        "jitter_samples_require_two_latency_observations",
    ),
    "RT_BENCH_STATISTICS_OUTPUT_001#behavior:2": (
        "init_bench_json_output_contains_startup_latency_fields",
        "project_bench_json_output_contains_budget_and_watched_globals",
        "project_bench_table_output_contains_tier1_executor_stats_when_enabled",
        "t0_shm_bench_json_output_contains_latency_and_overrun_fields",
        "mesh_zenoh_bench_json_output_contains_loss_and_reorder_fields",
        "dispatch_bench_table_output_contains_fanout_and_audit_metrics",
    ),
    "RT_BENCH_TRANSPORT_DISPATCH_001#behavior:0": (
        "mesh_bench_reports_requested_pubsub_and_query_payload_sizes",
        "dispatch_bench_reports_requested_payload_size_without_a_hidden_cap",
    ),
    "RT_BENCH_TRANSPORT_DISPATCH_001#behavior:1": (
        "t0_shm_bench_json_output_contains_latency_and_overrun_fields",
        "mesh_zenoh_bench_json_output_contains_loss_and_reorder_fields",
        "dispatch_bench_table_output_contains_fanout_and_audit_metrics",
    ),
    "RT_BENCH_TRANSPORT_DISPATCH_001#behavior:2": (
        "mesh_workload_rejects_out_of_range_rates",
    ),
    "RT_BUILD_COMMAND_001#behavior:1": (
        "build_compile_failure_preserves_existing_program_and_exits_11",
    ),
    "RT_BUILD_COMMAND_001#behavior:2": (
        "build_human_mode_discovers_current_project_and_summarizes_sources",
    ),
    "RT_CHECK_PROJECT_CONTRACT_001#behavior:0": (
        "check_accepts_project_without_writing_program_stbc",
        "check_ci_reports_sources_override_and_local_dependencies",
    ),
    "RT_CHECK_PROJECT_CONTRACT_001#behavior:1": (
        "check_aggregates_required_and_optional_config_errors_with_stable_exit_code",
        "check_mixed_config_and_compile_errors_uses_config_exit_and_reports_both",
        "check_reports_source_layout_error_with_invalid_config_exit_code",
    ),
    "RT_CI_EXIT_CLASSIFICATION_001#behavior:0": (
        "classify_test_failure_code",
        "classify_build_failure_code",
        "classify_invalid_config_code",
        "classify_internal_code",
        "classify_timeout_code",
    ),
    "RT_CI_EXIT_CLASSIFICATION_001#behavior:1": (
        "recognized_error_precedence_is_stable",
        "command_context_does_not_override_recognized_error",
    ),
    "RT_CI_EXIT_CLASSIFICATION_001#behavior:2": (
        "classify_with_command_falls_back_for_internal",
        "classify_internal_code",
    ),
    "RT_CONFIG_BUNDLE_LOAD_001#behavior:0": (
        "runtime_bundle_loads_ads_toml_when_enabled",
        "runtime_bundle_loads_opcua_client_toml_when_enabled",
    ),
    "RT_CONFIG_BUNDLE_LOAD_001#behavior:1": (
        "runtime_bundle_rejects_missing_enabled_ads_toml",
        "runtime_bundle_rejects_missing_enabled_opcua_client_toml",
    ),
    "RT_CONFIG_SCHEMA_001#behavior:2": (
        "runtime_schema_rejects_blank_explicit_collection_entries",
        "runtime_schema_rejects_blank_explicit_paths",
        "runtime_schema_rejects_blank_explicit_scalars",
    ),
    "RT_CONFIG_SCHEMA_001#behavior:3": (
        "runtime_schema_rejects_remote_web_without_tls_when_required",
        "runtime_schema_requires_control_auth_for_tcp_endpoints",
        "runtime_schema_requires_deploy_keyring_when_signed_deploy_enabled",
        "runtime_schema_requires_opcua_credentials_or_anonymous_when_enabled",
        "runtime_schema_requires_tls_credentials_when_tls_enabled",
        "runtime_schema_rejects_ads_server_plain_without_ack",
        "runtime_schema_rejects_ads_server_structured_client_without_source_pin",
        "runtime_schema_rejects_ads_server_writable_not_exposed",
    ),
    "RT_CONFIG_VALUE_BOUNDS_001#behavior:0": (
        "runtime_schema_preserves_signed_duration_bounds",
        "io_schema_preserves_safe_state_integer_widths",
    ),
    "RT_CONFIG_VALUE_BOUNDS_001#behavior:1": (
        "runtime_schema_preserves_signed_duration_bounds",
        "io_schema_preserves_safe_state_integer_widths",
    ),
    "RT_CONFORMANCE_RUNNER_INTEGRITY_001#behavior:0": (
        "conformance_command_supports_update_verify_and_mismatch_taxonomy",
    ),
    "RT_CONFORMANCE_RUNNER_INTEGRITY_001#behavior:1": (
        "discovery_rejects_manifest_identity_and_missing_suite_root",
        "discovery_rejects_case_directories_without_manifests",
        "discovery_rejects_unknown_category_directories",
        "case_sources_must_remain_inside_the_case_directory",
        "typed_value_parser_and_encoder_cover_supported_scalar_contract",
        "series_restart_and_step_validation_cover_closed_partitions",
        "connector_status_trace_rejects_unknown_sources_states_and_expectations",
    ),
    "RT_CONFORMANCE_RUNNER_INTEGRITY_001#behavior:2": (
        "conformance_command_supports_update_verify_and_mismatch_taxonomy",
    ),
    "RT_CONFORMANCE_RUNNER_INTEGRITY_001#behavior:3": (
        "series_restart_and_step_validation_cover_closed_partitions",
        "conformance_command_supports_update_verify_and_mismatch_taxonomy",
    ),
    "RT_CONTROL_CLI_CLIENT_001#behavior:0": (
        "ctl_target_resolution_enforces_endpoint_and_token_precedence",
        "control_actions_map_to_the_registered_wire_contract",
        "ctl_status_sends_authenticated_request_and_prints_summary",
    ),
    "RT_CONTROL_CLI_CLIENT_001#behavior:1": (
        "control_exchange_uses_one_json_line_and_rejects_invalid_responses",
        "control_value_exchange_enforces_the_established_read_timeout",
        "ctl_returns_nonzero_for_rejected_control_response",
        "ctl_returns_nonzero_for_malformed_success_results",
        "ctl_returns_nonzero_when_control_response_omits_ok",
    ),
    "RT_CONTROL_CLI_CLIENT_001#behavior:2": (
        "ctl_target_resolution_enforces_endpoint_and_token_precedence",
        "ctl_endpoint_override_preserves_project_token",
    ),
    "RT_CONTROL_CLI_CLIENT_001#behavior:3": (
        "config_values_and_control_results_cover_closed_scalar_partitions",
        "control_actions_map_to_the_registered_wire_contract",
        "ctl_status_sends_authenticated_request_and_prints_summary",
        "ctl_stats_renders_each_task_in_stable_compact_form",
    ),
    "RT_CORE_VALUE_PROGRAM_MODEL_001#behavior:18": (
        "normalize_assignment_materializes_safe_numeric_widening_tags",
    ),
    "RT_CORE_VALUE_PROGRAM_MODEL_001#behavior:30": (
        "default_null_reference",
    ),
    "RT_DIRECT_ADDRESS_PARSER_001#behavior:1": (
        "direct_address_parser_covers_hierarchy_and_exact_wildcards",
    ),
    "RT_DIRECT_ADDRESS_PARSER_001#behavior:2": (
        "direct_address_parser_rejects_malformed_wildcards",
    ),
    "RT_DIRECT_ADDRESS_PARSER_001#behavior:3": (
        "direct_address_parser_rejects_non_decimal_and_out_of_range_components",
    ),
    "RT_DIRECT_ADDRESS_PARSER_001#behavior:4": (
        "direct_address_parser_rejects_non_decimal_and_out_of_range_components",
    ),
    "RT_GIT_INITIALIZATION_001#behavior:0": (
        "git_init_creates_repository_and_accepts_existing_git_directory",
    ),
    "RT_GIT_INITIALIZATION_001#behavior:1": (
        "git_init_creates_repository_and_accepts_existing_git_directory",
        "git_init_accepts_a_valid_worktree_gitdir_marker",
        "git_init_accepts_directory_symlink_and_rejects_dangling_symlink",
    ),
    "RT_GIT_INITIALIZATION_001#behavior:2": (
        "git_init_rejects_invalid_worktree_marker_shapes",
        "git_init_rejects_an_invalid_git_file_marker",
        "git_init_rejects_unsupported_git_filesystem_object",
        "git_init_accepts_directory_symlink_and_rejects_dangling_symlink",
    ),
    "RT_GPIO_DRIVER_001#behavior:3": (
        "gpio_safe_state_handoff_writes_and_confirms_backend_output",
    ),
    "RT_GUIDED_PROJECT_ARTIFACTS_001#behavior:0": (
        "auto_bundle_migrates_legacy_sources_and_compiles_dependencies",
        "apply_setup_persists_runtime_artifacts",
        "setup_cli_mode_writes_artifacts_and_next_steps",
    ),
    "RT_GUIDED_PROJECT_ARTIFACTS_001#behavior:1": (
        "runtime_generation_preserves_existing_artifacts_when_sources_fail_to_compile",
        "zero_cycle_configuration_is_rejected_before_runtime_file_mutation",
        "selecting_system_io_rejects_project_io_directory",
    ),
    "RT_GUIDED_PROJECT_ARTIFACTS_001#behavior:2": (
        "selecting_system_io_removes_dangling_project_io_link",
        "selecting_system_io_rejects_project_io_directory",
    ),
    "RT_HARNESS_BOUNDARY_IO_001#behavior:0": (
        "trust_harness_set_input_then_get_output_roundtrips",
        "trust_harness_var_access_and_direct_io_commands_roundtrip",
    ),
    "RT_HARNESS_BOUNDARY_IO_001#behavior:1": (
        "trust_harness_var_access_and_direct_io_commands_roundtrip",
        "io_by_address",
    ),
    "RT_HARNESS_BOUNDARY_IO_001#behavior:2": (
        "trust_harness_error_kinds_separate_structure_values_runtime_and_boundaries",
    ),
    "RT_HARNESS_LIFECYCLE_CONTROL_001#behavior:0": (
        "trust_harness_failed_load_and_reload_preserve_the_live_session",
        "trust_harness_reload_preserves_retain_state",
    ),
    "RT_HARNESS_LIFECYCLE_CONTROL_001#behavior:1": (
        "trust_harness_time_alias_precedence_and_prechecked_run_until_are_deterministic",
        "trust_harness_advance_time_then_cycle_exposes_timer_progress",
        "trust_harness_cycle_dt_ms_advances_virtual_time",
    ),
    "RT_HARNESS_LIFECYCLE_CONTROL_001#behavior:2": (
        "trust_harness_snapshot_is_passive_and_restart_modes_are_explicit",
    ),
    "RT_HARNESS_TRANSPORT_PROTOCOL_001#behavior:0": (
        "trust_harness_reports_line_errors_and_continues_the_session",
        "trust_harness_cli_protocol_selection_overrides_environment_and_rejects_bad_arguments",
    ),
    "RT_HARNESS_TRANSPORT_PROTOCOL_001#behavior:1": (
        "trust_harness_protocol_version_1_keeps_legacy_watch_shape",
        "trust_harness_protocol_v1_promotes_watch_errors_to_request_errors",
    ),
    "RT_HARNESS_TRANSPORT_PROTOCOL_001#behavior:2": (
        "trust_harness_error_kinds_separate_structure_values_runtime_and_boundaries",
    ),
    "RT_HMI_SCAFFOLD_COMMAND_001#behavior:0": (
        "hmi_init_compiles_resolved_local_dependency_sources",
        "hmi_init_treats_glob_metacharacters_in_project_path_literally",
        "scaffold_output_is_deterministic_for_same_input",
    ),
    "RT_HMI_SCAFFOLD_COMMAND_001#behavior:1": (
        "hmi_source_and_compile_failures_leave_descriptor_unchanged",
        "hmi_without_project_preserves_bundle_detection_failure",
    ),
    "RT_IO_CYCLE_BINDING_001#behavior:7": (
        "alias_resolves_to_canonical_driver_name",
    ),
    "RT_NUMERIC_PROMPT_001#behavior:0": (
        "numeric_prompt_parser_accepts_the_complete_u64_range",
        "numeric_prompt_wrapper_preserves_zero_ordinary_default_and_errors",
    ),
    "RT_NUMERIC_PROMPT_001#behavior:1": (
        "numeric_prompt_wrapper_preserves_zero_ordinary_default_and_errors",
    ),
    "RT_NUMERIC_PROMPT_001#behavior:2": (
        "numeric_prompt_parser_rejects_negative_non_numeric_and_overflow_values",
        "numeric_prompt_wrapper_preserves_zero_ordinary_default_and_errors",
    ),
    "RT_OFFLINE_COMM_AUTHORING_CLI_001#behavior:0": (
        "comm_cli_runs_schema_apply_topology_and_cached_browse_offline",
        "comm_cli_validates_payloads_and_manages_the_selected_opcua_trust_store",
    ),
    "RT_OFFLINE_COMM_AUTHORING_CLI_001#behavior:1": (
        "comm_cli_rejects_malformed_local_authoring_inputs",
    ),
    "RT_OFFLINE_COMM_AUTHORING_CLI_001#behavior:2": (
        "comm_cli_runs_schema_apply_topology_and_cached_browse_offline",
    ),
    "RT_OPENOT_CONSUMER_RECONCILIATION_001#behavior:0": (
        "openot_capstone_fenced_cross_process",
    ),
    "RT_OPENOT_CONSUMER_RECONCILIATION_001#behavior:1": (
        "openot_capstone_fenced_cross_process",
    ),
    "RT_OPENOT_EVENT_MATERIALIZATION_001#behavior:0": (
        "openot_telemetry_authoring_fixed_width_values_round_trip",
        "openot_telemetry_authoring_parameter_change_round_trip",
        "openot_telemetry_authoring_condition_lifecycle_round_trip",
        "openot_telemetry_authoring_message_args_and_condition_correlation_round_trip",
    ),
    "RT_OPENOT_EVENT_MATERIALIZATION_001#behavior:1": (
        "openot_telemetry_authoring_condition_lifecycle_round_trip",
        "openot_telemetry_authoring_esignature_cross_scan_attests_prior_event",
        "openot_telemetry_authoring_esignature_same_scan_is_phased_last",
        "openot_telemetry_esignature_attestation_state_resets_on_epoch_transition",
    ),
    "RT_OPENOT_EVENT_MATERIALIZATION_001#behavior:2": (
        "openot_telemetry_authoring_esignature_never_emitted_target_fails_closed",
        "openot_telemetry_authoring_condition_ack_without_activation_fails_closed",
        "openot_telemetry_authoring_condition_shelve_without_activation_fails_closed",
        "openot_telemetry_authoring_condition_confirm_without_activation_fails_closed",
        "openot_telemetry_authoring_condition_unshelve_without_activation_fails_closed",
        "openot_telemetry_authoring_condition_reset_without_activation_fails_closed",
    ),
    "RT_OPENOT_EVENT_MATERIALIZATION_001#behavior:3": (
        "openot_telemetry_authoring_batch_recipe_round_trip",
        "openot_telemetry_authoring_operator_regulated_round_trip",
    ),
    "RT_OPENOT_EVENT_MATERIALIZATION_001#behavior:4": (
        "openot_telemetry_authoring_showcase_renders_typed_audit_log",
        "openot_telemetry_authoring_sampling_policy_round_trips_definition",
    ),
    "RT_OPENOT_ST_PRODUCER_CONFORMANCE_001#behavior:0": (
        "openot_st_scan_records_burst_pou_passes",
        "openot_telemetry_publishes_real_st_producer_records",
    ),
    "RT_OPENOT_ST_PRODUCER_CONFORMANCE_001#behavior:1": (
        "openot_st_value_changed_vectors_are_byte_exact",
        "openot_st_condition_lifecycle_vectors_are_byte_exact",
        "openot_st_batch_recipe_vectors_are_byte_exact",
        "openot_st_regulated_vectors_are_byte_exact",
    ),
    "RT_OPENOT_ST_PRODUCER_CONFORMANCE_001#behavior:2": (
        "openot_telemetry_st_fb_multi_record_scan_is_fail_closed",
        "openot_telemetry_parameter_change_oversize_drop_keeps_baseline",
        "openot_telemetry_authoring_condition_comment_oversize_fails_closed",
    ),
    "RT_OPENOT_TELEMETRY_PUBLICATION_001#behavior:2": (
        "openot_telemetry_publishes_multi_program_authoring_sources_to_one_ring",
        "openot_telemetry_drains_multi_program_source_high_water_to_one_ring",
        "openot_telemetry_publishes_st_transition_burst",
    ),
    "RT_PLAY_LAUNCH_001#behavior:1": (
        "play_explicit_project_shape_classification_is_fail_closed",
    ),
    "RT_PLAY_LAUNCH_001#behavior:2": (
        "play_explicit_project_shape_classification_is_fail_closed",
    ),
    "RT_PLAY_LAUNCH_001#behavior:3": (
        "play_explicit_project_shape_classification_is_fail_closed",
    ),
    "RT_PLAY_LAUNCH_001#behavior:4": (
        "play_project_resolution_distinguishes_absence_from_detection_failure",
    ),
    "RT_PLAY_LAUNCH_001#behavior:5": (
        "play_project_resolution_distinguishes_absence_from_detection_failure",
    ),
    "RT_PLAY_LAUNCH_001#behavior:6": (
        "play_project_resolution_distinguishes_absence_from_detection_failure",
    ),
    "RT_PROCESS_IMAGE_CONTRACT_001#behavior:7": (
        "io_snapshot_emitted_after_cycle_output_commit",
    ),
    "RT_REGISTRY_PROJECT_RESOLUTION_001#behavior:0": (
        "registry_publish_download_verify_round_trip",
    ),
    "RT_REGISTRY_PROJECT_RESOLUTION_001#behavior:1": (
        "registry_publish_without_explicit_project_uses_detected_bundle",
    ),
    "RT_REGISTRY_PROJECT_RESOLUTION_001#behavior:2": (
        "registry_publish_without_project_preserves_detection_failure",
    ),
    "RT_REGISTRY_PROJECT_RESOLUTION_001#behavior:3": (
        "registry_publish_help_describes_standard_bundle_detection",
        "registry_publish_without_explicit_project_uses_detected_bundle",
    ),
    "RT_RETAIN_PERSISTENCE_CODEC_001#behavior:7": (
        "retain_store_roundtrip",
    ),
    "RT_RETAIN_PERSISTENCE_CODEC_001#behavior:8": (
        "retain_store_loads_legacy_v1_snapshot",
    ),
    "RT_RUNTIME_CLI_ENTRYPOINT_001#behavior:1": (
        "mistyped_check_command_suggests_the_real_subcommand",
        "distant_unknown_command_is_not_given_a_misleading_suggestion",
        "subcommand_suggestion_ignores_global_flags",
    ),
    "RT_RUNTIME_CLI_ENTRYPOINT_001#behavior:2": (
        "deprecated_bundle_alias_warns_and_missing_project_gets_creation_tip",
    ),
    "RT_RUNTIME_CLI_ENTRYPOINT_001#behavior:3": (
        "ci_mode_classifies_an_invalid_validate_project",
        "deprecated_bundle_alias_warns_and_missing_project_gets_creation_tip",
    ),
    "RT_SAFE_DEADLINE_001#behavior:7": (
        "disabled_watchdog_does_not_arm_cycle_deadlines",
    ),
    "RT_SAFE_IO_WORKER_001#behavior:0": (
        "modbus_safe_state_handoff_reports_unconfirmed_worker_delivery",
        "mqtt_safe_state_handoff_reports_unconfirmed_connecting_worker",
        "safe_state_handoff_trace_cases",
        "requested_stop_faults_when_safe_state_handoff_is_unconfirmed",
    ),
    "RT_SAFE_RESTART_001#behavior:5": (
        "restart_instance_construction_failure_preserves_live_state",
    ),
    "RT_SAFE_STOP_001#behavior:0": (
        "requested_stop_faults_when_safe_state_handoff_is_unconfirmed",
        "safe_state_handoff_trace_cases",
    ),
    "RT_SETUP_BROWSER_SECURITY_001#behavior:0": (
        "setup_access_allows_when_token_not_required",
        "browser_profile_local_enforces_loopback_and_no_token",
    ),
    "RT_SETUP_BROWSER_SECURITY_001#behavior:1": (
        "setup_access_requires_matching_token",
        "setup_query_parsing_preserves_token_parameter",
        "remote_setup_token_expiry_uses_checked_minutes",
        "setup_access_enforces_expiry",
    ),
    "RT_SETUP_BROWSER_SECURITY_001#behavior:2": (
        "setup_access_requires_matching_token",
        "setup_access_enforces_expiry",
    ),
    "RT_SETUP_BROWSER_SECURITY_001#behavior:3": (
        "remote_setup_ttl_overflow_returns_error_instead_of_panicking",
        "remote_setup_token_expiry_uses_checked_minutes",
    ),
    "RT_SETUP_BROWSER_SECURITY_001#behavior:4": (
        "remote_setup_token_generation_fails_closed_when_secure_randomness_fails",
        "remote_setup_token_contains_exact_secure_random_bytes",
    ),
    "RT_SETUP_MODE_ROUTING_001#behavior:1": (
        "setup_cli_dry_run_reports_plan_without_creating_project",
        "setup_browser_remote_dry_run_shows_token_requirements",
    ),
    "RT_SETUP_MODE_ROUTING_001#behavior:2": (
        "setup_rejects_system_and_guided_flag_mix_without_mutation",
        "setup_cli_rejects_browser_only_options_without_mutation",
        "setup_cli_rejects_explicit_default_browser_options_without_mutation",
        "setup_without_mode_rejects_noninteractive_input",
    ),
    "RT_STANDALONE_IDE_BOOTSTRAP_001#behavior:1": (
        "ide_serve_reports_invalid_primary_runtime_before_server_start",
        "ide_serve_rejects_non_directory_project_before_server_start",
    ),
    "RT_STANDALONE_IDE_BOOTSTRAP_001#behavior:2": (
        "config_mode_control_state_is_local_nonexecuting_and_source_backed",
        "source_registry_uses_the_selected_workspace_runtime_and_build_source_order",
    ),
    "RT_VERSIONED_DEPLOY_STATE_001#behavior:2": (
        "bundle_targets_include_current_and_immediate_previous",
        "deploy_without_a_current_bundle_clears_a_stale_previous_pointer",
        "deploy_with_relative_root_keeps_current_and_immediate_previous_for_rollback",
        "prune_bundles_keeps_only_selected_directories",
    ),
    "RT_VERSIONED_DEPLOY_STATE_001#behavior:3": (
        "safe_state_comparison_uses_structural_addresses",
        "source_diff_propagates_invalid_source_directory_errors",
        "source_collection_handles_glob_metacharacters_in_project_paths",
        "deployment_summary_reports_current_bundle_and_any_runtime_file_change",
    ),
    "RT_WATCHDOG_POLICY_INSTALL_001#behavior:2": (
        "watchdog_policy_normalization_and_installation_cover_boundaries",
    ),
    "RT_WORKBENCH_ALIAS_FORWARDING_001#behavior:1": (
        "docs_alias_ignores_an_unusable_sibling_and_uses_path",
        "trust_runtime_docs_alias_forwards_to_trust_dev",
    ),
    "RT_WORKBENCH_ALIAS_FORWARDING_001#behavior:2": (
        "docs_alias_reports_an_unlaunchable_explicit_trust_dev",
        "docs_alias_propagates_a_normal_child_exit_code",
        "docs_alias_maps_a_signaled_child_to_shell_status",
    ),
}
