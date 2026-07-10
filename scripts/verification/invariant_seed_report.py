"""Canonical report model for the Phase 4 invariant-seed audit."""

from __future__ import annotations

import hashlib
import json
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping

from .invariant_seed_contract import SeedAuditRow


GENERATOR = "invariant-seed-audit"
GENERATOR_VERSION = 1
DEFAULT_JSON_PATH = Path("target/gate-artifacts/verification/invariant-seed-audit.json")
DEFAULT_MARKDOWN_PATH = Path("target/gate-artifacts/verification/invariant-seed-audit.md")
SCOPE = {
    "source": "initial_high_risk_invariant_seeds",
    "manifest": "verification/invariant-seeds.toml",
    "proof_created": False,
    "spec_gaps_closed": False,
    "runtime_behavior_changed": False,
}
LIMITATIONS = (
    "This audit proves registry completeness and metadata posture, not product behavior.",
    "S0 associations and proof_kind none evidence do not close an invariant or specification gap.",
    "verification/evidence-index.toml is live-validated but excluded from the input digest to avoid a report-evidence cycle.",
)


@dataclass(frozen=True)
class SeedAuditProvenance:
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
class SeedAuditReport:
    provenance: SeedAuditProvenance
    input_digest: str
    rows: tuple[SeedAuditRow, ...]

    def to_dict(self) -> dict[str, Any]:
        rows = [row.to_dict() for row in self.rows]
        return {
            "schema_version": 1,
            "generator": GENERATOR,
            "generator_version": GENERATOR_VERSION,
            "report_status": "complete",
            "input_digest": self.input_digest,
            **self.provenance.to_dict(),
            "scope": dict(SCOPE),
            "rows": rows,
            "summary": build_summary(rows),
            "limitations": list(LIMITATIONS),
        }

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), indent=2, sort_keys=True) + "\n"

    def to_markdown(self, *, json_digest: str) -> str:
        return render_markdown(self.to_dict(), json_digest=json_digest)


def build_summary(rows: list[Mapping[str, Any]]) -> dict[str, Any]:
    by_board = Counter(str(row["board_row"]) for row in rows)
    canonical = {str(row["canonical_invariant_id"]) for row in rows}
    return {
        "seeds": len(rows),
        "canonical_invariants": len(canonical),
        "merged_seed_aliases": len(rows) - len(canonical),
        "phase4_records": sum(row["origin"] == "phase4" for row in rows),
        "preexisting_seed_mappings": sum(
            row["origin"] == "preexisting" for row in rows
        ),
        "gap_open": sum(row["status"] == "gap_open" for row in rows),
        "spec_gap": sum(row["status"] == "spec_gap" for row in rows),
        "p4_000_risks": sum(row["p4_000_risk_id"] is not None for row in rows),
        "by_board_row": [
            {"board_row": board_row, "seeds": count}
            for board_row, count in sorted(by_board.items())
        ],
    }


def render_markdown(payload: Mapping[str, Any], *, json_digest: str) -> str:
    summary = payload["summary"]
    lines = [
        "# Phase 4 Invariant-Seed Audit",
        "",
        f"Generator: `{GENERATOR} v{GENERATOR_VERSION}`",
        f"Source revision: `{payload['commit']}`",
        f"Generated: `{payload['timestamp']}`",
        f"Platform: `{payload['platform']}`",
        f"Generated JSON SHA-256: `{json_digest}`",
        f"Input SHA-256: `{payload['input_digest']}`",
        "",
        "This is a registry-completeness report. It creates no behavior proof,",
        "closes no specification gap, and changes no runtime behavior.",
        "",
        "## Summary",
        "",
        f"- Written seeds: {summary['seeds']}",
        f"- Canonical invariants: {summary['canonical_invariants']}",
        f"- Authorized merged aliases: {summary['merged_seed_aliases']}",
        f"- Newly introduced Phase 4 records: {summary['phase4_records']}",
        f"- Pre-existing seed mappings: {summary['preexisting_seed_mappings']}",
        f"- Gap-open records: {summary['gap_open']}",
        f"- Spec-gap records: {summary['spec_gap']}",
        f"- Imported P4-000 review risks: {summary['p4_000_risks']}",
        "",
        "| Board row | Seeds |",
        "| --- | ---: |",
    ]
    for item in summary["by_board_row"]:
        lines.append(f"| `{item['board_row']}` | {item['seeds']} |")
    lines.extend(
        [
            "",
            "## Seed Registry",
            "",
            "| Seed | Canonical invariant | Area | Row | Origin | Status | Oracle | P4-000 risk |",
            "| --- | --- | --- | --- | --- | --- | --- | --- |",
        ]
    )
    for row in payload["rows"]:
        lines.append(
            f"| `{row['seed_id']}` | `{row['canonical_invariant_id']}` | "
            f"`{row['invariant_area']}` | `{row['board_row']}` | `{row['origin']}` | "
            f"`{row['status']}/{row['proof_level']}` | `{row['oracle_ref']}` | "
            f"`{row['p4_000_risk_id'] or 'none'}` |"
        )
    lines.extend(["", "## Limitations", ""])
    lines.extend(f"- {item}" for item in payload["limitations"])
    return "\n".join(lines) + "\n"


def write_reports(
    report: SeedAuditReport,
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
