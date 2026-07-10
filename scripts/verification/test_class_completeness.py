"""Deterministic test-class completeness analysis and rendering."""

from __future__ import annotations

import hashlib
import json
from collections import Counter, defaultdict
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .metadata_validator.integrity import RUNNABLE_TEST_STATUSES
from .test_catalog_models import InferredTestFact


GENERATOR = "test-class-completeness"
GENERATOR_VERSION = 1
DEFAULT_JSON_PATH = Path("target/gate-artifacts/verification/test-class-completeness.json")
DEFAULT_MARKDOWN_PATH = Path("target/gate-artifacts/verification/test-class-completeness.md")
REPORT_CONTRACT_PATHS = (
    "scripts/report_test_class_completeness.py",
    "scripts/validate_test_class_completeness_report.py",
    "scripts/verification/metadata_validator/constants.py",
    "scripts/verification/metadata_validator/core.py",
    "scripts/verification/metadata_validator/integrity.py",
    "scripts/verification/metadata_validator/schema_contracts.py",
    "scripts/verification/test_catalog_common.py",
    "scripts/verification/test_catalog_intent.py",
    "scripts/verification/test_catalog_json_schema.py",
    "scripts/verification/test_catalog_models.py",
    "scripts/verification/test_catalog_rust.py",
    "scripts/verification/test_catalog_scanner.py",
    "scripts/verification/test_catalog_st.py",
    "scripts/verification/test_catalog_staleness.py",
    "scripts/verification/test_catalog_surfaces.py",
    "scripts/verification/test_catalog_validation.py",
    "scripts/verification/test_catalog_vscode.py",
    "scripts/verification/test_class_completeness.py",
    "scripts/verification/test_class_completeness_cli.py",
    "scripts/verification/test_class_completeness_validation.py",
    "verification/schemas/catalog.schema.json",
    "verification/schemas/matrix.schema.json",
    "verification/schemas/test-class-completeness-report.schema.json",
)
INFERENCE_PROHIBITED = (
    "area",
    "expected_result",
    "invariant",
    "oracle",
    "test_class",
)
LIMITATIONS = (
    "A scanner fact is classified only by an exact discovery_id on a generated_test catalog row.",
    "Case-table and mutation-runner artifacts never enter the scanner-fact classification denominator.",
    "A required class is complete only when the mapped area has at least one catalog row in a runnable status.",
    "Planned and other non-runnable rows remain visible but do not satisfy required-class completeness.",
    "An unmapped fact or missing class is report debt, not proof that no relevant executable test exists.",
    "The report never infers area, class, invariant, oracle, or expected behavior from names or lexical references.",
    "Generated tests marked ignored or conditional by the scanner do not satisfy runnable class completeness.",
    "Scanner exclusions remain those documented by the generated existing-test catalog.",
    "Platform is historical provenance requiring evidence review; at-rest validation cannot rederive a prior host.",
)


@dataclass(frozen=True)
class CompletenessProvenance:
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
class TestClassCompletenessReport:
    provenance: CompletenessProvenance
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
        scanner = self.analysis["scanner_classification"]
        lines = [
            "# Test-Class Completeness Report",
            "",
            f"Generator: `{GENERATOR} v{GENERATOR_VERSION}`",
            f"Source revision: `{self.provenance.commit}`",
            f"Generated: `{self.provenance.timestamp}`",
            f"Platform: `{self.provenance.platform}`",
            f"Generated JSON SHA-256: `{json_digest}`",
            f"Input SHA-256: `{self.input_digest}`",
            "",
            "`complete` means the report was generated and bound successfully. It does not",
            "mean every scanner fact or required test class is mapped.",
            "",
            "## Summary",
            "",
            f"- Scanner facts: {summary['scanner_facts']}",
            f"- Classified scanner facts: {summary['classified_scanner_facts']}",
            f"- Unmapped scanner facts: {summary['unmapped_scanner_facts']}",
            f"- Catalog records: {summary['catalog_records']}",
            f"- Runnable catalog records: {summary['runnable_catalog_records']}",
            f"- Non-runnable catalog records: {summary['non_runnable_catalog_records']}",
            f"- Mapped areas: {summary['mapped_areas']}",
            f"- Required class slots: {summary['required_class_slots']}",
            f"- Complete required class slots: {summary['complete_class_slots']}",
            f"- Missing required class slots: {summary['missing_class_slots']}",
            "",
            "## Scanner Classification",
            "",
            "| Source kind | Facts | Classified | Unmapped |",
            "| --- | ---: | ---: | ---: |",
        ]
        for row in scanner["by_source_kind"]:
            lines.append(
                f"| `{row['source_kind']}` | {row['facts']} | "
                f"{row['classified']} | {row['unmapped']} |"
            )
        lines.extend(["", "Classified mappings:", ""])
        if scanner["classified_mappings"]:
            lines.extend(
                f"- `{row['discovery_id']}` -> `{row['test_id']}`"
                for row in scanner["classified_mappings"]
            )
        else:
            lines.append("- none")

        for area in self.analysis["areas"]:
            lines.extend(
                [
                    "",
                    f"## Area: `{area['area']}`",
                    "",
                    "| Required class | Runnable tests | Non-runnable rows | Complete |",
                    "| --- | --- | --- | --- |",
                ]
            )
            for item in area["required_classes"]:
                runnable = _markdown_ids(item["runnable_test_ids"])
                non_runnable = _markdown_non_runnable(item["non_runnable_tests"])
                complete = "yes" if item["complete"] else "no"
                lines.append(
                    f"| `{item['test_class']}` | {runnable} | {non_runnable} | {complete} |"
                )
            if area["additional_classes"]:
                lines.extend(["", "Additional catalog classes:", ""])
                for item in area["additional_classes"]:
                    lines.append(
                        f"- `{item['test_class']}`: runnable "
                        f"{_markdown_ids(item['runnable_test_ids'])}; non-runnable "
                        f"{_markdown_non_runnable(item['non_runnable_tests'])}"
                    )

        lines.extend(["", "## Limitations", ""])
        lines.extend(f"- {item}" for item in self.analysis["limitations"])
        return "\n".join(lines) + "\n"


def analyze_test_class_completeness(
    *,
    matrix: Mapping[str, Any],
    tests: Sequence[Mapping[str, Any]],
    facts: Sequence[InferredTestFact],
) -> dict[str, Any]:
    """Join reviewed catalog intent to scanner facts and matrix class requirements."""

    tests_by_id = _unique_tests(tests)
    facts_by_id = _unique_facts(facts)
    classified_mappings: list[dict[str, str]] = []
    classified_ids: set[str] = set()
    bound_facts_by_test: dict[str, InferredTestFact] = {}
    for test_id, record in sorted(tests_by_id.items()):
        if record.get("subject_kind") != "generated_test":
            continue
        discovery_id = record.get("discovery_id")
        if not isinstance(discovery_id, str) or discovery_id not in facts_by_id:
            raise ValueError(f"{test_id} does not resolve exactly one current scanner fact")
        if discovery_id in classified_ids:
            raise ValueError(f"scanner fact {discovery_id} is classified more than once")
        classified_ids.add(discovery_id)
        bound_facts_by_test[test_id] = facts_by_id[discovery_id]
        classified_mappings.append({"discovery_id": discovery_id, "test_id": test_id})

    fact_totals = Counter(fact.source_kind for fact in facts_by_id.values())
    classified_totals = Counter(facts_by_id[item].source_kind for item in classified_ids)
    source_kinds = sorted(fact_totals)
    scanner_classification = {
        "facts": len(facts_by_id),
        "classified_facts": len(classified_ids),
        "unmapped_facts": len(facts_by_id) - len(classified_ids),
        "classified_mappings": classified_mappings,
        "by_source_kind": [
            {
                "source_kind": source_kind,
                "facts": fact_totals[source_kind],
                "classified": classified_totals[source_kind],
                "unmapped": fact_totals[source_kind] - classified_totals[source_kind],
            }
            for source_kind in source_kinds
        ],
    }

    mapped_areas = _mapped_areas(matrix)
    area_reports: list[dict[str, Any]] = []
    required_slots = 0
    complete_slots = 0
    for area_id, area in mapped_areas.items():
        required = area.get("required_test_classes", [])
        if not isinstance(required, list) or not all(isinstance(item, str) for item in required):
            raise ValueError(f"mapped area {area_id} has invalid required_test_classes")
        if len(required) != len(set(required)):
            raise ValueError(f"mapped area {area_id} duplicates required_test_classes")
        required_set = set(required)
        by_class: dict[str, list[Mapping[str, Any]]] = defaultdict(list)
        for record in tests_by_id.values():
            if record.get("area") == area_id and isinstance(record.get("test_class"), str):
                by_class[record["test_class"]].append(record)

        required_reports = [
            _class_report(test_class, by_class.get(test_class, []), bound_facts_by_test)
            for test_class in sorted(required_set)
        ]
        additional_reports = [
            _class_report(test_class, by_class[test_class], bound_facts_by_test)
            for test_class in sorted(set(by_class) - required_set)
        ]
        required_slots += len(required_reports)
        complete_slots += sum(1 for item in required_reports if item["complete"])
        area_reports.append(
            {
                "area": area_id,
                "required_classes": required_reports,
                "additional_classes": additional_reports,
            }
        )

    runnable_records = sum(
        1
        for test_id, record in tests_by_id.items()
        if _non_runnable_reason(record, bound_facts_by_test.get(test_id)) is None
    )
    summary = {
        "scanner_facts": len(facts_by_id),
        "classified_scanner_facts": len(classified_ids),
        "unmapped_scanner_facts": len(facts_by_id) - len(classified_ids),
        "catalog_records": len(tests_by_id),
        "runnable_catalog_records": runnable_records,
        "non_runnable_catalog_records": len(tests_by_id) - runnable_records,
        "mapped_areas": len(mapped_areas),
        "required_class_slots": required_slots,
        "complete_class_slots": complete_slots,
        "missing_class_slots": required_slots - complete_slots,
    }
    return {
        "scope": {
            "classification_basis": "exact_generated_test_discovery_id",
            "class_completeness_basis": "mapped_area_required_classes_with_effectively_runnable_catalog_rows",
            "runnable_statuses": sorted(RUNNABLE_TEST_STATUSES),
            "excluded_scanner_ignore_states": ["conditional", "ignored"],
            "debt_is_report_failure": False,
            "inference_prohibited": list(INFERENCE_PROHIBITED),
        },
        "summary": summary,
        "scanner_classification": scanner_classification,
        "areas": area_reports,
        "limitations": list(LIMITATIONS),
    }


def write_reports(
    report: TestClassCompletenessReport,
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


def _unique_tests(tests: Sequence[Mapping[str, Any]]) -> dict[str, Mapping[str, Any]]:
    result: dict[str, Mapping[str, Any]] = {}
    for record in tests:
        test_id = record.get("id")
        if not isinstance(test_id, str) or not test_id:
            raise ValueError("catalog record lacks a string id")
        if test_id in result:
            raise ValueError(f"catalog duplicates test id {test_id}")
        result[test_id] = record
    return result


def _unique_facts(facts: Sequence[InferredTestFact]) -> dict[str, InferredTestFact]:
    result: dict[str, InferredTestFact] = {}
    for fact in facts:
        if fact.stable_id in result:
            raise ValueError(f"scanner duplicates discovery id {fact.stable_id}")
        result[fact.stable_id] = fact
    return result


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


def _class_report(
    test_class: str,
    records: Sequence[Mapping[str, Any]],
    bound_facts_by_test: Mapping[str, InferredTestFact],
) -> dict[str, Any]:
    runnable = sorted(
        record["id"]
        for record in records
        if _non_runnable_reason(record, bound_facts_by_test.get(record["id"])) is None
    )
    non_runnable = sorted(
        (
            {
                "test_id": record["id"],
                "status": str(record.get("status")),
                "reason": _non_runnable_reason(
                    record,
                    bound_facts_by_test.get(record["id"]),
                ),
            }
            for record in records
            if _non_runnable_reason(record, bound_facts_by_test.get(record["id"])) is not None
        ),
        key=lambda item: (item["test_id"], item["status"], item["reason"]),
    )
    return {
        "test_class": test_class,
        "runnable_test_ids": runnable,
        "non_runnable_tests": non_runnable,
        "complete": bool(runnable),
    }


def _non_runnable_reason(
    record: Mapping[str, Any],
    bound_fact: InferredTestFact | None,
) -> str | None:
    status = record.get("status")
    if status not in RUNNABLE_TEST_STATUSES:
        return f"catalog_status:{status}"
    if record.get("subject_kind") == "generated_test":
        if bound_fact is None:
            return "scanner_binding:missing"
        if bound_fact.ignore_state != "not_ignored":
            return f"scanner_ignore_state:{bound_fact.ignore_state}"
    return None


def _markdown_ids(values: Sequence[str]) -> str:
    return ", ".join(f"`{value}`" for value in values) if values else "none"


def _markdown_non_runnable(values: Sequence[Mapping[str, str]]) -> str:
    if not values:
        return "none"
    return ", ".join(
        f"`{item['test_id']}` ({item['status']}; {item['reason']})" for item in values
    )
