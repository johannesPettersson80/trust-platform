"""Compile and apply the Phase 18 runtime-cloud mapping transaction."""

from __future__ import annotations

import argparse
import hashlib
import json
import tomllib
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .case_backed_behavior_proof import replace_table_record_fields
from .frozen_catalog_gaps import analyze_frozen_catalog_gaps
from .mapping_catalog_updates import (
    append_invariant_tests,
    reconcile_invariant_catalog_state,
)
from .mapping_transaction import apply_mapping_transaction
from .runtime_cloud_behavior_campaign import (
    DEFAULT_GAP_REPORT,
    DEFAULT_SCAN_REPORT,
)
from .runtime_cloud_behavior_proof import build_assignment_plan
from .runtime_other_behavior_mapping_support import (
    inventories,
    json_document,
    load_invariant_documents,
    load_invariants,
    records,
    records_by_id,
    string,
    strings,
    toml_document,
    toml_source,
)


CATALOG_PATH = Path("verification/test-catalog.toml")
DENOMINATOR_PATH = Path("verification/test-catalog-denominator.toml")
LOCK_PATH = Path(
    "verification/p18/review-locks/"
    "runtime-cloud-23-semantic-lock.json"
)
ARTIFACT_ROOT = Path(
    "target/gate-artifacts/verification/p18-runtime-cloud-23"
)
CAMPAIGN_PLAN_PATH = ARTIFACT_ROOT / "campaign-plan.json"
CAMPAIGN_PATH = ARTIFACT_ROOT / "campaign.json"
ASSERTIONS_PATH = ARTIFACT_ROOT / "assertions.json"
CAMPAIGN_SOURCE_COMMIT = "dce893b021f95d2156ce882c1d43c9c0a19ca7df"
ASSERTION_SOURCE_COMMIT = "dce893b021f95d2156ce882c1d43c9c0a19ca7df"
REVIEWED_DATE = "2026-07-29"


@dataclass(frozen=True)
class RuntimeCloudMappingCompilation:
    summary: dict[str, int]
    replacements: dict[str, bytes]


def compile_runtime_cloud_mapping(
    *,
    root: Path,
) -> RuntimeCloudMappingCompilation:
    """Return the complete, unapplied runtime-cloud transaction."""

    root = root.resolve()
    if (root / LOCK_PATH).is_file():
        return _reconstruct_applied(root)

    assignment = build_assignment_plan(
        root=root,
        gap_report=root / DEFAULT_GAP_REPORT,
        scan_report=root / DEFAULT_SCAN_REPORT,
    )
    campaign_plan = json_document(root / CAMPAIGN_PLAN_PATH)
    campaign = json_document(root / CAMPAIGN_PATH)
    assertion_inventories = inventories(root / ASSERTIONS_PATH)
    _validate_frozen_inputs(
        assignment=assignment,
        campaign_plan=campaign_plan,
        campaign=campaign,
        assertion_inventories=assertion_inventories,
    )

    catalog_source = (root / CATALOG_PATH).read_text()
    catalog = toml_source(catalog_source, CATALOG_PATH.as_posix())
    catalog_by_id = records_by_id(catalog.get("tests"), "catalog tests")
    assignments = [
        dict(row)
        for row in records(assignment.get("assignments"), "assignments")
    ]
    catalog_updates: dict[str, dict[str, Any]] = {}
    test_links: dict[str, set[str]] = {}
    assertion_reviews = []
    for selected in assignments:
        test_id = string(selected, "catalog_test_id", "assignment")
        test = catalog_by_id.get(test_id)
        if test is None:
            raise ValueError(f"runtime-cloud catalog lacks {test_id}")
        if test.get("mapping_decision_id") or test.get("mapping_proposal_id"):
            raise ValueError(
                f"{test_id} unexpectedly requires provenance refresh"
            )
        selected_invariants = strings(
            selected.get("invariant_ids"),
            f"{test_id} invariant IDs",
        )
        selected_refs = strings(
            selected.get("behavior_refs"),
            f"{test_id} behavior refs",
        )
        if not set(selected_invariants) <= set(
            strings(test.get("invariants"), f"{test_id} catalog invariants")
        ):
            raise ValueError(f"{test_id} is not linked to selected authority")
        current_refs = strings(
            test.get("mapping_behavior_refs", []),
            f"{test_id} current behavior refs",
        )
        catalog_updates[test_id] = {
            "mapping_behavior_refs": sorted(
                set(current_refs) | set(selected_refs)
            )
        }
        for invariant_id in selected_invariants:
            test_links.setdefault(invariant_id, set()).add(test_id)
        discovery_id = string(selected, "discovery_id", test_id)
        assertion_reviews.append(
            _assertion_review(
                test_id=test_id,
                discovery_id=discovery_id,
                behavior_refs=selected_refs,
                inventory=assertion_inventories[discovery_id],
            )
        )

    catalog_source = replace_table_record_fields(
        catalog_source,
        table="tests",
        updates=catalog_updates,
    )
    catalog = toml_source(catalog_source, "runtime-cloud refined catalog")
    replacements: dict[str, bytes] = {
        CATALOG_PATH.as_posix(): catalog_source.encode()
    }

    invariant_sources, invariant_documents = load_invariants(root)
    touched_invariants = sorted(test_links)
    if len(touched_invariants) != 7:
        raise ValueError(
            "runtime-cloud authority population drifted: "
            f"{len(touched_invariants)}"
        )
    final_invariant_sources: dict[str, str] = {}
    for invariant_id in touched_invariants:
        path, source = invariant_sources[invariant_id]
        document = invariant_documents[invariant_id]
        current_tests = set(
            strings(document.get("tests"), f"{invariant_id} tests")
        )
        additions = sorted(test_links[invariant_id] - current_tests)
        if additions:
            source = append_invariant_tests(source, additions)
        source = reconcile_invariant_catalog_state(
            source,
            invariant_id=invariant_id,
            catalog_source=catalog_source,
        )
        final_invariant_sources[invariant_id] = source
        if source.encode() != (root / path).read_bytes():
            replacements[path] = source.encode()

    denominator = toml_document(root / DENOMINATOR_PATH)
    all_invariants = []
    for path, document in load_invariant_documents(root):
        invariant_id = string(document, "id", path)
        source = final_invariant_sources.get(invariant_id)
        all_invariants.append(
            (
                path,
                toml_source(source, invariant_id)
                if source is not None
                else document,
            )
        )
    gap_analysis = analyze_frozen_catalog_gaps(
        denominator=denominator,
        catalog=catalog,
        invariants=all_invariants,
    )
    confirmed_missing = int(
        gap_analysis["summary"]["confirmed_missing_tests"]
    )
    if confirmed_missing != 67:
        raise ValueError(
            f"runtime-cloud transaction left {confirmed_missing} behaviors"
        )
    if int(gap_analysis["summary"]["missing_spec_authority_tests"]) != 0:
        raise ValueError("runtime-cloud transaction reopened authority debt")

    summary = {
        "behavior_demand_count": 23,
        "campaign_test_count": 52,
        "existing_catalog_test_count": 52,
        "confirmed_missing_test_count": confirmed_missing,
        "touched_invariant_count": len(touched_invariants),
    }
    lock = {
        "schema_version": 1,
        "id": "RUNTIME_CLOUD_23_SEMANTIC_LOCK",
        "reviewed_date": REVIEWED_DATE,
        "campaign_source_commit": CAMPAIGN_SOURCE_COMMIT,
        "assertion_source_commit": ASSERTION_SOURCE_COMMIT,
        "summary": summary,
        "assignments": assignments,
        "assertion_reviews": sorted(
            assertion_reviews,
            key=lambda row: row["test_id"],
        ),
        "touched_invariant_paths": sorted(
            invariant_sources[invariant_id][0]
            for invariant_id in touched_invariants
        ),
        "evidence_digests": {
            path.as_posix(): _digest(root / path)
            for path in (
                CAMPAIGN_PLAN_PATH,
                CAMPAIGN_PATH,
                ASSERTIONS_PATH,
            )
        },
    }
    replacements[LOCK_PATH.as_posix()] = (
        json.dumps(lock, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    ).encode()
    return RuntimeCloudMappingCompilation(
        summary=summary,
        replacements=dict(sorted(replacements.items())),
    )


def apply_runtime_cloud_mapping(
    *,
    root: Path,
) -> RuntimeCloudMappingCompilation:
    """Apply the reviewed transaction atomically and validate its equations."""

    root = root.resolve()
    compilation = compile_runtime_cloud_mapping(root=root)
    if (root / LOCK_PATH).is_file():
        return compilation
    expected_originals = {
        path: (root / path).read_bytes() if (root / path).exists() else None
        for path in compilation.replacements
    }
    apply_mapping_transaction(
        root=root,
        replacements=compilation.replacements,
        expected_originals=expected_originals,
        validate=lambda: _validate_applied(root),
    )
    return _reconstruct_applied(root)


def _validate_frozen_inputs(
    *,
    assignment: Mapping[str, Any],
    campaign_plan: Mapping[str, Any],
    campaign: Mapping[str, Any],
    assertion_inventories: Mapping[str, Mapping[str, Any]],
) -> None:
    expected = {
        "demand_count": 23,
        "assigned_test_count": 52,
        "existing_catalog_test_count": 52,
        "new_catalog_test_count": 0,
        "assignment_link_count": 53,
        "expected_post_transaction_missing_tests": 67,
    }
    for field, value in expected.items():
        if assignment.get(field) != value:
            raise ValueError(f"runtime-cloud assignment {field} drifted")
    if assignment.get("unresolved") != []:
        raise ValueError("runtime-cloud assignment contains unresolved routes")
    if (
        campaign_plan.get("basis")
        != "p18_runtime_cloud_exact_behavior_assignment_23"
        or campaign_plan.get("behavior_count") != 23
        or campaign_plan.get("test_count") != 52
        or campaign_plan.get("group_count") != 4
        or campaign.get("source_commit") != CAMPAIGN_SOURCE_COMMIT
        or campaign.get("test_count") != 52
        or campaign.get("passed_count") != 52
        or campaign.get("all_passed") is not True
        or campaign.get("failures") != []
    ):
        raise ValueError("runtime-cloud campaign is not the exact green set")
    selected_ids = {
        string(row, "discovery_id", "runtime-cloud assignment")
        for row in records(assignment.get("assignments"), "assignments")
    }
    if set(assertion_inventories) != selected_ids:
        raise ValueError("runtime-cloud assertion inventory population drifted")


def _assertion_review(
    *,
    test_id: str,
    discovery_id: str,
    behavior_refs: list[str],
    inventory: Mapping[str, Any],
) -> dict[str, Any]:
    assertion_rows = records(
        inventory.get("assertions"),
        f"{test_id} assertions",
    )
    oracle_assertion_ids = [
        string(row, "assertion_id", f"{test_id} assertion")
        for row in assertion_rows
        if (
            str(row.get("kind", "")).startswith("macro:assert")
            or row.get("kind") == "method:expect_err"
        )
    ]
    if not oracle_assertion_ids:
        raise ValueError(f"{test_id} has no direct reviewed assertion oracle")
    helpers = strings(
        inventory.get("unresolved_assertion_helpers", []),
        f"{test_id} unresolved assertion helpers",
    )
    return {
        "test_id": test_id,
        "discovery_id": discovery_id,
        "behavior_refs": behavior_refs,
        "inventory_digest": string(
            inventory,
            "inventory_digest",
            f"{test_id} inventory",
        ),
        "oracle_assertion_ids": sorted(oracle_assertion_ids),
        "unresolved_assertion_helpers": helpers,
    }


def _reconstruct_applied(root: Path) -> RuntimeCloudMappingCompilation:
    failures = _validate_applied(root)
    if failures:
        raise ValueError("; ".join(failures))
    lock = json_document(root / LOCK_PATH)
    raw_summary = lock.get("summary")
    if not isinstance(raw_summary, Mapping):
        raise ValueError("runtime-cloud semantic lock lacks summary")
    summary = {
        str(key): int(value)
        for key, value in raw_summary.items()
        if isinstance(value, int) and not isinstance(value, bool)
    }
    if len(summary) != len(raw_summary):
        raise ValueError("runtime-cloud semantic lock summary is invalid")
    paths = {
        CATALOG_PATH.as_posix(),
        LOCK_PATH.as_posix(),
        *strings(
            lock.get("touched_invariant_paths"),
            "touched invariant paths",
        ),
    }
    return RuntimeCloudMappingCompilation(
        summary=summary,
        replacements={
            path: (root / path).read_bytes() for path in sorted(paths)
        },
    )


def _validate_applied(root: Path) -> list[str]:
    failures: list[str] = []
    try:
        lock = json_document(root / LOCK_PATH)
        catalog = toml_document(root / CATALOG_PATH)
        denominator = toml_document(root / DENOMINATOR_PATH)
        tests = records_by_id(catalog.get("tests"), "catalog tests")
        assignments = records(lock.get("assignments"), "locked assignments")
        for assignment in assignments:
            test_id = string(assignment, "catalog_test_id", "assignment")
            test = tests.get(test_id)
            if test is None:
                failures.append(f"{test_id} is absent from catalog")
                continue
            required = set(
                strings(
                    assignment.get("behavior_refs"),
                    f"{test_id} behavior refs",
                )
            )
            actual = set(
                strings(
                    test.get("mapping_behavior_refs"),
                    f"{test_id} catalog behavior refs",
                )
            )
            if not required <= actual:
                failures.append(f"{test_id} lacks locked behavior refs")
        gaps = analyze_frozen_catalog_gaps(
            denominator=denominator,
            catalog=catalog,
            invariants=load_invariant_documents(root),
        )
        if gaps["summary"]["confirmed_missing_tests"] > 67:
            failures.append("runtime-cloud residual behavior count exceeds 67")
        if gaps["summary"]["missing_spec_authority_tests"] != 0:
            failures.append("runtime-cloud reopened test authority debt")
    except (
        OSError,
        ValueError,
        KeyError,
        tomllib.TOMLDecodeError,
        json.JSONDecodeError,
    ) as error:
        failures.append(str(error))
    return sorted(set(failures))


def _digest(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args(argv)
    root = args.root.resolve()
    try:
        compilation = (
            apply_runtime_cloud_mapping(root=root)
            if args.apply
            else compile_runtime_cloud_mapping(root=root)
        )
    except (
        OSError,
        ValueError,
        KeyError,
        json.JSONDecodeError,
        tomllib.TOMLDecodeError,
    ) as error:
        print(f"runtime-cloud mapping transaction failed: {error}")
        return 2
    verb = "applied" if args.apply else "ready"
    print(
        f"runtime-cloud mapping transaction {verb}: "
        f"{compilation.summary['campaign_test_count']} existing tests, "
        f"{compilation.summary['confirmed_missing_test_count']} residual"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
