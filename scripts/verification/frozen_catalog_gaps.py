"""Fast catalogue-gap analysis from committed metadata without a live scan."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import tomllib
from collections import Counter, defaultdict
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any


DEFAULT_JSON = Path(
    "target/gate-artifacts/verification/frozen-catalog-gaps/report.json"
)
DEFAULT_MARKDOWN = Path(
    "target/gate-artifacts/verification/frozen-catalog-gaps/report.md"
)
MISSING_SPEC_REASON = "no_reviewed_spec_or_invariant_binding"


def analyze_frozen_catalog_gaps(
    *,
    denominator: Mapping[str, Any],
    catalog: Mapping[str, Any],
    invariants: Sequence[tuple[str, Mapping[str, Any]]],
) -> dict[str, Any]:
    reviews = _objects(denominator.get("reviews"), "denominator reviews")
    tests = _objects(catalog.get("tests"), "catalog tests")
    _require_unique(reviews, "discovery_id", "denominator review")
    _require_unique(tests, "id", "catalog test")

    mapped = [row for row in reviews if row.get("disposition") == "catalog_mapped"]
    nonmapping = [
        row for row in reviews if row.get("disposition") == "reviewed_nonmapping"
    ]
    if len(mapped) + len(nonmapping) != len(reviews):
        raise ValueError("denominator contains an unsupported disposition")
    missing_spec = [
        row for row in nonmapping if row.get("reason_code") == MISSING_SPEC_REASON
    ]
    owned = [
        row for row in nonmapping if row.get("reason_code") != MISSING_SPEC_REASON
    ]

    grouped: dict[tuple[str, str], list[dict[str, str]]] = defaultdict(list)
    for row in missing_spec:
        source_kind = _string(row, "discovery_source_kind", "denominator review")
        path = _string(row, "path", "denominator review")
        grouped[(source_kind, path)].append(
            {
                "discovery_id": _string(row, "discovery_id", "denominator review"),
                "name": _string(row, "name", "denominator review"),
                "ignore_state": _string(row, "ignore_state", "denominator review"),
            }
        )
    batches = [
        {
            "source_kind": source_kind,
            "path": path,
            "count": len(rows),
            "tests": sorted(rows, key=lambda row: row["discovery_id"]),
        }
        for (source_kind, path), rows in grouped.items()
    ]
    batches.sort(key=lambda row: (-row["count"], row["source_kind"], row["path"]))

    mapped_behavior_refs = {
        behavior_ref
        for test in tests
        for behavior_ref in _string_list(
            test.get("mapping_behavior_refs", []),
            f"catalog test {test.get('id')} behavior refs",
        )
    }
    behavior_demands: list[dict[str, Any]] = []
    gap_cells: list[dict[str, Any]] = []
    exact_mapping_debt: list[dict[str, Any]] = []
    for path, invariant in invariants:
        invariant_id = _string(invariant, "id", path)
        behaviors = _objects(invariant.get("behavior", []), f"{invariant_id} behaviors")
        if invariant.get("spec", {}).get("status") == "specified":
            for index, behavior in enumerate(behaviors):
                behavior_ref = f"{invariant_id}#behavior:{index}"
                if behavior_ref not in mapped_behavior_refs:
                    behavior_demands.append(
                        {
                            "behavior_ref": behavior_ref,
                            "invariant_id": invariant_id,
                            "owner": invariant.get("owner"),
                            "case_family": behavior.get("case_family"),
                            "rationale": behavior.get("rationale"),
                            "path": path,
                        }
                    )
            coverage = invariant.get("coverage")
            cells = (
                coverage.get("cells", [])
                if isinstance(coverage, Mapping)
                else []
            )
            for cell in _objects(cells, f"{invariant_id} coverage cells"):
                if cell.get("state") == "gap_open":
                    gap_cells.append(
                        {
                            "invariant_id": invariant_id,
                            "owner": invariant.get("owner"),
                            "dimension": cell.get("dimension"),
                            "rationale": cell.get("rationale"),
                            "path": path,
                            "catalog_test_count": len(
                                _string_list(
                                    invariant.get("tests", []),
                                    f"{invariant_id} tests",
                                )
                            ),
                        }
                    )
        missing = _string_list(
            invariant.get("missing", []),
            f"{invariant_id} missing",
        )
        if "cataloged_behavior_tests" in missing:
            exact_mapping_debt.append(
                {
                    "invariant_id": invariant_id,
                    "owner": invariant.get("owner"),
                    "path": path,
                    "behavior_count": len(behaviors),
                    "catalog_test_count": len(
                        _string_list(
                            invariant.get("tests", []),
                            f"{invariant_id} tests",
                        )
                    ),
                }
            )

    existing_mapping_complete = not missing_spec
    confirmed_missing = behavior_demands if existing_mapping_complete else []
    return {
        "summary": {
            "frozen_live_tests": len(reviews),
            "catalog_mapped": len(mapped),
            "reviewed_nonmapping": len(nonmapping),
            "missing_spec_authority_tests": len(missing_spec),
            "owned_nonmapping_tests": len(owned),
            "specification_batches": len(batches),
            "specified_gap_open_cells": len(gap_cells),
            "invariants_requiring_exact_behavior_mapping": len(
                exact_mapping_debt
            ),
            "confirmed_missing_tests": len(confirmed_missing),
        },
        "owned_nonmapping_by_reason": dict(
            sorted(Counter(str(row.get("reason_code")) for row in owned).items())
        ),
        "specification_batches": batches,
        "specified_gap_open_cells": sorted(
            gap_cells,
            key=lambda row: (
                str(row["owner"]),
                row["invariant_id"],
                str(row["dimension"]),
            ),
        ),
        "exact_behavior_mapping_debt": sorted(
            exact_mapping_debt,
            key=lambda row: (str(row["owner"]), row["invariant_id"]),
        ),
        "missing_test_demand_status": (
            "ready"
            if existing_mapping_complete
            else "blocked_until_existing_test_mapping_complete"
        ),
        "missing_test_candidates": sorted(
            behavior_demands,
            key=lambda row: (str(row["owner"]), row["behavior_ref"]),
        ),
        "confirmed_missing_tests": sorted(
            confirmed_missing,
            key=lambda row: (str(row["owner"]), row["behavior_ref"]),
        ),
        "missing_test_policy": (
            "A specified behavior is confirmed missing only after every existing "
            "test has left the missing-spec-authority queue and no accepted "
            "catalog mapping binds that exact behavior."
        ),
    }


def generate_report(root: Path) -> dict[str, Any]:
    root = root.resolve()
    denominator_path = root / "verification/test-catalog-denominator.toml"
    catalog_path = root / "verification/test-catalog.toml"
    invariant_paths = sorted((root / "verification/invariants").rglob("*.toml"))
    denominator = _toml(denominator_path)
    catalog = _toml(catalog_path)
    invariants = [
        (path.relative_to(root).as_posix(), _toml(path))
        for path in invariant_paths
    ]
    analysis = analyze_frozen_catalog_gaps(
        denominator=denominator,
        catalog=catalog,
        invariants=invariants,
    )
    closure = hashlib.sha256()
    for path in invariant_paths:
        closure.update(path.relative_to(root).as_posix().encode())
        closure.update(b"\0")
        closure.update(path.read_bytes())
        closure.update(b"\0")
    return {
        "schema_version": 1,
        "kind": "frozen_test_catalog_gap_report",
        "source_commit": _git(root, "rev-parse", "HEAD"),
        "working_tree_dirty": bool(_git(root, "status", "--porcelain")),
        "basis": "committed_frozen_denominator_no_live_scan",
        "input_digests": {
            "verification/test-catalog-denominator.toml": _digest(
                denominator_path
            ),
            "verification/test-catalog.toml": _digest(catalog_path),
            "verification/invariants/**/*.toml": "sha256:" + closure.hexdigest(),
        },
        **analysis,
    }


def write_report(
    report: Mapping[str, Any],
    *,
    json_path: Path,
    markdown_path: Path,
) -> None:
    json_path.parent.mkdir(parents=True, exist_ok=True)
    markdown_path.parent.mkdir(parents=True, exist_ok=True)
    json_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    markdown_path.write_text(render_markdown(report))


def render_markdown(report: Mapping[str, Any]) -> str:
    summary = report["summary"]
    lines = [
        "# Frozen test-catalogue gap report",
        "",
        f"- Source commit: `{report['source_commit']}`",
        "- Basis: committed frozen denominator; no live scan, coverage run, build, or test execution",
        "",
        "## Summary",
        "",
    ]
    lines.extend(
        f"- {key.replace('_', ' ').capitalize()}: {value}"
        for key, value in summary.items()
    )
    lines.extend(
        [
            "",
            "## Specification batches",
            "",
            "| Tests | Source kind | Path |",
            "|---:|---|---|",
        ]
    )
    for batch in report["specification_batches"]:
        lines.append(
            f"| {batch['count']} | `{batch['source_kind']}` | `{batch['path']}` |"
        )
    lines.extend(["", "## Explicit owned nonmapping routes", ""])
    for reason, count in report["owned_nonmapping_by_reason"].items():
        lines.append(f"- `{reason}`: {count}")
    lines.extend(
        [
            "",
            "## Missing-test rule",
            "",
            str(report["missing_test_policy"]),
            "",
        ]
    )
    return "\n".join(lines)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--json-out", type=Path, default=DEFAULT_JSON)
    parser.add_argument("--markdown-out", type=Path, default=DEFAULT_MARKDOWN)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        report = generate_report(root)
        json_path = args.json_out if args.json_out.is_absolute() else root / args.json_out
        markdown_path = (
            args.markdown_out
            if args.markdown_out.is_absolute()
            else root / args.markdown_out
        )
        write_report(report, json_path=json_path, markdown_path=markdown_path)
    except (OSError, ValueError, tomllib.TOMLDecodeError) as error:
        print(f"frozen catalogue gap report failed: {error}", file=sys.stderr)
        return 2
    print(
        "frozen catalogue gaps: "
        f"{report['summary']['missing_spec_authority_tests']} missing-spec "
        f"existing tests, {report['summary']['confirmed_missing_tests']} "
        "confirmed missing tests"
    )
    return 0


def _objects(value: Any, label: str) -> list[Mapping[str, Any]]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be an array")
    if any(not isinstance(row, Mapping) for row in value):
        raise ValueError(f"{label} must contain objects")
    return list(value)


def _string_list(value: Any, label: str) -> list[str]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be an array")
    if any(not isinstance(item, str) or not item for item in value):
        raise ValueError(f"{label} must contain non-empty strings")
    return list(value)


def _string(record: Mapping[str, Any], field: str, label: str) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value


def _require_unique(
    rows: Sequence[Mapping[str, Any]],
    field: str,
    label: str,
) -> None:
    values = [_string(row, field, label) for row in rows]
    if len(values) != len(set(values)):
        raise ValueError(f"{label} duplicates {field}")


def _toml(path: Path) -> dict[str, Any]:
    value = tomllib.loads(path.read_text())
    if not isinstance(value, dict):
        raise ValueError(f"{path} root must be a table")
    return value


def _digest(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def _git(root: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        raise ValueError(f"git {' '.join(args)} failed")
    return completed.stdout.strip()


if __name__ == "__main__":
    raise SystemExit(main())
