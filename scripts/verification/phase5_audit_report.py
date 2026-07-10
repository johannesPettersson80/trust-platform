"""Canonical report model for the combined Phase 5 suite audit."""

from __future__ import annotations

import hashlib
import json
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping


GENERATOR = "phase5-suite-audit"
GENERATOR_VERSION = 1
DEFAULT_JSON_PATH = Path("target/gate-artifacts/verification/phase5-suite-audit.json")
DEFAULT_MARKDOWN_PATH = Path("target/gate-artifacts/verification/phase5-suite-audit.md")
BOUNDARIES = {
    "report_only_enforcement_unchanged": True,
    "report_emits_proof": False,
    "report_closes_spec_gaps": False,
    "suite_includes_interpreted": False,
    "p5_000b_remains_open": True,
}
LIMITATIONS = (
    "This report maps existing verification surfaces; the generator emits no behavior proof and closes no specification gap.",
    "Suite includes and excludes are displayed but not interpreted; VERIF-P14-000B still owns composition semantics.",
    "Report-only and planned inventory rows remain non-enforcing; this report changes no workflow or CI setting.",
    "VERIF-P5-000B is live-validated from the board but excluded from the source digest because board/evidence follow-up is mutable.",
)


@dataclass(frozen=True)
class Phase5AuditProvenance:
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
            "output_paths": {"json": self.output_json, "markdown": self.output_markdown},
        }


@dataclass(frozen=True)
class Phase5AuditReport:
    provenance: Phase5AuditProvenance
    input_digest: str
    inventory: tuple[dict[str, Any], ...]
    suites: tuple[dict[str, Any], ...]
    areas: tuple[dict[str, Any], ...]
    routes: tuple[dict[str, Any], ...]
    boundaries: dict[str, bool]

    def to_dict(self) -> dict[str, Any]:
        inventory = [dict(row) for row in self.inventory]
        suites = [dict(row) for row in self.suites]
        areas = [dict(row) for row in self.areas]
        routes = [dict(row) for row in self.routes]
        return {
            "schema_version": 1,
            "generator": GENERATOR,
            "generator_version": GENERATOR_VERSION,
            "report_status": "complete",
            "input_digest": self.input_digest,
            **self.provenance.to_dict(),
            "boundaries": dict(self.boundaries),
            "inventory": inventory,
            "suites": suites,
            "areas": areas,
            "routes": routes,
            "summary": build_summary(inventory, suites, areas, routes),
            "limitations": list(LIMITATIONS),
        }

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), indent=2, sort_keys=True) + "\n"

    def to_markdown(self, *, json_digest: str) -> str:
        return render_markdown(self.to_dict(), json_digest=json_digest)


def build_summary(inventory, suites, areas, routes) -> dict[str, Any]:
    return {
        "inventory_records": len(inventory),
        "live_inventory_records": sum(row["discovery_id"] is not None for row in inventory),
        "suite_records": len(suites),
        "suite_direct_commands": sum(len(row["direct_commands"]) for row in suites),
        "suite_inventory_refs": sum(len(row["direct_inventory_refs"]) for row in suites),
        "canonical_areas": len(areas),
        "taxonomy_routes": len(routes),
        "path_routes": sum(row["match_kind"] == "path" for row in routes),
        "intent_routes": sum(row["match_kind"] == "intent" for row in routes),
        "by_disposition": _counts(row["disposition"] for row in inventory),
        "by_source_kind": _counts(row["source_kind"] for row in inventory),
        "by_suite": _counts(suite for row in inventory for suite in row["suite_ids"]),
        "by_enforcement": _counts(row["enforcement"] for row in inventory),
        "by_artifact_kind": _counts(row["artifact_kind"] for row in inventory),
        "route_direct_suite_tiers": _counts(tier for row in routes for tier in row["direct_suite_tiers"]),
        "route_conditional_suite_tiers": _counts(
            tier for row in routes for tier in row["conditional_suite_tiers"]
        ),
    }


def _counts(values) -> list[dict[str, Any]]:
    return [{"name": name, "count": count} for name, count in sorted(Counter(values).items())]


def render_markdown(payload: Mapping[str, Any], *, json_digest: str) -> str:
    summary = payload["summary"]
    lines = [
        "# Phase 5 Suite and Gate Audit",
        "",
        f"Generator: `{GENERATOR} v{GENERATOR_VERSION}`",
        f"Source revision: `{payload['commit']}`",
        f"Generated: `{payload['timestamp']}`",
        f"Platform: `{payload['platform']}`",
        f"Generated JSON SHA-256: `{json_digest}`",
        f"Input SHA-256: `{payload['input_digest']}`",
        "",
        "This report inventories suite ownership and routing without creating proof,",
        "closing specification gaps, interpreting suite inheritance, or changing enforcement.",
        "",
        "## Summary",
        "",
        f"- Inventory records: {summary['inventory_records']} ({summary['live_inventory_records']} scanner-bound)",
        f"- Suite records: {summary['suite_records']}",
        f"- Direct suite commands: {summary['suite_direct_commands']}",
        f"- Suite inventory references: {summary['suite_inventory_refs']}",
        f"- Canonical areas: {summary['canonical_areas']}",
        f"- Ordered taxonomy routes: {summary['taxonomy_routes']}",
        "",
        "## Boundaries",
        "",
    ]
    for name in BOUNDARIES:
        value = payload["boundaries"][name]
        lines.append(f"- `{name}`: `{str(value).lower()}`")
    lines.extend(["", "## Inventory", "", "| ID | Source | Disposition | Suites | Enforcement | Artifact |", "| --- | --- | --- | --- | --- | --- |"])
    for row in payload["inventory"]:
        lines.append(
            f"| `{row['id']}` | `{row['source_kind']}` | `{row['disposition']}` | "
            f"`{', '.join(row['suite_ids']) or 'none'}` | `{row['enforcement']}` | "
            f"`{row['artifact_kind']}/{row['artifact_retention']}` |"
        )
    lines.extend(["", "## Suites", "", "| Suite | Environment | Direct commands | Inventory refs | Includes |", "| --- | --- | ---: | ---: | --- |"])
    for row in payload["suites"]:
        lines.append(
            f"| `{row['id']}` | `{row['environment']}` | {len(row['direct_commands'])} | "
            f"{len(row['direct_inventory_refs'])} | `{', '.join(row['includes']) or 'none'}` |"
        )
    lines.extend(["", "## Canonical Areas", "", "| Area | Owner | Direct suites | Required classes |", "| --- | --- | --- | --- |"])
    for row in payload["areas"]:
        lines.append(
            f"| `{row['id']}` | `{row['owner']}` | `{', '.join(row['direct_suite_tiers'])}` | "
            f"`{', '.join(row['required_test_classes'])}` |"
        )
    lines.extend(["", "## Ordered Taxonomy Routes", "", "| Order | Route | Areas | Direct suites | Conditional suites |", "| ---: | --- | --- | --- | --- |"])
    for row in payload["routes"]:
        lines.append(
            f"| {row['order']} | `{row['id']}` | `{', '.join(row['area_ids']) or 'intent-only'}` | "
            f"`{', '.join(row['direct_suite_tiers'])}` | "
            f"`{', '.join(row['conditional_suite_tiers']) or 'none'}` |"
        )
    lines.extend(["", "## Limitations", ""])
    lines.extend(f"- {item}" for item in payload["limitations"])
    return "\n".join(lines) + "\n"


def write_reports(report: Phase5AuditReport, *, json_path: Path, markdown_path: Path) -> None:
    rendered = report.to_json()
    digest = hashlib.sha256(rendered.encode()).hexdigest()
    json_path.parent.mkdir(parents=True, exist_ok=True)
    markdown_path.parent.mkdir(parents=True, exist_ok=True)
    json_path.write_text(rendered)
    markdown_path.write_text(report.to_markdown(json_digest=digest))
