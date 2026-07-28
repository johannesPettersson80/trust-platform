"""Build the exact Phase 18 ADS behavior-to-test assignment transaction."""

from __future__ import annotations

import json
import tomllib
from collections import defaultdict
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .case_backed_behavior_proof import replace_table_record_fields
from .frozen_catalog_gaps import analyze_frozen_catalog_gaps


CATALOG_PATH = Path("verification/test-catalog.toml")
DENOMINATOR_PATH = Path("verification/test-catalog-denominator.toml")
RESIDUAL_BEHAVIOR_REFS: tuple[str, ...] = ()

# These assignments refine the already accepted ADS coherent-batch authority
# recorded on the Phase 18 board.  Names are resolved only within the invariant
# already linked by the catalog; no lexical or ownership-only match is accepted.
BEHAVIOR_TEST_NAMES: Mapping[str, tuple[str, ...]] = {
    "RT_ADS_ACTIVE_DEVICE_ROUTE_MATCHING_001#behavior:0": (
        "active_device_snapshot_matches_configured_route_without_socket_io",
    ),
    "RT_ADS_ACTIVE_DEVICE_ROUTE_MATCHING_001#behavior:1": (
        "active_device_snapshot_rejects_partial_target_matches",
    ),
    "RT_ADS_ACTIVE_DEVICE_ROUTE_MATCHING_001#behavior:2": (
        "active_device_snapshot_skips_partial_match_and_selects_later_exact_route",
    ),
    "RT_ADS_CLIENT_CONFIGURATION_001#behavior:0": (
        "parses_ads_toml_and_applies_security_and_point_defaults",
    ),
    "RT_ADS_CLIENT_CONFIGURATION_001#behavior:1": (
        "locks_connection_and_declared_global_shape_validation",
        "rejects_noncanonical_target_ams_net_ids_during_configuration_parse",
        "rejects_duplicate_connection_names_with_distinct_point_bindings",
        "rejects_blank_or_noncanonical_explicit_local_ams_net_ids_during_configuration_parse",
    ),
    "RT_ADS_CLIENT_CONFIGURATION_001#behavior:2": (
        "locks_exact_point_address_shape_validation",
        "locks_point_enum_and_type_validation",
        "rejects_zero_string_capacity",
        "rejects_string_capacity_on_non_string_type",
        "rejects_index_address_size_that_disagrees_with_declared_type",
        "rejects_array_metadata_whose_byte_length_overflows",
        "rejects_notification_mode_without_notify_mode",
        "rejects_mixed_symbol_and_index_addressing",
    ),
    "RT_ADS_CLIENT_CONFIGURATION_001#behavior:3": (
        "rejects_plain_transport_without_explicit_ack",
        "plain_transport_ack_surfaces_security_warning",
        "production_local_identity_validation_requires_pinned_local_net_id",
        "production_local_identity_validation_rejects_mismatched_local_net_id",
        "production_local_identity_validation_accepts_matching_local_net_id",
        "rejects_insecure_ack_without_plain_transport",
    ),
    "RT_ADS_CLIENT_TRANSPORT_DATA_001#behavior:0": (
        "persistent_ads_symbol_sets_retain_guardrail_flag",
        "readonly_ads_symbol_does_not_get_write_flag",
    ),
    "RT_ADS_CLIENT_TRANSPORT_DATA_001#behavior:1": (
        "unsupported_compound_symbol_is_not_a_bindable_descriptor",
    ),
    "RT_ADS_CLIENT_TRANSPORT_DATA_001#behavior:2": (
        "subscribe_rejects_poll_mode_before_connection",
    ),
    "RT_ADS_CLIENT_TRANSPORT_LIFECYCLE_001#behavior:0": (
        "connect_accepts_canonical_authority_and_reaches_connector",
    ),
    "RT_ADS_CLIENT_TRANSPORT_LIFECYCLE_001#behavior:1": (
        "connect_rejects_invalid_local_identity_before_connector_or_state_cleanup",
        "connect_rejects_invalid_target_or_zero_port_before_connector_or_state_cleanup",
    ),
    "RT_ADS_CLIENT_TRANSPORT_LIFECYCLE_001#behavior:2": (
        "connect_rejects_auto_add_route_before_network",
        "connect_rejects_secure_transport_before_network",
    ),
    "RT_ADS_CLIENT_WORKER_CACHE_INTEGRITY_001#behavior:1": (
        "handle_resolution_requires_exact_response_bijection_and_cleanup",
        "poll_results_require_exact_order_and_cardinality_before_cache_commit",
        "subscription_and_notification_results_require_active_correlation",
        "write_response_cardinality_and_failure_quality_fail_closed",
    ),
    "RT_ADS_CLIENT_WORKER_CACHE_INTEGRITY_001#behavior:3": (
        "write_ack_does_not_drop_newer_queued_value",
        "write_response_cardinality_and_failure_quality_fail_closed",
        "output_queue_rejects_non_finite_values_before_enqueue",
    ),
    "RT_ADS_CLIENT_WORKER_CACHE_INTEGRITY_001#behavior:4": (
        "output_queue_rejects_non_finite_values_before_enqueue",
    ),
    "RT_ADS_CLIENT_WORKER_CACHE_INTEGRITY_001#behavior:5": (
        "worker_shutdown_and_drop_interrupt_idle_wait",
    ),
    "RT_ADS_CLI_AUTHORING_001#behavior:2": (
        "ads_validate_live_cli_requires_ads_wire_feature_without_reading_files",
        "ads_import_symbols_cli_requires_ads_wire_feature_without_faking_results",
    ),
    "RT_ADS_CLI_FAIL_CLOSED_001#behavior:0": (
        "ads_discover_cli_requires_ads_wire_feature_without_faking_results",
        "ads_browse_cli_requires_ads_wire_feature_without_faking_results",
        "ads_doctor_cli_requires_ads_wire_feature_without_faking_results",
        "ads_validate_live_cli_requires_ads_wire_feature_without_reading_files",
        "ads_import_symbols_cli_requires_ads_wire_feature_without_faking_results",
    ),
    "RT_ADS_CLI_FAIL_CLOSED_001#behavior:1": (
        "ads_server_json_rejection_exits_nonzero_and_preserves_error",
    ),
    "RT_ADS_DIAGNOSTIC_REPORT_TRUTH_001#behavior:0": (
        "production_ready_requires_pass_and_evidence",
        "production_readiness_is_not_ready_without_evidence",
    ),
    "RT_ADS_DIAGNOSTIC_REPORT_TRUTH_001#behavior:1": (
        "doctor_overall_fails_closed_for_empty_steps",
        "doctor_overall_keeps_nonblocking_failure_partial",
    ),
    "RT_ADS_DIAGNOSTIC_REPORT_TRUTH_001#behavior:3": (
        "production_readiness_honors_age_status_hash_and_clock_warning",
        "production_readiness_needs_recheck_on_mismatch_fault_or_expiry",
        "production_readiness_requires_matching_deployed_status",
        "production_ready_requires_pass_and_evidence",
        "server_production_ready_rejects_incomplete_or_client_role_evidence",
        "server_production_ready_requires_independent_client_evidence",
    ),
    "RT_ADS_DIAGNOSTIC_REPORT_TRUTH_001#behavior:5": (
        "diagnostic_wire_enums_keep_the_version_two_contract_names",
        "diagnostic_maps_and_action_builders_serialize_deterministically",
        "committed_golden_fixtures_match_rust_schema",
        "schema_reserves_server_role_without_server_modules",
        "serializes_documented_missing_route_shape_deterministically",
        "v1_client_evidence_json_still_deserializes_with_server_defaults",
    ),
    "RT_ADS_ONBOARDING_DISCOVERY_001#behavior:1": (
        "directed_identify_collects_target_fields_from_wire",
    ),
    "RT_ADS_ONBOARDING_DISCOVERY_001#behavior:2": (
        "discovery_deduplicates_by_ads_endpoint_not_host_ip",
    ),
    "RT_ADS_ONBOARDING_DISCOVERY_001#behavior:3": (
        "discovery_deduplicates_by_ads_endpoint_not_host_ip",
        "directed_broadcast_is_optional_and_deduplicated",
    ),
    "RT_ADS_ONBOARDING_DISCOVERY_001#behavior:4": (
        "broadcast_failure_preserves_successful_manual_identity",
    ),
    "RT_ADS_ONBOARDING_DOCTOR_ORCHESTRATION_001#behavior:0": (
        "required_doctor_steps_and_timeouts_match_spec",
        "doctor_happy_path_runs_required_steps_and_skips_write_probe_by_default",
        "production_ready_requires_pass_evidence_and_live_deployed_status",
    ),
    "RT_ADS_ONBOARDING_DOCTOR_ORCHESTRATION_001#behavior:1": (
        "doctor_rejects_invalid_manual_endpoint_before_wire_activity",
    ),
    "RT_ADS_ONBOARDING_DOCTOR_ORCHESTRATION_001#behavior:2": (
        "doctor_rejects_blank_or_absent_selected_symbol_at_upload_boundary",
    ),
    "RT_ADS_ONBOARDING_DOCTOR_ORCHESTRATION_001#behavior:3": (
        "active_device_read_only_uses_live_status_and_never_opens_wire_connection",
        "active_device_shortcut_requires_the_exact_requested_endpoint",
        "active_device_batch_read_requires_a_timestamped_good_point",
    ),
    "RT_ADS_ONBOARDING_DOCTOR_ORCHESTRATION_001#behavior:4": (
        "active_device_shortcut_requires_the_exact_requested_endpoint",
        "active_device_batch_read_requires_a_timestamped_good_point",
        "full_doctor_against_active_device_requires_explicit_pause",
    ),
    "RT_ADS_ONBOARDING_IDENTITY_001#behavior:3": (
        "network_classification_covers_lan_vpn_tailscale_loopback_public_and_nat",
    ),
    "RT_ADS_ONBOARDING_ROUTE_SECURITY_001#behavior:2": (
        "automatic_route_add_rejects_blank_credentials_before_wire",
    ),
    "RT_ADS_ONBOARDING_ROUTE_SECURITY_001#behavior:3": (
        "route_powershell_encodes_xml_against_here_string_breakout",
    ),
    "RT_ADS_ONBOARDING_WIRE_SAFETY_001#behavior:0": (
        "udp_wire_deduplicates_by_ads_endpoint_not_host_ip",
    ),
    "RT_ADS_ONBOARDING_WIRE_SAFETY_001#behavior:1": (
        "mock_wire_happy_path_covers_required_operations",
        "onboarding_boundary_keeps_raw_ads_types_out_of_public_schema",
    ),
    "RT_ADS_ONBOARDING_WIRE_SAFETY_001#behavior:2": (
        "guarded_probe_matching_readback_defers_normal_restore",
        "guarded_probe_mismatch_reports_restore_failure",
        "guarded_probe_read_error_attempts_and_reports_restore_failure",
        "guarded_probe_write_failure_attempts_and_reports_restore_failure",
    ),
    "RT_ADS_ONBOARDING_WIRE_SAFETY_001#behavior:3": (
        "guarded_probe_mismatch_reports_restore_failure",
        "guarded_probe_read_error_attempts_and_reports_restore_failure",
        "guarded_probe_write_failure_attempts_and_reports_restore_failure",
    ),
    "RT_ADS_ONBOARDING_WIRE_SAFETY_001#behavior:4": (
        "sumup_read_projection_rejects_incomplete_or_untrusted_results",
    ),
    "RT_ADS_SERVER_COMMAND_DISPATCH_001#behavior:0": (
        "policy_rejection_precedes_write_and_records_audit",
    ),
    "RT_ADS_SERVER_COMMAND_DISPATCH_001#behavior:2": (
        "handle_by_name_enforces_handle_limit",
        "handle_by_name_wraps_without_aliasing_live_handles",
    ),
    "RT_ADS_SERVER_COMMAND_DISPATCH_001#behavior:3": (
        "write_enqueues_and_audits_writable_symbol",
        "write_rejects_readonly_symbol_without_runtime_mutation",
        "direct_write_enforces_write_byte_limit",
        "policy_rejection_precedes_write_and_records_audit",
    ),
    "RT_ADS_SERVER_COMMAND_DISPATCH_001#behavior:4": (
        "malformed_command_payloads_fail_without_state_mutation",
        "sumup_readwrite_rejects_truncated_later_item_before_any_write",
        "sumup_write_rejects_truncated_later_item_before_any_write",
        "sumup_write_returns_per_item_results",
    ),
    "RT_ADS_SERVER_DOCTOR_TRUTH_001#behavior:0": (
        "status_uses_live_runtime_symbol_count_not_supplied_snapshot",
        "doctor_rejects_supplied_symbols_when_live_runtime_table_is_empty",
        "notification_requires_complete_requested_payload",
        "server_doctor_requires_twincat_external_client_for_production_ready",
        "server_doctor_self_test_does_not_grant_production_ready",
    ),
    "RT_ADS_SERVER_DOCTOR_TRUTH_001#behavior:2": (
        "zero_symbol_handle_is_rejected",
        "zero_notification_handle_is_rejected",
        "notification_requires_complete_requested_payload",
        "notification_rejects_malformed_declared_tail_after_valid_match",
        "notification_rejects_bytes_outside_declared_stream",
        "notification_rejects_duplicate_matching_handle_samples",
    ),
    "RT_ADS_SERVER_DOCTOR_TRUTH_001#behavior:3": (
        "loopback_response_requires_direction_and_complete_request_identity",
        "loopback_response_accepts_exact_correlated_response",
    ),
    "RT_ADS_SERVER_POLICY_WRITE_AUDIT_001#behavior:0": (
        "client_source_pins_enforce_exact_ip_and_ipv4_ipv6_cidr",
        "unpinned_client_requires_explicit_runtime_opt_in",
    ),
    "RT_ADS_SERVER_POLICY_WRITE_AUDIT_001#behavior:1": (
        "client_source_pins_enforce_exact_ip_and_ipv4_ipv6_cidr",
        "client_policy_requires_ams_net_id_and_source_pin",
        "invalid_writable_glob_rejects_write_port_construction",
    ),
    "RT_ADS_SERVER_POLICY_WRITE_AUDIT_001#behavior:2": (
        "write_port_error_priority_is_security_first_and_mutation_free",
        "write_port_rejects_faulted_runtime_without_mutation",
        "write_port_rejects_policy_failure_without_mutation",
        "write_port_rejects_read_only_gate_without_mutation",
    ),
    "RT_ADS_SERVER_POLICY_WRITE_AUDIT_001#behavior:3": (
        "write_port_accepts_only_ready_running_or_paused_resources",
    ),
    "RT_ADS_SERVER_POLICY_WRITE_AUDIT_001#behavior:4": (
        "audit_sink_records_accepted_and_rejected_ads_write_details",
        "audit_sink_preserves_policy_rejection_context",
        "refused_attempts_coalesce_and_history_is_bounded",
        "client_policy_records_refused_attempts_for_wait_for_client_flow",
    ),
    "RT_ADS_SERVER_PUBLICATION_LIFECYCLE_001#behavior:0": (
        "disabled_lifecycle_avoids_snapshot_and_socket_work",
        "server_runtime_defaults_preserve_bounded_product_limits",
    ),
    "RT_ADS_SERVER_PUBLICATION_LIFECYCLE_001#behavior:2": (
        "publisher_rejects_incompatible_values_and_size_mismatch_without_bytes",
        "runtime_symbols_skip_unsupported_values",
        "publisher_rejects_foreign_or_missing_runtime_globals_without_bytes",
        "empty_expose_publishes_nothing_and_invalid_globs_fail_closed",
    ),
    "RT_ADS_SERVER_PUBLICATION_LIFECYCLE_001#behavior:5": (
        "lifecycle_client_count_shutdown_and_drop_are_observable",
    ),
    "RT_ADS_SYMBOL_IMPORT_APPLY_001#behavior:1": (
        "apply_import_rejects_empty_selection",
    ),
    "RT_ADS_SYMBOL_IMPORT_APPLY_001#behavior:2": (
        "apply_import_rejects_malformed_existing_toml",
        "apply_import_merges_second_connection_and_is_idempotent",
    ),
    "RT_ADS_SYMBOL_IMPORT_APPLY_001#behavior:5": (
        "apply_import_rejects_write_binding_without_acknowledgement",
    ),
    "RT_ADS_SYMBOL_IMPORT_APPLY_001#behavior:6": (
        "import_symbols_apply_acknowledged_write_only_symbol_as_write",
        "import_symbols_apply_acknowledged_writable_symbols_as_read_write",
    ),
    "RT_ADS_SYMBOL_IMPORT_ARTIFACT_SAFETY_001#behavior:0": (
        "import_symbols_apply_writes_exact_selected_symbols_to_project_files",
    ),
    "RT_ADS_SYMBOL_IMPORT_ARTIFACT_SAFETY_001#behavior:1": (
        "import_symbols_apply_rejects_absolute_output_path",
        "import_symbols_apply_rejects_parent_path_traversal",
        "import_symbols_apply_rejects_connection_name_as_snapshot_path",
    ),
    "RT_ADS_SYMBOL_IMPORT_ARTIFACT_SAFETY_001#behavior:2": (
        "import_symbols_apply_rejects_undiscoverable_snapshot_path",
        "import_symbols_apply_rejects_connection_name_as_snapshot_path",
    ),
    "RT_ADS_SYMBOL_IMPORT_ARTIFACT_SAFETY_001#behavior:3": (
        "import_symbols_apply_rejects_symlink_path_escape",
    ),
    "RT_ADS_SYMBOL_IMPORT_ARTIFACT_SAFETY_001#behavior:4": (
        "import_symbols_apply_rejects_colliding_artifact_paths",
    ),
    "RT_ADS_SYMBOL_IMPORT_ARTIFACT_SAFETY_001#behavior:5": (
        "import_symbols_apply_accepts_identical_generated_source",
        "import_symbols_apply_updates_valid_current_generated_source",
    ),
    "RT_ADS_SYMBOL_IMPORT_ARTIFACT_SAFETY_001#behavior:6": (
        "import_symbols_apply_preserves_different_generated_source",
    ),
    "RT_ADS_SYMBOL_IMPORT_SELECTION_001#behavior:0": (
        "build_import_response_honors_explicit_symbol_list",
    ),
    "RT_ADS_SYMBOL_IMPORT_SELECTION_001#behavior:1": (
        "build_import_response_glob_backtracks_to_anchored_suffix",
    ),
    "RT_ADS_SYMBOL_IMPORT_SELECTION_001#behavior:2": (
        "build_import_response_empty_pattern_selects_nothing",
    ),
    "RT_ADS_SYMBOL_IMPORT_SELECTION_001#behavior:3": (
        "build_import_response_prefixes_all_reserved_st_keywords",
    ),
}


def build_assignment_plan(*, root: Path, gap_report: Path) -> dict[str, Any]:
    """Resolve and validate the frozen ADS assignment without writing files."""

    report = _json(gap_report)
    demands = sorted(
        (
            dict(row)
            for row in _records(
                report.get("confirmed_missing_tests"),
                "confirmed missing tests",
            )
            if "ADS_" in _string(row, "invariant_id", "behavior demand")
        ),
        key=lambda row: _string(row, "behavior_ref", "behavior demand"),
    )
    demand_refs = {
        _string(row, "behavior_ref", "behavior demand") for row in demands
    }
    assigned_refs = set(BEHAVIOR_TEST_NAMES)
    residual_refs = set(RESIDUAL_BEHAVIOR_REFS)
    if assigned_refs & residual_refs:
        raise ValueError("ADS assigned and residual behaviors overlap")
    if assigned_refs | residual_refs != demand_refs:
        missing = sorted(demand_refs - assigned_refs - residual_refs)
        extra = sorted(assigned_refs | residual_refs - demand_refs)
        raise ValueError(
            f"ADS frozen behavior classification drifted: "
            f"missing={missing!r}, extra={extra!r}"
        )

    catalog_path = root / CATALOG_PATH
    catalog_source = catalog_path.read_text()
    catalog = _toml_source(catalog_source, CATALOG_PATH.as_posix())
    catalog_tests = _records(catalog.get("tests"), "catalog tests")
    by_invariant_and_name: dict[tuple[str, str], list[dict[str, Any]]] = (
        defaultdict(list)
    )
    for raw_test in catalog_tests:
        test = dict(raw_test)
        name = test.get("name")
        if not isinstance(name, str) or not name:
            continue
        for invariant_id in _strings(
            test.get("invariants", []),
            f"{test.get('id')} invariants",
        ):
            by_invariant_and_name[
                (invariant_id, name)
            ].append(test)

    assignments = []
    refs_by_test: dict[str, set[str]] = defaultdict(set)
    test_by_id: dict[str, dict[str, Any]] = {}
    for behavior_ref, names in sorted(BEHAVIOR_TEST_NAMES.items()):
        invariant_id = behavior_ref.split("#behavior:", 1)[0]
        resolved = []
        for name in names:
            candidates = by_invariant_and_name.get((invariant_id, name), [])
            if len(candidates) != 1:
                raise ValueError(
                    f"{behavior_ref} expected one catalog test named "
                    f"{name!r}, found {len(candidates)}"
                )
            test = candidates[0]
            test_id = _string(test, "id", name)
            if test.get("status") != "mapped":
                raise ValueError(f"{test_id} is not mapped")
            if invariant_id not in _strings(
                test.get("invariants", []),
                f"{test_id} invariants",
            ):
                raise ValueError(
                    f"{test_id} is not linked to {invariant_id}"
                )
            if test_id in test_by_id and test_by_id[test_id] != test:
                raise ValueError(f"catalog test {test_id} is not unique")
            test_by_id[test_id] = test
            refs_by_test[test_id].add(behavior_ref)
            resolved.append(test)
        assignments.append(
            {
                "behavior_ref": behavior_ref,
                "invariant_id": invariant_id,
                "test_ids": sorted(
                    _string(test, "id", behavior_ref) for test in resolved
                ),
                "tests": sorted(
                    resolved,
                    key=lambda test: _string(test, "id", behavior_ref),
                ),
            }
        )

    updates = {}
    for test_id, new_refs in sorted(refs_by_test.items()):
        test = test_by_id[test_id]
        current_refs = set(
            _strings(
                test.get("mapping_behavior_refs", []),
                f"{test_id} mapping behavior refs",
            )
        )
        updates[test_id] = {
            "mapping_behavior_refs": sorted(current_refs | new_refs)
        }
    replacement_source = replace_table_record_fields(
        catalog_source,
        table="tests",
        updates=updates,
    )
    replacement_catalog = _toml_source(
        replacement_source,
        "replacement catalog",
    )
    invariants = _load_invariants(root)
    analysis = analyze_frozen_catalog_gaps(
        denominator=_toml(root / DENOMINATOR_PATH),
        catalog=replacement_catalog,
        invariants=invariants,
    )
    if analysis["summary"]["confirmed_missing_tests"] != 196:
        raise ValueError(
            "ADS assignment did not close exactly 71 frozen demands: "
            f"{analysis['summary']['confirmed_missing_tests']}"
        )
    if analysis["summary"]["missing_spec_authority_tests"] != 0:
        raise ValueError("ADS assignment reopened missing spec authority")

    paths = sorted(
        {
            _string(test, "path", _string(test, "id", "catalog test"))
            for test in test_by_id.values()
        }
    )
    return {
        "demand_count": len(demands),
        "assigned_behavior_count": len(assignments),
        "assigned_test_count": len(test_by_id),
        "assigned_path_count": len(paths),
        "assigned_paths": paths,
        "assignments": assignments,
        "residual_behavior_refs": list(RESIDUAL_BEHAVIOR_REFS),
        "catalog_updates": updates,
        "post_assignment_gap_summary": analysis["summary"],
        "replacements": {
            CATALOG_PATH.as_posix(): replacement_source,
        },
    }


def _load_invariants(
    root: Path,
) -> list[tuple[str, Mapping[str, Any]]]:
    result = []
    seen = set()
    for path in sorted((root / "verification/invariants").rglob("*.toml")):
        document = _toml(path)
        invariant_id = _string(document, "id", path.as_posix())
        if invariant_id in seen:
            raise ValueError(f"duplicate invariant {invariant_id}")
        seen.add(invariant_id)
        result.append((path.relative_to(root).as_posix(), document))
    return result


def _json(path: Path) -> dict[str, Any]:
    document = json.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} is not a JSON object")
    return document


def _toml(path: Path) -> dict[str, Any]:
    return _toml_source(path.read_text(), path.as_posix())


def _toml_source(source: str, label: str) -> dict[str, Any]:
    document = tomllib.loads(source)
    if not isinstance(document, dict):
        raise ValueError(f"{label} is not a TOML document")
    return document


def _records(value: Any, label: str) -> list[Mapping[str, Any]]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be a list")
    if any(not isinstance(row, Mapping) for row in value):
        raise ValueError(f"{label} contains a non-object")
    return list(value)


def _strings(value: Any, label: str) -> list[str]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be a string list")
    if any(not isinstance(item, str) or not item for item in value):
        raise ValueError(f"{label} contains an invalid string")
    return list(value)


def _string(row: Mapping[str, Any], key: str, label: str) -> str:
    value = row.get(key)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} lacks {key}")
    return value
