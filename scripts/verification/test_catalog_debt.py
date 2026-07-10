"""Deterministic unmapped-test debt analysis and report rendering."""

from __future__ import annotations

import hashlib
import json
from collections import Counter
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .test_catalog_models import InferredTestFact


GENERATOR = "unmapped-test-debt"
GENERATOR_VERSION = 1
DEFAULT_JSON_PATH = Path("target/gate-artifacts/verification/unmapped-test-debt.json")
DEFAULT_MARKDOWN_PATH = Path("target/gate-artifacts/verification/unmapped-test-debt.md")
SUPPORTED_ARTIFACT_KINDS = {"case_table_artifact", "mutation_shard_runner"}
INFERENCE_PROHIBITED = (
    "area",
    "expected_result",
    "invariant",
    "oracle",
    "test_class",
)
LIMITATIONS = (
    "Debt is the exact subtraction of reviewed generated_test discovery IDs from current scanner facts.",
    "Case-table and mutation-runner artifacts never classify scanner facts.",
    "Ignored and conditionally ignored scanner facts remain visible when they are unmapped.",
    "An unmapped fact is catalog debt, not evidence about expected behavior or test adequacy.",
    "No area, class, invariant, oracle, or expected behavior is inferred from a name or path.",
    "Scanner exclusions remain those documented by the generated existing-test catalog.",
    "Nonzero debt does not fail this report-only command or change CI enforcement.",
    "Platform is historical provenance requiring evidence review; at-rest validation cannot rederive a prior host.",
)
REPORT_CONTRACT_PATHS = (
    "scripts/report_unmapped_test_debt.py",
    "scripts/validate_unmapped_test_debt_report.py",
    "scripts/verification/metadata_validator/constants.py",
    "scripts/verification/metadata_validator/core.py",
    "scripts/verification/metadata_validator/integrity.py",
    "scripts/verification/metadata_validator/oracle_refs.py",
    "scripts/verification/metadata_validator/schema_contracts.py",
    "scripts/verification/test_catalog_common.py",
    "scripts/verification/test_catalog_debt.py",
    "scripts/verification/test_catalog_debt_cli.py",
    "scripts/verification/test_catalog_debt_validation.py",
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
    "verification/schemas/catalog.schema.json",
    "verification/schemas/generated-test-catalog.schema.json",
    "verification/schemas/unmapped-test-debt-report.schema.json",
)


@dataclass(frozen=True)
class UnmappedDebtProvenance:
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
class UnmappedTestDebtReport:
    provenance: UnmappedDebtProvenance
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
            "# Unmapped Test Debt Report",
            "",
            f"Generator: `{GENERATOR} v{GENERATOR_VERSION}`",
            f"Source revision: `{self.provenance.commit}`",
            f"Generated: `{self.provenance.timestamp}`",
            f"Platform: `{self.provenance.platform}`",
            f"Generated JSON SHA-256: `{json_digest}`",
            f"Input SHA-256: `{self.input_digest}`",
            "",
            "`complete` means the source inventory and exact catalog subtraction succeeded.",
            "It does not mean that every scanner fact has reviewed catalog intent.",
            "",
            "## Summary",
            "",
            f"- Scanner facts: {summary['scanner_facts']}",
            f"- Mapped scanner facts: {summary['mapped_scanner_facts']}",
            f"- Unmapped scanner facts: {summary['unmapped_scanner_facts']}",
            f"- Generated-test catalog rows: {summary['generated_test_catalog_rows']}",
            f"- Artifact catalog rows: {summary['artifact_catalog_rows']}",
            f"- Ignored unmapped facts: {summary['ignored_unmapped_scanner_facts']}",
            f"- Conditional unmapped facts: {summary['conditional_unmapped_scanner_facts']}",
            "- Debt fails this report: no",
            "",
            "| Source kind | Scanner facts | Mapped | Unmapped |",
            "| --- | ---: | ---: | ---: |",
        ]
        for row in summary["by_source_kind"]:
            lines.append(
                f"| `{_markdown_cell(row['source_kind'])}` | {row['scanner_facts']} | "
                f"{row['mapped']} | {row['unmapped']} |"
            )
        lines.extend(
            [
                "",
                "## Unmapped Scanner Facts",
                "",
                "| Discovery ID | Source kind | Path | Name | Ignore state |",
                "| --- | --- | --- | --- | --- |",
            ]
        )
        for row in self.analysis["unmapped_tests"]:
            lines.append(
                f"| `{_markdown_cell(row['discovery_id'])}` | "
                f"`{_markdown_cell(row['source_kind'])}` | "
                f"`{_markdown_cell(row['path'])}` | "
                f"`{_markdown_cell(row['name'])}` | "
                f"`{_markdown_cell(row['ignore_state'])}` |"
            )
        if not self.analysis["unmapped_tests"]:
            lines.append("| none | none | none | none | none |")
        lines.extend(["", "## Limitations", ""])
        lines.extend(f"- {item}" for item in self.analysis["limitations"])
        return "\n".join(lines) + "\n"


def analyze_unmapped_test_debt(
    *,
    tests: Sequence[Mapping[str, Any]],
    facts: Sequence[InferredTestFact],
) -> dict[str, Any]:
    """Subtract exact reviewed generated-test discovery identities from scanner facts."""

    facts_by_id: dict[str, InferredTestFact] = {}
    for fact in facts:
        if fact.stable_id in facts_by_id:
            raise ValueError(f"scanner duplicates discovery id {fact.stable_id}")
        facts_by_id[fact.stable_id] = fact

    test_ids: set[str] = set()
    mapped_by_discovery_id: dict[str, str] = {}
    artifact_rows = 0
    for record in tests:
        test_id = record.get("id")
        if not isinstance(test_id, str) or not test_id:
            raise ValueError("catalog record lacks a string id")
        if test_id in test_ids:
            raise ValueError(f"catalog duplicates test id {test_id}")
        test_ids.add(test_id)
        subject_kind = record.get("subject_kind")
        if subject_kind == "generated_test":
            discovery_id = record.get("discovery_id")
            if not isinstance(discovery_id, str) or not discovery_id:
                raise ValueError(f"{test_id} generated_test lacks discovery_id")
            if discovery_id not in facts_by_id:
                raise ValueError(
                    f"{test_id} discovery_id is absent from current scanner facts: {discovery_id}"
                )
            owner = mapped_by_discovery_id.get(discovery_id)
            if owner is not None:
                raise ValueError(
                    f"scanner fact {discovery_id} is classified by both {owner} and {test_id}"
                )
            mapped_by_discovery_id[discovery_id] = test_id
        elif subject_kind in SUPPORTED_ARTIFACT_KINDS:
            artifact_rows += 1
        else:
            raise ValueError(f"{test_id} has unsupported subject_kind {subject_kind!r}")

    unmapped = sorted(
        (fact for discovery_id, fact in facts_by_id.items() if discovery_id not in mapped_by_discovery_id),
        key=_fact_identity_key,
    )
    unmapped_rows = [
        {
            "discovery_id": fact.stable_id,
            "source_kind": fact.source_kind,
            "path": fact.path,
            "name": fact.name,
            "ignore_state": fact.ignore_state,
        }
        for fact in unmapped
    ]
    fact_counts = Counter(fact.source_kind for fact in facts_by_id.values())
    mapped_counts = Counter(
        facts_by_id[discovery_id].source_kind for discovery_id in mapped_by_discovery_id
    )
    source_kinds = sorted(fact_counts)
    summary = {
        "scanner_facts": len(facts_by_id),
        "mapped_scanner_facts": len(mapped_by_discovery_id),
        "unmapped_scanner_facts": len(unmapped_rows),
        "generated_test_catalog_rows": len(mapped_by_discovery_id),
        "artifact_catalog_rows": artifact_rows,
        "ignored_unmapped_scanner_facts": sum(
            row["ignore_state"] == "ignored" for row in unmapped_rows
        ),
        "conditional_unmapped_scanner_facts": sum(
            row["ignore_state"] == "conditional" for row in unmapped_rows
        ),
        "by_source_kind": [
            {
                "source_kind": source_kind,
                "scanner_facts": fact_counts[source_kind],
                "mapped": mapped_counts[source_kind],
                "unmapped": fact_counts[source_kind] - mapped_counts[source_kind],
            }
            for source_kind in source_kinds
        ],
    }
    return {
        "scope": {
            "classification_basis": "exact_generated_test_discovery_id_subtraction",
            "artifact_rows_classify_facts": False,
            "debt_is_report_failure": False,
            "inference_prohibited": list(INFERENCE_PROHIBITED),
        },
        "summary": summary,
        "unmapped_tests": unmapped_rows,
        "limitations": list(LIMITATIONS),
    }


def write_reports(
    report: UnmappedTestDebtReport,
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


def _fact_identity_key(fact: InferredTestFact) -> tuple[str, str, str, str]:
    return (fact.source_kind, fact.path, fact.name, fact.stable_id)


def _markdown_cell(value: str) -> str:
    return value.replace("\\", "\\\\").replace("|", "\\|").replace("`", "\\`").replace("\n", "\\n")
