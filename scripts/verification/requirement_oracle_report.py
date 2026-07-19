"""Canonical Phase 6 requirement/oracle audit report model and renderer."""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping


GENERATOR = "requirement-oracle-audit"
GENERATOR_VERSION = 2
DEFAULT_JSON_PATH = Path("target/gate-artifacts/verification/requirement-oracle-audit.json")
DEFAULT_MARKDOWN_PATH = Path("target/gate-artifacts/verification/requirement-oracle-audit.md")
BOUNDARIES = {
    "audit_creates_proof": False,
    "audit_closes_spec_gaps": False,
    "missing_oracle_enforcement_enabled": True,
    "public_claims_are_oracles": False,
    "public_docs_inventory_exhaustive": False,
    "forward_traceability_complete": True,
    "reverse_traceability_complete": True,
    "orphan_traceability_complete": True,
    "p4a_005_public_claim_inventory_remains_open": True,
    "p6_007_enforcement_remains_open": False,
    "p6_008_to_p6_010_remain_open": False,
    "p14_000_grace_rule_remains_open": False,
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
    "traceability_basis": "explicit_metadata_identifiers_only",
    "orphan_basis": "complete_registered_metadata_denominators",
    "debt_is_report_failure": False,
}
LIMITATIONS = (
    "Mappings come only from explicit invariant spec/oracle references; names, paths, and prose do not create mappings.",
    "An eligible source listed as context does not replace an invariant's explicit open spec-gap oracle.",
    "Public claims are provenance and proof obligations, never behavior oracles.",
    "Product contracts and reviewed IEC decisions or deviations are not external IEC conformance proof; the registered external IEC source remains non-oracle and its ignored local bytes are not provenance inputs.",
    "The separate specification-source audit exhaustively classifies its tracked-document denominator and records conflict, checklist-staleness, and removed-behavior dispositions without creating proof.",
    "This report's public-claim view is registered-spec-sources-only; the separate audit exhaustively dispositions rendered prose and conservatively reports every substantive unbound block without an invariant or oracle.",
    "Forward and reverse paths use only explicit source, invariant, test, suite, evidence, gap, and public-claim identifiers; names, paths, and prose create no edges.",
    "verification/evidence-index.toml is excluded from the input digest to avoid a report-evidence digest cycle.",
    "The committed governance contract enforces overdue high-risk missing-oracle debt after its reviewed grace period; this report itself remains non-proof.",
    "An orphan is a registered record without the explicit links named by the report contract; an orphan finding does not infer that the underlying document, test, or evidence is useless.",
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
            "schema_version": 2,
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
            "forward_traceability": list(self.analysis["forward_traceability"]),
            "reverse_public_claim_traceability": list(
                self.analysis["reverse_public_claim_traceability"]
            ),
            "orphans": dict(self.analysis["orphans"]),
            "incomplete_chains": list(self.analysis["incomplete_chains"]),
            "traceability_summary": dict(self.analysis["traceability_summary"]),
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
        "This is a requirement/oracle and explicit traceability audit. It creates no",
        "behavior proof and closes no specification gap. Its invariant denominator is",
        "all committed invariant records; its public-claim denominator is the complete",
        "registered claim inventory, not all public prose.",
        "",
        "## Summary",
        "",
        f"- Invariants: {summary['invariants_total']}",
        f"- Phase 6 mapped invariants: {summary['mapped_phase6_invariants']}",
        f"- Other-area invariants: {summary['other_area_invariants']}",
        f"- Eligible oracles: {summary['eligible_oracles']}",
        f"- Missing oracles: {summary['missing_oracles']}",
        f"- Future enforcement candidates: {summary['future_enforcement_candidates']}",
        f"- Complete forward chains to evidence: {payload['traceability_summary']['complete_to_evidence']}/{payload['traceability_summary']['forward_invariants']}",
        f"- Linked registered public claims: {payload['traceability_summary']['linked_public_claims']}/{payload['traceability_summary']['reverse_public_claims']}",
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
    lines.extend(
        [
            "",
            "## Forward Traceability",
            "",
            "| Invariant | Sources | Tests | Suites | Evidence | Public claims | Missing links |",
            "| --- | --- | ---: | --- | ---: | --- | --- |",
        ]
    )
    for row in payload["forward_traceability"]:
        lines.append(
            f"| `{row['invariant_id']}` | {_ids(row['spec_source_ids'])} | "
            f"{len(row['test_ids'])} | {_ids(row['suite_ids'])} | "
            f"{len(row['evidence_ids'])} | {_ids(row['public_claim_ids'])} | "
            f"{_ids(row['missing_links'])} |"
        )
    lines.extend(
        [
            "",
            "## Reverse Public-Claim Traceability",
            "",
            "| Public claim | State | Invariants | Tests | Suites | Evidence |",
            "| --- | --- | ---: | ---: | --- | ---: |",
        ]
    )
    for row in payload["reverse_public_claim_traceability"]:
        lines.append(
            f"| `{row['public_claim_id']}` | `{row['binding_state']}` | "
            f"{len(row['invariant_ids'])} | {len(row['test_ids'])} | "
            f"{_ids(row['suite_ids'])} | {len(row['evidence_ids'])} |"
        )
    lines.extend(["", "## Orphans", ""])
    for label, field in (
        ("Spec sources", "spec_source_ids"),
        ("Tests", "test_ids"),
        ("Invariants", "invariant_ids"),
        ("Public claims", "public_claim_ids"),
        ("Evidence", "evidence_ids"),
    ):
        lines.append(f"- {label}: {_ids(payload['orphans'][field])}")
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
