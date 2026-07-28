"""Validate and reconstruct an already-applied stale-authority transaction."""

from __future__ import annotations

import json
import tomllib
from collections import defaultdict
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from .case_backed_behavior_assignments import EVIDENCE_PATH, _load_invariants
from .frozen_catalog_gaps import analyze_frozen_catalog_gaps
from .proof_contract import proof_contract_digest
from .stale_case_review_refresh import refresh_source_closure_reviews
from .stale_case_authority_refresh import (
    CATALOG_PATH,
    DECISIONS_PATH,
    EXPECTED_CASE_COUNT,
    EXPECTED_EXECUTION_TEST_COUNT,
    EXPECTED_GENERATED_AUTHORITY_COUNT,
    EXPECTED_SOURCE_CLOSURE_REFRESH_TEST_COUNT,
    LOCK_PATH,
    SOURCE_CLOSURE_INVENTORIES_PATH,
    _digest_path,
    _digest_text,
    _json,
    _records,
    _require_frozen_population,
    _string,
    _strings,
)


def applied_refresh_plan_from_documents(
    *,
    root: Path,
    report: Mapping[str, Any],
    catalog_source: str,
    evidence_source: str,
    lock_source: str,
) -> dict[str, Any]:
    """Reconstruct and validate the no-op plan after final atomic application."""

    lock = json.loads(lock_source)
    if not isinstance(lock, dict) or lock.get("state") != "executed":
        raise ValueError("applied stale authority lock is not executed")
    authorities = _records(lock.get("authorities"), "applied authorities")
    rejected = _records(
        lock.get("rejected_authorities"), "applied rejected authorities"
    )
    assignments = _records(lock.get("assignments"), "applied assignments")
    _require_frozen_population(
        invariant_count=len(authorities),
        case_count=int(lock.get("case_count", -1)),
        generated_authority_count=int(
            lock.get("generated_authority_count", -1)
        ),
        affected_catalog_test_count=int(
            lock.get("affected_catalog_test_count", -1)
        ),
        execution_test_count=int(lock.get("execution_test_count", -1)),
        runner_source_count=int(lock.get("runner_source_count", -1)),
        behavior_count=len(assignments),
        assigned_case_count=int(lock.get("assigned_case_count", -1)),
        assignment_digest=str(lock.get("assignment_digest", "")),
        rejected_invariants={
            _string(row, "invariant_id", "applied rejected authority")
            for row in rejected
        },
    )
    source_closure_test_ids = _strings(
        lock.get("source_closure_refresh_test_ids"),
        "applied source-closure refresh test IDs",
    )
    if (
        len(source_closure_test_ids)
        != EXPECTED_SOURCE_CLOSURE_REFRESH_TEST_COUNT
        or lock.get("source_closure_refresh_test_count")
        != EXPECTED_SOURCE_CLOSURE_REFRESH_TEST_COUNT
    ):
        raise ValueError("applied source-closure refresh population drifted")

    catalog = tomllib.loads(catalog_source)
    catalog_tests = {
        _string(row, "id", "applied catalog test"): row
        for row in _records(catalog.get("tests"), "applied catalog tests")
    }
    replacements: dict[str, str] = {}
    runner_paths: set[str] = set()
    behavior_refs_by_test: dict[str, set[str]] = defaultdict(set)
    for assignment in assignments:
        behavior_refs_by_test[
            _string(assignment, "test_id", "applied assignment")
        ].add(_string(assignment, "behavior_ref", "applied assignment"))
    for authority in authorities:
        case_file = _string(authority, "case_file", "applied authority")
        case_source = (root / case_file).read_text()
        case_document = tomllib.loads(case_source)
        current_case_digest = _string(
            authority, "current_case_file_digest", "applied authority"
        )
        if (
            _digest_text(case_source) != current_case_digest
            or case_document.get("source_digest")
            != authority.get("current_source_digest")
            or len(
                _records(
                    case_document.get("case"), f"{case_file} applied cases"
                )
            )
            != authority.get("case_count")
        ):
            raise ValueError(f"{case_file} applied authority drifted")
        replacements[case_file] = case_source
        for test_id in _strings(
            authority.get("catalog_test_ids"),
            f"{case_file} applied catalog tests",
        ):
            if (
                catalog_tests[test_id].get("case_file_digest")
                != current_case_digest
            ):
                raise ValueError(f"{test_id} applied case digest drifted")
        mapping_test_id = _string(
            authority, "mapping_test_id", "applied authority"
        )
        if not behavior_refs_by_test[mapping_test_id].issubset(
            set(catalog_tests[mapping_test_id].get("mapping_behavior_refs", []))
        ):
            raise ValueError(
                f"{mapping_test_id} applied behavior refs drifted"
            )
        for path in _strings(
            authority.get("runner_source_paths"),
            f"{case_file} applied runner sources",
        ):
            source = (root / path).read_text()
            if current_case_digest not in source:
                raise ValueError(f"{path} applied case digest drifted")
            replacements[path] = source
            runner_paths.add(path)

    for test_id in source_closure_test_ids:
        test = catalog_tests[test_id]
        matched = False
        path = test.get("path")
        if (
            isinstance(path, str)
            and path in runner_paths
            and "mapping_reviewed_source_digest" in test
        ):
            matched = True
            if test["mapping_reviewed_source_digest"] != _digest_path(
                root / path
            ):
                raise ValueError(
                    f"{test_id} reviewed source digest drifted"
                )
        closure = test.get("mapping_assertion_source_digests")
        if isinstance(closure, Mapping):
            for source_path in sorted(set(closure).intersection(runner_paths)):
                matched = True
                if closure[source_path] != _digest_path(root / source_path):
                    raise ValueError(
                        f"{test_id} assertion source closure drifted"
                    )
        if not matched:
            raise ValueError(
                f"{test_id} lacks applied runner source-closure binding"
            )

    inventory_path = root / SOURCE_CLOSURE_INVENTORIES_PATH
    inventory_document = _json(inventory_path)
    source_commit = _string(lock, "source_commit", "applied lock")
    (
        catalog_source,
        decisions_source,
        refreshed_decision_test_ids,
    ) = refresh_source_closure_reviews(
        catalog_source=catalog_source,
        decisions_source=(root / DECISIONS_PATH).read_text(),
        inventory_document=inventory_document,
        test_ids=source_closure_test_ids,
        source_commit=source_commit,
        expected_count=EXPECTED_SOURCE_CLOSURE_REFRESH_TEST_COUNT,
    )
    if (
        len(refreshed_decision_test_ids)
        != EXPECTED_SOURCE_CLOSURE_REFRESH_TEST_COUNT
    ):
        raise ValueError("applied source-closure review population drifted")
    lock["source_closure_refresh_decision_count"] = len(
        refreshed_decision_test_ids
    )
    lock["source_closure_assertion_inventory_path"] = (
        SOURCE_CLOSURE_INVENTORIES_PATH
    )
    lock["source_closure_assertion_inventory_digest"] = _digest_path(
        inventory_path
    )
    lock["source_closure_assertion_inventory_source_commit"] = source_commit
    lock["source_closure_review_ledger_path"] = DECISIONS_PATH
    catalog = tomllib.loads(catalog_source)
    catalog_tests = {
        _string(row, "id", "applied refreshed catalog test"): row
        for row in _records(
            catalog.get("tests"), "applied refreshed catalog tests"
        )
    }

    evidence = tomllib.loads(evidence_source)
    evidence_records = {
        _string(row, "id", "applied evidence record"): row
        for row in _records(
            evidence.get("evidence"), "applied evidence records"
        )
    }
    for evidence_id in _strings(
        lock.get("retired_current_evidence_ids"),
        "retired current evidence IDs",
    ):
        if (
            evidence_records[evidence_id].get("proof_contract_binding")
            != "source_revision"
        ):
            raise ValueError(f"{evidence_id} was not retired")
    execution = lock.get("execution")
    if not isinstance(execution, Mapping):
        raise ValueError("applied stale authority lock lacks execution")
    proof_record_ids = _strings(
        execution.get("proof_record_ids"), "applied proof record IDs"
    )
    invariant_documents = {
        invariant_id: record
        for invariant_id, (_, record) in _load_invariants(root).items()
    }
    for evidence_id in proof_record_ids:
        record = evidence_records.get(evidence_id)
        if record is None:
            raise ValueError(f"{evidence_id} is absent from applied evidence")
        linked_tests = _strings(
            record.get("linked_tests"), f"{evidence_id} linked tests"
        )
        if len(linked_tests) != 1:
            raise ValueError(f"{evidence_id} links multiple tests")
        test = catalog_tests[linked_tests[0]]
        if (
            record.get("path") != EVIDENCE_PATH
            or record.get("commit") != source_commit
            or record.get("proof_contract_binding") != "current"
            or record.get("proof_contract_digest")
            != proof_contract_digest(
                test=test,
                invariants=invariant_documents,
            )
        ):
            raise ValueError(f"{evidence_id} applied proof drifted")
    if (
        len(proof_record_ids) != EXPECTED_EXECUTION_TEST_COUNT
        or execution.get("execution_test_count")
        != EXPECTED_EXECUTION_TEST_COUNT
        or execution.get("passed_case_count") != EXPECTED_CASE_COUNT
        or execution.get("generated_authority_equality_count")
        != EXPECTED_GENERATED_AUTHORITY_COUNT
    ):
        raise ValueError("applied execution summary drifted")

    gap_analysis = analyze_frozen_catalog_gaps(
        denominator=tomllib.loads(
            (
                root / "verification/test-catalog-denominator.toml"
            ).read_text()
        ),
        catalog=catalog,
        invariants=[
            (path.as_posix(), record)
            for path, record in sorted(
                _load_invariants(root).values(),
                key=lambda value: value[0].as_posix(),
            )
        ],
    )
    summary = gap_analysis["summary"]
    if (
        summary["confirmed_missing_tests"] != 267
        or summary["missing_spec_authority_tests"] != 0
        or len(
            _records(
                report.get("confirmed_missing_tests"),
                "frozen missing tests",
            )
        )
        != 321
    ):
        raise ValueError("applied frozen gap summary drifted")

    replacements[CATALOG_PATH] = catalog_source
    replacements[DECISIONS_PATH] = decisions_source
    replacements[EVIDENCE_PATH] = evidence_source
    replacements[LOCK_PATH] = (
        json.dumps(lock, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    )
    return {
        **lock,
        "generated_authority_equality_count": (
            execution["generated_authority_equality_count"]
        ),
        "passed_case_count": execution["passed_case_count"],
        "proof_record_count": len(proof_record_ids),
        "proof_record_ids": proof_record_ids,
        "post_refresh_gap_summary": summary,
        "replacements": replacements,
    }
