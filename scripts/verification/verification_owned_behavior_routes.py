"""Reviewed exact-test routes for the Phase 18 verification-owned tranche."""

from __future__ import annotations

from collections.abc import Mapping


BEHAVIOR_TEST_NAMES: Mapping[str, tuple[str, ...]] = {
    "VERIFICATION_CASE_ARTIFACT_HELPER_001#behavior:2": (
        "blocked_cases_are_recorded_without_executing_the_runner",
    ),
    "VERIFICATION_CASE_ARTIFACT_HELPER_001#behavior:3": (
        "runnable_cases_capture_snapshots_and_state_delta",
    ),
    "VERIFICATION_CASE_ARTIFACT_HELPER_001#behavior:4": (
        "expected_case_file_digest_is_enforced_before_execution",
        "schema_version_mismatch_is_rejected_before_execution",
    ),
    "VERIFICATION_CASE_ARTIFACT_HELPER_001#behavior:5": (
        "trust_verify_env_stamps_are_recorded_in_artifact",
    ),
    "VERIFICATION_CASE_ARTIFACT_HELPER_001#behavior:6": (
        "partial_trust_verify_env_stamps_fail_before_execution",
    ),
    "VERIFICATION_CASE_ARTIFACT_HELPER_001#behavior:7": (
        "mismatched_trust_verify_test_id_fails_before_execution",
        "mismatched_trust_verify_case_file_digest_fails_before_execution",
        "mismatched_trust_verify_artifact_dir_fails_before_execution",
    ),
    "VERIFICATION_IEC_TABLE_INVENTORY_001#behavior:0": (
        "iec_table_coverage_report",
    ),
    "VERIFICATION_IEC_TABLE_INVENTORY_001#behavior:1": (
        "malformed_iec_table_map_is_rejected",
    ),
    "VERIFICATION_IEC_TABLE_INVENTORY_001#behavior:2": (
        "iec_table_coverage_report",
    ),
    "VERIFICATION_IEC_TABLE_INVENTORY_001#behavior:3": (
        "iec_table_coverage_report",
    ),
    "VERIFICATION_IEC_TABLE_INVENTORY_001#behavior:4": (
        "missing_iec_table_test_name_is_reported_and_rejected",
    ),
}

PROVENANCE_REFRESH_TEST_NAMES: Mapping[str, tuple[str, ...]] = {
    "VERIFICATION_CASE_ARTIFACT_HELPER_001#behavior:0": (
        "workspace_absolute_case_path_is_recorded_as_committed_relative_identity",
    ),
    "VERIFICATION_CASE_ARTIFACT_HELPER_001#behavior:1": (
        "default_artifact_dir_is_workspace_target_gate_artifacts",
        "stamped_artifact_dir_overrides_compile_time_workspace_path",
    ),
}
