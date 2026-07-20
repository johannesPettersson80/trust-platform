"""Deterministic malformed-input coverage analysis and rendering."""

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


GENERATOR = "malformed-input-coverage"
GENERATOR_VERSION = 1
DEFAULT_JSON_PATH = Path("target/gate-artifacts/verification/malformed-input-coverage.json")
DEFAULT_MARKDOWN_PATH = Path("target/gate-artifacts/verification/malformed-input-coverage.md")
COVERAGE_STATES = (
    "covered",
    "covered_by_fuzz",
    "not_applicable",
    "blocked",
    "spec_gap",
    "gap_open",
    "deferred",
)
REPORT_CONTRACT_PATHS = (
    "docs/specs/12-bytecode.md",
    "scripts/report_malformed_input_coverage.py",
    "scripts/validate_malformed_input_coverage_report.py",
    "scripts/verification/malformed_input_contract.py",
    "scripts/verification/malformed_input_coverage.py",
    "scripts/verification/malformed_input_coverage_cli.py",
    "scripts/verification/malformed_input_coverage_validation.py",
    "scripts/verification/metadata_validator/constants.py",
    "scripts/verification/metadata_validator/core.py",
    "scripts/verification/metadata_validator/integrity.py",
    "scripts/verification/metadata_validator/schema_contracts.py",
    "scripts/verification/test_catalog_common.py",
    "scripts/verification/test_catalog_intent.py",
    "scripts/verification/test_catalog_json_schema.py",
    "scripts/verification/test_catalog_models.py",
    "scripts/verification/test_catalog_scanner.py",
    "scripts/verification/test_catalog_staleness.py",
    "scripts/verification/test_catalog_rust.py",
    "scripts/verification/test_catalog_st.py",
    "scripts/verification/test_catalog_surfaces.py",
    "scripts/verification/test_catalog_validation.py",
    "scripts/verification/test_catalog_vscode.py",
    "verification/malformed-input-taxonomy.md",
    "verification/malformed-input-taxonomy.toml",
    "verification/schemas/catalog.schema.json",
    "verification/schemas/malformed-input-coverage-report.schema.json",
    "verification/schemas/malformed-input-taxonomy.schema.json",
    "verification/spec-gaps.toml",
    "verification/spec-sources.toml",
    "verification/test-catalog.toml",
)
LIMITATIONS = (
    "The v1 machine taxonomy covers only the inventoried bytecode_vm area and bytecode container/instruction-stream surface.",
    "Mappings come only from reviewed malformed_input_class_ids on generated native or fuzz rows.",
    "Names, paths, commands, lexical references, case IDs, and mutation associations never create coverage.",
    "A spec-gap disposition remains spec_gap even when an associated test exists.",
    "Covered means an explicit effectively runnable catalog mapping exists; it is not behavior proof or spec-gap closure.",
    "Unmapped classes and tests are report debt and do not make generation fail.",
    "Platform is historical provenance requiring evidence review; at-rest validation cannot rederive a prior host.",
)


@dataclass(frozen=True)
class MalformedCoverageProvenance:
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
class MalformedInputCoverageReport:
    provenance: MalformedCoverageProvenance
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
            "# Malformed-Input Coverage Report",
            "",
            f"Generator: `{GENERATOR} v{GENERATOR_VERSION}`",
            f"Source revision: `{self.provenance.commit}`",
            f"Generated: `{self.provenance.timestamp}`",
            f"Platform: `{self.provenance.platform}`",
            f"Generated JSON SHA-256: `{json_digest}`",
            f"Input SHA-256: `{self.input_digest}`",
            "",
            "`complete` means the reviewed taxonomy and live joins validated. It does not",
            "mean every malformed-input class is covered.",
            "",
            "## Summary",
            "",
            f"- Taxonomy classes: {summary['taxonomy_classes']}",
            f"- Classes with catalog mappings: {summary['mapped_classes']}",
            f"- Explicit test mappings: {summary['test_mappings']}",
        ]
        for state in COVERAGE_STATES:
            lines.append(f"- `{state}`: {summary['by_state'][state]}")
        lines.extend(
            [
                "",
                "## Classes",
                "",
                "| Class | Disposition | State | Runnable tests | Fuzz tests | Non-runnable tests | Open spec gaps |",
                "| --- | --- | --- | --- | --- | --- | --- |",
            ]
        )
        for item in self.analysis["classes"]:
            lines.append(
                f"| `{item['class_id']}` | `{item['disposition']}` | `{item['state']}` | "
                f"{_markdown_ids(item['runnable_test_ids'])} | "
                f"{_markdown_ids(item['fuzz_test_ids'])} | "
                f"{_markdown_ids(item['non_runnable_test_ids'])} | "
                f"{_markdown_ids(item['open_spec_gap_refs'])} |"
            )
        lines.extend(["", "## Limitations", ""])
        lines.extend(f"- {item}" for item in self.analysis["limitations"])
        return "\n".join(lines) + "\n"


def analyze_malformed_input_coverage(
    *,
    taxonomy: Mapping[str, Any],
    tests: Sequence[Mapping[str, Any]],
    facts: Sequence[InferredTestFact],
) -> dict[str, Any]:
    """Derive coverage only from reviewed taxonomy and explicit catalog bindings."""

    tests_by_id = _unique_tests(tests)
    facts_by_id = _unique_facts(facts)
    mappings: dict[str, list[Mapping[str, Any]]] = defaultdict(list)
    for test_id, record in sorted(tests_by_id.items()):
        class_ids = record.get("malformed_input_class_ids")
        if not isinstance(class_ids, list):
            continue
        if record.get("subject_kind") != "generated_test":
            raise ValueError(f"{test_id} malformed-input mapping is not a generated_test")
        for class_id in class_ids:
            mappings[class_id].append(record)

    class_reports: list[dict[str, Any]] = []
    state_counts: Counter[str] = Counter()
    test_mapping_count = 0
    for malformed_class in taxonomy.get("classes", []):
        class_id = malformed_class["id"]
        records = sorted(mappings.get(class_id, []), key=lambda item: item["id"])
        effective: list[Mapping[str, Any]] = []
        non_runnable: list[str] = []
        for record in records:
            discovery_id = record.get("discovery_id")
            fact = facts_by_id.get(discovery_id) if isinstance(discovery_id, str) else None
            if fact is None:
                raise ValueError(f"{record['id']} does not resolve a scanner fact")
            if record.get("status") not in RUNNABLE_TEST_STATUSES or fact.ignore_state != "not_ignored":
                non_runnable.append(record["id"])
            else:
                effective.append(record)
        normal_ids = sorted(record["id"] for record in effective if record.get("test_class") != "fuzz")
        fuzz_ids = sorted(record["id"] for record in effective if record.get("test_class") == "fuzz")
        disposition = malformed_class["disposition"]
        state = _derive_state(disposition, normal_ids, fuzz_ids)
        taxonomy_gap = malformed_class.get("spec_gap_ref")
        open_gaps = {taxonomy_gap} if isinstance(taxonomy_gap, str) else set()
        open_gaps.update(
            record.get("spec_gap_ref")
            for record in records
            if isinstance(record.get("spec_gap_ref"), str)
        )
        test_mapping_count += len(records)
        state_counts[state] += 1
        class_reports.append(
            {
                "class_id": class_id,
                "title": malformed_class["title"],
                "disposition": disposition,
                "state": state,
                "mapped_test_ids": sorted(record["id"] for record in records),
                "runnable_test_ids": normal_ids,
                "fuzz_test_ids": fuzz_ids,
                "non_runnable_test_ids": sorted(non_runnable),
                "open_spec_gap_refs": sorted(open_gaps),
                "rationale": malformed_class["rationale"],
            }
        )
    summary = {
        "taxonomy_classes": len(class_reports),
        "mapped_classes": sum(1 for item in class_reports if item["mapped_test_ids"]),
        "test_mappings": test_mapping_count,
        "by_state": {state: state_counts[state] for state in COVERAGE_STATES},
    }
    return {
        "scope": {
            "area": taxonomy.get("area"),
            "surface_id": taxonomy.get("surface_id"),
            "mapping_basis": "explicit_generated_test_malformed_input_class_ids",
            "debt_is_report_failure": False,
            "coverage_states": list(COVERAGE_STATES),
        },
        "summary": summary,
        "classes": class_reports,
        "limitations": list(LIMITATIONS),
    }


def write_reports(
    report: MalformedInputCoverageReport,
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


def _derive_state(disposition: str, normal_ids: list[str], fuzz_ids: list[str]) -> str:
    if disposition != "required":
        return disposition
    if normal_ids:
        return "covered"
    if fuzz_ids:
        return "covered_by_fuzz"
    return "gap_open"


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


def _markdown_ids(values: Sequence[str]) -> str:
    return ", ".join(f"`{value}`" for value in values) if values else "none"
