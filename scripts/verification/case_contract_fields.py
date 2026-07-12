"""Closed field vocabularies shared by case metadata and proof consumers."""

from __future__ import annotations


CASE_FILE_REQUIRED_ROOT_FIELDS = frozenset(
    {
        "schema_version",
        "id",
        "title",
        "area",
        "owner",
        "status",
        "invariant",
        "source_digest",
        "last_reviewed",
        "case",
    }
)
CASE_FILE_ROOT_FIELDS = CASE_FILE_REQUIRED_ROOT_FIELDS | {
    "case_provenance_kind",
    "generator",
    "generator_digest",
    "trace_definition_digest",
}
CASE_FILE_CASE_FIELDS = frozenset(
    {"id", "family", "input", "state", "spec_gap_ref", "expect", "trace"}
)
GENERATED_BLOCKED_CASE_FIELDS = frozenset(
    {"id", "family", "input", "state", "spec_gap_ref"}
)
GENERATED_RUNNABLE_CASE_FIELDS = frozenset({"id", "family", "input", "expect"})
HAND_AUTHORED_BLOCKED_CASE_FIELDS = GENERATED_BLOCKED_CASE_FIELDS
HAND_AUTHORED_RUNNABLE_CASE_FIELDS = GENERATED_RUNNABLE_CASE_FIELDS | {"trace"}
TRACE_STEP_FIELDS = frozenset({"sequence", "stimulus", "expected", "oracle_ref"})

CASE_ARTIFACT_HELPER_VERSION = "verification-cases v1"
CASE_ARTIFACT_ROOT_FIELDS = frozenset(
    {
        "schema_version",
        "test_id",
        "case_file",
        "case_file_digest",
        "helper_version",
        "case_provenance_kind",
        "trace_definition_digest",
        "trust_verify_test_id",
        "trust_verify_run_id",
        "trust_verify_case_file_digest",
        "trust_verify_artifact_dir",
        "cases",
    }
)
CASE_ARTIFACT_CASE_FIELDS = frozenset(
    {
        "id",
        "family",
        "result",
        "spec_gap_ref",
        "observed_error",
        "observed_status",
        "state_delta",
        "before",
        "after",
    }
)
CASE_ARTIFACT_SNAPSHOT_FIELDS = frozenset(
    {"process_image_hash", "retain_hash", "target", "siblings", "diagnostics"}
)
