"""Reviewed exact-test routes for the Phase 18 runtime-cloud tranche."""

from __future__ import annotations

from collections.abc import Mapping


BEHAVIOR_TEST_NAMES: Mapping[str, tuple[str, ...]] = {
    "RT_MANAGED_FLEET_LIFECYCLE_001#behavior:0": (
        "empty_template_uses_valid_loopback_io",
        "fleet_runtime_add_auto_selects_distinct_available_loopback_ports",
        "fleet_runtime_add_creates_simulated_project_and_manifest",
        "generated_control_token_contains_32_random_bytes",
        "resource_names_are_deterministic_and_never_start_with_a_digit",
    ),
    "RT_MANAGED_FLEET_LIFECYCLE_001#behavior:1": (
        "control_token_generation_fails_closed_when_secure_randomness_fails",
        "fleet_runtime_add_rejects_any_manifest_port_collision",
        "fleet_runtime_add_rejects_duplicate_name_without_rewriting",
        "fleet_runtime_add_rejects_malformed_manifest_before_project_mutation",
        "fleet_runtime_add_rejects_whitespace_name_before_mutation",
        "load_manifest_rejects_invalid_endpoints_and_port_collisions",
        "load_manifest_rejects_unsafe_or_duplicate_runtime_names",
    ),
    "RT_MANAGED_FLEET_LIFECYCLE_001#behavior:2": (
        "control_request_rejects_a_mismatched_response_id",
        "control_request_rejects_malformed_or_ambiguous_response_envelopes",
        "control_response_reader_rejects_an_oversized_line",
        "fleet_runtime_status_rejects_a_connected_control_error",
        "fleet_runtime_status_rejects_malformed_runtime_auth_config",
    ),
    "RT_MANAGED_FLEET_LIFECYCLE_001#behavior:3": (
        "fleet_runtime_logs_return_requested_tail",
        "fleet_runtime_start_aborts_when_pid_persistence_fails",
        "fleet_runtime_start_removes_pid_when_child_exits_before_ready",
        "fleet_runtime_status_reports_stopped_when_endpoint_unreachable",
        "fleet_runtime_status_uses_endpoint_evidence_not_a_stale_pid",
        "managed_runtime_lifecycle_runs_through_the_shipped_cli",
        "missing_runtime_logs_are_empty_without_creating_state",
        "terminate_started_child_kills_and_reaps_a_live_process",
    ),
    "RT_RUNTIME_CLOUD_ACTION_POLICY_001#behavior:0": (
        "preflight_distinguishes_stale_target_and_allows_local_target",
        "runtime_cloud_preflight_allows_cross_site_cfg_apply_with_allowlist",
    ),
    "RT_RUNTIME_CLOUD_ACTION_POLICY_001#behavior:1": (
        "preflight_rejects_empty_blank_or_duplicate_target_lists",
    ),
    "RT_RUNTIME_CLOUD_ACTION_POLICY_001#behavior:2": (
        "dev_and_plant_profiles_do_not_require_wan_write_allowlist",
        "runtime_cloud_preflight_dev_allows_cross_site_cfg_apply_without_allowlist",
    ),
    "RT_RUNTIME_CLOUD_ACTION_POLICY_001#behavior:3": (
        "wan_profile_denies_write_without_matching_rule",
    ),
    "RT_RUNTIME_CLOUD_CONFIG_RECONCILE_001#behavior:0": (
        "runtime_cloud_config_agent_reconciles_desired_reported_meta_and_status",
    ),
    "RT_RUNTIME_CLOUD_CONFIG_RECONCILE_001#behavior:1": (
        "superseded_apply_success_keeps_newer_desired_revision_pending",
    ),
    "RT_RUNTIME_CLOUD_CONFIG_RECONCILE_001#behavior:2": (
        "superseded_apply_failure_keeps_newer_desired_revision_pending",
    ),
    "RT_RUNTIME_CLOUD_HA_FENCING_001#behavior:0": (
        "replay_guard_deduplicates_and_rejects_stale_sequences",
    ),
    "RT_RUNTIME_CLOUD_HA_FENCING_001#behavior:1": (
        "dual_host_requires_external_consistent_lease_authority",
        "lease_loss_for_active_runtime_requires_demoted_safe_behavior",
    ),
    "RT_RUNTIME_CLOUD_HA_FENCING_001#behavior:2": (
        "runtime_cloud_ha_split_brain_preflight_checks_candidates_outside_action_subset",
        "runtime_cloud_ha_split_brain_preflight_denies_dual_active_candidates",
        "split_brain_candidates_are_detected_and_rejected",
        "split_brain_detection_uses_complete_group_view_not_action_subset",
    ),
    "RT_RUNTIME_CLOUD_HA_FENCING_001#behavior:3": (
        "live_in_flight_duplicate_is_denied_but_recovery_can_resume_once",
    ),
    "RT_RUNTIME_CLOUD_HA_FENCING_001#behavior:4": (
        "crash_mid_command_can_retry_after_state_roundtrip",
    ),
    "RT_RUNTIME_CLOUD_PROXY_PLAN_001#behavior:1": (
        "proxy_plan_rejects_blank_request_id_when_supplied",
        "write_plan_rejects_empty_actor",
        "write_plan_validates_target_before_actor",
    ),
    "RT_RUNTIME_CLOUD_PROXY_PLAN_001#behavior:2": (
        "proxy_plan_rejects_breaking_api_version",
        "proxy_plan_rejects_invalid_api_versions",
    ),
    "RT_RUNTIME_CLOUD_SCHEMA_KEYSPACE_001#behavior:0": (
        "schema_layout_accepts_forward_additive_changes_and_rejects_breaking_changes",
    ),
    "RT_RUNTIME_CLOUD_SCHEMA_KEYSPACE_001#behavior:1": (
        "schema_layout_rejects_ambiguous_or_non_appended_fields",
    ),
    "RT_RUNTIME_CLOUD_TOPOLOGY_LINK_POLICY_001#behavior:0": (
        "runtime_cloud_projection_contract_reports_topology_edges_and_warnings",
    ),
    "RT_RUNTIME_CLOUD_TOPOLOGY_LINK_POLICY_001#behavior:1": (
        "runtime_cloud_link_transport_endpoint_switches_edge_mode_and_checks_same_host",
    ),
    "RT_RUNTIME_CLOUD_TOPOLOGY_LINK_POLICY_001#behavior:3": (
        "runtime_cloud_link_transport_endpoint_switches_edge_mode_and_checks_same_host",
    ),
}
