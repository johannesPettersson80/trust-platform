"""Bind current case-backed behavior demands to their already executed tests."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import tomllib
from collections import defaultdict
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .case_backed_behavior_proof import (
    proof_digest_updates,
    replace_table_record_fields,
    select_reused_evidence,
)
from .execution_contract import invariant_execution_contract_digest
from .frozen_catalog_gaps import analyze_frozen_catalog_gaps
from .mapping_transaction import apply_mapping_transaction
from .proof_contract import proof_contract_digest


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_GAP_REPORT = ROOT / (
    "target/gate-artifacts/verification/p18-remaining-authority-142/"
    "frozen-gaps-after-local.json"
)
CATALOG_PATH = "verification/test-catalog.toml"
EVIDENCE_PATH = "verification/evidence-index.toml"
LOCK_PATH = (
    "verification/p18/review-locks/"
    "case-backed-behavior-101-semantic-lock.json"
)
ASSIGNMENT_DIGEST = (
    "cb4a4f71406292b32a6d4c3aa0697d1a0d216fe21482c11272e465dc1c87fd18"
)
EXPECTED_BEHAVIOR_COUNT = 101
EXPECTED_TEST_COUNT = 37
EXPECTED_INVARIANT_COUNT = 37
EXPECTED_CASE_COUNT = 105
BEHAVIOR_EXPECTATION_FIELDS = (
    "outcome",
    "delta",
    "error_code",
    "no_partial_apply",
    "fault_surface",
    "oracle_ref",
)


def match_behavior_cases(
    *,
    behavior: Mapping[str, Any],
    cases: Sequence[Mapping[str, Any]],
) -> list[str]:
    """Return every case with the exact reviewed partition and expectation."""

    partition = behavior.get("partition")
    case_family = behavior.get("case_family")
    matches: list[str] = []
    for case in cases:
        case_id = case.get("id")
        case_input = case.get("input")
        case_expect = case.get("expect")
        if (
            not isinstance(case_id, str)
            or not case_id
            or not isinstance(case_input, Mapping)
            or not isinstance(case_expect, Mapping)
        ):
            continue
        partition_matches = case_input.get("source_partition") == partition
        if (
            not partition_matches
            and isinstance(partition, Mapping)
            and set(partition) == {"equals"}
            and "source_partition" not in case_input
        ):
            partition_matches = case_input.get("scenario") == partition["equals"]
        if not partition_matches:
            continue
        if (
            isinstance(case_family, str)
            and case_family
            and case.get("family") != case_family
        ):
            continue
        if any(
            case_expect.get(field) != behavior.get(field)
            for field in BEHAVIOR_EXPECTATION_FIELDS
            if field in behavior or field in case_expect
        ):
            continue
        matches.append(case_id)
    if len(matches) != len(set(matches)):
        raise ValueError("case table duplicates an exact behavior case ID")
    return sorted(matches)


def build_case_backed_plan(
    *,
    root: Path = ROOT,
    gap_report: Path = DEFAULT_GAP_REPORT,
) -> dict[str, Any]:
    root = root.resolve()
    report = _json(gap_report)
    catalog_source = (root / CATALOG_PATH).read_text()
    catalog_document = tomllib.loads(catalog_source)
    catalog_tests = _records(catalog_document.get("tests"), "catalog tests")
    evidence_source = (root / EVIDENCE_PATH).read_text()
    evidence_document = tomllib.loads(evidence_source)
    evidence_records = _records(
        evidence_document.get("evidence"), "evidence records"
    )
    lock_path = root / LOCK_PATH
    if lock_path.is_file():
        return applied_plan_from_documents(
            root=root,
            report=report,
            catalog_source=catalog_source,
            evidence_source=evidence_source,
            lock_source=lock_path.read_text(),
        )
    invariants = _load_invariants(root)
    tests_by_invariant: dict[str, list[Mapping[str, Any]]] = defaultdict(list)
    tests_by_id: dict[str, Mapping[str, Any]] = {}
    mapped_behavior_refs: set[str] = set()
    for test in catalog_tests:
        test_id = _string(test, "id", "catalog test")
        if test_id in tests_by_id:
            raise ValueError(f"catalog duplicates {test_id}")
        tests_by_id[test_id] = test
        for invariant_id in _strings(test.get("invariants", []), "test invariants"):
            tests_by_invariant[invariant_id].append(test)
        mapped_behavior_refs.update(
            _strings(
                test.get("mapping_behavior_refs", []),
                "mapping behavior refs",
            )
        )

    demands = _records(
        report.get("confirmed_missing_tests"),
        "confirmed missing tests",
    )
    assignments: list[dict[str, Any]] = []
    rejected: dict[str, set[str]] = defaultdict(set)
    for demand in demands:
        behavior_ref = _string(demand, "behavior_ref", "behavior demand")
        if behavior_ref in mapped_behavior_refs:
            continue
        invariant_id = _string(demand, "invariant_id", "behavior demand")
        invariant_path, invariant = invariants[invariant_id]
        behavior_index = _behavior_index(behavior_ref, invariant_id)
        behaviors = _records(invariant.get("behavior"), f"{invariant_id} behaviors")
        if behavior_index >= len(behaviors):
            raise ValueError(f"{behavior_ref} is outside its invariant")
        candidates, reasons = _case_test_candidates(
            root=root,
            invariant_id=invariant_id,
            invariant=invariant,
            tests=tests_by_invariant.get(invariant_id, []),
        )
        if not candidates:
            if reasons:
                rejected[invariant_id].update(reasons)
            continue
        case_paths = {candidate[1].as_posix() for candidate in candidates}
        if len(case_paths) != 1:
            raise ValueError(
                f"{invariant_id} has multiple current case authorities"
            )
        test, case_path, case_document = sorted(
            candidates,
            key=lambda candidate: (
                str(candidate[0].get("id", "")).startswith("TEST_CASE_TABLE_"),
                str(candidate[0].get("id", "")),
            ),
        )[0]
        case_ids = match_behavior_cases(
            behavior=behaviors[behavior_index],
            cases=_records(case_document.get("case"), f"{invariant_id} cases"),
        )
        if not case_ids:
            rejected[invariant_id].add(f"no_exact_case:{behavior_ref}")
            continue
        test_id = _string(test, "id", "case-backed catalog test")
        assignments.append(
            {
                "behavior_ref": behavior_ref,
                "invariant_id": invariant_id,
                "invariant_path": invariant_path.as_posix(),
                "invariant_contract_digest": invariant_execution_contract_digest(
                    invariant
                ),
                "catalog_test_id": test_id,
                "catalog_discovery_id": test.get("discovery_id"),
                "case_file": case_path.as_posix(),
                "case_file_digest": _digest(root / case_path),
                "case_ids": case_ids,
            }
        )

    assignments.sort(
        key=lambda row: (row["behavior_ref"], row["catalog_test_id"])
    )
    assignment_payload = [
        {
            "behavior_ref": row["behavior_ref"],
            "catalog_test_id": row["catalog_test_id"],
            "case_ids": row["case_ids"],
        }
        for row in assignments
    ]
    assignment_digest = _canonical_digest(assignment_payload).removeprefix(
        "sha256:"
    )
    test_ids = {str(row["catalog_test_id"]) for row in assignments}
    invariant_ids = {str(row["invariant_id"]) for row in assignments}
    case_count = sum(len(row["case_ids"]) for row in assignments)
    _require_frozen_population(
        behavior_count=len(assignments),
        test_count=len(test_ids),
        invariant_count=len(invariant_ids),
        case_count=case_count,
        assignment_digest=assignment_digest,
    )

    refs_by_test: dict[str, set[str]] = defaultdict(set)
    cases_by_test: dict[str, set[str]] = defaultdict(set)
    assignment_by_test: dict[str, dict[str, Any]] = {}
    for assignment in assignments:
        test_id = str(assignment["catalog_test_id"])
        refs_by_test[test_id].add(str(assignment["behavior_ref"]))
        cases_by_test[test_id].update(
            str(case_id) for case_id in assignment["case_ids"]
        )
        assignment_by_test[test_id] = assignment
    catalog_updates = {}
    for test_id in sorted(test_ids):
        existing_refs = set(
            _strings(
                tests_by_id[test_id].get("mapping_behavior_refs", []),
                f"{test_id} mapping behavior refs",
            )
        )
        catalog_updates[test_id] = {
            "mapping_behavior_refs": sorted(
                existing_refs | refs_by_test[test_id]
            )
        }
    updated_catalog_source = replace_table_record_fields(
        catalog_source,
        table="tests",
        updates=catalog_updates,
    )
    updated_catalog_tests = {
        _string(test, "id", "updated catalog test"): test
        for test in _records(
            tomllib.loads(updated_catalog_source).get("tests"),
            "updated catalog tests",
        )
    }

    reused_evidence = select_reused_evidence(
        evidence_records=evidence_records,
        assignment_by_test=assignment_by_test,
        cases_by_test=cases_by_test,
    )
    proof_updates = proof_digest_updates(
        evidence_records=evidence_records,
        selected_test_ids=test_ids,
        tests_by_id=updated_catalog_tests,
        invariants={key: value[1] for key, value in invariants.items()},
    )
    updated_evidence_source = replace_table_record_fields(
        evidence_source,
        table="evidence",
        updates={
            row["evidence_id"]: {
                "proof_contract_digest": row["proof_contract_digest"]
            }
            for row in proof_updates
        },
    )

    rejected_rows = [
        {
            "invariant_id": invariant_id,
            "reasons": sorted(reasons),
        }
        for invariant_id, reasons in sorted(rejected.items())
    ]
    lock = {
        "schema_version": 1,
        "id": "P18_CASE_BACKED_BEHAVIOR_101_SEMANTIC_LOCK",
        "source_commit": _git(root, "rev-parse", "HEAD"),
        "behavior_count": len(assignments),
        "catalog_test_count": len(test_ids),
        "invariant_count": len(invariant_ids),
        "case_count": case_count,
        "assignment_digest": assignment_digest,
        "assignments": assignments,
        "reused_execution_evidence": reused_evidence,
        "proof_digest_updates": proof_updates,
        "rejected_invariants": rejected_rows,
    }
    lock_source = json.dumps(
        lock, indent=2, sort_keys=True, ensure_ascii=False
    ) + "\n"
    replacements = {
        CATALOG_PATH: updated_catalog_source,
        EVIDENCE_PATH: updated_evidence_source,
        LOCK_PATH: lock_source,
    }
    plan = {
        **lock,
        "proof_digest_update_count": len(proof_updates),
        "replacements": replacements,
    }
    failures = _validate_replacement_documents(
        root=root,
        report=report,
        plan=plan,
    )
    if failures:
        raise ValueError(
            "case-backed replacement validation failed: "
            + "; ".join(failures)
        )
    return plan


def applied_plan_from_documents(
    *,
    root: Path,
    report: Mapping[str, Any],
    catalog_source: str,
    evidence_source: str,
    lock_source: str,
) -> dict[str, Any]:
    """Reconstruct and validate the no-op plan after its atomic application."""

    lock = json.loads(lock_source)
    if not isinstance(lock, dict):
        raise ValueError("case-backed semantic lock must be an object")
    _require_frozen_population(
        behavior_count=int(lock.get("behavior_count", -1)),
        test_count=int(lock.get("catalog_test_count", -1)),
        invariant_count=int(lock.get("invariant_count", -1)),
        case_count=int(lock.get("case_count", -1)),
        assignment_digest=str(lock.get("assignment_digest", "")),
    )
    assignments = _records(
        lock.get("assignments"), "semantic-lock assignments"
    )
    if len(assignments) != EXPECTED_BEHAVIOR_COUNT:
        raise ValueError("semantic lock assignment count drifted")
    catalog = tomllib.loads(catalog_source)
    catalog_tests = {
        _string(test, "id", "applied catalog test"): test
        for test in _records(catalog.get("tests"), "applied catalog tests")
    }
    evidence = tomllib.loads(evidence_source)
    evidence_records = _records(
        evidence.get("evidence"), "applied evidence records"
    )
    selected_test_ids = {
        _string(assignment, "catalog_test_id", "semantic-lock assignment")
        for assignment in assignments
    }
    invariants = {
        invariant_id: record
        for invariant_id, (_, record) in _load_invariants(root).items()
    }
    proof_updates = proof_digest_updates(
        evidence_records=evidence_records,
        selected_test_ids=selected_test_ids,
        tests_by_id=catalog_tests,
        invariants=invariants,
    )
    if len(proof_updates) != 72:
        raise ValueError("semantic lock proof digest update count drifted")
    evidence_source = replace_table_record_fields(
        evidence_source,
        table="evidence",
        updates={
            row["evidence_id"]: {
                "proof_contract_digest": row["proof_contract_digest"]
            }
            for row in proof_updates
        },
    )
    lock["proof_digest_updates"] = proof_updates
    lock_source = json.dumps(
        lock, indent=2, sort_keys=True, ensure_ascii=False
    ) + "\n"
    reused = _records(
        lock.get("reused_execution_evidence"),
        "semantic-lock execution evidence",
    )
    if len(reused) != EXPECTED_TEST_COUNT:
        raise ValueError("semantic lock execution evidence count drifted")
    replacements = {
        CATALOG_PATH: catalog_source,
        EVIDENCE_PATH: evidence_source,
        LOCK_PATH: lock_source,
    }
    plan = {
        **lock,
        "proof_digest_update_count": len(proof_updates),
        "replacements": replacements,
    }
    failures = _validate_replacement_documents(
        root=root.resolve(),
        report=report,
        plan=plan,
        allow_downstream_closure=True,
    )
    if failures:
        raise ValueError(
            "applied case-backed transaction is invalid: "
            + "; ".join(failures)
        )
    return plan


def apply_case_backed_plan(*, root: Path, plan: Mapping[str, Any]) -> None:
    replacements = {
        path: content.encode()
        for path, content in plan["replacements"].items()
    }
    expected_originals = {
        path: (root / path).read_bytes() if (root / path).exists() else None
        for path in replacements
    }

    def validate() -> list[str]:
        report = _json(DEFAULT_GAP_REPORT)
        applied_plan = dict(plan)
        applied_plan["replacements"] = {
            path: (root / path).read_text() for path in replacements
        }
        return _validate_replacement_documents(
            root=root,
            report=report,
            plan=applied_plan,
        )

    apply_mapping_transaction(
        root=root,
        replacements=replacements,
        expected_originals=expected_originals,
        validate=validate,
    )


def _case_test_candidates(
    *,
    root: Path,
    invariant_id: str,
    invariant: Mapping[str, Any],
    tests: Sequence[Mapping[str, Any]],
) -> tuple[
    list[tuple[Mapping[str, Any], Path, Mapping[str, Any]]],
    set[str],
]:
    current_digest = invariant_execution_contract_digest(invariant)
    current: list[tuple[Mapping[str, Any], Path, Mapping[str, Any]]] = []
    reasons: set[str] = set()
    saw_case_file = False
    for test in tests:
        case_file = test.get("case_file")
        if not isinstance(case_file, str) or not case_file:
            continue
        case_path = Path(case_file)
        absolute = root / case_path
        if not absolute.is_file():
            continue
        case_document = _toml(absolute)
        if case_document.get("invariant") != invariant_id:
            continue
        saw_case_file = True
        if case_document.get("source_digest") != current_digest:
            reasons.add("stale_source_digest")
            continue
        actual_case_digest = _digest(absolute)
        if test.get("case_file_digest") != actual_case_digest:
            reasons.add("stale_catalog_case_digest")
            continue
        current.append((test, case_path, case_document))
    if saw_case_file and not current and not reasons:
        reasons.add("no_current_case_authority")
    return current, reasons


def _validate_replacement_documents(
    *,
    root: Path,
    report: Mapping[str, Any],
    plan: Mapping[str, Any],
    allow_downstream_closure: bool = False,
) -> list[str]:
    failures: list[str] = []
    replacements = plan.get("replacements")
    if not isinstance(replacements, Mapping):
        return ["plan replacements are missing"]
    try:
        catalog = tomllib.loads(str(replacements[CATALOG_PATH]))
        evidence = tomllib.loads(str(replacements[EVIDENCE_PATH]))
        lock = json.loads(str(replacements[LOCK_PATH]))
    except (KeyError, tomllib.TOMLDecodeError, json.JSONDecodeError) as error:
        return [f"replacement parse failed: {error}"]
    mapped_refs = {
        ref
        for test in _records(catalog.get("tests"), "replacement catalog tests")
        for ref in _strings(
            test.get("mapping_behavior_refs", []),
            "replacement mapping refs",
        )
    }
    expected_refs = {
        str(row["behavior_ref"]) for row in plan["assignments"]
    }
    if not expected_refs.issubset(mapped_refs):
        failures.append("replacement catalog lacks an assigned behavior ref")
    if lock.get("assignment_digest") != ASSIGNMENT_DIGEST:
        failures.append("replacement semantic lock digest drifted")

    invariants = _load_invariants(root)
    invariant_records = {key: value[1] for key, value in invariants.items()}
    catalog_tests = {
        _string(test, "id", "replacement catalog test"): test
        for test in _records(catalog.get("tests"), "replacement catalog tests")
    }
    proof_updates = {
        str(row["evidence_id"]): str(row["proof_contract_digest"])
        for row in plan["proof_digest_updates"]
    }
    for row in _records(evidence.get("evidence"), "replacement evidence"):
        evidence_id = row.get("id")
        if evidence_id in proof_updates:
            if row.get("proof_contract_digest") != proof_updates[evidence_id]:
                failures.append(f"{evidence_id} proof digest was not updated")
            linked = _strings(
                row.get("linked_tests", []),
                f"{evidence_id} linked tests",
            )
            selected = set(linked).intersection(catalog_tests)
            if len(selected) == 1:
                expected = proof_contract_digest(
                    test=catalog_tests[next(iter(selected))],
                    invariants=invariant_records,
                )
                if row.get("proof_contract_digest") != expected:
                    failures.append(
                        f"{evidence_id} proof digest does not match replacement"
                    )

    denominator = _toml(root / "verification/test-catalog-denominator.toml")
    analysis = analyze_frozen_catalog_gaps(
        denominator=denominator,
        catalog=catalog,
        invariants=[
            (path.as_posix(), record)
            for path, record in sorted(
                invariants.values(), key=lambda value: value[0].as_posix()
            )
        ],
    )
    before_count = len(
        _records(
            report.get("confirmed_missing_tests"),
            "original confirmed missing tests",
        )
    )
    after_count = analysis["summary"]["confirmed_missing_tests"]
    expected_after = before_count - EXPECTED_BEHAVIOR_COUNT
    if (
        after_count > expected_after
        if allow_downstream_closure
        else after_count != expected_after
    ):
        failures.append(
            f"confirmed missing behavior count is {after_count}, "
            f"expected {'at most ' if allow_downstream_closure else ''}"
            f"{expected_after}"
        )
    if analysis["summary"]["missing_spec_authority_tests"] != 0:
        failures.append("replacement reopened missing specification authority")
    return failures


def _load_invariants(
    root: Path,
) -> dict[str, tuple[Path, dict[str, Any]]]:
    result: dict[str, tuple[Path, dict[str, Any]]] = {}
    for absolute in sorted((root / "verification/invariants").rglob("*.toml")):
        document = _toml(absolute)
        invariant_id = _string(document, "id", absolute.as_posix())
        if invariant_id in result:
            raise ValueError(f"duplicate invariant {invariant_id}")
        result[invariant_id] = (
            absolute.relative_to(root),
            document,
        )
    return result


def _require_frozen_population(
    *,
    behavior_count: int,
    test_count: int,
    invariant_count: int,
    case_count: int,
    assignment_digest: str,
) -> None:
    actual = (
        behavior_count,
        test_count,
        invariant_count,
        case_count,
        assignment_digest,
    )
    expected = (
        EXPECTED_BEHAVIOR_COUNT,
        EXPECTED_TEST_COUNT,
        EXPECTED_INVARIANT_COUNT,
        EXPECTED_CASE_COUNT,
        ASSIGNMENT_DIGEST,
    )
    if actual != expected:
        raise ValueError(
            "case-backed frozen population drifted: "
            f"actual={actual!r}, expected={expected!r}"
        )


def _behavior_index(behavior_ref: str, invariant_id: str) -> int:
    prefix = f"{invariant_id}#behavior:"
    if not behavior_ref.startswith(prefix):
        raise ValueError(f"{behavior_ref} does not belong to {invariant_id}")
    suffix = behavior_ref.removeprefix(prefix)
    if not suffix.isdigit():
        raise ValueError(f"{behavior_ref} has an invalid behavior index")
    return int(suffix)


def _canonical_digest(value: Any) -> str:
    encoded = json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def _digest(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def _toml(path: Path) -> dict[str, Any]:
    document = tomllib.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} is not a TOML document")
    return document


def _json(path: Path) -> dict[str, Any]:
    document = json.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} is not a JSON object")
    return document


def _records(value: Any, label: str) -> list[Mapping[str, Any]]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be an object array")
    if any(not isinstance(row, Mapping) for row in value):
        raise ValueError(f"{label} must contain only objects")
    return list(value)


def _strings(value: Any, label: str) -> list[str]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be a string array")
    result = [item for item in value if isinstance(item, str) and item]
    if len(result) != len(value) or len(result) != len(set(result)):
        raise ValueError(f"{label} must contain distinct non-empty strings")
    return result


def _string(record: Mapping[str, Any], field: str, label: str) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} lacks {field}")
    return value


def _git(root: Path, *args: str) -> str:
    return subprocess.run(
        ["git", *args],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


def _summary(plan: Mapping[str, Any]) -> dict[str, Any]:
    return {
        key: plan[key]
        for key in (
            "id",
            "source_commit",
            "behavior_count",
            "catalog_test_count",
            "invariant_count",
            "case_count",
            "assignment_digest",
            "proof_digest_update_count",
            "assignments",
            "reused_execution_evidence",
            "proof_digest_updates",
            "rejected_invariants",
        )
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--gap-report", type=Path, default=DEFAULT_GAP_REPORT)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args(argv)
    try:
        plan = build_case_backed_plan(
            root=args.root,
            gap_report=args.gap_report,
        )
        if args.apply:
            apply_case_backed_plan(root=args.root.resolve(), plan=plan)
        rendered = json.dumps(
            _summary(plan), indent=2, sort_keys=True, ensure_ascii=False
        ) + "\n"
        if args.out is None:
            print(rendered, end="")
        else:
            args.out.parent.mkdir(parents=True, exist_ok=True)
            args.out.write_text(rendered)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"case-backed behavior assignment error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
