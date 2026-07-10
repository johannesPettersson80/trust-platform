"""Deterministic coverage-matrix gap analysis and rendering."""

from __future__ import annotations

import hashlib
import json
import tomllib
from collections import Counter, defaultdict
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .metadata_validator.integrity import OPEN_GAP_RESOLUTIONS
from .report_input_contract import validator_code_input_paths


GENERATOR = "coverage-matrix-gap-report"
GENERATOR_VERSION = 1
DEFAULT_JSON_PATH = Path("target/gate-artifacts/verification/coverage-matrix-gaps.json")
DEFAULT_MARKDOWN_PATH = Path("target/gate-artifacts/verification/coverage-matrix-gaps.md")
COVERAGE_STATES = (
    "covered",
    "covered_by_fuzz",
    "not_applicable",
    "blocked",
    "spec_gap",
    "gap_open",
    "deferred",
)
TOOL_INPUT_PATHS = {
    "scripts/report_coverage_matrix_gaps.py",
    "scripts/validate_coverage_matrix_gap_report.py",
    "scripts/verification/coverage_matrix_gap_contract.py",
    "scripts/verification/coverage_matrix_gaps.py",
    "scripts/verification/coverage_matrix_gap_cli.py",
    "scripts/verification/coverage_matrix_gap_validation.py",
    "scripts/verification/metadata_validator/__init__.py",
    "scripts/verification/metadata_validator/case_files.py",
    "scripts/verification/metadata_validator/constants.py",
    "scripts/verification/metadata_validator/core.py",
    "scripts/verification/metadata_validator/evidence_proof.py",
    "scripts/verification/metadata_validator/integrity.py",
    "scripts/verification/metadata_validator/mutation_contracts.py",
    "scripts/verification/metadata_validator/mutation_reports.py",
    "scripts/verification/metadata_validator/mutation_shards.py",
    "scripts/verification/metadata_validator/oracle_refs.py",
    "scripts/verification/metadata_validator/schema_contracts.py",
    "scripts/verification/metadata_validator/taxonomy.py",
    "scripts/verification/test_catalog_common.py",
    "scripts/verification/test_catalog_json_schema.py",
    "scripts/verification/test_catalog_scanner.py",
    "scripts/verification/test_catalog_validation.py",
    "docs/internal/testing/checklists/plc-verification-program/test-taxonomy.md",
    "verification/schemas/catalog.schema.json",
    "verification/schemas/case-file.schema.json",
    "verification/schemas/coverage-matrix-gap-report.schema.json",
    "verification/schemas/invariant.schema.json",
    "verification/schemas/matrix.schema.json",
    "verification/schemas/spec-gap.schema.json",
    "verification/schemas/spec-source.schema.json",
}
LIMITATIONS = (
    "Completeness is assessed only for invariants in mapped planning-matrix areas.",
    "Missing required slots are structural debt and never receive a synthetic coverage state.",
    "Recorded coverage states are copied from invariant metadata and are not independently promoted.",
    "Committed cases are planning observations only; blocked or runnable cases never upgrade a state.",
    "Covered and covered_by_fuzz remain metadata claims rather than standalone behavior proof.",
    "Debt is report output and does not make successful report generation fail.",
    "Platform is historical provenance requiring evidence review; at-rest validation cannot rederive a prior host.",
)


@dataclass(frozen=True)
class CoverageMatrixGapProvenance:
    command: tuple[str, ...]
    commit: str
    timestamp: str
    platform: str
    input_paths: tuple[str, ...]
    output_json: str
    output_markdown: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "command": list(self.command),
            "commit": self.commit,
            "timestamp": self.timestamp,
            "platform": self.platform,
            "input_paths": list(self.input_paths),
            "output_paths": {
                "json": self.output_json,
                "markdown": self.output_markdown,
            },
        }


@dataclass(frozen=True)
class CoverageMatrixGapReport:
    provenance: CoverageMatrixGapProvenance
    input_digest: str
    analysis: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": 1,
            "generator": GENERATOR,
            "generator_version": GENERATOR_VERSION,
            "report_status": "complete",
            "input_digest": self.input_digest,
            **self.provenance.to_dict(),
            **self.analysis,
        }

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), indent=2, sort_keys=True) + "\n"

    def to_markdown(self, *, json_digest: str) -> str:
        summary = self.analysis["summary"]
        lines = [
            "# Coverage-Matrix Gap Report",
            "",
            f"Generator: `{GENERATOR} v{GENERATOR_VERSION}`",
            f"Source revision: `{self.provenance.commit}`",
            f"Generated: `{self.provenance.timestamp}`",
            f"Platform: `{self.provenance.platform}`",
            f"Generated JSON SHA-256: `{json_digest}`",
            f"Input SHA-256: `{self.input_digest}`",
            "",
            "`complete` means the report was generated and bound successfully. It does not",
            "mean every required coverage slot is assigned or covered.",
            "",
            "## Summary",
            "",
            f"- Mapped areas: {summary['mapped_areas']}",
            f"- Mapped-area invariants: {summary['mapped_area_invariants']}",
            f"- Out-of-scope invariants: {summary['out_of_scope_invariants']}",
            f"- Required family slots: {summary['required_family_slots']}",
            f"- Assigned required slots: {summary['assigned_required_slots']}",
            f"- Missing required slots: {summary['missing_required_slots']}",
            f"- Additional recorded cells: {summary['additional_recorded_cells']}",
            f"- Recorded mapped-area cells: {summary['recorded_cells']}",
            f"- Catalog-bound case files: {summary['case_files']}",
            f"- Case observations: {summary['case_observations']}",
            f"- Blocked case observations: {summary['blocked_case_observations']}",
            "",
            "## Declared State Counts",
            "",
            "| State | Cells |",
            "| --- | ---: |",
        ]
        lines.extend(
            f"| `{state}` | {summary['state_counts'][state]} |" for state in COVERAGE_STATES
        )
        for area in self.analysis["areas"]:
            lines.extend(
                [
                    "",
                    f"## Area: `{area['area']}`",
                    "",
                    f"Required families: {_markdown_ids(area['required_case_families'])}",
                ]
            )
            for invariant in area["invariants"]:
                lines.extend(
                    [
                        "",
                        f"### `{invariant['id']}`",
                        "",
                        "| Dimension | Assignment | Declared state | Blocked cases | Issues |",
                        "| --- | --- | --- | --- | --- |",
                    ]
                )
                for slot in [*invariant["required_slots"], *invariant["additional_cells"]]:
                    state = f"`{slot['coverage_state']}`" if slot["coverage_state"] else "none"
                    lines.append(
                        f"| `{slot['dimension']}` | `{slot['assignment']}` | {state} | "
                        f"{_markdown_ids(slot['blocked_case_ids'])} | "
                        f"{_markdown_ids(slot['state_issues'])} |"
                    )
                if invariant["additional_case_families"]:
                    lines.extend(["", "Additional case-only families:", ""])
                    lines.extend(
                        f"- `{item['dimension']}`: {_markdown_ids(item['case_ids'])}"
                        for item in invariant["additional_case_families"]
                    )
        lines.extend(["", "## Out-Of-Scope Invariants", ""])
        if self.analysis["out_of_scope_invariants"]:
            lines.extend(
                f"- `{item['id']}` (`{item['area']}`): {len(item['recorded_cells'])} recorded cells"
                for item in self.analysis["out_of_scope_invariants"]
            )
        else:
            lines.append("- none")
        lines.extend(["", "## Limitations", ""])
        lines.extend(f"- {item}" for item in self.analysis["limitations"])
        return "\n".join(lines) + "\n"


def analyze_coverage_matrix_gaps(
    *,
    matrix: Mapping[str, Any],
    invariants: Sequence[Mapping[str, Any]],
    tests: Sequence[Mapping[str, Any]],
    case_tables: Sequence[Mapping[str, Any]],
    spec_gaps: Mapping[str, Mapping[str, Any]],
) -> dict[str, Any]:
    """Report declared cells, missing required slots, and case observations."""

    mapped_areas = _mapped_areas(matrix)
    invariants_by_id = _unique_records(invariants, "invariant")
    tests_by_id = _unique_records(tests, "test")
    case_index, case_file_count = _case_observations(
        case_tables=case_tables,
        tests_by_id=tests_by_id,
        invariants_by_id=invariants_by_id,
    )
    area_invariants: dict[str, list[Mapping[str, Any]]] = defaultdict(list)
    out_of_scope: list[dict[str, Any]] = []
    for invariant_id, invariant in sorted(invariants_by_id.items()):
        area = invariant.get("area")
        if area in mapped_areas:
            area_invariants[area].append(invariant)
        else:
            cells = _coverage_cells(invariant_id, invariant)
            out_of_scope.append(
                {
                    "id": invariant_id,
                    "area": str(area),
                    "recorded_cells": [
                        _declared_cell(
                            cell,
                            assignment="out_of_scope_recorded",
                            linked_tests=invariant.get("tests", []),
                            cases=(),
                            spec_gaps=spec_gaps,
                        )
                        for cell in sorted(cells.values(), key=lambda item: item["dimension"])
                    ],
                }
            )

    areas: list[dict[str, Any]] = []
    state_counts: Counter[str] = Counter()
    required_slots = assigned_slots = additional_cells = recorded_cells = 0
    mapped_case_paths: set[str] = set()
    mapped_case_ids: set[str] = set()
    mapped_blocked_ids: set[str] = set()
    for area_id, area in mapped_areas.items():
        required = area.get("required_case_families")
        if not isinstance(required, list) or not required or not all(
            isinstance(item, str) and item for item in required
        ):
            raise ValueError(f"mapped area {area_id} has invalid required_case_families")
        if len(required) != len(set(required)):
            raise ValueError(f"mapped area {area_id} duplicates required_case_families")
        required_families = sorted(required)
        invariant_reports: list[dict[str, Any]] = []
        for invariant in sorted(area_invariants.get(area_id, []), key=lambda item: item["id"]):
            invariant_id = invariant["id"]
            cells = _coverage_cells(invariant_id, invariant)
            observations = case_index.get(invariant_id, {})
            required_reports = []
            for family in required_families:
                cases = observations.get(family, ())
                if family in cells:
                    cell = _declared_cell(
                        cells[family],
                        assignment="assigned",
                        linked_tests=invariant.get("tests", []),
                        cases=cases,
                        spec_gaps=spec_gaps,
                    )
                    assigned_slots += 1
                    state_counts[cell["coverage_state"]] += 1
                else:
                    cell = _missing_cell(family, cases)
                required_reports.append(cell)
            extras = []
            for family in sorted(set(cells) - set(required_families)):
                cell = _declared_cell(
                    cells[family],
                    assignment="additional_recorded",
                    linked_tests=invariant.get("tests", []),
                    cases=observations.get(family, ()),
                    spec_gaps=spec_gaps,
                )
                extras.append(cell)
                additional_cells += 1
                state_counts[cell["coverage_state"]] += 1
            case_only = [
                _case_only_family(family, observations[family])
                for family in sorted(set(observations) - set(required_families) - set(cells))
            ]
            for cases in observations.values():
                for case in cases:
                    mapped_case_paths.add(case["case_file"])
                    mapped_case_ids.add(case["id"])
                    if case["blocked"]:
                        mapped_blocked_ids.add(case["id"])
            required_slots += len(required_reports)
            recorded_cells += len(cells)
            invariant_reports.append(
                {
                    "id": invariant_id,
                    "risk": str(invariant.get("risk")),
                    "status": str(invariant.get("status")),
                    "contract_kind": str(invariant.get("contract_kind")),
                    "proof_level": str(invariant.get("proof_level")),
                    "linked_test_ids": sorted(invariant.get("tests", [])),
                    "required_slots": required_reports,
                    "additional_cells": extras,
                    "additional_case_families": case_only,
                }
            )
        area_required = len(invariant_reports) * len(required_families)
        area_assigned = sum(
            1
            for invariant in invariant_reports
            for slot in invariant["required_slots"]
            if slot["assignment"] == "assigned"
        )
        areas.append(
            {
                "area": area_id,
                "required_case_families": required_families,
                "invariant_count": len(invariant_reports),
                "required_family_slots": area_required,
                "assigned_required_slots": area_assigned,
                "missing_required_slots": area_required - area_assigned,
                "additional_recorded_cells": sum(
                    len(invariant["additional_cells"]) for invariant in invariant_reports
                ),
                "invariants": invariant_reports,
            }
        )

    normalized_counts = {state: state_counts[state] for state in COVERAGE_STATES}
    summary = {
        "mapped_areas": len(mapped_areas),
        "mapped_area_invariants": sum(len(items) for items in area_invariants.values()),
        "out_of_scope_invariants": len(out_of_scope),
        "required_family_slots": required_slots,
        "assigned_required_slots": assigned_slots,
        "missing_required_slots": required_slots - assigned_slots,
        "additional_recorded_cells": additional_cells,
        "recorded_cells": recorded_cells,
        "case_files": len(mapped_case_paths),
        "case_observations": len(mapped_case_ids),
        "blocked_case_observations": len(mapped_blocked_ids),
        "state_counts": normalized_counts,
    }
    if case_file_count < len(mapped_case_paths):
        raise ValueError("mapped case observations exceed loaded case files")
    return {
        "scope": {
            "area_basis": "planning_matrix_status_mapped",
            "slot_basis": "mapped_area_invariant_x_required_case_family",
            "coverage_states": list(COVERAGE_STATES),
            "missing_slot_semantics": "structural_debt_without_synthetic_state",
            "case_observation_semantics": "planning_observation_only_never_state_upgrade",
            "debt_is_report_failure": False,
        },
        "summary": summary,
        "areas": areas,
        "out_of_scope_invariants": out_of_scope,
        "limitations": list(LIMITATIONS),
    }


def load_repository_inputs(root: Path, validator: Any) -> tuple[dict[str, Any], list[str]]:
    """Adapt already-validated repository metadata to the pure analyzer."""

    tests = list(validator.tests.values())
    case_tables: list[dict[str, Any]] = []
    case_paths: set[str] = set()
    for test in tests:
        if test.get("subject_kind") != "case_table_artifact":
            continue
        case_path = test.get("case_file")
        if not isinstance(case_path, str) or case_path in case_paths:
            continue
        case_paths.add(case_path)
        data = tomllib.loads((root / case_path).read_text())
        case_tables.append(
            {
                "path": case_path,
                "invariant": data.get("invariant"),
                "cases": data.get("case", []),
            }
        )
    invariants = list(validator.invariants.values())
    analysis = analyze_coverage_matrix_gaps(
        matrix=validator.matrix,
        invariants=invariants,
        tests=tests,
        case_tables=case_tables,
        spec_gaps=validator.spec_gaps,
    )
    input_paths = {
        "verification/matrix.toml",
        "verification/test-catalog.toml",
        "verification/spec-gaps.toml",
        "verification/spec-sources.toml",
        *TOOL_INPUT_PATHS,
        *case_paths,
    }
    input_paths.update(validator_code_input_paths(root))
    input_paths.update(
        path.relative_to(root).as_posix()
        for path in sorted((root / "verification/invariants").rglob("*.toml"))
    )
    return analysis, sorted(input_paths)


def write_reports(
    report: CoverageMatrixGapReport,
    *,
    json_path: Path,
    markdown_path: Path,
) -> None:
    rendered_json = report.to_json()
    digest = hashlib.sha256(rendered_json.encode()).hexdigest()
    json_path.parent.mkdir(parents=True, exist_ok=True)
    markdown_path.parent.mkdir(parents=True, exist_ok=True)
    json_path.write_text(rendered_json)
    markdown_path.write_text(report.to_markdown(json_digest=digest))


def _mapped_areas(matrix: Mapping[str, Any]) -> dict[str, Mapping[str, Any]]:
    rows = matrix.get("areas")
    if not isinstance(rows, list):
        raise ValueError("matrix lacks [[areas]] rows")
    result: dict[str, Mapping[str, Any]] = {}
    for row in rows:
        if not isinstance(row, Mapping) or row.get("status") != "mapped":
            continue
        area_id = row.get("id")
        if not isinstance(area_id, str) or not area_id:
            raise ValueError("mapped matrix area lacks a string id")
        if area_id in result:
            raise ValueError(f"matrix duplicates mapped area {area_id}")
        result[area_id] = row
    return dict(sorted(result.items()))


def _unique_records(
    records: Sequence[Mapping[str, Any]],
    kind: str,
) -> dict[str, Mapping[str, Any]]:
    result: dict[str, Mapping[str, Any]] = {}
    for record in records:
        record_id = record.get("id")
        if not isinstance(record_id, str) or not record_id:
            raise ValueError(f"{kind} record lacks a string id")
        if record_id in result:
            raise ValueError(f"{kind} records duplicate id {record_id}")
        result[record_id] = record
    return result


def _coverage_cells(
    invariant_id: str,
    invariant: Mapping[str, Any],
) -> dict[str, Mapping[str, Any]]:
    cells = invariant.get("coverage", {}).get("cells")
    if not isinstance(cells, list) or not cells:
        raise ValueError(f"{invariant_id} lacks coverage cells")
    result: dict[str, Mapping[str, Any]] = {}
    for cell in cells:
        if not isinstance(cell, Mapping):
            raise ValueError(f"{invariant_id} has a non-table coverage cell")
        dimension = cell.get("dimension")
        state = cell.get("state")
        rationale = cell.get("rationale")
        if not isinstance(dimension, str) or not dimension:
            raise ValueError(f"{invariant_id} has a coverage cell without a dimension")
        if dimension in result:
            raise ValueError(f"{invariant_id} duplicates coverage dimension {dimension}")
        if state not in COVERAGE_STATES:
            raise ValueError(f"{invariant_id} coverage dimension {dimension} has unknown state {state!r}")
        if not isinstance(rationale, str) or not rationale.strip():
            raise ValueError(f"{invariant_id} coverage dimension {dimension} lacks rationale")
        result[dimension] = cell
    return result


def _case_observations(
    *,
    case_tables: Sequence[Mapping[str, Any]],
    tests_by_id: Mapping[str, Mapping[str, Any]],
    invariants_by_id: Mapping[str, Mapping[str, Any]],
) -> tuple[dict[str, dict[str, tuple[dict[str, Any], ...]]], int]:
    catalog_case_paths = {
        test.get("case_file"): tuple(test.get("invariants", []))
        for test in tests_by_id.values()
        if test.get("subject_kind") == "case_table_artifact"
        and isinstance(test.get("case_file"), str)
    }
    seen_paths: set[str] = set()
    seen_cases: set[str] = set()
    observations: dict[str, dict[str, list[dict[str, Any]]]] = defaultdict(
        lambda: defaultdict(list)
    )
    for table in case_tables:
        path = table.get("path")
        invariant_id = table.get("invariant")
        if not isinstance(path, str) or path in seen_paths:
            raise ValueError(f"case table path is missing or duplicated: {path!r}")
        seen_paths.add(path)
        if invariant_id not in invariants_by_id:
            raise ValueError(f"case table {path} references unknown invariant {invariant_id!r}")
        if invariant_id not in catalog_case_paths.get(path, ()):
            raise ValueError(f"case table {path} is not catalog-bound to {invariant_id}")
        cases = table.get("cases")
        if not isinstance(cases, list):
            raise ValueError(f"case table {path} lacks cases")
        for case in cases:
            if not isinstance(case, Mapping):
                raise ValueError(f"case table {path} contains a non-table case")
            case_id = case.get("id")
            family = case.get("family")
            if not isinstance(case_id, str) or not case_id or case_id in seen_cases:
                raise ValueError(f"case id is missing or duplicated: {case_id!r}")
            if not isinstance(family, str) or not family:
                raise ValueError(f"case {case_id} lacks a family")
            seen_cases.add(case_id)
            observations[invariant_id][family].append(
                {
                    "id": case_id,
                    "case_file": path,
                    "blocked": case.get("state") == "blocked",
                }
            )
    frozen = {
        invariant_id: {
            family: tuple(sorted(items, key=lambda item: item["id"]))
            for family, items in sorted(families.items())
        }
        for invariant_id, families in sorted(observations.items())
    }
    return frozen, len(seen_paths)


def _declared_cell(
    cell: Mapping[str, Any],
    *,
    assignment: str,
    linked_tests: Any,
    cases: Sequence[Mapping[str, Any]],
    spec_gaps: Mapping[str, Mapping[str, Any]],
) -> dict[str, Any]:
    state = cell["state"]
    gap_ref = cell.get("spec_gap_ref")
    issues: list[str] = []
    if state == "spec_gap":
        if not isinstance(gap_ref, str) or not gap_ref:
            issues.append("spec_gap_ref_missing")
        elif gap_ref not in spec_gaps:
            issues.append(f"spec_gap_ref_unknown:{gap_ref}")
        elif spec_gaps[gap_ref].get("resolution_status") not in OPEN_GAP_RESOLUTIONS:
            issues.append(f"spec_gap_ref_not_open:{gap_ref}")
    if state in {"covered", "covered_by_fuzz"} and not linked_tests:
        issues.append(f"declared_{state}_without_linked_tests")
    case_ids = sorted(case["id"] for case in cases)
    blocked = sorted(case["id"] for case in cases if case["blocked"])
    return {
        "dimension": cell["dimension"],
        "assignment": assignment,
        "coverage_state": state,
        "rationale": cell.get("rationale"),
        "spec_gap_ref": gap_ref if isinstance(gap_ref, str) else None,
        "decision_ref": cell.get("decision_ref") if isinstance(cell.get("decision_ref"), str) else None,
        "state_issues": issues,
        "case_ids": case_ids,
        "blocked_case_ids": blocked,
    }


def _missing_cell(
    dimension: str,
    cases: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    return {
        "dimension": dimension,
        "assignment": "missing_cell",
        "coverage_state": None,
        "rationale": None,
        "spec_gap_ref": None,
        "decision_ref": None,
        "state_issues": [],
        "case_ids": sorted(case["id"] for case in cases),
        "blocked_case_ids": sorted(case["id"] for case in cases if case["blocked"]),
    }


def _case_only_family(
    dimension: str,
    cases: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    return {
        "dimension": dimension,
        "case_ids": sorted(case["id"] for case in cases),
        "blocked_case_ids": sorted(case["id"] for case in cases if case["blocked"]),
    }


def _markdown_ids(values: Sequence[str]) -> str:
    return ", ".join(f"`{value}`" for value in values) if values else "none"
