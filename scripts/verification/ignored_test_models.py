"""Closed data model and deterministic rendering for ignored-test inventory."""

from __future__ import annotations

import hashlib
import json
from collections import Counter
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, Mapping


GENERATOR = "ignored-test-inventory"
GENERATOR_VERSION = 1
DEFAULT_JSON_PATH = Path("target/gate-artifacts/verification/ignored-test-inventory.json")
DEFAULT_MARKDOWN_PATH = Path("target/gate-artifacts/verification/ignored-test-inventory.md")

SCOPE = {
    "classification_included": False,
    "rust_basis": "existing_test_catalog_declared_ignore_state",
    "vscode_basis": "existing_test_catalog_runtime_this_skip_diagnostic",
    "playwright_basis": "tracked_capture_spec_literal_test_skip",
    "shell_basis": "limitation_no_static_test_identity_convention",
    "conformance_basis": "limitation_runtime_skipped_is_not_source_ignore",
}


@dataclass(frozen=True)
class IgnoredTestFact:
    discovery_id: str
    native_id: str
    discovery_source_kind: str
    name: str
    path: str
    line: int
    package: str | None
    command_hint: str
    ignore_state: str
    ignore_mechanism: str
    ignore_reason: str
    reference_candidates: tuple[str, ...]

    def to_dict(self) -> dict[str, Any]:
        record = asdict(self)
        record["reference_candidates"] = list(self.reference_candidates)
        return record


@dataclass(frozen=True)
class InventoryDiagnostic:
    severity: str
    kind: str
    path: str
    line: int
    message: str

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


@dataclass(frozen=True)
class InventoryProvenance:
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
class IgnoredTestInventoryReport:
    provenance: InventoryProvenance
    input_digest: str
    records: tuple[IgnoredTestFact, ...]
    diagnostics: tuple[InventoryDiagnostic, ...]
    surface_summary: tuple[Mapping[str, Any], ...]
    limitations: tuple[str, ...]

    def to_dict(self) -> dict[str, Any]:
        records = sorted(
            (record.to_dict() for record in self.records),
            key=record_sort_key,
        )
        diagnostics = sorted(
            (item.to_dict() for item in self.diagnostics),
            key=diagnostic_sort_key,
        )
        surfaces = sorted(
            (dict(item) for item in self.surface_summary),
            key=lambda item: item["surface"],
        )
        errors = sum(item["severity"] == "error" for item in diagnostics)
        warnings = sum(item["severity"] == "warning" for item in diagnostics)
        by_kind = Counter(item["discovery_source_kind"] for item in records)
        return {
            "schema_version": 1,
            "generator": GENERATOR,
            "generator_version": GENERATOR_VERSION,
            "report_status": "complete" if errors == 0 else "incomplete",
            "input_digest": self.input_digest,
            **self.provenance.to_dict(),
            "scope": dict(SCOPE),
            "records": records,
            "diagnostics": diagnostics,
            "surface_summary": surfaces,
            "limitations": list(self.limitations),
            "summary": {
                "records": len(records),
                "ignored": sum(item["ignore_state"] == "ignored" for item in records),
                "conditional": sum(
                    item["ignore_state"] == "conditional" for item in records
                ),
                "diagnostics": len(diagnostics),
                "errors": errors,
                "warnings": warnings,
                "by_source_kind": [
                    {"source_kind": source_kind, "records": count}
                    for source_kind, count in sorted(by_kind.items())
                ],
            },
        }

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), indent=2, sort_keys=True) + "\n"

    def to_markdown(self, *, json_digest: str) -> str:
        return render_markdown(self.to_dict(), json_digest=json_digest)


def render_markdown(payload: Mapping[str, Any], *, json_digest: str) -> str:
    summary = payload["summary"]
    lines = [
        "# Ignored-Test Inventory",
        "",
        f"Generator: `{GENERATOR} v{GENERATOR_VERSION}`",
        f"Source revision: `{payload['commit']}`",
        f"Generated: `{payload['timestamp']}`",
        f"Platform: `{payload['platform']}`",
        f"Generated JSON SHA-256: `{json_digest}`",
        f"Input SHA-256: `{payload['input_digest']}`",
        "",
        "This report is a mechanical inventory. It does not classify an ignored test,",
        "establish expected behavior, or count as product proof.",
        "",
        "## Summary",
        "",
        f"- Records: {summary['records']}",
        f"- Statically ignored: {summary['ignored']}",
        f"- Conditional ignore observations: {summary['conditional']}",
        f"- Diagnostics: {summary['diagnostics']}",
        f"- Errors: {summary['errors']}",
        f"- Warnings: {summary['warnings']}",
        "",
        "| Source kind | Records |",
        "| --- | ---: |",
    ]
    for row in summary["by_source_kind"]:
        lines.append(f"| `{row['source_kind']}` | {row['records']} |")
    lines.extend(
        [
            "",
            "## Surface Coverage",
            "",
            "| Surface | Scanned files | Records | Ignored | Conditional | Coverage |",
            "| --- | ---: | ---: | ---: | ---: | --- |",
        ]
    )
    for row in payload["surface_summary"]:
        lines.append(
            f"| `{row['surface']}` | {row['scanned_files']} | {row['records']} | "
            f"{row['ignored']} | {row['conditional']} | `{row['coverage']}` |"
        )
    lines.extend(["", "Surface notes:", ""])
    for row in payload["surface_summary"]:
        lines.append(f"- `{row['surface']}`: {row['note']}")
    lines.extend(
        [
            "",
            "## Inventory",
            "",
            "| Discovery ID | State | Mechanism | Source | Path | Name | Reason |",
            "| --- | --- | --- | --- | --- | --- | --- |",
        ]
    )
    for row in payload["records"]:
        lines.append(
            f"| `{row['discovery_id']}` | `{row['ignore_state']}` | "
            f"`{row['ignore_mechanism']}` | `{row['discovery_source_kind']}` | "
            f"`{row['path']}:{row['line']}` | `{_cell(row['name'])}` | "
            f"{_cell(row['ignore_reason'])} |"
        )
    lines.extend(["", "## Limitations", ""])
    lines.extend(f"- {item}" for item in payload["limitations"])
    if payload["diagnostics"]:
        lines.extend(["", "## Diagnostics", ""])
        for item in payload["diagnostics"]:
            lines.append(
                f"- `{item['path']}:{item['line']}` "
                f"`{item['severity']}/{item['kind']}`: {item['message']}"
            )
    return "\n".join(lines) + "\n"


def write_reports(
    report: IgnoredTestInventoryReport,
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


def record_sort_key(record: Mapping[str, Any]) -> tuple[Any, ...]:
    return (
        record["discovery_source_kind"],
        record["path"],
        record["name"],
        record["discovery_id"],
    )


def diagnostic_sort_key(record: Mapping[str, Any]) -> tuple[Any, ...]:
    return (
        record["path"],
        record["line"],
        record["severity"],
        record["kind"],
        record["message"],
    )


def _cell(value: str) -> str:
    return value.replace("|", "\\|").replace("\n", " ")
