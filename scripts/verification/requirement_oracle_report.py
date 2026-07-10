"""Canonical Phase 6 requirement/oracle audit report model and renderer."""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping


GENERATOR = "requirement-oracle-audit"
GENERATOR_VERSION = 1
DEFAULT_JSON_PATH = Path("target/gate-artifacts/verification/requirement-oracle-audit.json")
DEFAULT_MARKDOWN_PATH = Path("target/gate-artifacts/verification/requirement-oracle-audit.md")
BOUNDARIES = {
    "audit_creates_proof": False,
    "audit_closes_spec_gaps": False,
    "missing_oracle_enforcement_enabled": False,
    "public_claims_are_oracles": False,
    "public_docs_inventory_exhaustive": False,
    "forward_traceability_complete": False,
    "reverse_traceability_complete": False,
    "p4a_005_public_claim_inventory_remains_open": True,
    "p6_007_enforcement_remains_open": True,
    "p6_008_to_p6_010_remain_open": True,
    "p14_000_grace_rule_remains_open": True,
}
SCOPE = {
    "invariant_basis": "all_committed_invariant_records",
    "mapping_basis": "explicit_invariant_spec_oracle_and_gap_refs",
    "mapping_rows": [
        "VERIF-P6-001",
        "VERIF-P6-002",
        "VERIF-P6-003",
        "VERIF-P6-004",
        "VERIF-P6-005",
    ],
    "public_claim_basis": "registered_spec_sources_only",
    "public_docs_exhaustive": False,
    "debt_is_report_failure": False,
}
LIMITATIONS = (
    "Mappings come only from explicit invariant spec/oracle references; names, paths, and prose do not create mappings.",
    "An eligible source listed as context does not replace an invariant's explicit open spec-gap oracle.",
    "Public claims are provenance and proof obligations, never behavior oracles.",
    "Product contracts and reviewed IEC decisions or deviations are not external IEC conformance proof; external-source availability remains open under VERIF-P1A-007.",
    "Specification-source discovery, classification, and conflict scanning remain incomplete under VERIF-P1A-002, VERIF-P1A-003, and VERIF-P1A-006.",
    "The public-doc inventory is registered-spec-sources-only and remains non-exhaustive while VERIF-P4A-005 is open.",
    "Invariant test, gate, and evidence IDs are copied explicit associations, not a completed forward trace; referenced metadata is live-validated at rest.",
    "verification/evidence-index.toml is excluded from the input digest to avoid a report-evidence digest cycle.",
    "Missing-oracle debt is report-only until VERIF-P14-000 defines the grace period required by VERIF-P6-007.",
    "Forward, reverse, and orphan traceability remain outside this slice under VERIF-P6-008 through VERIF-P6-010.",
    "The blocked-row posture is checked live from the implementation board, which is excluded from the digest because board and evidence closure follow report generation.",
    "The report creates no proof, closes no specification gap, and changes no runtime or product behavior.",
)


@dataclass(frozen=True)
class RequirementOracleProvenance:
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
class RequirementOracleReport:
    provenance: RequirementOracleProvenance
    input_digest: str
    analysis: Mapping[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": 1,
            "generator": GENERATOR,
            "generator_version": GENERATOR_VERSION,
            "report_status": "complete",
            "input_digest": self.input_digest,
            **self.provenance.to_dict(),
            "scope": dict(SCOPE),
            "boundaries": dict(BOUNDARIES),
            "mapping_groups": list(self.analysis["mapping_groups"]),
            "invariants": list(self.analysis["invariants"]),
            "missing_oracles": list(self.analysis["missing_oracles"]),
            "summary": dict(self.analysis["summary"]),
            "limitations": list(LIMITATIONS),
        }

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), indent=2, sort_keys=True) + "\n"

    def to_markdown(self, *, json_digest: str) -> str:
        return render_markdown(self.to_dict(), json_digest=json_digest)


def render_markdown(payload: Mapping[str, Any], *, json_digest: str) -> str:
    summary = payload["summary"]
    lines = [
        "# Phase 6 Requirement and Oracle Audit",
        "",
        f"Generator: `{GENERATOR} v{GENERATOR_VERSION}`",
        f"Source revision: `{payload['commit']}`",
        f"Generated: `{payload['timestamp']}`",
        f"Platform: `{payload['platform']}`",
        f"Generated JSON SHA-256: `{json_digest}`",
        f"Input SHA-256: `{payload['input_digest']}`",
        "",
        "This is a report-only requirement/oracle association audit. It creates no",
        "behavior proof, closes no specification gap, and enables no enforcement.",
        "Its invariant denominator is all committed invariant records; public-claim",
        "context is limited to the non-exhaustive registered source inventory.",
        "",
        "## Summary",
        "",
        f"- Invariants: {summary['invariants_total']}",
        f"- Phase 6 mapped invariants: {summary['mapped_phase6_invariants']}",
        f"- Other-area invariants: {summary['other_area_invariants']}",
        f"- Eligible oracles: {summary['eligible_oracles']}",
        f"- Missing oracles: {summary['missing_oracles']}",
        f"- Future enforcement candidates: {summary['future_enforcement_candidates']}",
        "",
        "## Mapping Groups",
        "",
        "| Board row | Areas | Invariants | Eligible oracle | Spec-gap blocked |",
        "| --- | --- | ---: | ---: | ---: |",
    ]
    for group in payload["mapping_groups"]:
        lines.append(
            f"| `{group['board_row']}` | `{', '.join(group['area_ids'])}` | "
            f"{group['invariant_count']} | {group['eligible_oracle_count']} | "
            f"{group['spec_gap_blocked_count']} |"
        )
    lines.extend(
        [
            "",
            "## Invariant Oracle Ledger",
            "",
            "| Invariant | Area | Risk | Status | Oracle state | Oracle ref | Sources | Gaps |",
            "| --- | --- | --- | --- | --- | --- | --- | --- |",
        ]
    )
    for row in payload["invariants"]:
        lines.append(
            f"| `{row['invariant_id']}` | `{row['area']}` | `{row['risk']}` | "
            f"`{row['invariant_status']}/{row['proof_level']}` | "
            f"`{row['oracle_state']}` | `{row['oracle_ref']}` | "
            f"{_ids(row['spec_source_refs'])} | {_ids(row['spec_gap_refs'])} |"
        )
    lines.extend(
        [
            "",
            "## Missing Oracles",
            "",
            "| Invariant | Risk | Gap | Future enforcement candidate |",
            "| --- | --- | --- | --- |",
        ]
    )
    for row in payload["missing_oracles"]:
        lines.append(
            f"| `{row['invariant_id']}` | `{row['risk']}` | `{row['oracle_ref']}` | "
            f"`{str(row['future_enforcement_candidate']).lower()}` |"
        )
    lines.extend(["", "## Boundaries", ""])
    for name in BOUNDARIES:
        value = payload["boundaries"][name]
        lines.append(f"- `{name}`: `{str(value).lower()}`")
    lines.extend(["", "## Limitations", ""])
    lines.extend(f"- {item}" for item in payload["limitations"])
    lines.append("")
    return "\n".join(lines)


def write_reports(
    report: RequirementOracleReport,
    *,
    json_path: Path,
    markdown_path: Path,
) -> None:
    json_path.parent.mkdir(parents=True, exist_ok=True)
    markdown_path.parent.mkdir(parents=True, exist_ok=True)
    json_text = report.to_json()
    json_path.write_text(json_text)
    digest = hashlib.sha256(json_text.encode()).hexdigest()
    markdown_path.write_text(report.to_markdown(json_digest=digest))


def _ids(values: list[str]) -> str:
    return ", ".join(f"`{value}`" for value in values) if values else "none"
