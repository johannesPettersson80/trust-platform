"""Reviewed exact-test routes for the Phase 18 protocol tranche."""

from __future__ import annotations

from collections.abc import Mapping


BEHAVIOR_TEST_NAMES: Mapping[str, tuple[str, ...]] = {
    "RT_MQTT_DEFAULT_FAULT_LATEST_VALUE_001#behavior:0": (
        "mqtt_raw_read_uses_newest_payload_from_drained_batch",
    ),
    "RT_MQTT_DEFAULT_FAULT_LATEST_VALUE_001#behavior:1": (
        "mqtt_default_fault_rejects_stale_snapshot_when_refresh_times_out",
    ),
    "RT_MQTT_DEFAULT_FAULT_LATEST_VALUE_001#behavior:2": (
        "mqtt_policy_preserves_typed_output_preflight_failure",
    ),
    "RT_MQTT_DEFAULT_FAULT_LATEST_VALUE_001#behavior:3": (
        "mqtt_policy_preserves_sparkplug_output_preflight_failure",
    ),
    "RT_MQTT_DEFAULT_FAULT_LATEST_VALUE_001#behavior:4": (
        "mqtt_default_fault_rejects_output_handoff_timeout",
    ),
    "RT_MQTT_DEFAULT_FAULT_LATEST_VALUE_001#behavior:5": (
        "mqtt_stale_snapshot_is_returned_without_fresh_payload",
    ),
    "RT_MQTT_DEFAULT_FAULT_LATEST_VALUE_001#behavior:7": (
        "mqtt_policy_preserves_typed_output_preflight_failure",
    ),
    "RT_MQTT_DEFAULT_FAULT_LATEST_VALUE_001#behavior:8": (
        "mqtt_policy_preserves_sparkplug_output_preflight_failure",
    ),
    "RT_OPCUA_CLIENT_WORKER_CACHE_INTEGRITY_001#behavior:0": (
        "persistent_worker_applies_subscription_updates_without_reconnecting_per_scan",
    ),
    "RT_OPCUA_CLIENT_WORKER_CACHE_INTEGRITY_001#behavior:1": (
        "current_session_samples_require_exact_configured_identity",
    ),
    "RT_OPCUA_CLIENT_WORKER_CACHE_INTEGRITY_001#behavior:2": (
        "older_active_session_sample_cannot_replace_newer_value",
        "older_session_callbacks_cannot_regress_reconnected_worker",
    ),
    "RT_OPCUA_CLIENT_WORKER_CACHE_INTEGRITY_001#behavior:3": (
        "write_completion_preserves_newer_pending_generation",
    ),
    "RT_OPCUA_CLIENT_WORKER_CACHE_INTEGRITY_001#behavior:4": (
        "non_finite_outputs_cancel_pending_and_allow_later_finite_value",
    ),
    "RT_OPCUA_CLIENT_WORKER_CACHE_INTEGRITY_001#behavior:5": (
        "wire_write_status_cardinality_must_match_request",
    ),
    "RT_OPCUA_CLIENT_WORKER_CACHE_INTEGRITY_001#behavior:6": (
        "worker_shutdown_interrupts_idle_wait_and_revokes_good_authority",
    ),
    "RT_OPCUA_CLIENT_CONFIG_TRUST_INTEGRITY_001#behavior:0": (
        "opcua_client_accepts_vs_code_global_var_names",
    ),
    "RT_OPCUA_CLIENT_CONFIG_TRUST_INTEGRITY_001#behavior:1": (
        "client_config_rejects_ambiguous_identity_auth_and_access",
    ),
    "RT_OPCUA_CLIENT_CONFIG_TRUST_INTEGRITY_001#behavior:2": (
        "client_pki_operations_do_not_follow_symbolic_links",
        "client_pki_root_links_are_not_traversed",
    ),
    "RT_OPCUA_CLIENT_CONFIG_TRUST_INTEGRITY_001#behavior:3": (
        "client_pki_promotion_target_link_is_rejected",
        "client_pki_root_links_are_not_traversed",
    ),
    "RT_OPCUA_CLIENT_CONFIG_TRUST_INTEGRITY_001#behavior:4": (
        "one_shot_service_vectors_require_exact_cardinality",
    ),
    "RT_OPCUA_SERVER_SURFACE_001#behavior:4": (
        "opcua_server_cold_starts_before_first_runtime_snapshot",
    ),
    "RT_OPCUA_SERVER_SURFACE_001#behavior:5": (
        "opcua_interop_reads_exposed_scalars_with_reference_client",
    ),
    "RT_OPCUA_SERVER_SURFACE_001#behavior:6": (
        "opcua_security_enforces_user_auth_and_certificate_trust",
    ),
    "RT_MODBUS_WORKER_DEADLINE_POLICY_001#behavior:0": (
        "input_deadline_follows_error_policy_without_fault_copy",
    ),
    "RT_MODBUS_WORKER_DEADLINE_POLICY_001#behavior:2": (
        "output_deadline_follows_error_policy_without_cancelling_handoff",
    ),
    "RT_MODBUS_WORKER_DEADLINE_POLICY_001#behavior:5": (
        "modbus_cold_start_no_data_follows_on_error_policy",
    ),
    "RT_MODBUS_WORKER_DEADLINE_POLICY_001#behavior:7": (
        "modbus_safe_state_handoff_succeeds_when_worker_confirms_delivery",
    ),
    "RT_MODBUS_DECLARED_F32_FINITE_001#behavior:0": (
        "mapped_f32_process_image_encoding_rejects_narrowing_overflow",
        "mapped_f32_wire_encoding_rejects_narrowing_overflow",
        "modbus_inverse_scaled_f32_write_rejects_declared_width_overflow_before_send",
        "modbus_scaled_f32_read_rejects_declared_width_overflow_without_mutation",
    ),
    "RT_MODBUS_DRIVER_001#behavior:7": (
        "modbus_reconnect_backoff_is_bounded_and_non_spinning",
    ),
}
