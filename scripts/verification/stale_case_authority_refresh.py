"""Refresh every stale case authority whose complete payload still matches."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import tomllib
from collections import defaultdict
from collections.abc import Mapping, Sequence
from pathlib import Path
from types import SimpleNamespace
from typing import Any

from .case_backed_behavior_assignments import (
    CATALOG_PATH,
    EVIDENCE_PATH,
    _load_invariants,
    match_behavior_cases,
)
from .case_backed_behavior_proof import replace_table_record_fields
from .case_generator import (
    CaseGenerationError,
    generate_case_file as generate_case_file_v1,
    render_toml as render_case_toml,
)
from .case_generator_v2 import generate_case_file as generate_case_file_v2
from .execution_contract import invariant_execution_contract_digest
from .frozen_catalog_gaps import analyze_frozen_catalog_gaps
from .mapping_transaction import apply_mapping_transaction


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_GAP_REPORT = ROOT / (
    "target/gate-artifacts/verification/p18-case-backed-behavior-101/"
    "frozen-gaps-after-local.json"
)
PREVIOUS_LOCK_PATH = (
    "verification/p18/review-locks/"
    "case-backed-behavior-101-semantic-lock.json"
)
LOCK_PATH = (
    "verification/p18/review-locks/"
    "stale-case-authority-refresh-54-semantic-lock.json"
)
DECISIONS_PATH = "verification/test-catalog-mapping-decisions.toml"
SOURCE_CLOSURE_INVENTORIES_PATH = (
    "target/gate-artifacts/verification/"
    "p18-stale-case-authority-refresh-54/"
    "source-closure-assertion-inventories.json"
)
ASSIGNMENT_DIGEST = (
    "7647b73d2f76c3347416b085255cf05fb4a9b21f98620406fe7bc4e4c85956d7"
)
EXPECTED_INVARIANT_COUNT = 13
EXPECTED_CASE_COUNT = 82
EXPECTED_GENERATED_AUTHORITY_COUNT = 6
EXPECTED_AFFECTED_CATALOG_TEST_COUNT = 16
EXPECTED_EXECUTION_TEST_COUNT = 13
EXPECTED_RUNNER_SOURCE_COUNT = 12
EXPECTED_BEHAVIOR_COUNT = 54
EXPECTED_ASSIGNED_CASE_COUNT = 59
EXPECTED_RETIRED_EVIDENCE_COUNT = 26
EXPECTED_SOURCE_CLOSURE_REFRESH_TEST_COUNT = 46
EXPECTED_REJECTED_INVARIANTS = {
    "RT_SAFE_STOP_001",
    "VM_SEAM_VALID_001",
}


def build_refresh_plan(
    *,
    root: Path = ROOT,
    gap_report: Path = DEFAULT_GAP_REPORT,
) -> dict[str, Any]:
    """Build the frozen preparation transaction without executing a runner."""

    root = root.resolve()
    report = _json(gap_report)
    catalog_source = (root / CATALOG_PATH).read_text()
    catalog = tomllib.loads(catalog_source)
    catalog_tests = _records(catalog.get("tests"), "catalog tests")
    evidence_source = (root / EVIDENCE_PATH).read_text()
    evidence = tomllib.loads(evidence_source)
    evidence_records = _records(evidence.get("evidence"), "evidence records")
    applied_lock_path = root / LOCK_PATH
    if applied_lock_path.is_file():
        return applied_refresh_plan_from_documents(
            root=root,
            report=report,
            catalog_source=catalog_source,
            evidence_source=evidence_source,
            lock_source=applied_lock_path.read_text(),
        )
    previous_lock = _json(root / PREVIOUS_LOCK_PATH)
    invariants = _load_invariants(root)
    stale_ids = {
        _string(row, "invariant_id", "rejected invariant")
        for row in _records(
            previous_lock.get("rejected_invariants"),
            "rejected invariants",
        )
        if "stale_source_digest"
        in _strings(row.get("reasons", []), "rejection reasons")
    }
    if len(stale_ids) != 15:
        raise ValueError("stale case-authority population drifted")

    tests_by_case: dict[str, list[Mapping[str, Any]]] = defaultdict(list)
    tests_by_id: dict[str, Mapping[str, Any]] = {}
    for test in catalog_tests:
        test_id = _string(test, "id", "catalog test")
        tests_by_id[test_id] = test
        case_file = test.get("case_file")
        if isinstance(case_file, str) and case_file:
            tests_by_case[case_file].append(test)

    demands_by_invariant: dict[str, list[str]] = defaultdict(list)
    for demand in _records(
        report.get("confirmed_missing_tests"),
        "confirmed missing tests",
    ):
        demands_by_invariant[
            _string(demand, "invariant_id", "behavior demand")
        ].append(_string(demand, "behavior_ref", "behavior demand"))

    authorities: list[dict[str, Any]] = []
    rejected_authorities: list[dict[str, Any]] = []
    assignments: list[dict[str, Any]] = []
    catalog_case_updates: dict[str, dict[str, Any]] = {}
    changed_case_sources: dict[str, str] = {}
    changed_runner_sources: dict[str, str] = {}
    affected_catalog_test_ids: set[str] = set()
    execution_test_ids: set[str] = set()
    structural_test_ids: set[str] = set()

    for invariant_id in sorted(stale_ids):
        invariant_path, invariant = invariants[invariant_id]
        candidates = [
            (case_file, tests)
            for case_file, tests in tests_by_case.items()
            if any(invariant_id in test.get("invariants", []) for test in tests)
        ]
        if len(candidates) != 1:
            raise ValueError(
                f"{invariant_id} does not have one stale case authority"
            )
        case_file, case_tests = candidates[0]
        case_path = root / case_file
        case_source = case_path.read_text()
        case_document = tomllib.loads(case_source)
        if case_document.get("invariant") != invariant_id:
            raise ValueError(f"{case_file} invariant binding drifted")
        behaviors = _records(
            invariant.get("behavior"), f"{invariant_id} behaviors"
        )
        case_provenance_kind = case_document.get(
            "case_provenance_kind", "generated_decision_table_v1"
        )
        old_source_digest = _string(
            case_document, "source_digest", f"{case_file} case authority"
        )
        current_source_digest = invariant_execution_contract_digest(invariant)
        if old_source_digest == current_source_digest:
            raise ValueError(f"{case_file} is no longer stale")
        generation_error: str | None = None
        if case_provenance_kind == "generated_decision_table_v1":
            generator = case_document.get("generator")
            try:
                if generator == "gen_cases.py v1":
                    generated = generate_case_file_v1(
                        invariant_id,
                        SimpleNamespace(invariants={invariant_id: invariant}),
                    )
                elif generator == "gen_cases_v2.py v1":
                    generated = generate_case_file_v2(
                        invariant_id,
                        SimpleNamespace(invariants={invariant_id: invariant}),
                    )
                else:
                    raise CaseGenerationError(
                        f"{case_file} names unsupported generator {generator!r}"
                    )
            except CaseGenerationError as error:
                generation_error = str(error)
                updated_case_source = case_source
                updated_case_document = case_document
            else:
                updated_case_source = render_case_toml(generated)
                updated_case_document = tomllib.loads(updated_case_source)
        elif case_provenance_kind == "hand_authored_state_machine_v1":
            updated_case_source = replace_top_level_string(
                case_source,
                field="source_digest",
                value=current_source_digest,
            )
            updated_case_document = tomllib.loads(updated_case_source)
        else:
            raise ValueError(
                f"{case_file} has unsupported provenance {case_provenance_kind!r}"
            )
        cases = _records(
            updated_case_document.get("case"), f"{invariant_id} cases"
        )
        case_to_behavior_refs: dict[str, list[str]] = defaultdict(list)
        for index, behavior in enumerate(behaviors):
            behavior_ref = f"{invariant_id}#behavior:{index}"
            for case_id in match_behavior_cases(
                behavior=behavior,
                cases=cases,
            ):
                case_to_behavior_refs[case_id].append(behavior_ref)
        unmatched_case_ids = sorted(
            _string(case, "id", f"{invariant_id} case")
            for case in cases
            if _string(case, "id", f"{invariant_id} case")
            not in case_to_behavior_refs
        )
        ambiguous_case_ids = sorted(
            case_id
            for case_id, refs in case_to_behavior_refs.items()
            if len(refs) != 1
        )
        if generation_error or unmatched_case_ids or ambiguous_case_ids:
            rejected_authorities.append(
                {
                    "invariant_id": invariant_id,
                    "case_file": case_file,
                    "case_provenance_kind": case_provenance_kind,
                    "generation_error": generation_error,
                    "unmatched_case_ids": unmatched_case_ids,
                    "ambiguous_case_ids": ambiguous_case_ids,
                }
            )
            continue

        historical_commit = _historical_contract_match(
            root=root,
            invariant_path=invariant_path,
            digest=old_source_digest,
        )
        updated_case_digest = _digest_text(updated_case_source)
        changed_case_sources[case_file] = updated_case_source

        sorted_case_tests = sorted(
            case_tests, key=lambda test: _string(test, "id", "case test")
        )
        primary_test = sorted(
            sorted_case_tests,
            key=lambda test: (
                _string(test, "id", "case test").startswith(
                    "TEST_CASE_TABLE_"
                ),
                _string(test, "id", "case test"),
            ),
        )[0]
        primary_test_id = _string(primary_test, "id", "primary case test")
        runner_source_paths: set[str] = set()
        for test in sorted_case_tests:
            test_id = _string(test, "id", "case test")
            affected_catalog_test_ids.add(test_id)
            catalog_case_updates[test_id] = {
                "case_file_digest": updated_case_digest,
            }
            if test.get("subject_kind") == "case_table_artifact":
                structural_test_ids.add(test_id)
                continue
            execution_test_ids.add(test_id)
            runner_source = _string(test, "path", f"{test_id} runner")
            runner_source_paths.add(runner_source)
            source = changed_runner_sources.get(
                runner_source, (root / runner_source).read_text()
            )
            occurrences = source.count(_digest_path(case_path))
            if occurrences != 1:
                raise ValueError(
                    f"{runner_source} contains {occurrences} copies of the "
                    f"stale {case_file} digest"
                )
            changed_runner_sources[runner_source] = source.replace(
                _digest_path(case_path),
                updated_case_digest,
                1,
            )

        behavior_case_ids: dict[str, list[str]] = defaultdict(list)
        for case_id, behavior_refs in case_to_behavior_refs.items():
            behavior_case_ids[behavior_refs[0]].append(case_id)
        for behavior_ref in sorted(demands_by_invariant[invariant_id]):
            case_ids = sorted(behavior_case_ids.get(behavior_ref, []))
            if not case_ids:
                continue
            assignments.append(
                {
                    "behavior_ref": behavior_ref,
                    "test_id": primary_test_id,
                    "case_ids": case_ids,
                }
            )

        authorities.append(
            {
                "invariant_id": invariant_id,
                "invariant_path": invariant_path.as_posix(),
                "historical_source_commit": historical_commit,
                "old_source_digest": old_source_digest,
                "current_source_digest": current_source_digest,
                "case_file": case_file,
                "case_provenance_kind": case_provenance_kind,
                "old_case_file_digest": _digest_path(case_path),
                "current_case_file_digest": updated_case_digest,
                "case_count": len(cases),
                "current_behavior_refs": [
                    f"{invariant_id}#behavior:{index}"
                    for index in range(len(behaviors))
                ],
                "case_bindings": [
                    {
                        "case_id": case_id,
                        "behavior_refs": sorted(behavior_refs),
                    }
                    for case_id, behavior_refs in sorted(
                        case_to_behavior_refs.items()
                    )
                ],
                "catalog_test_ids": [
                    _string(test, "id", "case test")
                    for test in sorted_case_tests
                ],
                "mapping_test_id": primary_test_id,
                "runner_source_paths": sorted(runner_source_paths),
            }
        )

    assignments.sort(
        key=lambda row: (row["behavior_ref"], row["test_id"])
    )
    assignment_digest = _canonical_digest(assignments)
    refs_by_test: dict[str, set[str]] = defaultdict(set)
    for assignment in assignments:
        refs_by_test[assignment["test_id"]].add(
            assignment["behavior_ref"]
        )
    for test_id, refs in refs_by_test.items():
        existing = _strings(
            tests_by_id[test_id].get("mapping_behavior_refs", []),
            f"{test_id} mapping behavior refs",
        )
        catalog_case_updates[test_id]["mapping_behavior_refs"] = sorted(
            set(existing) | refs
        )

    source_closure_refresh_test_ids: list[str] = []
    for test in catalog_tests:
        test_id = _string(test, "id", "catalog test")
        updates: dict[str, Any] = {}
        test_path = test.get("path")
        if (
            isinstance(test_path, str)
            and test_path in changed_runner_sources
            and "mapping_reviewed_source_digest" in test
        ):
            updates["mapping_reviewed_source_digest"] = _digest_text(
                changed_runner_sources[test_path]
            )
        closure = test.get("mapping_assertion_source_digests")
        if isinstance(closure, Mapping):
            refreshed_closure = dict(closure)
            closure_changed = False
            for path in sorted(set(closure).intersection(changed_runner_sources)):
                refreshed_closure[path] = _digest_text(
                    changed_runner_sources[path]
                )
                closure_changed = True
            if closure_changed:
                updates["mapping_assertion_source_digests"] = refreshed_closure
        if updates:
            catalog_case_updates.setdefault(test_id, {}).update(updates)
            source_closure_refresh_test_ids.append(test_id)
    if (
        len(source_closure_refresh_test_ids)
        != EXPECTED_SOURCE_CLOSURE_REFRESH_TEST_COUNT
    ):
        raise ValueError("runner source-closure refresh population drifted")

    _require_frozen_population(
        invariant_count=len(authorities),
        case_count=sum(row["case_count"] for row in authorities),
        generated_authority_count=sum(
            row["case_provenance_kind"]
            == "generated_decision_table_v1"
            for row in authorities
        ),
        affected_catalog_test_count=len(affected_catalog_test_ids),
        execution_test_count=len(execution_test_ids),
        runner_source_count=len(changed_runner_sources),
        behavior_count=len(assignments),
        assigned_case_count=sum(
            len(row["case_ids"]) for row in assignments
        ),
        assignment_digest=assignment_digest,
        rejected_invariants={
            row["invariant_id"] for row in rejected_authorities
        },
    )

    updated_catalog_source = replace_table_record_fields(
        catalog_source,
        table="tests",
        updates=catalog_case_updates,
    )
    retired_current_evidence_ids = sorted(
        _string(row, "id", "evidence record")
        for row in evidence_records
        if row.get("proof_contract_version") == "execution_contract_v2"
        and row.get("proof_contract_binding", "current") == "current"
        and execution_test_ids.intersection(
            _strings(
                row.get("linked_tests", []),
                "evidence linked tests",
            )
        )
    )
    if len(retired_current_evidence_ids) != EXPECTED_RETIRED_EVIDENCE_COUNT:
        raise ValueError("current proof retirement population drifted")
    updated_evidence_source = replace_table_record_fields(
        evidence_source,
        table="evidence",
        updates={
            evidence_id: {"proof_contract_binding": "source_revision"}
            for evidence_id in retired_current_evidence_ids
        },
    )

    updated_catalog = tomllib.loads(updated_catalog_source)
    gap_analysis = analyze_frozen_catalog_gaps(
        denominator=tomllib.loads(
            (root / "verification/test-catalog-denominator.toml").read_text()
        ),
        catalog=updated_catalog,
        invariants=[
            (path.as_posix(), record)
            for path, record in sorted(
                invariants.values(), key=lambda value: value[0].as_posix()
            )
        ],
    )
    summary = gap_analysis["summary"]
    if summary["confirmed_missing_tests"] != 267:
        raise ValueError("stale case refresh did not close exactly 54 demands")
    if summary["missing_spec_authority_tests"] != 0:
        raise ValueError("stale case refresh reopened specification authority")

    lock = {
        "schema_version": 1,
        "id": "P18_STALE_CASE_AUTHORITY_REFRESH_54_SEMANTIC_LOCK",
        "state": "prepared",
        "source_commit": _git(root, "rev-parse", "HEAD"),
        "invariant_count": len(authorities),
        "case_count": sum(row["case_count"] for row in authorities),
        "generated_authority_count": sum(
            row["case_provenance_kind"]
            == "generated_decision_table_v1"
            for row in authorities
        ),
        "affected_catalog_test_count": len(affected_catalog_test_ids),
        "execution_test_count": len(execution_test_ids),
        "runner_source_count": len(changed_runner_sources),
        "behavior_count": len(assignments),
        "assigned_case_count": sum(
            len(row["case_ids"]) for row in assignments
        ),
        "assignment_digest": assignment_digest,
        "authorities": authorities,
        "assignments": assignments,
        "rejected_authorities": rejected_authorities,
        "execution_test_ids": sorted(execution_test_ids),
        "structural_test_ids": sorted(structural_test_ids),
        "retired_current_evidence_ids": retired_current_evidence_ids,
        "source_closure_refresh_test_count": len(
            source_closure_refresh_test_ids
        ),
        "source_closure_refresh_test_ids": sorted(
            source_closure_refresh_test_ids
        ),
    }
    replacements = {
        **changed_case_sources,
        **changed_runner_sources,
        CATALOG_PATH: updated_catalog_source,
        EVIDENCE_PATH: updated_evidence_source,
        LOCK_PATH: json.dumps(
            lock, indent=2, sort_keys=True, ensure_ascii=False
        )
        + "\n",
    }
    return {
        **lock,
        "post_refresh_gap_summary": summary,
        "replacements": replacements,
    }


def replace_top_level_string(
    source: str,
    *,
    field: str,
    value: str,
) -> str:
    """Replace one top-level TOML string and preserve every other byte."""

    pattern = re.compile(rf"(?m)^{re.escape(field)}\s*=.*$")
    matches = list(pattern.finditer(source))
    if len(matches) != 1:
        raise ValueError(f"case authority must contain one {field}")
    rendered = f"{field} = {json.dumps(value)}"
    result = pattern.sub(rendered, source, count=1)
    before = tomllib.loads(source)
    after = tomllib.loads(result)
    expected = dict(before)
    expected[field] = value
    if after != expected:
        raise ValueError(f"case authority {field} replacement drifted")
    return result


def apply_prepared_plan(
    *,
    root: Path,
    plan: Mapping[str, Any],
) -> None:
    """Apply the already validated preparation through the shared transaction."""

    root = root.resolve()
    raw_replacements = plan.get("replacements")
    if not isinstance(raw_replacements, Mapping):
        raise ValueError("refresh plan replacements are missing")
    replacements = {
        str(path): str(content).encode()
        for path, content in raw_replacements.items()
    }
    expected_originals = {
        path: (root / path).read_bytes() if (root / path).exists() else None
        for path in replacements
    }

    def validate() -> list[str]:
        return [
            f"{path} prepared replacement drifted"
            for path, expected in replacements.items()
            if not (root / path).is_file()
            or (root / path).read_bytes() != expected
        ]

    apply_mapping_transaction(
        root=root,
        replacements=replacements,
        expected_originals=expected_originals,
        validate=validate,
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
    """Finalize execution through the dedicated evidence owner."""

    from .stale_case_authority_execution import (
        finalize_refresh_plan as finalize_execution,
    )

    return finalize_execution(
        root=root,
        plan=plan,
        campaign=campaign,
        proof_records_dir=proof_records_dir,
        case_artifact_dir=case_artifact_dir,
        generated_authority_equality=generated_authority_equality,
    )


def applied_refresh_plan_from_documents(
    *,
    root: Path,
    report: Mapping[str, Any],
    catalog_source: str,
    evidence_source: str,
    lock_source: str,
) -> dict[str, Any]:
    """Reconstruct through the dedicated applied-state validator."""

    from .stale_case_authority_applied import (
        applied_refresh_plan_from_documents as reconstruct_applied,
    )

    return reconstruct_applied(
        root=root,
        report=report,
        catalog_source=catalog_source,
        evidence_source=evidence_source,
        lock_source=lock_source,
    )


def _historical_contract_match(
    *,
    root: Path,
    invariant_path: Path,
    digest: str,
) -> str:
    commits = _git(
        root,
        "log",
        "--format=%H",
        "--all",
        "--",
        invariant_path.as_posix(),
    ).splitlines()
    for commit in commits:
        result = subprocess.run(
            [
                "git",
                "show",
                f"{commit}:{invariant_path.as_posix()}",
            ],
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            continue
        try:
            document = tomllib.loads(result.stdout)
        except tomllib.TOMLDecodeError:
            continue
        if invariant_execution_contract_digest(document) == digest:
            return commit
    raise ValueError(
        f"{invariant_path} stale source digest has no historical authority"
    )


def _require_frozen_population(
    *,
    invariant_count: int,
    case_count: int,
    generated_authority_count: int,
    affected_catalog_test_count: int,
    execution_test_count: int,
    runner_source_count: int,
    behavior_count: int,
    assigned_case_count: int,
    assignment_digest: str,
    rejected_invariants: set[str],
) -> None:
    actual = (
        invariant_count,
        case_count,
        generated_authority_count,
        affected_catalog_test_count,
        execution_test_count,
        runner_source_count,
        behavior_count,
        assigned_case_count,
        assignment_digest,
        rejected_invariants,
    )
    expected = (
        EXPECTED_INVARIANT_COUNT,
        EXPECTED_CASE_COUNT,
        EXPECTED_GENERATED_AUTHORITY_COUNT,
        EXPECTED_AFFECTED_CATALOG_TEST_COUNT,
        EXPECTED_EXECUTION_TEST_COUNT,
        EXPECTED_RUNNER_SOURCE_COUNT,
        EXPECTED_BEHAVIOR_COUNT,
        EXPECTED_ASSIGNED_CASE_COUNT,
        ASSIGNMENT_DIGEST,
        EXPECTED_REJECTED_INVARIANTS,
    )
    if actual != expected:
        raise ValueError(
            "stale case-authority frozen population drifted: "
            f"actual={actual!r}, expected={expected!r}"
        )


def _canonical_digest(value: Any) -> str:
    encoded = json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()
    return hashlib.sha256(encoded).hexdigest()


def _digest_path(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def _digest_text(value: str) -> str:
    return "sha256:" + hashlib.sha256(value.encode()).hexdigest()


def _json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text())
    if not isinstance(value, dict):
        raise ValueError(f"{path} is not a JSON object")
    return value


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


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--gap-report", type=Path, default=DEFAULT_GAP_REPORT)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--apply-preparation", action="store_true")
    args = parser.parse_args(argv)
    try:
        plan = build_refresh_plan(
            root=args.root,
            gap_report=args.gap_report,
        )
        if args.apply_preparation:
            apply_prepared_plan(root=args.root, plan=plan)
        rendered = json.dumps(
            {
                key: value
                for key, value in plan.items()
                if key != "replacements"
            },
            indent=2,
            sort_keys=True,
            ensure_ascii=False,
        ) + "\n"
        if args.out is None:
            print(rendered, end="")
        else:
            args.out.parent.mkdir(parents=True, exist_ok=True)
            args.out.write_text(rendered)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"stale case-authority refresh error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
