"""Hand-owned intent and subject rules for committed test-catalog records."""

from __future__ import annotations

import re
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from .malformed_input_contract import (
    load_malformed_input_taxonomy,
    validate_catalog_malformed_bindings,
    validate_malformed_input_contract,
)


DISCOVERY_ID_RE = re.compile(r"^DISC_[A-F0-9]{20}$")
SUBJECT_KINDS = {
    "generated_test",
    "case_table_artifact",
    "mutation_shard_runner",
}
GENERATED_SOURCE_KINDS = {
    "rust_integration_test",
    "rust_unit_test",
    "structured_text_test",
    "vscode_test",
    "conformance_case",
    "fuzz_target",
    "gate_script",
    "github_workflow_job",
}
DISCOVERY_FIELDS = {"discovery_id", "discovery_source_kind", "name"}
NON_MAPPING_STATUSES = {"planned", "gap_open"}
ROOT = Path(__file__).resolve().parents[2]


def validate_catalog_intent(
    *,
    tests: Mapping[str, Mapping[str, Any]],
    matrix: Mapping[str, Any],
    invariants: Mapping[str, Mapping[str, Any]],
    spec_sources: Mapping[str, Mapping[str, Any]],
    spec_gaps: Mapping[str, Mapping[str, Any]],
) -> list[str]:
    """Validate review-owned fields without consulting generated scanner facts."""

    failures: list[str] = []
    mapped_areas = {
        area.get("id")
        for area in matrix.get("areas", [])
        if isinstance(area, Mapping) and area.get("status") == "mapped"
    }
    discoveries: dict[str, str] = {}

    for record_id in sorted(tests):
        record = tests[record_id]
        subject_kind = record.get("subject_kind")
        if subject_kind not in SUBJECT_KINDS:
            failures.append(f"{record_id} has unknown subject_kind {subject_kind!r}")
            continue

        for field in ("expected_result", "expected_failure_mode", "evidence_destination"):
            value = record.get(field)
            if not isinstance(value, str) or not value.strip():
                failures.append(f"{record_id} requires non-empty {field}")

        area = record.get("area")
        if record.get("status") not in NON_MAPPING_STATUSES and area not in mapped_areas:
            failures.append(f"{record_id} maps an uninventoried area {area}")

        for invariant_id in record.get("invariants", []):
            _check_reference_area(
                failures,
                record_id,
                area,
                "invariant",
                invariant_id,
                invariants,
            )
        for field, label, records in (
            ("oracle_ref", "oracle_ref", spec_sources),
            ("spec_gap_ref", "spec_gap_ref", spec_gaps),
        ):
            reference = record.get(field)
            if isinstance(reference, str):
                _check_reference_area(failures, record_id, area, label, reference, records)

        if subject_kind == "generated_test":
            _validate_generated_subject(record_id, record, discoveries, failures)
        elif subject_kind == "case_table_artifact":
            _validate_case_table_subject(record_id, record, failures)
        else:
            _validate_mutation_subject(record_id, record, failures)

    try:
        malformed_taxonomy = load_malformed_input_taxonomy(ROOT)
    except Exception as exc:
        failures.append(f"malformed-input taxonomy cannot be read: {exc}")
    else:
        failures.extend(validate_malformed_input_contract(ROOT, malformed_taxonomy))
        failures.extend(
            validate_catalog_malformed_bindings(
                tests=tests,
                taxonomy=malformed_taxonomy,
            )
        )

    return failures


def _check_reference_area(
    failures: list[str],
    record_id: str,
    area: Any,
    label: str,
    reference: Any,
    records: Mapping[str, Mapping[str, Any]],
) -> None:
    target = records.get(reference)
    if target is not None and target.get("area") != area:
        failures.append(
            f"{record_id} {label} {reference} area {target.get('area')} does not match {area}"
        )


def _validate_generated_subject(
    record_id: str,
    record: Mapping[str, Any],
    discoveries: dict[str, str],
    failures: list[str],
) -> None:
    for field in ("discovery_id", "discovery_source_kind", "name"):
        value = record.get(field)
        if not isinstance(value, str) or not value:
            failures.append(f"{record_id} generated_test requires {field}")
    discovery_id = record.get("discovery_id")
    if isinstance(discovery_id, str):
        if not DISCOVERY_ID_RE.fullmatch(discovery_id):
            failures.append(f"{record_id} has invalid discovery_id {discovery_id!r}")
        previous = discoveries.setdefault(discovery_id, record_id)
        if previous != record_id:
            failures.append(
                f"duplicate discovery_id {discovery_id} used by {previous} and {record_id}"
            )
    source_kind = record.get("discovery_source_kind")
    if source_kind not in GENERATED_SOURCE_KINDS:
        failures.append(f"{record_id} has unknown discovery_source_kind {source_kind!r}")


def _validate_case_table_subject(
    record_id: str,
    record: Mapping[str, Any],
    failures: list[str],
) -> None:
    _forbid_discovery_fields(record_id, record, "case_table_artifact", failures)
    case_file = record.get("case_file")
    if record.get("test_class") != "metadata_validation":
        failures.append(f"{record_id} case_table_artifact must use metadata_validation")
    if not isinstance(case_file, str) or not case_file.startswith("verification/cases/"):
        failures.append(f"{record_id} case_table_artifact requires a verification/cases case_file")
    if record.get("path") != case_file:
        failures.append(f"{record_id} case_table_artifact path must equal case_file")
    if "case_file_digest" not in record:
        failures.append(f"{record_id} case_table_artifact requires case_file_digest")


def _validate_mutation_subject(
    record_id: str,
    record: Mapping[str, Any],
    failures: list[str],
) -> None:
    _forbid_discovery_fields(record_id, record, "mutation_shard_runner", failures)
    if record.get("test_class") != "mutation":
        failures.append(f"{record_id} mutation_shard_runner must use mutation test_class")
    if record.get("path") != "scripts/bytecode_validator_mutation.py":
        failures.append(
            f"{record_id} mutation_shard_runner path is outside the reviewed validator shard"
        )
    if not isinstance(record.get("mutation_shard_id"), str):
        failures.append(f"{record_id} mutation_shard_runner requires mutation_shard_id")
    mutations = record.get("mutations")
    if not isinstance(mutations, list) or not mutations:
        failures.append(f"{record_id} mutation_shard_runner requires mutations")


def _forbid_discovery_fields(
    record_id: str,
    record: Mapping[str, Any],
    subject_kind: str,
    failures: list[str],
) -> None:
    for field in sorted(DISCOVERY_FIELDS & set(record)):
        failures.append(f"{record_id} {subject_kind} forbids {field}")
