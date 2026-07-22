"""Closed contract joins for the Phase 18 substrate."""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Callable, Mapping, Sequence
from pathlib import Path
from typing import Any


Validator = Callable[
    [Mapping[str, Any], Mapping[str, Any], Mapping[str, Any] | None],
    list[str],
]
SHA256_RE = re.compile(r"^sha256:[a-f0-9]{64}$")
SOURCE_SHA_RE = re.compile(r"^[a-f0-9]{40}$")
DATE_RE = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}$")
LEDGER_FIELDS = {
    "schema_version", "id", "source_sha", "input_closure_sha256", "state",
    "last_reviewed", "baseline", "current", "proof_classes",
}
BASELINE_FIELDS = {"code_facts", "spec_gap", "test_facts", "reviewed_nonmapping"}
CURRENT_FIELDS = {
    "spec_gap", "registered_backlog", "missing_spec", "missing_tests", "unmapped",
    "stale", "reviewed_nonmapping", "uncataloged", "unreviewed_tests",
    "ignored_skipped_filtered_unrun", "zero_test_filters", "unexplained_uncovered",
    "viable_mutants", "open_findings", "unmeasured_proof_classes",
}
PROOF_FIELDS = {
    "id", "validator_module", "prerequisites", "status", "denominator", "numerator",
    "debt", "stale", "unowned", "skipped", "filtered", "unrun", "timeouts",
    "harness_failures", "execution_count", "raw_artifact", "raw_sha256",
    "producer_command", "producer_exit", "validator_command", "validator_exit",
    "input_digest",
}
BUDGET_ROOT_FIELDS = {
    "schema_version", "id", "source_sha", "input_closure_sha256", "state",
    "last_reviewed", "budgets",
}
BUDGET_FIELDS = {
    "id", "proof_class", "owner", "review_status", "tool", "tool_version",
    "environment", "topology", "seed", "input_digest", "shards",
    "baseline_iterations", "current_iterations", "baseline_duration_seconds",
    "current_duration_seconds", "baseline_obligation", "current_obligation",
    "last_reviewed",
}
CLAIM_ROOT_FIELDS = {
    "schema_version", "id", "source_sha", "input_closure_sha256", "state",
    "last_reviewed", "claims",
}
CLAIM_FIELDS = {
    "id", "source_ref", "owner", "status", "operating_systems", "architectures",
    "features", "profiles", "packaging_obligations", "locale_path_encoding",
    "evidence_requirements", "evidence_refs", "retirement_decision",
}
PROOF_ORDER = (
    "semantic_chains", "test_mapping", "assertion_reachability", "coverage",
    "mutation", "fuzz_property", "concurrency_time", "unsafe_ffi",
    "hardware_endurance", "rendered_ux", "platform_claims", "release_evidence",
)


def document_closure_digest(document: Mapping[str, Any]) -> str:
    """Hash canonical document content with its self-referential field normalized."""

    normalized = dict(document)
    normalized["input_closure_sha256"] = "sha256:" + "0" * 64
    payload = (json.dumps(normalized, sort_keys=True, separators=(",", ":")) + "\n").encode()
    return "sha256:" + hashlib.sha256(payload).hexdigest()


def validate_bundle(
    ledger: Mapping[str, Any],
    budgets: Mapping[str, Any],
    claims: Mapping[str, Any],
    *,
    artifact_root: Path,
    expected_source_sha: str,
    validators: Mapping[str, object] | None = None,
) -> list[str]:
    if validators is None:
        from .orchestrator import PROOF_VALIDATORS

        validators = PROOF_VALIDATORS
    failures: list[str] = []
    _closed_fields(ledger, LEDGER_FIELDS, "ledger", failures)
    _closed_fields(budgets, BUDGET_ROOT_FIELDS, "budgets", failures)
    _closed_fields(claims, CLAIM_ROOT_FIELDS, "claims", failures)
    for label, document in (("ledger", ledger), ("budgets", budgets), ("claims", claims)):
        if document.get("schema_version") != 1:
            failures.append(f"{label} schema_version must equal 1")
        if document.get("source_sha") != expected_source_sha:
            failures.append(f"{label} source_sha is stale")
        if not SHA256_RE.fullmatch(str(document.get("input_closure_sha256", ""))):
            failures.append(f"{label} input_closure_sha256 is invalid")
        elif document.get("input_closure_sha256") != document_closure_digest(document):
            failures.append(f"{label} input_closure_sha256 is stale")
        if not DATE_RE.fullmatch(str(document.get("last_reviewed", ""))):
            failures.append(f"{label} last_reviewed is invalid")
    if not SOURCE_SHA_RE.fullmatch(expected_source_sha):
        failures.append("expected source SHA is invalid")

    baseline = ledger.get("baseline")
    if not isinstance(baseline, Mapping):
        failures.append("ledger baseline must be a table")
    else:
        _closed_fields(baseline, BASELINE_FIELDS, "ledger baseline", failures)
        expected_baseline = {
            "code_facts": 15229,
            "spec_gap": 15138,
            "test_facts": 4038,
            "reviewed_nonmapping": 3777,
        }
        if dict(baseline) != expected_baseline:
            failures.append("ledger Phase 17 baseline arithmetic drifted")
    current = ledger.get("current")
    if not isinstance(current, Mapping):
        failures.append("ledger current must be a table")
    else:
        _closed_fields(current, CURRENT_FIELDS, "ledger current", failures)
        for field in CURRENT_FIELDS:
            _nonnegative_integer(current.get(field), f"ledger current {field}", failures)

    proof_rows = _mapping_rows(ledger.get("proof_classes"), "proof_classes", failures)
    budget_rows = _mapping_rows(budgets.get("budgets"), "budgets", failures)
    claim_rows = _mapping_rows(claims.get("claims"), "claims", failures)
    proof_by_id = _unique_rows(
        proof_rows, "proof class", failures, expected_order=PROOF_ORDER
    )
    budget_by_class: dict[str, Mapping[str, Any]] = {}
    for index, row in enumerate(budget_rows):
        _closed_fields(row, BUDGET_FIELDS, f"budget {index}", failures)
        proof_class = row.get("proof_class")
        if not isinstance(proof_class, str) or not proof_class:
            failures.append(f"budget {index} requires proof_class")
            continue
        if proof_class in budget_by_class:
            failures.append(f"duplicate budget proof_class {proof_class}")
        budget_by_class[proof_class] = row
        _validate_budget(row, failures)
    _unique_rows(budget_rows, "budget", failures, check_order=False)
    if [row.get("proof_class") for row in budget_rows] != list(PROOF_ORDER):
        failures.append("budget rows are not in canonical proof-class order")
    _unique_rows(claim_rows, "claim", failures)
    for index, row in enumerate(claim_rows):
        _closed_fields(row, CLAIM_FIELDS, f"claim {index}", failures)
        _validate_claim(row, failures)

    expected_classes = set(validators)
    actual_classes = set(proof_by_id)
    for missing in sorted(expected_classes - actual_classes):
        failures.append(f"missing proof class {missing}")
    for unknown in sorted(actual_classes - expected_classes):
        failures.append(f"proof class {unknown} has no enabled validator")
    for proof_id in sorted(actual_classes):
        row = proof_by_id[proof_id]
        _closed_fields(row, PROOF_FIELDS, f"proof class {proof_id}", failures)
        prerequisites = row.get("prerequisites")
        if not isinstance(prerequisites, list) or any(not isinstance(item, str) for item in prerequisites):
            failures.append(f"proof class {proof_id} prerequisites must be strings")
        else:
            for prerequisite in prerequisites:
                if prerequisite not in actual_classes:
                    failures.append(f"proof class {proof_id} has missing prerequisite {prerequisite}")
        budget = budget_by_class.get(proof_id)
        if budget is None:
            failures.append(f"proof class {proof_id} lacks a budget")
        validator = validators.get(proof_id)
        raw_artifact = _load_artifact(row, artifact_root, failures)
        if not callable(validator):
            failures.append(f"proof class {proof_id} validator is disabled")
        elif budget is not None:
            actual_module = getattr(validator, "__module__", None)
            if isinstance(actual_module, str) and actual_module.startswith("verification."):
                actual_module = "scripts." + actual_module
            if row.get("validator_module") != actual_module:
                failures.append(f"proof class {proof_id} validator_module is stale")
            failures.extend(validator(row, budget, raw_artifact))
        if row.get("status") == "closed" and isinstance(prerequisites, list):
            for prerequisite in prerequisites:
                prerequisite_row = proof_by_id.get(prerequisite)
                if prerequisite_row is not None and prerequisite_row.get("status") != "closed":
                    failures.append(
                        f"closed {proof_id} has open prerequisite {prerequisite}"
                    )
    if set(budget_by_class) != actual_classes:
        failures.append("budget/proof-class partition is not exact")
    if ledger.get("state") == "closed" and isinstance(current, Mapping):
        if any(isinstance(value, int) and value != 0 for value in current.values()):
            failures.append("closed ledger contains open zero-debt counters")
    if budgets.get("state") == "frozen" and any(
        row.get("review_status") != "accepted" for row in budget_rows
    ):
        failures.append("frozen budgets contain an unaccepted row")
    if claims.get("state") == "frozen" and not claim_rows:
        failures.append("frozen claim matrix is empty")
    return sorted(set(failures))


def _closed_fields(
    record: Mapping[str, Any], expected: set[str], label: str, failures: list[str]
) -> None:
    if not isinstance(record, Mapping):
        failures.append(f"{label} must be a table")
        return
    missing = sorted(expected - set(record))
    unknown = sorted(set(record) - expected)
    if missing:
        failures.append(f"{label} missing fields: {', '.join(missing)}")
    if unknown:
        failures.append(f"{label} unknown fields: {', '.join(unknown)}")


def _mapping_rows(value: Any, label: str, failures: list[str]) -> list[Mapping[str, Any]]:
    if not isinstance(value, list):
        failures.append(f"{label} must be an array")
        return []
    if any(not isinstance(row, Mapping) for row in value):
        failures.append(f"{label} rows must be tables")
        return []
    return list(value)


def _unique_rows(
    rows: Sequence[Mapping[str, Any]],
    label: str,
    failures: list[str],
    *,
    expected_order: Sequence[str] | None = None,
    check_order: bool = True,
) -> dict[str, Mapping[str, Any]]:
    result: dict[str, Mapping[str, Any]] = {}
    identities: list[str] = []
    for index, row in enumerate(rows):
        identity = row.get("id")
        if not isinstance(identity, str) or not identity:
            failures.append(f"{label} row {index} requires id")
            continue
        if identity in result:
            failures.append(f"duplicate {label} identity {identity}")
        result[identity] = row
        identities.append(identity)
    if expected_order is not None:
        if identities != list(expected_order):
            failures.append(f"{label} rows are not in the reviewed canonical order")
    elif check_order and identities != sorted(identities):
        failures.append(f"{label} rows are not canonically ordered")
    return result


def _nonnegative_integer(value: Any, label: str, failures: list[str]) -> None:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        failures.append(f"{label} must be a nonnegative integer")


def _validate_budget(row: Mapping[str, Any], failures: list[str]) -> None:
    for field in (
        "shards", "baseline_iterations", "current_iterations",
        "baseline_duration_seconds", "current_duration_seconds",
        "baseline_obligation", "current_obligation",
    ):
        _nonnegative_integer(row.get(field), f"budget {row.get('id')} {field}", failures)
    if row.get("review_status") == "accepted":
        for current, baseline in (
            ("current_iterations", "baseline_iterations"),
            ("current_duration_seconds", "baseline_duration_seconds"),
            ("current_obligation", "baseline_obligation"),
        ):
            if isinstance(row.get(current), int) and isinstance(row.get(baseline), int):
                if row[current] < row[baseline]:
                    failures.append(f"budget {row.get('id')} weakens {current}")
    if not SHA256_RE.fullmatch(str(row.get("input_digest", ""))):
        failures.append(f"budget {row.get('id')} input_digest is invalid")


def _validate_claim(row: Mapping[str, Any], failures: list[str]) -> None:
    for field in (
        "operating_systems", "architectures", "features", "profiles",
        "packaging_obligations", "locale_path_encoding", "evidence_requirements",
        "evidence_refs",
    ):
        value = row.get(field)
        if not isinstance(value, list) or not value or any(not isinstance(item, str) for item in value):
            failures.append(f"claim {row.get('id')} {field} must be nonempty strings")


def _load_artifact(
    row: Mapping[str, Any], artifact_root: Path, failures: list[str]
) -> Mapping[str, Any] | None:
    raw_path = row.get("raw_artifact")
    if not isinstance(raw_path, str) or not raw_path:
        failures.append(f"proof class {row.get('id')} raw_artifact is invalid")
        return None
    relative = Path(raw_path)
    if relative.is_absolute() or ".." in relative.parts:
        failures.append(f"proof class {row.get('id')} raw_artifact escapes root")
        return None
    absolute = artifact_root / relative
    if not absolute.is_file():
        failures.append(f"proof class {row.get('id')} raw artifact is absent")
        return None
    raw_bytes = absolute.read_bytes()
    digest = "sha256:" + hashlib.sha256(raw_bytes).hexdigest()
    if row.get("raw_sha256") != digest:
        failures.append(f"proof class {row.get('id')} raw artifact digest is stale")
    try:
        payload = json.loads(raw_bytes)
    except (UnicodeDecodeError, json.JSONDecodeError):
        failures.append(f"proof class {row.get('id')} raw artifact is not valid JSON")
        return None
    if not isinstance(payload, Mapping):
        failures.append(f"proof class {row.get('id')} raw artifact must be a JSON object")
        return None
    return payload
