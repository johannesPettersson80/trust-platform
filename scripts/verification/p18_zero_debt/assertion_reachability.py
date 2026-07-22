"""Dynamic reachability and assertion-strength validator boundary."""

from __future__ import annotations

import json
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from .proof_common import validate_proof_row
from ..test_catalog_json_schema import validate_json_schema_instance


SCHEMA_PATH = Path("verification/schemas/p18-assertion-reachability.schema.json")


def validate(
    row: Mapping[str, Any],
    budget: Mapping[str, Any],
    artifact: Mapping[str, Any] | None,
) -> list[str]:
    failures = validate_proof_row(row, budget, "assertion_reachability")
    if artifact is None:
        return [*failures, "assertion_reachability raw artifact is unavailable"]
    root = Path(__file__).resolve().parents[3]
    try:
        schema = json.loads((root / SCHEMA_PATH).read_text())
        failures.extend(validate_json_schema_instance(dict(artifact), schema))
    except (OSError, json.JSONDecodeError) as error:
        failures.append(
            f"assertion_reachability schema validation failed safely: {type(error).__name__}"
        )
        return sorted(set(failures))
    failures.extend(validate_artifact(artifact, row))
    return sorted(set(failures))


def validate_artifact(
    artifact: Mapping[str, Any], row: Mapping[str, Any] | None = None
) -> list[str]:
    failures: list[str] = []
    input_paths = artifact.get("input_paths")
    if not isinstance(input_paths, list) or input_paths != sorted(input_paths):
        failures.append("assertion_reachability input_paths are not canonically ordered")
    elif any(
        not isinstance(value, str)
        or Path(value).is_absolute()
        or ".." in Path(value).parts
        or "\\" in value
        for value in input_paths
    ):
        failures.append("assertion_reachability input_paths contain an unsafe path")
    records_value = artifact.get("records")
    if not isinstance(records_value, list) or not all(
        isinstance(record, Mapping) for record in records_value
    ):
        return ["assertion_reachability records must be an object array"]
    records = list(records_value)
    identities = [str(record.get("discovery_id", "")) for record in records]
    if identities != sorted(identities):
        failures.append("assertion_reachability records are not canonically ordered")
    if len(set(identities)) != len(identities):
        failures.append("assertion_reachability contains duplicate discovery identities")
    test_ids = [str(record.get("test_id", "")) for record in records]
    if len(set(test_ids)) != len(test_ids):
        failures.append("assertion_reachability contains duplicate test identities")

    proven = 0
    executed = 0
    mutation_counts = {
        "caught": 0,
        "survived": 0,
        "unviable": 0,
        "timeout": 0,
        "harness_failure": 0,
    }
    for record in records:
        test_id = record.get("test_id")
        baseline = record.get("baseline")
        mutations = record.get("mutations")
        blockers = record.get("blockers")
        if not isinstance(baseline, Mapping) or not isinstance(mutations, list):
            continue
        if baseline.get("result") == "passed":
            executed += 1
        for mutation in mutations:
            if not isinstance(mutation, Mapping):
                continue
            result = mutation.get("result")
            if isinstance(result, str) and result in mutation_counts:
                mutation_counts[result] += 1
            if result == "caught" and (
                mutation.get("build_exit_status") != 0
                or mutation.get("build_timed_out") is not False
                or mutation.get("selected_test_failed") is not True
                or mutation.get("test_timed_out") is not False
                or not isinstance(mutation.get("test_exit_status"), int)
                or mutation.get("test_exit_status") == 0
            ):
                failures.append(
                    f"assertion_reachability {test_id} caught mutant lacks selected-test failure"
                )
            if result != "caught" and mutation.get("selected_test_failed") is True:
                failures.append(
                    f"assertion_reachability {test_id} non-caught mutant claims selected-test failure"
                )
        if record.get("status") == "proven":
            proven += 1
            if baseline.get("result") != "passed" or baseline.get("exit_status") != 0:
                failures.append(
                    f"assertion_reachability {test_id} proven record lacks a passing baseline"
                )
            if not record.get("code_ids") or not record.get("invariant_ids"):
                failures.append(
                    f"assertion_reachability {test_id} proven record lacks exact bindings"
                )
            if blockers != []:
                failures.append(
                    f"assertion_reachability {test_id} proven record retains blockers"
                )
            if not mutations or any(
                not isinstance(mutation, Mapping)
                or mutation.get("result") != "caught"
                for mutation in mutations
            ):
                failures.append(
                    f"assertion_reachability {test_id} proven record lacks an all-caught mutation set"
                )
        elif not blockers:
            failures.append(
                f"assertion_reachability {test_id} open record must preserve blockers"
            )

    summary = artifact.get("summary")
    expected_summary = {
        "denominator": len(records),
        "proven": proven,
        "debt": len(records) - proven,
        "executed_tests": executed,
        "caught_mutants": mutation_counts["caught"],
        "survived_mutants": mutation_counts["survived"],
        "unviable_mutants": mutation_counts["unviable"],
        "timeouts": mutation_counts["timeout"],
        "harness_failures": mutation_counts["harness_failure"],
    }
    if summary != expected_summary:
        failures.append("assertion_reachability summary does not reconstruct from records")
    expected_status = (
        "open"
        if expected_summary["denominator"] == 0
        else "closed" if expected_summary["debt"] == 0 else "open"
    )
    if artifact.get("status") != expected_status:
        failures.append(f"assertion_reachability artifact status must be {expected_status}")
    if row is not None:
        for field, expected in (
            ("denominator", expected_summary["denominator"]),
            ("numerator", expected_summary["proven"]),
            ("debt", expected_summary["debt"]),
            ("timeouts", expected_summary["timeouts"]),
            ("harness_failures", expected_summary["harness_failures"]),
            ("execution_count", expected_summary["executed_tests"]),
        ):
            if row.get(field) != expected:
                failures.append(
                    f"assertion_reachability ledger {field} must reconstruct as {expected}"
                )
        if row.get("input_digest") != artifact.get("input_digest"):
            failures.append("assertion_reachability ledger input_digest is stale")
        if row.get("status") != artifact.get("status"):
            failures.append("assertion_reachability ledger status differs from artifact")
    return sorted(set(failures))
