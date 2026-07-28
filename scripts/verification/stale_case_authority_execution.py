"""Finalize one stale case-authority execution campaign and its evidence."""

from __future__ import annotations

import json
import re
import tomllib
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from .case_backed_behavior_assignments import EVIDENCE_PATH, _load_invariants
from .proof_contract import proof_contract_digest
from .proof_output import render_evidence_record
from .prover import case_result_digest
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
    _canonical_digest,
    _digest_path,
    _digest_text,
    _json,
    _records,
    _string,
    _strings,
)


def finalize_refresh_plan(
    *,
    root: Path,
    plan: Mapping[str, Any],
    campaign: Mapping[str, Any],
    proof_records_dir: Path,
    case_artifact_dir: Path,
    generated_authority_equality: Mapping[str, Any],
) -> dict[str, Any]:
    """Bind one complete remote execution campaign into the prepared transaction."""

    root = root.resolve()
    replacements = _string_replacements(plan)
    execution_test_ids = _strings(
        plan.get("execution_test_ids"), "prepared execution test IDs"
    )
    if len(execution_test_ids) != EXPECTED_EXECUTION_TEST_COUNT:
        raise ValueError("prepared execution population drifted")
    results = _records(campaign.get("results"), "execution campaign results")
    result_by_test = {
        _string(row, "test_id", "execution campaign result"): row
        for row in results
    }
    if (
        len(results) != EXPECTED_EXECUTION_TEST_COUNT
        or set(result_by_test) != set(execution_test_ids)
        or campaign.get("result_count") != EXPECTED_EXECUTION_TEST_COUNT
        or campaign.get("passed_count") != EXPECTED_EXECUTION_TEST_COUNT
        or any(row.get("status") != "passed" for row in results)
    ):
        raise ValueError("execution population is incomplete or drifted")
    if campaign.get("rerun_label") != "P18_CASE_REFRESH":
        raise ValueError("execution campaign rerun label drifted")
    source_commit = _string(campaign, "source_commit", "execution campaign")
    if not re.fullmatch(r"[0-9a-f]{40}", source_commit):
        raise ValueError("execution campaign source commit is not full SHA")

    equality_results = _records(
        generated_authority_equality.get("results"),
        "generated authority equality results",
    )
    expected_generated_ids = {
        _string(authority, "invariant_id", "prepared authority")
        for authority in _records(plan.get("authorities"), "prepared authorities")
        if authority.get("case_provenance_kind")
        == "generated_decision_table_v1"
    }
    equality_ids = {
        _string(row, "invariant_id", "generated authority equality result")
        for row in equality_results
    }
    if (
        generated_authority_equality.get("source_commit") != source_commit
        or generated_authority_equality.get("all_equal") is not True
        or generated_authority_equality.get("generated_authority_count")
        != EXPECTED_GENERATED_AUTHORITY_COUNT
        or len(equality_results) != EXPECTED_GENERATED_AUTHORITY_COUNT
        or equality_ids != expected_generated_ids
        or any(row.get("equal") is not True for row in equality_results)
    ):
        raise ValueError("generated authority equality proof is incomplete")

    if plan.get("state") == "executed":
        execution = plan.get("execution")
        if not isinstance(execution, Mapping):
            raise ValueError("executed refresh plan lacks execution evidence")
        if (
            execution.get("campaign_digest")
            != "sha256:" + _canonical_digest(campaign)
            or execution.get("generated_authority_equality_digest")
            != "sha256:" + _canonical_digest(generated_authority_equality)
            or execution.get("proof_record_ids")
            != sorted(
                _string(row, "record_id", "execution campaign result")
                for row in results
            )
        ):
            raise ValueError("executed refresh evidence does not match campaign")
        return dict(plan)

    catalog = tomllib.loads(replacements[CATALOG_PATH])
    catalog_by_id = {
        _string(row, "id", "prepared catalog test"): row
        for row in _records(catalog.get("tests"), "prepared catalog tests")
    }
    invariant_documents = {
        invariant_id: record
        for invariant_id, (_, record) in _load_invariants(root).items()
    }
    existing_evidence = tomllib.loads(replacements[EVIDENCE_PATH])
    existing_evidence_ids = {
        _string(row, "id", "prepared evidence")
        for row in _records(
            existing_evidence.get("evidence"), "prepared evidence records"
        )
    }
    proof_records: list[dict[str, Any]] = []
    proof_record_digests: dict[str, str] = {}
    case_artifact_digests: dict[str, str] = {}
    passed_case_count = 0
    for test_id in sorted(execution_test_ids):
        result = result_by_test[test_id]
        expected_record_id = (
            f"EVID_{test_id}_LOCK_BASELINE_P18_CASE_REFRESH"
        )
        if result.get("record_id") != expected_record_id:
            raise ValueError(f"{test_id} execution record ID drifted")
        record_path = proof_records_dir / f"{expected_record_id}.toml"
        record_document = tomllib.loads(record_path.read_text())
        records = _records(
            record_document.get("evidence"),
            f"{test_id} proof records",
        )
        if len(records) != 1:
            raise ValueError(f"{test_id} must have one proof record")
        record = dict(records[0])
        if expected_record_id in existing_evidence_ids:
            raise ValueError(f"evidence already contains {expected_record_id}")
        test = catalog_by_id[test_id]
        case_file = _string(test, "case_file", f"{test_id} catalog test")
        case_source = replacements.get(case_file)
        if case_source is None:
            raise ValueError(f"{test_id} prepared case source is missing")
        case_document = tomllib.loads(case_source)
        expected_case_ids = [
            _string(case, "id", f"{test_id} case")
            for case in _records(
                case_document.get("case"), f"{test_id} case records"
            )
        ]
        expected_summary = [
            f"{case_id}:passed" for case_id in expected_case_ids
        ]
        expected_case_digest = _digest_text(case_source)
        expected_result_digest = case_result_digest(
            command_exit_status=0,
            per_case_summary=expected_summary,
        )
        required_record_fields = {
            "id": expected_record_id,
            "commit": source_commit,
            "command": test.get("command"),
            "linked_tests": [test_id],
            "linked_invariants": test.get("invariants"),
            "proof_kind": "lock_baseline",
            "proof_contract_binding": "current",
            "proof_contract_version": "execution_contract_v2",
            "failure_kind": "none",
            "command_exit_status": 0,
            "case_file_digest": expected_case_digest,
            "case_result_digest": expected_result_digest,
            "per_case_summary": expected_summary,
            "proof_contract_digest": proof_contract_digest(
                test=test,
                invariants=invariant_documents,
            ),
        }
        drifted = [
            field
            for field, expected in required_record_fields.items()
            if record.get(field) != expected
        ]
        if drifted:
            raise ValueError(
                f"{test_id} proof record drifted fields {drifted}"
            )
        if (
            result.get("case_count") != len(expected_case_ids)
            or result.get("case_file_digest") != expected_case_digest
            or result.get("case_result_digest") != expected_result_digest
        ):
            raise ValueError(f"{test_id} campaign summary drifted")

        artifact_path = case_artifact_dir / f"{test_id}.json"
        artifact = _json(artifact_path)
        artifact_cases = _records(
            artifact.get("cases"), f"{test_id} case artifact cases"
        )
        artifact_summary = [
            f"{_string(case, 'id', f'{test_id} artifact case')}:"
            f"{case.get('result')}"
            for case in artifact_cases
        ]
        if (
            artifact.get("test_id") != test_id
            or artifact.get("case_file") != case_file
            or artifact.get("case_file_digest") != expected_case_digest
            or artifact.get("trust_verify_test_id") != test_id
            or artifact.get("trust_verify_run_id")
            != record.get("trust_verify_run_id")
            or artifact_summary != expected_summary
        ):
            raise ValueError(f"{test_id} case artifact drifted")
        artifact_digest = _digest_path(artifact_path)
        if record.get("case_artifact_digest") != artifact_digest:
            raise ValueError(f"{test_id} case artifact digest drifted")

        record["path"] = EVIDENCE_PATH
        proof_records.append(record)
        proof_record_digests[expected_record_id] = _digest_path(record_path)
        case_artifact_digests[test_id] = artifact_digest
        passed_case_count += len(expected_case_ids)

    if (
        passed_case_count != EXPECTED_CASE_COUNT
        or campaign.get("case_count") != EXPECTED_CASE_COUNT
    ):
        raise ValueError("execution passed-case population drifted")

    evidence_source = replacements[EVIDENCE_PATH]
    for record in proof_records:
        evidence_source += (
            "" if evidence_source.endswith("\n\n") else "\n"
        ) + render_evidence_record(record)
    finalized_evidence = tomllib.loads(evidence_source)
    finalized_ids = [
        _string(row, "id", "finalized evidence")
        for row in _records(
            finalized_evidence.get("evidence"),
            "finalized evidence records",
        )
    ]
    proof_record_ids = sorted(record["id"] for record in proof_records)
    if any(finalized_ids.count(record_id) != 1 for record_id in proof_record_ids):
        raise ValueError("finalized evidence proof population drifted")

    inventory_path = (
        case_artifact_dir.parent
        / "source-closure-assertion-inventories.json"
    )
    inventory_document = _json(inventory_path)
    (
        refreshed_catalog_source,
        refreshed_decisions_source,
        refreshed_decision_test_ids,
    ) = refresh_source_closure_reviews(
        catalog_source=replacements[CATALOG_PATH],
        decisions_source=replacements.get(
            DECISIONS_PATH, (root / DECISIONS_PATH).read_text()
        ),
        inventory_document=inventory_document,
        test_ids=_strings(
            plan.get("source_closure_refresh_test_ids"),
            "prepared source-closure refresh test IDs",
        ),
        source_commit=source_commit,
        expected_count=EXPECTED_SOURCE_CLOSURE_REFRESH_TEST_COUNT,
    )
    if (
        len(refreshed_decision_test_ids)
        != EXPECTED_SOURCE_CLOSURE_REFRESH_TEST_COUNT
    ):
        raise ValueError("finalized source-closure review population drifted")

    lock = json.loads(replacements[LOCK_PATH])
    lock["state"] = "executed"
    lock["source_commit"] = source_commit
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
    lock["execution"] = {
        "campaign_digest": "sha256:" + _canonical_digest(campaign),
        "case_artifact_digests": case_artifact_digests,
        "execution_test_count": len(execution_test_ids),
        "generated_authority_equality_count": len(equality_results),
        "generated_authority_equality_digest": (
            "sha256:" + _canonical_digest(generated_authority_equality)
        ),
        "passed_case_count": passed_case_count,
        "proof_record_digests": proof_record_digests,
        "proof_record_ids": proof_record_ids,
        "rerun_label": "P18_CASE_REFRESH",
    }
    replacements[CATALOG_PATH] = refreshed_catalog_source
    replacements[DECISIONS_PATH] = refreshed_decisions_source
    replacements[EVIDENCE_PATH] = evidence_source
    replacements[LOCK_PATH] = (
        json.dumps(lock, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    )
    return {
        **lock,
        "generated_authority_equality_count": len(equality_results),
        "passed_case_count": passed_case_count,
        "proof_record_count": len(proof_record_ids),
        "proof_record_ids": proof_record_ids,
        "post_refresh_gap_summary": plan.get("post_refresh_gap_summary"),
        "replacements": replacements,
    }


def _string_replacements(plan: Mapping[str, Any]) -> dict[str, str]:
    raw = plan.get("replacements")
    if not isinstance(raw, Mapping):
        raise ValueError("refresh plan replacements are missing")
    if any(
        not isinstance(path, str) or not isinstance(source, str)
        for path, source in raw.items()
    ):
        raise ValueError("refresh plan replacements must be text paths")
    return {str(path): str(source) for path, source in raw.items()}
