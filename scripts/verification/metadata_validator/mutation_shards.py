"""Connect focused mutation contracts and reports to committed metadata."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Callable

from .constants import ROOT
from .mutation_contracts import (
    MutationContract,
    MutationContractError,
    MutationSpec,
    load_mutation_contract,
    mutation_contract_from_record,
    safe_workspace_path,
    sha256_file,
    validate_mutation_test_record,
)
from .mutation_reports import has_infrastructure_failure, validate_mutation_report


Fail = Callable[[Path, str], None]


def validate_committed_mutation_metadata(
    *,
    fail: Fail,
    tests: dict[str, dict[str, Any]],
    evidence: dict[str, dict[str, Any]],
    root: Path = ROOT,
) -> None:
    contracts: dict[str, MutationContract] = {}
    for record in tests.values():
        if record.get("test_class") != "mutation":
            continue
        path = record.get("_path", root / "verification/test-catalog.toml")
        try:
            contract = mutation_contract_from_record(record, root=root)
        except MutationContractError as exc:
            for message in str(exc).split("; "):
                fail(path, message)
            continue
        contracts[contract.test_id] = contract

    for record in evidence.values():
        report_path_text = record.get("mutation_report_path")
        if not report_path_text:
            continue
        path = record.get("_path", root / "verification/evidence-index.toml")
        test_id = record.get("mutation_test_id")
        contract = contracts.get(test_id)
        if not contract:
            fail(path, f"{record.get('id')} references unknown mutation_test_id {test_id!r}")
            continue
        if record.get("linked_tests") != [test_id]:
            fail(path, f"{record.get('id')} mutation evidence must link exactly {test_id}")
        report_path = safe_workspace_path(root, report_path_text)
        if report_path is None or not report_path.is_file():
            fail(path, f"{record.get('id')} mutation report does not exist: {report_path_text}")
            continue
        actual_digest = sha256_file(report_path)
        if record.get("mutation_report_digest") != actual_digest:
            fail(path, f"{record.get('id')} mutation_report_digest mismatch")
        try:
            report = json.loads(report_path.read_text())
        except Exception as exc:
            fail(path, f"{record.get('id')} mutation report JSON failed to parse: {exc}")
            continue
        for message in validate_mutation_report(report, contract):
            fail(path, f"{record.get('id')}: {message}")
        if not isinstance(report, dict):
            continue
        evidence_commit = str(record.get("commit", "")).removeprefix("dirty:")
        if report.get("source_commit") != evidence_commit:
            fail(path, f"{record.get('id')} mutation report commit does not match evidence commit")
