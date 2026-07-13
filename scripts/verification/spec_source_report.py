"""Canonical specification-source audit report model and renderer."""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping


GENERATOR = "spec-source-audit"
GENERATOR_VERSION = 1
DEFAULT_JSON_PATH = Path("target/gate-artifacts/verification/spec-source-audit.json")
DEFAULT_MARKDOWN_PATH = Path("target/gate-artifacts/verification/spec-source-audit.md")
BOUNDARIES = {
    "audit_creates_proof": False,
    "audit_closes_spec_gaps": False,
    "audit_promotes_invariants": False,
    "source_authority_is_inferred": False,
    "public_claims_are_oracles": False,
    "semantic_claim_review_complete": False,
    "conflict_review_complete": False,
    "checklist_row_staleness_complete": False,
    "removed_behavior_reference_review_complete": False,
    "missing_spec_enforcement_enabled": False,
}
LIMITATIONS = (
    "Document and public-prose discovery is mechanical; authority, ownership, area, and oracle eligibility come only from reviewed metadata.",
    "A title, path fragment, heading, lexical candidate, or similar prose never creates a source, requirement, claim, invariant, test, or proof mapping.",
    "The public-prose denominator includes every block reached from README.md or tracked docs/public surfaces and their tracked recursive snippet includes.",
    "Blocks without an exact registered claim-text binding remain unreviewed candidates; exhaustive discovery is not semantic claim review.",
    "Review-due status compares the reviewed date to the last tracked path change and is a review signal, not a claim that content changed semantically.",
    "External-source rows expose reviewed locator metadata only; expected ignored local bytes are neither read nor included in report provenance.",
    "Mutable evidence trees are outside the document denominator except for exact registered review sources; the audit's own evidence Markdown cannot enter its input closure.",
    "Conflicts are limited to explicit metadata references and mechanical broken-reference diagnostics; prose similarity does not establish a conflict.",
    "Conflict review is not complete: equal-authority overlaps are candidates and have not received an exhaustive semantic conflict disposition.",
    "Checklist-row staleness and references to removed product behavior are not exhaustively classified by this mechanical pass.",
    "The report is report-only, creates no product proof, closes no specification gap, promotes no invariant, and changes no runtime behavior.",
)


@dataclass(frozen=True)
class SpecSourceAuditProvenance:
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
class SpecSourceAuditReport:
    provenance: SpecSourceAuditProvenance
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
            "scope": dict(self.analysis["scope"]),
            "boundaries": dict(BOUNDARIES),
            "documents": list(self.analysis["documents"]),
            "source_bindings": list(self.analysis["source_bindings"]),
            "required_topics": list(self.analysis["required_topics"]),
            "obvious_missing_specs": list(self.analysis["obvious_missing_specs"]),
            "public_prose_blocks": list(self.analysis["public_prose_blocks"]),
            "registered_public_claims": list(self.analysis["registered_public_claims"]),
            "findings": list(self.analysis["findings"]),
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
        "# Specification Source and Public Prose Audit",
        "",
        f"Generator: `{GENERATOR} v{GENERATOR_VERSION}`",
        f"Source revision: `{payload['commit']}`",
        f"Generated: `{payload['timestamp']}`",
        f"Platform: `{payload['platform']}`",
        f"Generated JSON SHA-256: `{json_digest}`",
        f"Input SHA-256: `{payload['input_digest']}`",
        "",
        "This report is the mechanical denominator for tracked specification documents,",
        "required-topic metadata, and public rendered prose. Unreviewed prose stays visible",
        "as debt and creates no semantic claim or proof mapping.",
        "",
        "## Summary",
        "",
        f"- Documents: {summary['documents_total']}",
        f"- Registered sources: {summary['registered_sources']} ({summary['bound_sources']} tracked-file bound, {summary['external_sources']} external, {summary['unbound_sources']} unbound)",
        f"- Unreviewed documents: {summary['unreviewed_documents']}",
        f"- Required topics: {summary['required_topics_total']} ({summary['required_topics_mapped']} mapped, {summary['required_topics_gap_open']} gap-open, {summary['required_topics_broken']} broken)",
        f"- Obvious specification topics: {summary['obvious_spec_topics_total']} ({summary['obvious_spec_source_present']} source-present, {summary['obvious_spec_gap']} gap, {summary['obvious_spec_partial']} partial, {summary['obvious_spec_unrepresented']} unrepresented, {summary['obvious_spec_reference_broken']} broken refs)",
        f"- Public surfaces: {summary['public_surfaces']}",
        f"- Public prose blocks: {summary['public_prose_blocks']}",
        f"- Registered public claims: {summary['registered_public_claims']} ({summary['bound_public_claims']} bound, {summary['unbound_public_claims']} unbound)",
        f"- Unreviewed public blocks: {summary['unreviewed_public_blocks']}",
        f"- Scanner diagnostics: {summary['scanner_diagnostics']}",
        f"- Source reviews due: {summary['source_reviews_due']}",
        f"- Findings: {summary['blocking_findings']} blocking, {summary['warning_findings']} warning",
        "",
        "## Registered Source Bindings",
        "",
        "| Source | Area | Authority | Locator | Path / external ref | Document | Binding | Review due | Availability |",
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ]
    for row in payload["source_bindings"]:
        lines.append(
            f"| `{row['source_id']}` | `{row['area']}` | `{row['authority']}` | "
            f"`{row['locator_kind']}` | `{row['path'] or row['external_ref']}` | "
            f"`{row['document_id']}` | `{row['binding_state']}` | "
            f"`{str(row['review_due']).lower()}` | `{row['availability']}` |"
        )
    lines.extend(
        [
            "",
            "## Required Topics",
            "",
            "| Topic | Area | Tag | Status | Source | Gap | Mapping |",
            "| --- | --- | --- | --- | --- | --- | --- |",
        ]
    )
    for row in payload["required_topics"]:
        lines.append(
            f"| `{row['topic_id']}` | `{row['area']}` | `{row['tag']}` | "
            f"`{row['status']}` | `{row['source_ref']}` | `{row['spec_gap_ref']}` | "
            f"`{row['mapping_state']}` |"
        )
    lines.extend(
        [
            "",
            "## Obvious Missing-Spec Denominator",
            "",
            "| Topic | Areas | Reviewed posture | Eligible sources | Non-oracle sources | Open gaps | Public context | Reference health |",
            "| --- | --- | --- | --- | --- | --- | --- | --- |",
        ]
    )
    for row in payload["obvious_missing_specs"]:
        lines.append(
            f"| `{row['board_topic']}` (`{row['topic_id']}`) | {_ids(row['areas'])} | "
            f"`{row['reviewed_posture']}` | {_ids(row['eligible_source_ids'])} | "
            f"{_ids(row['nonoracle_source_ids'])} | {_ids(row['open_spec_gap_ids'])} | "
            f"{_ids(row['public_claim_context_ids'])} | `{row['reference_health']}` |"
        )
    lines.extend(
        [
            "",
            "## Registered Public Claims",
            "",
            "| Claim | Path | Surface | Block IDs | Binding |",
            "| --- | --- | --- | --- | --- |",
        ]
    )
    for row in payload["registered_public_claims"]:
        lines.append(
            f"| `{row['claim_id']}` | `{row['path']}` | `{row['surface_ref']}` (`{row['surface_path']}`) | "
            f"{_ids(row['block_ids'])} | `{row['binding_state']}` |"
        )
    lines.extend(
        [
            "",
            "## Document Denominator",
            "",
            "| Document | Format | Spec scope | Public entries | Path | SHA-256 | Registered sources | Review state |",
            "| --- | --- | --- | --- | --- | --- | --- | --- |",
        ]
    )
    for row in payload["documents"]:
        lines.append(
            f"| `{row['document_id']}` | `{row['format']}` | "
            f"`{str(row['in_spec_document_scope']).lower()}` | {_ids(row['public_entry_paths'])} | "
            f"`{row['path']}` | `{row['content_sha256']}` | {_ids(row['registered_source_ids'])} | "
            f"`{row['review_state']}` |"
        )
    lines.extend(
        [
            "",
            "## Public Prose Denominator",
            "",
            "| Block | Source | Lines | Heading | Surfaces | Claims | Review state | SHA-256 |",
            "| --- | --- | --- | --- | --- | --- | --- | --- |",
        ]
    )
    for row in payload["public_prose_blocks"]:
        heading = " / ".join(row["heading_path"]) or "(root)"
        lines.append(
            f"| `{row['block_id']}` | `{row['path']}` | `{row['line_start']}-{row['line_end']}` | "
            f"{_escape(heading)} | {_ids(row['public_entry_paths'])} | "
            f"{_ids(row['registered_claim_ids'])} | `{row['review_state']}` | "
            f"`{row['visible_text_sha256']}` |"
        )
    lines.extend(
        [
            "",
            "## Findings",
            "",
            "| Severity | Code | Record | Path | Message |",
            "| --- | --- | --- | --- | --- |",
        ]
    )
    for row in payload["findings"]:
        lines.append(
            f"| `{row['severity']}` | `{row['code']}` | `{row['record_id']}` | "
            f"`{row['path']}` | {_escape(row['message'])} |"
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
    report: SpecSourceAuditReport,
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


def _escape(value: object) -> str:
    return str(value).replace("|", "\\|").replace("\n", " ")
