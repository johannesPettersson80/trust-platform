"""Deterministic fact-level burn-down ledger for Phase 18 row P18-002."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import tomllib
from collections import Counter
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from ..area_routing import classify_changed_path
from ..code_spec_cli import collect_report_inputs
from ..code_spec_live import build_code_spec_live_state
from ..code_spec_provenance import input_closure_digest
from ..test_catalog_denominator import analyze_test_catalog_denominator
from ..test_catalog_json_schema import validate_json_schema_instance
from ..test_catalog_scanner import scan_repository


ARTIFACT_PATH = Path("verification/p18/burn-down-ledger.json")
SCHEMA_PATH = Path("verification/schemas/p18-burn-down-ledger.schema.json")
GENERATOR_PATHS = {
    "scripts/p18_burn_down_ledger_gate.py",
    "scripts/verification/p18_zero_debt/burn_down_ledger.py",
    "scripts/verification/p18_burn_down_ledger_tests.py",
    SCHEMA_PATH.as_posix(),
}
TEST_INPUT_PATHS = {
    "verification/test-catalog-denominator.toml",
    "verification/test-catalog.toml",
    "verification/ignored-tests.toml",
}
P18_INPUT_PATHS = {
    "verification/p18-proof-budgets.toml",
    "docs/internal/testing/checklists/plc-verification-program/phase18-zero-debt-execution-board.md",
}
AREA_ORDER = (
    "runtime_safety",
    "control_security",
    "bytecode_vm",
    "compiler_iec",
    "protocols",
    "editor_safety",
    "hmi_ui",
    "plcopen_devtools",
    "supply_chain_platform",
    "release",
    "verification",
)
CODE_DEPENDENCIES = (
    "VERIF-P18-003",
    "VERIF-P18-005",
    "VERIF-P18-006",
    "VERIF-P18-004",
    "VERIF-P18-007",
)
TEST_DEPENDENCIES = (
    "VERIF-P18-003",
    "VERIF-P18-005",
    "VERIF-P18-007",
)
ALL_BUDGETS = (
    "BUDGET_SEMANTIC_CHAINS",
    "BUDGET_TEST_MAPPING",
    "BUDGET_ASSERTION_REACHABILITY",
    "BUDGET_COVERAGE",
    "BUDGET_MUTATION",
    "BUDGET_FUZZ_PROPERTY",
    "BUDGET_CONCURRENCY_TIME",
    "BUDGET_UNSAFE_FFI",
    "BUDGET_HARDWARE_ENDURANCE",
    "BUDGET_RENDERED_UX",
    "BUDGET_PLATFORM_CLAIMS",
    "BUDGET_RELEASE_EVIDENCE",
)
COMMON_CODE_BLOCKERS = (
    "assertion_reachability_required",
    "adequacy_and_environment_proof_required",
)
COMMON_TEST_BLOCKERS = (
    "exact_code_slice_binding_required",
    "executed_evidence_required",
    "assertion_reachability_required",
    "adequacy_and_environment_proof_required",
)


def canonical_json(payload: Mapping[str, Any]) -> str:
    return json.dumps(payload, sort_keys=True, separators=(",", ":")) + "\n"


def build_payload(
    *,
    code_facts: Mapping[str, Any],
    code_mappings: Mapping[str, Mapping[str, Any]],
    spec_gaps: Mapping[str, Mapping[str, Any]],
    test_facts: Sequence[Any],
    test_reviews: Mapping[str, Mapping[str, Any]],
    test_catalog: Mapping[str, Mapping[str, Any]],
    ignored_tests: Mapping[str, Mapping[str, Any]],
    matrix: Mapping[str, Any],
    source_sha: str,
    input_paths: Sequence[str],
    input_digest: str,
) -> dict[str, Any]:
    """Build the no-semantic-inference overlay over authoritative identities."""

    areas = {
        str(row["id"]): row
        for row in matrix.get("areas", [])
        if isinstance(row, Mapping) and isinstance(row.get("id"), str)
    }
    catalog_by_discovery = {
        str(row["discovery_id"]): row
        for row in test_catalog.values()
        if row.get("subject_kind") == "generated_test"
        and isinstance(row.get("discovery_id"), str)
    }
    ignored_by_discovery = {
        str(row["discovery_id"]): row
        for row in ignored_tests.values()
        if isinstance(row.get("discovery_id"), str)
    }

    code_rows = [
        _code_row(code_facts[code_id], code_mappings[code_id], spec_gaps, areas)
        for code_id in sorted(code_facts)
    ]
    code_rows.sort(key=lambda row: _code_order_key(row))
    for sequence, row in enumerate(code_rows, 1):
        row["sequence"] = sequence

    test_rows = [
        _test_row(
            fact,
            test_reviews[fact.stable_id],
            catalog_by_discovery.get(fact.stable_id),
            ignored_by_discovery.get(fact.stable_id),
            matrix,
            areas,
        )
        for fact in sorted(test_facts, key=lambda item: item.stable_id)
    ]
    test_rows.sort(key=lambda row: _test_order_key(row))
    for sequence, row in enumerate(test_rows, 1):
        row["sequence"] = sequence

    code_by_area = []
    for area in AREA_ORDER:
        rows = [row for row in code_rows if row["area"] == area]
        code_by_area.append(
            {
                "area": area,
                "facts": len(rows),
                "registered_backlog": sum(
                    "registered_backlog" in row["blockers"] for row in rows
                ),
                "spec_gap": sum(row["current_disposition"] == "spec_gap" for row in rows),
            }
        )
    test_area_counts = Counter(row["accounting_area"] for row in test_rows)
    test_by_area = [
        {"area": area, "facts": test_area_counts[area]}
        for area in sorted(test_area_counts)
    ]
    return {
        "schema_version": 1,
        "generator": "p18-burn-down-ledger",
        "generator_version": 1,
        "source_sha": source_sha,
        "input_digest": input_digest,
        "input_paths": sorted(input_paths),
        "status": "frozen_open",
        "historical_baseline": {
            "code_facts": 15229,
            "spec_gap": 15138,
            "test_facts": 4038,
            "reviewed_nonmapping": 3777,
        },
        "slice_policy": {
            "code_grouping": "one_live_code_identity_per_slice",
            "test_grouping": "one_live_test_identity_per_slice",
            "semantic_inference": False,
            "unresolved_test_area_owner": "verification",
            "area_order": list(AREA_ORDER),
            "code_dependency_order": list(CODE_DEPENDENCIES),
            "test_dependency_order": list(TEST_DEPENDENCIES),
        },
        "summary": {
            "code_facts": len(code_rows),
            "test_facts": len(test_rows),
            "code_spec_gap": sum(
                row["current_disposition"] == "spec_gap" for row in code_rows
            ),
            "code_registered_backlog": sum(
                "registered_backlog" in row["blockers"] for row in code_rows
            ),
            "test_reviewed_nonmapping": sum(
                row["current_disposition"] == "reviewed_nonmapping"
                for row in test_rows
            ),
            "test_ignored_or_conditional": sum(
                row["ignore_state"] != "not_ignored" for row in test_rows
            ),
            "unresolved_test_area": sum(
                row["accounting_area"] == "unresolved" for row in test_rows
            ),
            "unowned": 0,
            "duplicate_identities": 0,
            "omitted_identities": 0,
            "code_by_area": code_by_area,
            "test_by_accounting_area": test_by_area,
        },
        "code_slices": code_rows,
        "test_slices": test_rows,
    }


def validate_payload(
    payload: Mapping[str, Any],
    *,
    expected_code_ids: set[str] | None = None,
    expected_test_ids: set[str] | None = None,
) -> list[str]:
    """Reconstruct every count and reject omission, duplication, or weak rows."""

    failures: list[str] = []
    code_rows = payload.get("code_slices")
    test_rows = payload.get("test_slices")
    if not isinstance(code_rows, list) or not all(isinstance(row, Mapping) for row in code_rows):
        return ["code_slices must be an array of objects"]
    if not isinstance(test_rows, list) or not all(isinstance(row, Mapping) for row in test_rows):
        return ["test_slices must be an array of objects"]
    _validate_rows(code_rows, "code_id", "code", failures)
    _validate_rows(test_rows, "discovery_id", "test", failures)
    code_ids = {str(row.get("code_id")) for row in code_rows}
    test_ids = {str(row.get("discovery_id")) for row in test_rows}
    if expected_code_ids is not None:
        _validate_partition(code_ids, expected_code_ids, "code", failures)
    if expected_test_ids is not None:
        _validate_partition(test_ids, expected_test_ids, "test", failures)
    for label, rows in (("code", code_rows), ("test", test_rows)):
        for row in rows:
            if row.get("status") != "open":
                failures.append(f"{label} slice {row.get('slice_id')} must remain open")
            for field in ("blockers", "budget_refs", "environment_refs", "dependency_rows"):
                value = row.get(field)
                if not isinstance(value, list) or not value:
                    failures.append(f"{label} slice {row.get('slice_id')} requires {field}")
            if row.get("budget_refs") != list(ALL_BUDGETS):
                failures.append(f"{label} slice {row.get('slice_id')} omits a proof budget")
    summary = payload.get("summary")
    if not isinstance(summary, Mapping):
        failures.append("summary must be an object")
        return sorted(set(failures))
    expected_scalars = {
        "code_facts": len(code_rows),
        "test_facts": len(test_rows),
        "code_spec_gap": sum(row.get("current_disposition") == "spec_gap" for row in code_rows),
        "code_registered_backlog": sum(
            "registered_backlog" in row.get("blockers", []) for row in code_rows
        ),
        "test_reviewed_nonmapping": sum(
            row.get("current_disposition") == "reviewed_nonmapping" for row in test_rows
        ),
        "test_ignored_or_conditional": sum(
            row.get("ignore_state") != "not_ignored" for row in test_rows
        ),
        "unresolved_test_area": sum(
            row.get("accounting_area") == "unresolved" for row in test_rows
        ),
        "unowned": 0,
        "duplicate_identities": 0,
        "omitted_identities": 0,
    }
    for field, expected in expected_scalars.items():
        if summary.get(field) != expected:
            failures.append(f"summary {field} must reconstruct as {expected}")
    expected_code_areas = []
    for area in AREA_ORDER:
        rows = [row for row in code_rows if row.get("area") == area]
        expected_code_areas.append(
            {
                "area": area,
                "facts": len(rows),
                "registered_backlog": sum(
                    "registered_backlog" in row.get("blockers", []) for row in rows
                ),
                "spec_gap": sum(row.get("current_disposition") == "spec_gap" for row in rows),
            }
        )
    if summary.get("code_by_area") != expected_code_areas:
        failures.append("summary code_by_area does not reconstruct from code slices")
    test_counts = Counter(str(row.get("accounting_area")) for row in test_rows)
    expected_test_areas = [
        {"area": area, "facts": test_counts[area]} for area in sorted(test_counts)
    ]
    if summary.get("test_by_accounting_area") != expected_test_areas:
        failures.append("summary test_by_accounting_area does not reconstruct from test slices")
    return sorted(set(failures))


def build_live_payload(root: Path, source_sha: str) -> dict[str, Any]:
    code_state = build_code_spec_live_state(root)
    if code_state.failures:
        raise ValueError("code-spec live state has findings: " + "; ".join(code_state.failures))
    scan = scan_repository(root, timestamp="1970-01-01T00:00:00Z")
    if any(item.severity == "error" for item in scan.diagnostics):
        raise ValueError("test scanner contains error diagnostics")
    denominator = _load_toml(root / "verification/test-catalog-denominator.toml")
    catalog_document = _load_toml(root / "verification/test-catalog.toml")
    ignored_document = _load_toml(root / "verification/ignored-tests.toml")
    reviews = _index(denominator, "reviews", "discovery_id")
    catalog = _index(catalog_document, "tests", "id")
    ignored = _index(ignored_document, "ignored_tests", "id")
    analyze_test_catalog_denominator(
        facts=scan.inferred_facts,
        tests=list(catalog.values()),
        reviews=list(reviews.values()),
        ignored_tests=list(ignored.values()),
    )
    matrix = _load_toml(root / "verification/matrix.toml")
    input_paths = sorted(
        set(collect_report_inputs(root, code_state))
        | set(scan.provenance.input_paths)
        | GENERATOR_PATHS
        | TEST_INPUT_PATHS
        | P18_INPUT_PATHS
    )
    return build_payload(
        code_facts=code_state.facts,
        code_mappings=code_state.metadata.mappings,
        spec_gaps=code_state.metadata.spec_gaps,
        test_facts=scan.inferred_facts,
        test_reviews=reviews,
        test_catalog=catalog,
        ignored_tests=ignored,
        matrix=matrix,
        source_sha=source_sha,
        input_paths=input_paths,
        input_digest=input_closure_digest(root, input_paths),
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--artifact", type=Path, default=ARTIFACT_PATH)
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args(argv)
    root = args.root.resolve()
    artifact = args.artifact if args.artifact.is_absolute() else root / args.artifact
    try:
        if args.write:
            source_sha = _clean_head(root)
        else:
            committed = json.loads(artifact.read_text())
            source_sha = str(committed.get("source_sha", ""))
        payload = build_live_payload(root, source_sha)
        schema = json.loads((root / SCHEMA_PATH).read_text())
        failures = validate_json_schema_instance(payload, schema)
        failures.extend(
            validate_payload(
                payload,
                expected_code_ids={row["code_id"] for row in payload["code_slices"]},
                expected_test_ids={row["discovery_id"] for row in payload["test_slices"]},
            )
        )
        if not args.write:
            failures.extend(
                _source_commit_failures(root, source_sha, payload["input_paths"])
            )
        rendered = canonical_json(payload)
        if not args.write:
            if artifact.read_bytes() != rendered.encode():
                failures.append("committed burn-down ledger differs from live canonical output")
        if failures:
            for failure in sorted(set(failures)):
                print(f"P18 burn-down ledger failed: {failure}")
            return 1
        if args.write:
            artifact.parent.mkdir(parents=True, exist_ok=True)
            artifact.write_text(rendered)
        print(
            "P18 burn-down ledger validated: "
            f"{len(payload['code_slices'])} code facts; "
            f"{len(payload['test_slices'])} test facts; "
            f"sha256:{hashlib.sha256(rendered.encode()).hexdigest()}"
        )
        return 0
    except (OSError, ValueError, KeyError, json.JSONDecodeError, tomllib.TOMLDecodeError) as error:
        print(f"P18 burn-down ledger failed: {error}")
        return 1


def _code_row(
    fact: Any,
    mapping: Mapping[str, Any],
    spec_gaps: Mapping[str, Mapping[str, Any]],
    areas: Mapping[str, Mapping[str, Any]],
) -> dict[str, Any]:
    disposition = str(mapping["disposition"])
    authority_refs: list[str] = []
    decision_refs: list[str] = []
    invariant_refs = sorted(str(value) for value in mapping.get("invariant_ids", []))
    test_refs = sorted(str(value) for value in mapping.get("test_ids", []))
    blockers = list(COMMON_CODE_BLOCKERS)
    if disposition == "spec_gap":
        gap_ref = str(mapping["spec_gap_ref"])
        authority_refs.append(gap_ref)
        gap = spec_gaps.get(gap_ref, {})
        resolution = str(gap.get("resolution_status", "open"))
        blockers.extend(
            (
                resolution,
                "authoritative_specification_required",
                "reviewed_invariant_required",
                "mapped_executable_test_required",
            )
        )
        source_ref = gap.get("resolution_source_ref")
        if isinstance(source_ref, str):
            decision_refs.append(source_ref)
    elif disposition == "specified":
        authority_refs.append(str(mapping["spec_ref"]))
        blockers.append("complete_bidirectional_chain_revalidation_required")
    elif disposition == "inherits_spec":
        authority_refs.append(str(mapping["parent_code_id"]))
        decision = mapping.get("inheritance_review_ref")
        if isinstance(decision, str):
            decision_refs.append(decision)
        blockers.append("structural_inheritance_proof_required")
    elif disposition == "generated":
        authority_refs.append(str(mapping.get("generator_code_id", mapping.get("generator_path", ""))))
        blockers.append("generator_reproducibility_and_boundary_proof_required")
    elif disposition == "non_behavioral":
        decision_refs.append(str(mapping["non_behavioral_reason"]))
        blockers.append("source_derived_non_behavioral_proof_required")
    elif disposition == "vendored_external":
        authority_refs.append(str(mapping["boundary_spec_ref"]))
        decision_refs.append(str(mapping["version_revision"]))
        blockers.append("vendored_provenance_and_boundary_execution_required")
    for field in ("cross_area_decision_ref", "inheritance_review_ref"):
        value = mapping.get(field)
        if isinstance(value, str) and value not in decision_refs:
            decision_refs.append(value)
    area = str(fact.area)
    risk_default = str(areas.get(area, {}).get("risk_default", "unreviewed"))
    return {
        "slice_id": f"P18-CODE-{fact.code_id}",
        "sequence": 0,
        "code_id": fact.code_id,
        "source_kind": fact.source_kind,
        "path": fact.path,
        "qualified_symbol": fact.qualified_symbol,
        "item_kind": fact.item_kind,
        "area": area,
        "owner": str(fact.owner),
        "risk_classes": sorted({risk_default, *fact.risk_signals}),
        "risk_signals": list(fact.risk_signals),
        "current_disposition": disposition,
        "authority_refs": sorted(set(authority_refs)),
        "decision_refs": sorted(set(decision_refs)),
        "invariant_refs": invariant_refs,
        "test_refs": test_refs,
        "environment_refs": ["P18_ENVIRONMENT_SCOPE_PENDING"],
        "budget_refs": list(ALL_BUDGETS),
        "dependency_rows": list(CODE_DEPENDENCIES),
        "blockers": sorted(set(blockers)),
        "status": "open",
    }


def _test_row(
    fact: Any,
    review: Mapping[str, Any],
    catalog: Mapping[str, Any] | None,
    ignored: Mapping[str, Any] | None,
    matrix: Mapping[str, Any],
    areas: Mapping[str, Mapping[str, Any]],
) -> dict[str, Any]:
    route = classify_changed_path(matrix, fact.path)
    candidate_areas = list(route.area_ids)
    authority_refs: list[str] = []
    invariant_refs: list[str] = []
    environment_refs: list[str] = []
    blockers = list(COMMON_TEST_BLOCKERS)
    catalog_id = ""
    if catalog is not None:
        accounting_area = str(catalog["area"])
        owner = str(catalog["owner"])
        catalog_id = str(catalog["id"])
        authority = catalog.get("oracle_ref", catalog.get("spec_gap_ref"))
        if isinstance(authority, str):
            authority_refs.append(authority)
        invariant_refs.extend(str(value) for value in catalog.get("invariants", []))
        environment_refs.extend(f"suite:{value}" for value in catalog.get("suite_tiers", []))
        environment_refs.extend(
            (
                f"hardware_required:{str(bool(catalog.get('requires_hardware'))).lower()}",
                f"network_required:{str(bool(catalog.get('requires_network'))).lower()}",
            )
        )
    elif ignored is not None:
        accounting_area = str(ignored["area"])
        owner = str(ignored["owner"])
        environment_refs.append(f"ignored_registry:{ignored['id']}")
    elif len(candidate_areas) == 1:
        accounting_area = candidate_areas[0]
        owner = str(areas.get(accounting_area, {}).get("owner", "verification"))
        environment_refs.append("native_command_unreviewed")
    else:
        accounting_area = "unresolved"
        owner = "verification"
        environment_refs.append("native_command_unreviewed")
        blockers.append("domain_area_owner_review_required")
    disposition = str(review["disposition"])
    if disposition == "reviewed_nonmapping":
        blockers.append("authoritative_specification_or_invariant_mapping_required")
    else:
        blockers.append("complete_bidirectional_chain_revalidation_required")
    if fact.ignore_state != "not_ignored":
        blockers.append("enabled_execution_or_reviewed_removal_required")
    risk_classes = sorted(
        {
            str(areas[area].get("risk_default", "unreviewed"))
            for area in candidate_areas
            if area in areas
        }
        or {"unreviewed"}
    )
    return {
        "slice_id": f"P18-TEST-{fact.stable_id}",
        "sequence": 0,
        "discovery_id": fact.stable_id,
        "source_kind": fact.source_kind,
        "path": fact.path,
        "name": fact.name,
        "ignore_state": fact.ignore_state,
        "accounting_area": accounting_area,
        "candidate_areas": candidate_areas,
        "owner": owner,
        "risk_classes": risk_classes,
        "current_disposition": disposition,
        "review_reason": str(review.get("reason_code", "catalog_mapped")),
        "catalog_test_id": catalog_id,
        "authority_refs": sorted(set(authority_refs)),
        "invariant_refs": sorted(set(invariant_refs)),
        "code_slice_refs": [],
        "environment_refs": sorted(set(environment_refs)),
        "budget_refs": list(ALL_BUDGETS),
        "dependency_rows": list(TEST_DEPENDENCIES),
        "blockers": sorted(set(blockers)),
        "status": "open",
    }


def _code_order_key(row: Mapping[str, Any]) -> tuple[Any, ...]:
    return (
        AREA_ORDER.index(str(row["area"])),
        0 if row["risk_signals"] else 1,
        str(row["path"]),
        str(row["qualified_symbol"]),
        str(row["code_id"]),
    )


def _test_order_key(row: Mapping[str, Any]) -> tuple[Any, ...]:
    area = str(row["accounting_area"])
    area_index = AREA_ORDER.index(area) if area in AREA_ORDER else len(AREA_ORDER)
    return (
        area_index,
        0 if row["ignore_state"] != "not_ignored" else 1,
        str(row["path"]),
        str(row["name"]),
        str(row["discovery_id"]),
    )


def _validate_rows(
    rows: Sequence[Mapping[str, Any]],
    identity_field: str,
    label: str,
    failures: list[str],
) -> None:
    identities = [row.get(identity_field) for row in rows]
    if len(identities) != len(set(identities)):
        failures.append(f"duplicate {label} identities")
    slice_ids = [row.get("slice_id") for row in rows]
    if len(slice_ids) != len(set(slice_ids)):
        failures.append(f"duplicate {label} slice identities")
    sequences = [row.get("sequence") for row in rows]
    if sequences != list(range(1, len(rows) + 1)):
        failures.append(f"{label} sequence is not contiguous canonical order")


def _validate_partition(
    actual: set[str], expected: set[str], label: str, failures: list[str]
) -> None:
    missing = expected - actual
    invented = actual - expected
    if missing:
        failures.append(f"omitted {len(missing)} live {label} identities")
    if invented:
        failures.append(f"invented {len(invented)} {label} identities")


def _clean_head(root: Path) -> str:
    head = subprocess.run(
        ["git", "-C", str(root), "rev-parse", "HEAD"],
        check=False,
        capture_output=True,
        text=True,
    )
    status = subprocess.run(
        ["git", "-C", str(root), "status", "--porcelain", "--untracked-files=all"],
        check=False,
        capture_output=True,
        text=True,
    )
    source_sha = head.stdout.strip()
    if head.returncode != 0 or re.fullmatch(r"[0-9a-f]{40}", source_sha) is None:
        raise ValueError("producer Git HEAD is unavailable")
    if status.returncode != 0 or status.stdout:
        raise ValueError("--write requires a clean producer worktree")
    return source_sha


def _source_commit_failures(
    root: Path, source_sha: str, input_paths: Sequence[str]
) -> list[str]:
    """Reject a self-asserted SHA unless Git proves the exact live input bytes."""

    failures: list[str] = []
    if re.fullmatch(r"[0-9a-f]{40}", source_sha) is None:
        return ["ledger source_sha is not a full lower-case Git SHA"]
    commit = subprocess.run(
        ["git", "-C", str(root), "cat-file", "-e", f"{source_sha}^{{commit}}"],
        check=False,
        capture_output=True,
    )
    if commit.returncode != 0:
        return ["ledger source_sha is not an available Git commit"]
    ancestor = subprocess.run(
        ["git", "-C", str(root), "merge-base", "--is-ancestor", source_sha, "HEAD"],
        check=False,
        capture_output=True,
    )
    if ancestor.returncode != 0:
        failures.append("ledger source_sha is not an ancestor of repository HEAD")
    for offset in range(0, len(input_paths), 200):
        paths = list(input_paths[offset : offset + 200])
        comparison = subprocess.run(
            ["git", "-C", str(root), "diff", "--quiet", source_sha, "--", *paths],
            check=False,
            capture_output=True,
        )
        if comparison.returncode != 0:
            failures.append("ledger input bytes differ from its asserted source_sha")
            break
    status = subprocess.run(
        ["git", "-C", str(root), "status", "--porcelain", "--untracked-files=all"],
        check=False,
        capture_output=True,
    )
    if status.returncode != 0 or status.stdout:
        failures.append("ledger check requires a clean worktree")
    return sorted(set(failures))


def _load_toml(path: Path) -> dict[str, Any]:
    return tomllib.loads(path.read_text())


def _index(
    document: Mapping[str, Any], field: str, identity_field: str
) -> dict[str, Mapping[str, Any]]:
    rows = document.get(field)
    if not isinstance(rows, list):
        raise ValueError(f"{field} must be an array")
    result: dict[str, Mapping[str, Any]] = {}
    for row in rows:
        if not isinstance(row, Mapping) or not isinstance(row.get(identity_field), str):
            raise ValueError(f"{field} contains an invalid row")
        identity = str(row[identity_field])
        if identity in result:
            raise ValueError(f"{field} duplicates {identity}")
        result[identity] = row
    return result
