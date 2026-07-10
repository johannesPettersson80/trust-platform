"""Closed contract for reviewed test-refactor proposals and redirect files."""

from __future__ import annotations

import json
import re
import tomllib
from collections import Counter
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .test_catalog_json_schema import validate_json_schema_instance
from .test_catalog_models import InferredTestFact
from .test_catalog_common import stable_discovery_id
from .test_catalog_validation import SOURCE_KINDS
from .test_refactor_behavior_lock import (
    PROPOSAL_EVIDENCE_FIELDS,
    validate_behavior_lock_evidence_reuse,
    validate_behavior_lock_pair,
)
from .test_refactor_identity import (
    IDENTITY_FIELDS,
    catalog_identity,
    safe_relative_path,
    validate_identity,
    validate_live_path,
)
from .test_refactor_redirects import (
    REDIRECT_FIELDS,
    resolve_redirect_endpoint,
    validate_redirect_contract,
)


PROPOSALS_PATH = Path("verification/test-refactor-proposals.toml")
REDIRECTS_PATH = Path("verification/test-catalog-redirects.toml")
PROPOSAL_SCHEMA_PATH = Path("verification/schemas/test-refactor-proposal.schema.json")
REDIRECT_SCHEMA_PATH = Path("verification/schemas/test-catalog-redirect.schema.json")
ASSESSMENT_REPORT_PATH = (
    "docs/internal/testing/evidence/plc-verification-program/2026-07-10/"
    "p2a-test-refactor-assessment.md"
)
PROPOSAL_FIELDS = {
    "schema_version", "id", "test_id", "disposition", "status", "lifecycle",
    "source_paths", "source_identity", "target_identity", "decision_inputs",
    "finding_refs", "before_command", "after_command", "invariant_ids",
    "coverage_dimensions", "fixture_ownership", "stale_path_updates",
    "expected_behavior_delta", "design_review", "rationale", "last_reviewed",
}
FIXTURE_FIELDS = {"before_owner", "after_owner", "review"}
DESIGN_FIELDS = {"solid", "kiss", "dry", "rationale"}
DISPOSITIONS = {"move", "split", "rename", "no_refactor_needed"}
REFACTOR_DISPOSITIONS = DISPOSITIONS - {"no_refactor_needed"}
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
DIMENSION_RE = re.compile(r"^[a-z][a-z0-9_]*:[A-Za-z0-9_]+$")
PROPOSAL_ID_RE = re.compile(r"^TEST_REFACTOR_[A-Z0-9_]+$")
TEST_ID_RE = re.compile(r"^TEST_[A-Z0-9_]+$")
REFACTOR_LIFECYCLES = {
    "proposed": ["proposed"],
    "approved": ["proposed", "approved"],
    "in_progress": ["proposed", "approved", "in_progress"],
    "completed": ["proposed", "approved", "in_progress", "completed"],
    "validated": ["proposed", "approved", "in_progress", "completed", "validated"],
    "rejected": ["proposed", "rejected"],
}


def validate_test_refactor_records(
    *,
    root: Path,
    proposals: Mapping[str, Mapping[str, Any]],
    redirects: Mapping[str, Mapping[str, Any]],
    tests: Mapping[str, Mapping[str, Any]],
    evidence: Mapping[str, Mapping[str, Any]],
    facts: Sequence[InferredTestFact],
    assessment: Mapping[str, Any] | None = None,
) -> list[str]:
    failures: list[str] = []
    facts_by_id: dict[str, list[InferredTestFact]] = {}
    for fact in facts:
        facts_by_id.setdefault(fact.stable_id, []).append(fact)
    for proposal_id in sorted(proposals):
        _validate_proposal(
            root=root,
            key=proposal_id,
            proposal=proposals[proposal_id],
            redirects=redirects,
            tests=tests,
            evidence=evidence,
            assessment=assessment,
            facts_by_id=facts_by_id,
            failures=failures,
        )
    validate_behavior_lock_evidence_reuse(proposals, failures)
    validate_redirect_contract(
        root=root,
        redirects=redirects,
        proposals=proposals,
        tests=tests,
        facts=facts,
        failures=failures,
    )
    return sorted(set(failures))


def validate_repository_test_refactors(
    root: Path,
) -> tuple[list[str], int, int, int, int]:
    """Recompute the complete live assessment and proposal join."""

    from .test_refactor_live import build_live_test_refactor_state

    try:
        state = build_live_test_refactor_state(root)
    except ValueError as exc:
        return [str(exc)], 0, 0, 0, 0
    return (
        [],
        state.proposal_count,
        state.redirect_count,
        state.catalog_count,
        state.fact_count,
    )


def load_test_refactor_files(
    root: Path,
) -> tuple[list[str], dict[str, Mapping[str, Any]], dict[str, Mapping[str, Any]]]:
    failures: list[str] = []
    proposal_data = _load_toml(root / PROPOSALS_PATH, "proposal file", failures)
    redirect_data = _load_toml(root / REDIRECTS_PATH, "redirect file", failures)
    proposal_schema = _load_json(root / PROPOSAL_SCHEMA_PATH, "proposal schema", failures)
    redirect_schema = _load_json(root / REDIRECT_SCHEMA_PATH, "redirect schema", failures)
    if proposal_schema is not None:
        failures.extend(
            _validate_file_schema(
                proposal_schema,
                "proposals",
                PROPOSAL_FIELDS | PROPOSAL_EVIDENCE_FIELDS,
                PROPOSAL_FIELDS,
            )
        )
        if proposal_data is not None:
            failures.extend(validate_json_schema_instance(proposal_data, proposal_schema))
    if redirect_schema is not None:
        failures.extend(
            _validate_file_schema(
                redirect_schema,
                "redirects",
                REDIRECT_FIELDS,
                REDIRECT_FIELDS,
            )
        )
        if redirect_data is not None:
            failures.extend(validate_json_schema_instance(redirect_data, redirect_schema))
    proposals = _index_file_records(proposal_data, "proposals", failures)
    redirects = _index_file_records(redirect_data, "redirects", failures)
    return sorted(set(failures)), proposals, redirects


def load_named_records(
    path: Path,
    table: str,
) -> tuple[list[str], dict[str, Mapping[str, Any]]]:
    failures: list[str] = []
    data = _load_toml(path, table, failures)
    return failures, _index_file_records(data, table, failures)


def _validate_proposal(
    *,
    root: Path,
    key: str,
    proposal: Mapping[str, Any],
    redirects: Mapping[str, Mapping[str, Any]],
    tests: Mapping[str, Mapping[str, Any]],
    evidence: Mapping[str, Mapping[str, Any]],
    assessment: Mapping[str, Any] | None,
    facts_by_id: Mapping[str, list[InferredTestFact]],
    failures: list[str],
) -> None:
    label = f"proposal {key}"
    _check_fields(
        proposal,
        PROPOSAL_FIELDS,
        PROPOSAL_FIELDS | PROPOSAL_EVIDENCE_FIELDS,
        label,
        failures,
    )
    if proposal.get("id") != key:
        failures.append(f"{label} id does not match record key")
    if not PROPOSAL_ID_RE.fullmatch(str(proposal.get("id", ""))):
        failures.append(f"{label} id is invalid")
    if not TEST_ID_RE.fullmatch(str(proposal.get("test_id", ""))):
        failures.append(f"{label} test_id is invalid")
    if proposal.get("schema_version") != 1:
        failures.append(f"{label} must use schema_version 1")
    disposition = proposal.get("disposition")
    if disposition not in DISPOSITIONS:
        failures.append(f"{label} has unknown disposition {disposition!r}")
    _validate_date(proposal.get("last_reviewed"), label, failures)
    source = validate_identity(
        proposal.get("source_identity"), f"{label} source identity", failures
    )
    target = validate_identity(
        proposal.get("target_identity"), f"{label} target identity", failures
    )
    source_paths = _string_list(
        proposal.get("source_paths"), f"{label} source_paths", failures, nonempty=True
    )
    decision_inputs = _string_list(
        proposal.get("decision_inputs"),
        f"{label} decision_inputs",
        failures,
        nonempty=True,
    )
    findings = _string_list(proposal.get("finding_refs"), f"{label} finding_refs", failures)
    invariants = _string_list(
        proposal.get("invariant_ids"), f"{label} invariant_ids", failures, nonempty=True
    )
    dimensions = _string_list(
        proposal.get("coverage_dimensions"),
        f"{label} coverage_dimensions",
        failures,
    )
    stale_updates = _string_list(
        proposal.get("stale_path_updates"), f"{label} stale_path_updates", failures
    )
    for field, values in (
        ("source_paths", source_paths),
        ("decision_inputs", decision_inputs),
        ("stale_path_updates", stale_updates),
    ):
        for value in values:
            if not safe_relative_path(value):
                failures.append(f"{label} {field} has unsafe workspace path {value!r}")
    for dimension in dimensions:
        if not DIMENSION_RE.fullmatch(dimension):
            failures.append(f"{label} has invalid coverage dimension {dimension!r}")
    if decision_inputs != [ASSESSMENT_REPORT_PATH]:
        failures.append(
            f"{label} decision_inputs must equal the single reviewed P2A assessment"
        )
    _validate_review_object(
        proposal.get("fixture_ownership"),
        FIXTURE_FIELDS,
        f"{label} fixture_ownership",
        failures,
    )
    _validate_design_review(proposal.get("design_review"), label, failures)
    if proposal.get("expected_behavior_delta") != "none":
        failures.append(f"{label} expected_behavior_delta must equal 'none'")
    for field in ("before_command", "after_command", "rationale"):
        if not _nonempty(proposal.get(field)):
            failures.append(f"{label} requires non-empty {field}")
    if source is not None and source.get("path") not in source_paths:
        failures.append(f"{label} source_paths must include source identity path")
    if source is not None and source_paths != [source["path"]]:
        failures.append(f"{label} source_paths must equal the single source identity path")
    for stale_path in stale_updates:
        validate_live_path(root, stale_path, f"{label} stale_path_updates", failures)
    _validate_planned_target_identity(
        proposal,
        disposition,
        source,
        target,
        facts_by_id,
        label,
        failures,
    )

    test_id = proposal.get("test_id")
    catalog = tests.get(test_id) if isinstance(test_id, str) else None
    if catalog is None:
        failures.append(f"{label} references unknown catalog test {test_id!r}")
    else:
        _validate_proposal_catalog(
            root=root,
            proposal=proposal,
            disposition=disposition,
            source=source,
            target=target,
            source_paths=source_paths,
            invariants=invariants,
            dimensions=dimensions,
            test_id=test_id,
            catalog=catalog,
            redirects=redirects,
            label=label,
            failures=failures,
        )

    _validate_lifecycle(proposal, disposition, label, failures)
    if disposition == "no_refactor_needed":
        _validate_no_refactor_shape(
            proposal, source, target, stale_updates, findings, label, failures
        )
    elif disposition in REFACTOR_DISPOSITIONS:
        _validate_refactor_shape(
            disposition, source, target, stale_updates, findings, label, failures
        )
        status = proposal.get("status")
        if status not in {"proposed", "rejected"} and not dimensions:
            failures.append(
                f"{label} cannot advance without explicit catalog-backed coverage dimensions"
            )
        if status in {"completed", "validated"}:
            if proposal.get("before_command") != proposal.get("after_command"):
                failures.append(
                    f"{label} current lock proof requires identical commands; "
                    "command-changing refactors remain blocked"
                )
            if catalog is not None and (
                not _nonempty(catalog.get("case_file"))
                or not _nonempty(catalog.get("case_file_digest"))
            ):
                failures.append(
                    f"{label} completed refactor requires catalog case_file and "
                    "case_file_digest for production lock proof"
                )
            validate_behavior_lock_pair(root, proposal, catalog, evidence, label, failures)
        elif PROPOSAL_EVIDENCE_FIELDS & set(proposal):
            failures.append(f"{label} non-completed proposal forbids behavior-lock evidence")
    if assessment is not None:
        _validate_assessment(proposal, assessment, label, failures)


def _validate_proposal_catalog(
    *,
    root: Path,
    proposal: Mapping[str, Any],
    disposition: Any,
    source: dict[str, str] | None,
    target: dict[str, str] | None,
    source_paths: list[str],
    invariants: list[str],
    dimensions: list[str],
    test_id: str,
    catalog: Mapping[str, Any],
    redirects: Mapping[str, Mapping[str, Any]],
    label: str,
    failures: list[str],
) -> None:
    if catalog.get("subject_kind") != "generated_test":
        failures.append(f"{label} requires a generated_test catalog subject")
    status = proposal.get("status")
    if (
        disposition == "no_refactor_needed"
        or status not in {"completed", "validated"}
    ) and catalog.get("path") not in source_paths:
        failures.append(f"{label} source_paths must include catalog path")
    if set(invariants) != set(catalog.get("invariants", [])):
        failures.append(f"{label} invariant_ids do not match catalog invariants")
    expected_dimensions = {
        f"malformed_input_class:{class_id}"
        for class_id in catalog.get("malformed_input_class_ids", [])
        if isinstance(class_id, str)
    }
    if set(dimensions) != expected_dimensions:
        failures.append(
            f"{label} coverage_dimensions do not match explicit catalog "
            "malformed-input bindings"
        )
    expected_identity = target if status in {"completed", "validated"} else source
    resolved_identity = resolve_redirect_endpoint(expected_identity, test_id, redirects)
    if resolved_identity is not None and catalog_identity(catalog) != resolved_identity:
        failures.append(f"{label} catalog identity does not match lifecycle endpoint")
    if resolved_identity is not None:
        validate_live_path(
            root, resolved_identity["path"], f"{label} lifecycle endpoint", failures
        )
    expected_command = (
        proposal.get("after_command")
        if status in {"completed", "validated"}
        else proposal.get("before_command")
    )
    if resolved_identity == expected_identity and catalog.get("command") != expected_command:
        failures.append(f"{label} command does not match catalog lifecycle endpoint")


def _validate_planned_target_identity(
    proposal: Mapping[str, Any],
    disposition: Any,
    source: dict[str, str] | None,
    target: dict[str, str] | None,
    facts_by_id: Mapping[str, list[InferredTestFact]],
    label: str,
    failures: list[str],
) -> None:
    if (
        disposition not in {"move", "rename"}
        or proposal.get("status") in {"completed", "validated", "rejected"}
        or source is None
        or target is None
    ):
        return
    matches = facts_by_id.get(source["discovery_id"], [])
    if len(matches) != 1:
        failures.append(
            f"{label} source identity must resolve once before deriving target identity"
        )
        return
    fact = matches[0]
    expected_native_id = f"{source['path']}#{source['name']}"
    if fact.native_id != expected_native_id:
        failures.append(
            f"{label} target discovery_id cannot be derived from non-default native identity"
        )
        return
    expected_id = stable_discovery_id(
        source_kind=target["discovery_source_kind"],
        package=fact.package,
        native_id=f"{target['path']}#{target['name']}",
    )
    if target["discovery_id"] != expected_id:
        failures.append(
            f"{label} target discovery_id does not match the derived scanner identity"
        )


def _validate_lifecycle(
    proposal: Mapping[str, Any],
    disposition: Any,
    label: str,
    failures: list[str],
) -> None:
    status = proposal.get("status")
    lifecycle = proposal.get("lifecycle")
    if disposition == "no_refactor_needed":
        expected = ["proposed", "reviewed", "validated"]
        if status != "validated" or lifecycle != expected:
            failures.append(f"{label} no_refactor_needed lifecycle must equal {expected}")
        return
    expected = REFACTOR_LIFECYCLES.get(status)
    if expected is None:
        failures.append(f"{label} has unknown lifecycle status {status!r}")
    elif lifecycle != expected:
        failures.append(f"{label} lifecycle must equal {expected} for status {status}")
    if status == "completed":
        failures.append(
            f"{label} completed is transient and cannot be committed; validate with a redirect or revert"
        )


def _validate_no_refactor_shape(
    proposal: Mapping[str, Any],
    source: dict[str, str] | None,
    target: dict[str, str] | None,
    stale_updates: list[str],
    findings: list[str],
    label: str,
    failures: list[str],
) -> None:
    if source != target:
        failures.append(f"{label} no_refactor_needed target identity must equal source identity")
    if stale_updates:
        failures.append(f"{label} no_refactor_needed stale_path_updates must be empty")
    if findings:
        failures.append(f"{label} no_refactor_needed finding_refs must be empty")
    if PROPOSAL_EVIDENCE_FIELDS & set(proposal):
        failures.append(f"{label} no_refactor_needed forbids behavior-lock evidence")
    if proposal.get("before_command") != proposal.get("after_command"):
        failures.append(f"{label} no_refactor_needed before and after commands must match")
    ownership = proposal.get("fixture_ownership")
    if isinstance(ownership, Mapping) and ownership.get("before_owner") != ownership.get(
        "after_owner"
    ):
        failures.append(
            f"{label} no_refactor_needed fixture owner must remain unchanged"
        )


def _validate_refactor_shape(
    disposition: str,
    source: dict[str, str] | None,
    target: dict[str, str] | None,
    stale_updates: list[str],
    findings: list[str],
    label: str,
    failures: list[str],
) -> None:
    if disposition == "split":
        failures.append(f"{label} split is blocked until a reviewed multi-target contract exists")
        return
    if source == target:
        failures.append(f"{label} {disposition} target identity must differ from source identity")
    if not stale_updates or "verification/test-catalog.toml" not in stale_updates:
        failures.append(f"{label} {disposition} must include the catalog stale-path update")
    if not findings:
        failures.append(f"{label} {disposition} requires assessment finding_refs")
    if source is None or target is None:
        return
    if disposition == "rename" and not (
        source["name"] != target["name"]
        and source["path"] == target["path"]
        and source["discovery_source_kind"] == target["discovery_source_kind"]
    ):
        failures.append(f"{label} rename must change only the test name and derived discovery_id")
    if disposition == "move" and not (
        source["path"] != target["path"]
        and source["name"] == target["name"]
        and source["discovery_source_kind"] == target["discovery_source_kind"]
    ):
        failures.append(f"{label} move must change only the path and derived discovery_id")


def _validate_assessment(
    proposal: Mapping[str, Any],
    assessment: Mapping[str, Any],
    label: str,
    failures: list[str],
) -> None:
    rows = assessment.get("proposal_evaluations")
    if not isinstance(rows, list):
        failures.append(f"{label} assessment lacks proposal_evaluations")
        return
    matches = [
        row
        for row in rows
        if isinstance(row, Mapping) and row.get("proposal_id") == proposal.get("id")
    ]
    if len(matches) != 1:
        failures.append(f"{label} must resolve to exactly one assessment evaluation")
        return
    row = matches[0]
    if (
        row.get("disposition") != proposal.get("disposition")
        or row.get("source_paths") != proposal.get("source_paths")
    ):
        failures.append(f"{label} assessment identity does not match proposal")
    supported = row.get("supported") is True
    may_remain_unapproved = (
        proposal.get("disposition") in REFACTOR_DISPOSITIONS
        and proposal.get("status") in {"proposed", "rejected"}
    )
    if not supported and not may_remain_unapproved:
        failures.append(f"{label} assessment does not support the proposal disposition")
    observed = row.get("observed_signals")
    if observed != proposal.get("finding_refs"):
        failures.append(f"{label} assessment signals do not match finding_refs")
    if proposal.get("disposition") == "no_refactor_needed" and observed:
        failures.append(f"{label} no_refactor_needed has actionable assessment signals")


def _validate_review_object(
    value: Any,
    fields: set[str],
    label: str,
    failures: list[str],
) -> None:
    if not isinstance(value, Mapping):
        failures.append(f"{label} must be an object")
        return
    _check_fields(value, fields, fields, label, failures)
    for field in fields:
        if not _nonempty(value.get(field)):
            failures.append(f"{label} requires non-empty {field}")


def _validate_design_review(value: Any, label: str, failures: list[str]) -> None:
    _validate_review_object(value, DESIGN_FIELDS, f"{label} design_review", failures)
    if isinstance(value, Mapping):
        for principle in ("solid", "kiss", "dry"):
            if value.get(principle) != "pass":
                failures.append(f"{label} design_review {principle} must equal pass")


def _validate_date(value: Any, label: str, failures: list[str]) -> None:
    if not isinstance(value, str) or not DATE_RE.fullmatch(value):
        failures.append(f"{label} last_reviewed must be YYYY-MM-DD")


def _string_list(
    value: Any,
    label: str,
    failures: list[str],
    *,
    nonempty: bool = False,
) -> list[str]:
    if not isinstance(value, list) or not all(_nonempty(item) for item in value):
        failures.append(f"{label} must be a string array")
        return []
    if nonempty and not value:
        failures.append(f"{label} must not be empty")
    if value != sorted(set(value)):
        failures.append(f"{label} must be sorted and unique")
    return list(value)


def _check_fields(
    value: Mapping[str, Any],
    required: set[str],
    allowed: set[str],
    label: str,
    failures: list[str],
) -> None:
    for field in sorted(required - set(value)):
        failures.append(f"{label} missing required field {field}")
    for field in sorted(set(value) - allowed):
        failures.append(f"{label} has additional field {field}")


def _nonempty(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def _load_toml(
    path: Path,
    label: str,
    failures: list[str],
) -> Mapping[str, Any] | None:
    try:
        return tomllib.loads(path.read_text())
    except Exception as exc:
        failures.append(f"{label} cannot be read at {path}: {exc}")
        return None


def _load_json(
    path: Path,
    label: str,
    failures: list[str],
) -> Mapping[str, Any] | None:
    try:
        data = json.loads(path.read_text())
    except Exception as exc:
        failures.append(f"{label} cannot be read at {path}: {exc}")
        return None
    if not isinstance(data, Mapping):
        failures.append(f"{label} root must be an object")
        return None
    return data


def _index_file_records(
    data: Mapping[str, Any] | None,
    table: str,
    failures: list[str],
) -> dict[str, Mapping[str, Any]]:
    if data is None:
        return {}
    records = data.get(table)
    if not isinstance(records, list):
        failures.append(f"{table} file must contain a {table} array")
        return {}
    counts = Counter(
        record.get("id") for record in records if isinstance(record, Mapping)
    )
    result: dict[str, Mapping[str, Any]] = {}
    for record in records:
        if not isinstance(record, Mapping) or not isinstance(record.get("id"), str):
            failures.append(f"{table} contains a record without a string id")
            continue
        record_id = record["id"]
        if counts[record_id] != 1:
            failures.append(f"{table} contains duplicate id {record_id}")
            continue
        result[record_id] = record
    return result


def _validate_file_schema(
    schema: Mapping[str, Any],
    table: str,
    record_fields: set[str],
    required_fields: set[str],
) -> list[str]:
    failures: list[str] = []
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append(f"{table} schema root must be a closed object")
    if set(schema.get("required", [])) != {table}:
        failures.append(f"{table} schema root required fields drift from contract")
    if set(schema.get("properties", {})) != {table}:
        failures.append(f"{table} schema root properties drift from contract")
    definitions = schema.get("$defs")
    record = definitions.get("record") if isinstance(definitions, Mapping) else None
    if not isinstance(record, Mapping):
        failures.append(f"{table} schema lacks record definition")
    else:
        if record.get("type") != "object" or record.get("additionalProperties") is not False:
            failures.append(f"{table} schema record must be a closed object")
        if set(record.get("properties", {})) != record_fields:
            failures.append(f"{table} schema record fields drift from contract")
        if set(record.get("required", [])) != required_fields:
            failures.append(f"{table} schema required record fields drift from contract")
        if table == "proposals":
            disposition = record.get("properties", {}).get("disposition", {})
            if set(disposition.get("enum", [])) != DISPOSITIONS:
                failures.append("proposals schema disposition enum drifts from contract")
    identity = definitions.get("identity") if isinstance(definitions, Mapping) else None
    if isinstance(identity, Mapping):
        source_kind = identity.get("properties", {}).get("discovery_source_kind", {})
        if set(source_kind.get("enum", [])) != SOURCE_KINDS:
            failures.append(f"{table} schema discovery_source_kind enum drifts from scanner")
    if isinstance(definitions, Mapping):
        for name, definition in definitions.items():
            if (
                isinstance(definition, Mapping)
                and definition.get("type") == "object"
                and definition.get("additionalProperties") is not False
            ):
                failures.append(f"{table} schema {name} must be a closed object")
    return failures
