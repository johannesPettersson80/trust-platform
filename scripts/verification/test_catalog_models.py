"""Data model and deterministic rendering for generated test catalogs."""

from __future__ import annotations

import hashlib
import json
from collections import Counter
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any


GENERATOR = "test-catalog-scanner"
GENERATOR_VERSION = 1
DEFAULT_JSON_PATH = Path("target/gate-artifacts/verification/existing-test-catalog.json")
DEFAULT_MARKDOWN_PATH = Path("target/gate-artifacts/verification/existing-test-catalog.md")
HAND_OWNED_FIELDS = [
    "area",
    "owner",
    "status",
    "test_class",
    "invariants",
    "expected_result",
    "suite_tiers",
    "requires_hardware",
    "requires_network",
    "duration_class",
    "oracle_ref",
    "expected_failure_mode",
    "evidence_destination",
]


@dataclass(frozen=True)
class InferredTestFact:
    stable_id: str
    native_id: str
    source_kind: str
    name: str
    path: str
    line: int
    package: str | None
    command_hint: str
    command_hint_authority: str
    discovery_confidence: str
    ignore_state: str
    ignore_reason: str | None
    reference_candidates: tuple[str, ...]
    provenance: str = "inferred"

    def to_dict(self) -> dict[str, Any]:
        record = asdict(self)
        record["reference_candidates"] = list(self.reference_candidates)
        return record


@dataclass(frozen=True)
class ScanDiagnostic:
    severity: str
    kind: str
    path: str
    line: int
    message: str

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


@dataclass(frozen=True)
class ReportProvenance:
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
class GeneratedTestCatalog:
    provenance: ReportProvenance
    input_digest: str
    inferred_facts: tuple[InferredTestFact, ...]
    diagnostics: tuple[ScanDiagnostic, ...]
    limitations: tuple[str, ...]

    def to_dict(self) -> dict[str, Any]:
        by_kind = Counter(fact.source_kind for fact in self.inferred_facts)
        errors = sum(1 for item in self.diagnostics if item.severity == "error")
        warnings = sum(1 for item in self.diagnostics if item.severity == "warning")
        return {
            "schema_version": 1,
            "generator": GENERATOR,
            "generator_version": GENERATOR_VERSION,
            "scan_status": "complete" if errors == 0 else "incomplete",
            "input_digest": self.input_digest,
            **self.provenance.to_dict(),
            "hand_owned_intent": {
                "included": False,
                "fields": HAND_OWNED_FIELDS,
            },
            "inferred_facts": [fact.to_dict() for fact in self.inferred_facts],
            "diagnostics": [item.to_dict() for item in self.diagnostics],
            "limitations": list(self.limitations),
            "summary": {
                "records": len(self.inferred_facts),
                "files": len({fact.path for fact in self.inferred_facts}),
                "ignored": sum(1 for fact in self.inferred_facts if fact.ignore_state == "ignored"),
                "conditional_ignores": sum(
                    1 for fact in self.inferred_facts if fact.ignore_state == "conditional"
                ),
                "diagnostics": len(self.diagnostics),
                "errors": errors,
                "warnings": warnings,
                "by_source_kind": dict(sorted(by_kind.items())),
            },
        }

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), indent=2, sort_keys=True) + "\n"

    def to_markdown(self, *, json_digest: str) -> str:
        payload = self.to_dict()
        summary = payload["summary"]
        lines = [
            "# Generated Existing-Test Catalog",
            "",
            f"Generator: `{GENERATOR} v{GENERATOR_VERSION}`",
            f"Source revision: `{self.provenance.commit}`",
            f"Generated: `{self.provenance.timestamp}`",
            f"Platform: `{self.provenance.platform}`",
            f"Generated JSON SHA-256: `{json_digest}`",
            f"Input SHA-256: `{self.input_digest}`",
            "",
            "This is a mechanical source inventory. It does not map tests to claims,",
            "infer expected behavior, or replace hand-owned test catalog metadata.",
            "",
            "## Summary",
            "",
            f"- Records: {summary['records']}",
            f"- Source files with records: {summary['files']}",
            f"- Ignored records: {summary['ignored']}",
            f"- Conditional ignore markers: {summary['conditional_ignores']}",
            f"- Visible scan diagnostics: {summary['diagnostics']}",
            f"- Scan errors: {summary['errors']}",
            f"- Scan warnings: {summary['warnings']}",
            "",
            "| Source kind | Records |",
            "| --- | ---: |",
        ]
        for kind, count in summary["by_source_kind"].items():
            lines.append(f"| `{kind}` | {count} |")
        lines.extend(
            [
                "",
                "## Hand-Owned Intent",
                "",
                "The generated JSON explicitly excludes:",
            ]
        )
        lines.extend(f"- `{field}`" for field in HAND_OWNED_FIELDS)
        lines.extend(["", "## Limitations", ""])
        lines.extend(f"- {item}" for item in self.limitations)
        if self.diagnostics:
            lines.extend(["", "## Diagnostics", ""])
            for item in self.diagnostics[:25]:
                lines.append(
                    f"- `{item.path}:{item.line}` `{item.severity}/{item.kind}`: {item.message}"
                )
            if len(self.diagnostics) > 25:
                lines.append(f"- {len(self.diagnostics) - 25} additional diagnostics are in the generated JSON.")
        return "\n".join(lines) + "\n"


def write_reports(
    report: GeneratedTestCatalog,
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
