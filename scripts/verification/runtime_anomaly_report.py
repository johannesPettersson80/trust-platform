"""Canonical Phase 8 runtime-anomaly audit report and renderer."""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping

from .runtime_anomaly_restart_contract import restart_reference_text


GENERATOR = "runtime-anomaly-audit"
GENERATOR_VERSION = 3
DEFAULT_JSON_PATH = Path("target/gate-artifacts/verification/runtime-anomaly-audit.json")
DEFAULT_MARKDOWN_PATH = Path("target/gate-artifacts/verification/runtime-anomaly-audit.md")
PRIMARY_SUITE_ORDER = ("pr", "nightly", "release", "hardware_lab")
SCOPE = {
    "taxonomy_basis": "exact_phase8_19_class_order",
    "mapping_basis": "explicit_reviewed_discovery_id_only",
    "scanner_population": "production_rust_test_facts",
    "denominator_basis": "explicit_per_discovery_id_review",
    "gap_basis": "no_effectively_runnable_direct_mapping",
    "tier_basis": "planned_primary_and_conditional",
    "debt_is_report_failure": True,
}
BOUNDARIES = {
    "report_creates_proof": False,
    "report_creates_invariant_coverage": False,
    "report_closes_spec_gaps": False,
    "semantic_oracles_assessed": False,
    "faults_executed": False,
    "fault_interfaces_implemented": True,
    "production_fault_hooks_added": False,
    "p8_002_exhaustive_review_complete": True,
    "p8_005_fault_toggle_row_remains_open": False,
    "p8_006_production_hook_guard_remains_open": False,
    "runtime_or_product_behavior_changed": False,
    "ci_enforcement_changed": False,
}
LIMITATIONS = (
    "Mappings are hand-reviewed associations joined only by live discovery_id; names, paths, comments, and lexical candidates never create an association.",
    "A runnable direct association means an existing non-ignored test asserts part of the named anomaly stimulus; it is not invariant coverage or behavioral proof.",
    "Partial, context-only, ignored, and conditional associations remain test-gap rows and cannot satisfy the class.",
    "The committed denominator ledger binds every live Rust fact by discovery ID, source kind, path, and name to either an existing explicit association or an explicit reviewed-nonmapping rationale.",
    "Exhaustive denominator review does not turn nonmapping facts into anomaly coverage, proof, or an assertion that their ordinary behavior is adequate.",
    "Suite tiers are planned routing metadata. This report does not wire commands, change suite enforcement, or claim that a tier ran.",
    "The allocation-policy review reuses an active written contract; allocation-failure and OOM testing remains visible debt outside that claimed scan path.",
    "The restart-timebase review uses one closed schema-v1 state: existing_open_gap requires an actionable gap, while resolved_source binds an active reviewed source and any later closed gap must name that same resolution source; neither state creates test coverage, proof, or gap closure.",
    "Fault stimuli are admitted only through exact scanner-bound test_harness or external_harness mappings; ordinary_input records are not fault toggles and no general production toggle is introduced.",
    "The metadata gate rejects production Cargo features and public runtime symbols with fault-hook vocabulary. Production hooks remain prohibited pending an explicit reviewed design and contract update.",
    "The implementation board is checked live but excluded from the digest because board and evidence closure follow report generation.",
)


@dataclass(frozen=True)
class RuntimeAnomalyProvenance:
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
class RuntimeAnomalyReport:
    provenance: RuntimeAnomalyProvenance
    input_digest: str
    spec_gap_reviews: Mapping[str, Any]
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
            "spec_gap_reviews": {
                key: dict(value) for key, value in self.spec_gap_reviews.items()
            },
            "denominator_review": dict(self.analysis["denominator_review"]),
            "classes": list(self.analysis["classes"]),
            "mappings": list(self.analysis["mappings"]),
            "gap_rows": list(self.analysis["gap_rows"]),
            "summary": dict(self.analysis["summary"]),
            "limitations": list(LIMITATIONS),
        }

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), indent=2, sort_keys=True) + "\n"

    def to_markdown(self, *, json_digest: str) -> str:
        return render_markdown(self.to_dict(), json_digest=json_digest)


def render_markdown(payload: Mapping[str, Any], *, json_digest: str) -> str:
    summary = payload["summary"]
    allocation = payload["spec_gap_reviews"]["scan_cycle_allocation_policy"]
    restart = payload["spec_gap_reviews"]["restart_timebase"]
    lines = [
        "# Phase 8 Runtime Anomaly Audit",
        "",
        f"Generator: `{GENERATOR} v{GENERATOR_VERSION}`",
        f"Source revision: `{payload['commit']}`",
        f"Generated: `{payload['timestamp']}`",
        f"Platform: `{payload['platform']}`",
        f"Generated JSON SHA-256: `{json_digest}`",
        f"Input SHA-256: `{payload['input_digest']}`",
        "",
        "This is a report-only audit of the reviewed runtime-anomaly taxonomy,",
        "explicit existing-test associations, open test gaps, and planned suite tiers.",
        "It executes no fault and creates no proof or invariant coverage.",
        "",
        "## Summary",
        "",
        f"- Taxonomy classes: {summary['taxonomy_classes']}",
        f"- Explicit mapping records: {summary['mapping_records']}",
        f"- Live Rust scanner facts: {summary['scanner_denominator']}",
        f"- Denominator mapped facts: {payload['denominator_review']['summary']['mapped_facts']}",
        f"- Denominator reviewed-nonmapping facts: {payload['denominator_review']['summary']['reviewed_nonmapping_facts']}",
        f"- Denominator review SHA-256: `{payload['denominator_review']['review_digest']}`",
        f"- Effectively runnable direct mappings: {summary['effectively_runnable_mappings']}",
        f"- Ignored or conditional mappings: {summary['ignored_or_conditional_mappings']}",
        f"- Gap classes: {summary['gap_classes']}",
        "",
        "## Classes",
        "",
        "| Class | State | Primary suite | Conditional suites | Runnable mappings | Other mappings |",
        "| --- | --- | --- | --- | --- | --- |",
    ]
    for row in payload["classes"]:
        lines.append(
            f"| `{row['class_id']}` | `{row['state']}` | `{row['primary_suite']}` | "
            f"{_ids(row['conditional_suites'])} | {_ids(row['runnable_mapping_ids'])} | "
            f"{_ids(row['non_runnable_or_partial_mapping_ids'])} |"
        )
    lines.extend(
        [
            "",
            "## Explicit Associations",
            "",
            "| Mapping | Class | Test | Kind | Ignore state | Runnable | Mechanism |",
            "| --- | --- | --- | --- | --- | --- | --- |",
        ]
    )
    for row in payload["mappings"]:
        lines.append(
            f"| `{row['mapping_id']}` | `{row['class_id']}` | "
            f"`{row['discovery_id']}` | `{row['association_kind']}` | "
            f"`{row['ignore_state']}` | `{_bool(row['effectively_runnable'])}` | "
            f"`{row['injection_mechanism']}` |"
        )
    lines.extend(
        [
            "",
            "## Test Gaps",
            "",
            "| Class | State | Reason | Planned suite | Associations |",
            "| --- | --- | --- | --- | --- |",
        ]
    )
    for row in payload["gap_rows"]:
        lines.append(
            f"| `{row['class_id']}` | `{row['state']}` | `{row['reason']}` | "
            f"`{row['primary_suite']}` | {_ids(row['mapping_ids'])} |"
        )
    lines.extend(
        [
            "",
            "## Spec-Gap Review",
            "",
            f"- Scan-cycle allocation policy: `{allocation['outcome']}` via "
            f"`{allocation['source_ref']}` (`{allocation['source_path']}`).",
            f"- Restart time base: `{restart['outcome']}` via "
            f"{restart_reference_text(restart)}.",
            "",
            "## Planned Tier Counts",
            "",
        ]
    )
    for tier in PRIMARY_SUITE_ORDER:
        lines.append(f"- `{tier}`: {summary['by_primary_suite'][tier]}")
    lines.extend(["", "## Boundaries", ""])
    for name in BOUNDARIES:
        lines.append(f"- `{name}`: `{_bool(payload['boundaries'][name])}`")
    lines.extend(["", "## Limitations", ""])
    lines.extend(f"- {item}" for item in payload["limitations"])
    lines.append("")
    return "\n".join(lines)


def write_reports(
    report: RuntimeAnomalyReport,
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


def _bool(value: object) -> str:
    return str(value).lower()
