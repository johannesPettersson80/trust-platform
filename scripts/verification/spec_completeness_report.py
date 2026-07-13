"""Deterministic Phase 4A specification-completeness analysis and rendering."""

from __future__ import annotations

import hashlib
import json
from collections import Counter, defaultdict
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .metadata_validator.integrity import OPEN_GAP_RESOLUTIONS, RUNNABLE_TEST_STATUSES


GENERATOR = "spec-completeness"
GENERATOR_VERSION = 1
DEFAULT_JSON_PATH = Path("target/gate-artifacts/verification/spec-completeness.json")
DEFAULT_MARKDOWN_PATH = Path("target/gate-artifacts/verification/spec-completeness.md")
PILOT_AREA = "bytecode_vm"
PILOT_CLASSIFICATIONS = (
    "test_gap",
    "spec_gap",
    "hardware_tool_blocked",
    "not_applicable",
)
ORACLE_BINDING_FIELDS = ("oracle_ref", "spec_ref", "spec_gap_ref")
REPORT_CONTRACT_PATHS = (
    "scripts/report_spec_completeness.py",
    "scripts/validate_spec_completeness_report.py",
    "scripts/verification/spec_completeness_cli.py",
    "scripts/verification/spec_completeness_contract.py",
    "scripts/verification/spec_completeness_live.py",
    "scripts/verification/spec_completeness_report.py",
    "scripts/verification/metadata_validator/spec_gap_closure.py",
    "verification/ignored-tests.toml",
    "verification/matrix.toml",
    "verification/risk-register.toml",
    "verification/spec-matrix.toml",
    "verification/schemas/catalog.schema.json",
    "verification/schemas/evidence.schema.json",
    "verification/schemas/ignored-test.schema.json",
    "verification/schemas/invariant.schema.json",
    "verification/schemas/matrix.schema.json",
    "verification/schemas/risk-register.schema.json",
    "verification/schemas/spec-completeness-report.schema.json",
    "verification/schemas/spec-gap.schema.json",
    "verification/schemas/spec-matrix.schema.json",
    "verification/schemas/spec-source.schema.json",
    "verification/spec-gaps.toml",
    "verification/spec-sources.toml",
    "verification/test-catalog.toml",
)
LIMITATIONS = (
    "The invariant, catalog, coverage-cell, and bytecode pilot sections are exhaustive only for committed verification metadata at the bound source revision.",
    "The bytecode pilot denominator is exactly the union of open bytecode_vm spec-gap records and required bytecode_vm test-class slots lacking an effectively runnable, non-ignored catalog row.",
    "The pilot does not infer hardware/tool-blocked or not-applicable entries; those classifications remain zero unless a future reviewed metadata source extends the denominator contract.",
    "A test is oracle-bound only by a non-empty oracle_ref, spec_ref, or spec_gap_ref field; names, paths, expected-result prose, and inferred references never create a binding.",
    "Public-claim rows in this report are registered-spec-source context only; the separate source audit inventories all rendered public prose, but semantic claim dispositions remain incomplete and VERIF-P4A-005 stays open.",
    "verification/evidence-index.toml is live-validated but excluded from the input digest to avoid a report-evidence digest cycle; close-out evidence relationships are recomputed at rest.",
    "Report debt is visibility, not proof, spec-gap closure, test adequacy, or CI enforcement.",
    "Platform is historical provenance requiring evidence review; at-rest validation cannot rederive a prior host.",
)


@dataclass(frozen=True)
class SpecCompletenessProvenance:
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
class SpecCompletenessReport:
    provenance: SpecCompletenessProvenance
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
        pilot = self.analysis["bytecode_pilot"]
        lines = [
            "# Specification Completeness Report",
            "",
            f"Generator: `{GENERATOR} v{GENERATOR_VERSION}`",
            f"Source revision: `{self.provenance.commit}`",
            f"Generated: `{self.provenance.timestamp}`",
            f"Platform: `{self.provenance.platform}`",
            f"Generated JSON SHA-256: `{json_digest}`",
            f"Input SHA-256: `{self.input_digest}`",
            "",
            "`complete` means the committed metadata was exhaustively analyzed under the",
            "declared scopes. It does not mean the specifications or tests are complete.",
            "",
            "## Summary",
            "",
            f"- Invariants: {summary['invariants_total']}",
            f"- Invariants without specified specs: {summary['invariants_without_spec']}",
            f"- Tests with expected results: {summary['expected_result_tests']}",
            f"- Tests without oracle/spec/gap binding: {summary['tests_without_oracle']}",
            f"- Coverage cells: {summary['coverage_cells']}",
            f"- Coverage cells marked spec_gap: {summary['spec_gap_cells']}",
            f"- Bytecode pilot gaps: {summary['bytecode_pilot_gaps']}",
            f"- Registered public-claim sources: {summary['registered_public_claims']}",
            "",
            "## Invariants Without Specified Specs",
            "",
            "| Invariant | Area | Risk | Invariant status | Spec status | Spec gaps |",
            "| --- | --- | --- | --- | --- | --- |",
        ]
        if self.analysis["invariants_without_spec"]:
            for row in self.analysis["invariants_without_spec"]:
                lines.append(
                    f"| `{row['invariant_id']}` | `{row['area']}` | `{row['risk']}` | "
                    f"`{row['invariant_status']}` | `{row['spec_status']}` | "
                    f"{_markdown_ids(row['spec_gap_refs'])} |"
                )
        else:
            lines.append("| none | - | - | - | - | - |")
        lines.extend(
            [
                "",
                "## Expected-Result Tests Without Oracle Binding",
                "",
                "| Test | Area | Class | Status | Missing bindings |",
                "| --- | --- | --- | --- | --- |",
            ]
        )
        if self.analysis["tests_without_oracle"]:
            for row in self.analysis["tests_without_oracle"]:
                lines.append(
                    f"| `{row['test_id']}` | `{row['area']}` | `{row['test_class']}` | "
                    f"`{row['status']}` | {_markdown_ids(row['missing_bindings'])} |"
                )
        else:
            lines.append("| none | - | - | - | - |")
        lines.extend(
            [
                "",
                "## Spec-Gap Coverage Cells",
                "",
                "| Invariant | Area | Risk | Cell | Dimension | Spec gap |",
                "| --- | --- | --- | ---: | --- | --- |",
            ]
        )
        if self.analysis["spec_gap_cells"]:
            for row in self.analysis["spec_gap_cells"]:
                lines.append(
                    f"| `{row['invariant_id']}` | `{row['area']}` | `{row['risk']}` | "
                    f"{row['cell_index']} | `{row['dimension']}` | `{row['spec_gap_ref']}` |"
                )
        else:
            lines.append("| none | - | - | - | - | - |")
        lines.extend(
            [
                "",
                "## Bytecode/VM Pilot Gap Classification",
                "",
                f"Denominator: `{pilot['denominator']['basis']}`",
                "",
            ]
        )
        for classification in PILOT_CLASSIFICATIONS:
            lines.append(
                f"- `{classification}`: {pilot['summary']['by_classification'][classification]}"
            )
        lines.extend(
            [
                "",
                "| Gap | Classification | Source kind | Detail | Related records |",
                "| --- | --- | --- | --- | --- |",
            ]
        )
        for row in pilot["gaps"]:
            lines.append(
                f"| `{row['gap_id']}` | `{row['classification']}` | `{row['source_kind']}` | "
                f"{row['detail']} | {_markdown_ids(row['related_record_ids'])} |"
            )
        context = self.analysis["public_claim_context"]
        lines.extend(
            [
                "",
                "## Registered Public-Claim Context",
                "",
                f"Basis: `{context['basis']}`. Exhaustive public-doc scan: `no`.",
                "",
                "| Source | Area | Status | Surface | Invariants | Oracles | Spec gaps |",
                "| --- | --- | --- | --- | --- | --- | --- |",
            ]
        )
        for row in context["claims"]:
            lines.append(
                f"| `{row['source_id']}` | `{row['area']}` | `{row['source_status']}` | "
                f"`{row['surface_ref']}` | {_markdown_ids(row['linked_invariant_ids'])} | "
                f"{_markdown_ids(row['oracle_invariant_ids'])} | "
                f"{_markdown_ids(row['linked_spec_gap_ids'])} |"
            )
        if not context["claims"]:
            lines.append("| none | - | - | - | - | - | - |")
        lines.extend(["", "## Limitations", ""])
        lines.extend(f"- {item}" for item in self.analysis["limitations"])
        return "\n".join(lines) + "\n"


def analyze_spec_completeness(
    *,
    invariants: Mapping[str, Mapping[str, Any]],
    tests: Mapping[str, Mapping[str, Any]],
    ignored_tests: Mapping[str, Mapping[str, Any]],
    spec_gaps: Mapping[str, Mapping[str, Any]],
    spec_sources: Mapping[str, Mapping[str, Any]],
    matrix: Mapping[str, Any],
) -> dict[str, Any]:
    """Derive completeness debt solely from explicit committed metadata."""

    invariant_rows = _invariants_without_spec(invariants)
    test_rows = _tests_without_oracle(tests)
    cell_rows, cell_total = _spec_gap_cells(invariants)
    pilot = _bytecode_pilot(
        tests=tests,
        ignored_tests=ignored_tests,
        spec_gaps=spec_gaps,
        matrix=matrix,
    )
    public_context = _public_claim_context(
        invariants=invariants,
        spec_gaps=spec_gaps,
        spec_sources=spec_sources,
    )
    expected_result_tests = sum(
        isinstance(record.get("expected_result"), str)
        for record in tests.values()
    )
    return {
        "scope": {
            "invariant_basis": "all_committed_invariant_records",
            "test_oracle_basis": "catalog_rows_with_expected_result",
            "coverage_basis": "all_committed_invariant_coverage_cells",
            "bytecode_pilot_basis": "open_spec_gaps_union_missing_required_runnable_test_classes",
            "public_claim_basis": "registered_spec_sources_non_exhaustive_context",
            "debt_is_report_failure": False,
        },
        "summary": {
            "invariants_total": len(invariants),
            "invariants_without_spec": len(invariant_rows),
            "expected_result_tests": expected_result_tests,
            "tests_without_oracle": len(test_rows),
            "coverage_cells": cell_total,
            "spec_gap_cells": len(cell_rows),
            "bytecode_pilot_gaps": pilot["summary"]["total"],
            "registered_public_claims": len(public_context["claims"]),
        },
        "invariants_without_spec": invariant_rows,
        "tests_without_oracle": test_rows,
        "spec_gap_cells": cell_rows,
        "bytecode_pilot": pilot,
        "public_claim_context": public_context,
        "limitations": list(LIMITATIONS),
    }


def write_reports(
    report: SpecCompletenessReport,
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


def _invariants_without_spec(
    invariants: Mapping[str, Mapping[str, Any]],
) -> list[dict[str, Any]]:
    result: list[dict[str, Any]] = []
    for invariant_id, record in sorted(invariants.items()):
        spec = record.get("spec", {})
        status = spec.get("status") if isinstance(spec, Mapping) else None
        if status == "specified":
            continue
        result.append(
            {
                "invariant_id": invariant_id,
                "area": record.get("area"),
                "risk": record.get("risk"),
                "invariant_status": record.get("status"),
                "spec_status": status,
                "spec_source_refs": sorted(_string_list(spec.get("source_refs", []))),
                "spec_gap_refs": sorted(_string_list(record.get("spec_gap_refs", []))),
            }
        )
    return result


def _tests_without_oracle(
    tests: Mapping[str, Mapping[str, Any]],
) -> list[dict[str, Any]]:
    result: list[dict[str, Any]] = []
    for test_id, record in sorted(tests.items()):
        if not isinstance(record.get("expected_result"), str):
            continue
        present = [
            field
            for field in ORACLE_BINDING_FIELDS
            if isinstance(record.get(field), str) and bool(record[field])
        ]
        if present:
            continue
        result.append(
            {
                "test_id": test_id,
                "area": record.get("area"),
                "test_class": record.get("test_class"),
                "status": record.get("status"),
                "missing_bindings": list(ORACLE_BINDING_FIELDS),
            }
        )
    return result


def _spec_gap_cells(
    invariants: Mapping[str, Mapping[str, Any]],
) -> tuple[list[dict[str, Any]], int]:
    result: list[dict[str, Any]] = []
    total = 0
    for invariant_id, record in sorted(invariants.items()):
        coverage = record.get("coverage", {})
        cells = coverage.get("cells", []) if isinstance(coverage, Mapping) else []
        if not isinstance(cells, list):
            continue
        total += len(cells)
        for index, cell in enumerate(cells):
            if not isinstance(cell, Mapping) or cell.get("state") != "spec_gap":
                continue
            result.append(
                {
                    "invariant_id": invariant_id,
                    "area": record.get("area"),
                    "risk": record.get("risk"),
                    "invariant_status": record.get("status"),
                    "cell_index": index,
                    "dimension": cell.get("dimension"),
                    "spec_gap_ref": cell.get("spec_gap_ref"),
                    "rationale": cell.get("rationale"),
                }
            )
    return result, total


def _bytecode_pilot(
    *,
    tests: Mapping[str, Mapping[str, Any]],
    ignored_tests: Mapping[str, Mapping[str, Any]],
    spec_gaps: Mapping[str, Mapping[str, Any]],
    matrix: Mapping[str, Any],
) -> dict[str, Any]:
    gaps: list[dict[str, Any]] = []
    for gap_id, record in sorted(spec_gaps.items()):
        if record.get("area") != PILOT_AREA:
            continue
        if record.get("resolution_status") not in OPEN_GAP_RESOLUTIONS:
            continue
        gaps.append(
            {
                "gap_id": gap_id,
                "classification": "spec_gap",
                "source_kind": "spec_gap_record",
                "area": PILOT_AREA,
                "risk": record.get("risk"),
                "detail": record.get("blocking_question"),
                "related_record_ids": sorted(_string_list(record.get("affected_invariants", []))),
            }
        )

    ignored_test_ids = {
        record["test_id"]
        for record in ignored_tests.values()
        if isinstance(record.get("test_id"), str)
    }
    ignored_discovery_ids = {
        record["discovery_id"]
        for record in ignored_tests.values()
        if isinstance(record.get("discovery_id"), str)
    }
    ignored_test_ids.update(
        test_id
        for test_id, record in tests.items()
        if isinstance(record.get("discovery_id"), str)
        and record["discovery_id"] in ignored_discovery_ids
    )
    area = _mapped_matrix_area(matrix, PILOT_AREA)
    required_classes = area.get("required_test_classes", [])
    if not isinstance(required_classes, list) or not all(
        isinstance(item, str) for item in required_classes
    ):
        raise ValueError("bytecode_vm matrix area has invalid required_test_classes")
    if len(required_classes) != len(set(required_classes)):
        raise ValueError("bytecode_vm matrix area duplicates required_test_classes")
    by_class: dict[str, list[tuple[str, Mapping[str, Any]]]] = defaultdict(list)
    for test_id, record in sorted(tests.items()):
        if record.get("area") == PILOT_AREA and isinstance(record.get("test_class"), str):
            by_class[record["test_class"]].append((test_id, record))
    for test_class in sorted(required_classes):
        records = by_class.get(test_class, [])
        runnable = [
            test_id
            for test_id, record in records
            if record.get("status") in RUNNABLE_TEST_STATUSES and test_id not in ignored_test_ids
        ]
        if runnable:
            continue
        related = sorted(test_id for test_id, _ in records)
        detail = (
            f"Required test class {test_class} has catalog rows but none are effectively runnable."
            if related
            else f"Required test class {test_class} has no catalog row."
        )
        gaps.append(
            {
                "gap_id": f"TEST_CLASS_GAP:{PILOT_AREA}:{test_class}",
                "classification": "test_gap",
                "source_kind": "required_test_class_slot",
                "area": PILOT_AREA,
                "risk": None,
                "detail": detail,
                "related_record_ids": related,
            }
        )
    gaps.sort(key=lambda item: item["gap_id"])
    ids = [item["gap_id"] for item in gaps]
    if len(ids) != len(set(ids)):
        raise ValueError("bytecode pilot denominator produced duplicate gap IDs")
    counts = Counter(item["classification"] for item in gaps)
    return {
        "denominator": {
            "area": PILOT_AREA,
            "basis": "open_spec_gaps_union_missing_required_runnable_test_classes",
            "open_resolution_statuses": sorted(OPEN_GAP_RESOLUTIONS),
            "runnable_test_statuses": sorted(RUNNABLE_TEST_STATUSES),
            "ignored_catalog_tests_are_runnable": False,
            "hardware_tool_or_na_inference": False,
        },
        "summary": {
            "total": len(gaps),
            "by_classification": {
                name: counts[name] for name in PILOT_CLASSIFICATIONS
            },
        },
        "gaps": gaps,
    }


def _public_claim_context(
    *,
    invariants: Mapping[str, Mapping[str, Any]],
    spec_gaps: Mapping[str, Mapping[str, Any]],
    spec_sources: Mapping[str, Mapping[str, Any]],
) -> dict[str, Any]:
    claims: list[dict[str, Any]] = []
    for source_id, source in sorted(spec_sources.items()):
        if source.get("authority") != "public_claim":
            continue
        linked_invariants = sorted(
            invariant_id
            for invariant_id, invariant in invariants.items()
            if source_id in invariant.get("spec", {}).get("source_refs", [])
        )
        oracle_invariants = sorted(
            invariant_id
            for invariant_id, invariant in invariants.items()
            if invariant.get("oracle", {}).get("ref") == source_id
        )
        linked_gaps = sorted(
            gap_id
            for gap_id, gap in spec_gaps.items()
            if source_id in gap.get("candidate_spec_sources", [])
        )
        claims.append(
            {
                "source_id": source_id,
                "area": source.get("area"),
                "source_status": source.get("source_status"),
                "surface_ref": source.get("surface_ref"),
                "linked_invariant_ids": linked_invariants,
                "oracle_invariant_ids": oracle_invariants,
                "linked_spec_gap_ids": linked_gaps,
            }
        )
    return {
        "basis": "registered_spec_sources_only",
        "exhaustive": False,
        "claims": claims,
    }


def _mapped_matrix_area(matrix: Mapping[str, Any], area_id: str) -> Mapping[str, Any]:
    matches = [
        area
        for area in matrix.get("areas", [])
        if isinstance(area, Mapping)
        and area.get("id") == area_id
        and area.get("status") == "mapped"
    ]
    if len(matches) != 1:
        raise ValueError(f"expected exactly one mapped matrix area {area_id}, found {len(matches)}")
    return matches[0]


def _string_list(value: Any) -> list[str]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, str)]


def _markdown_ids(values: list[str]) -> str:
    return ", ".join(f"`{value}`" for value in values) if values else "none"
