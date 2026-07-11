"""Canonical Phase 9 fuzz-program audit report and renderer."""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping

from .fuzz_program_live import LiveFuzzProgramState


GENERATOR = "fuzz-program-audit"
GENERATOR_VERSION = 1
DEFAULT_JSON_PATH = Path("target/gate-artifacts/verification/fuzz-program-audit.json")
DEFAULT_MARKDOWN_PATH = Path("target/gate-artifacts/verification/fuzz-program-audit.md")
TIER_ORDER = ("pr_smoke", "nightly", "manual_extended")
STATE_ORDER = ("cargo_fuzz_target", "smoke_only", "partial_only", "unmapped")
SCOPE = {
    "inventory_basis": "tracked_cargo_fuzz_manifests_and_reviewed_rust_candidate_census",
    "surface_basis": "exact_phase9_eight_surface_order",
    "mapping_basis": "explicit_live_identity_and_reviewed_surface_association_only",
    "gap_basis": "surface_without_direct_cargo_fuzz_target",
    "tier_basis": "explicit_primary_and_additional_execution_profiles",
    "corpus_contents_assessed": False,
    "debt_is_report_failure": False,
}
BOUNDARIES = {
    "report_creates_proof": False,
    "report_creates_invariant_coverage": False,
    "report_closes_spec_gaps": False,
    "semantic_oracles_assessed": False,
    "fuzz_campaign_executed": False,
    "corpus_contents_assessed": False,
    "crash_freedom_claimed": False,
    "p9_005_crash_regression_row_remains_open": True,
    "phase2_scanner_scope_changed": False,
    "runtime_or_product_behavior_changed": False,
    "ci_enforcement_changed": False,
}
LIMITATIONS = (
    "Cargo-fuzz facts come from every tracked root or crate-local fuzz/Cargo.toml; the historical Phase 2 scanner remains unchanged.",
    "Fuzz-like Rust candidates are production-scanner facts selected by the closed fuzz/property_smoke, constrained randomized/arbitrary smoke, or property-framework name vocabulary; names create candidates, never surface associations.",
    "Unmodeled proptest, quickcheck, or bolero source markers fail visibly, and the reviewed fuzz-gate command parsers reject extra filtered tests even when their names use no fuzz token.",
    "Direct and partial surface associations are reviewed planning metadata. They are not invariant coverage, an assessed oracle, or passing proof.",
    "A smoke_only surface has deterministic generated breadth but still appears as a gap because it has no cargo-fuzz target.",
    "Working corpus and raw crash contents are ignored machine-local or transient CI state and are deliberately not read, counted, digested, or treated as durable evidence.",
    "The inventory records existing and planned execution paths but executes no fuzz campaign and changes no suite or CI wiring.",
    "Every bounded Rust smoke is live-joined as not_ignored; ignored or conditional facts cannot retain a runnable tier claim.",
    "The Rust candidate census is lexical and does not prove ordinary cfg evaluation or parent-module reachability; wired means a reviewed required command path, not observed test execution.",
    "VERIF-P9-005 remains open because no exhaustive machine registry joins every minimized crash to a committed deterministic regression.",
    "The implementation board is checked live but excluded from the digest because board and evidence closure follow report generation.",
)


@dataclass(frozen=True)
class FuzzProgramProvenance:
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
class FuzzProgramReport:
    provenance: FuzzProgramProvenance
    input_digest: str
    corpus_policy: Mapping[str, Any]
    crash_regression_handoff: Mapping[str, Any]
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
            "corpus_policy": dict(self.corpus_policy),
            "crash_regression_handoff": dict(self.crash_regression_handoff),
            "targets": list(self.analysis["targets"]),
            "surfaces": list(self.analysis["surfaces"]),
            "gap_rows": list(self.analysis["gap_rows"]),
            "summary": dict(self.analysis["summary"]),
            "limitations": list(LIMITATIONS),
        }

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), indent=2, sort_keys=True) + "\n"

    def to_markdown(self, *, json_digest: str) -> str:
        return render_markdown(self.to_dict(), json_digest=json_digest)


def report_from_state(
    state: LiveFuzzProgramState,
    *,
    output_json: str,
    output_markdown: str,
) -> FuzzProgramReport:
    command = (
        "python3",
        "scripts/report_fuzz_program_audit.py",
        "--json-out",
        output_json,
        "--markdown-out",
        output_markdown,
        "--timestamp",
        state.timestamp,
    )
    return FuzzProgramReport(
        provenance=FuzzProgramProvenance(
            command=command,
            commit=state.commit,
            timestamp=state.timestamp,
            platform=state.platform,
            input_paths=state.input_paths,
            output_json=output_json,
            output_markdown=output_markdown,
        ),
        input_digest=state.input_digest,
        corpus_policy=state.corpus_policy,
        crash_regression_handoff=state.crash_regression_handoff,
        analysis=state.analysis,
    )


def render_markdown(payload: Mapping[str, Any], *, json_digest: str) -> str:
    summary = payload["summary"]
    lines = [
        "# Phase 9 Fuzz Program Audit",
        "",
        f"Generator: `{GENERATOR} v{GENERATOR_VERSION}`",
        f"Source revision: `{payload['commit']}`",
        f"Generated: `{payload['timestamp']}`",
        f"Platform: `{payload['platform']}`",
        f"Generated JSON SHA-256: `{json_digest}`",
        f"Input SHA-256: `{payload['input_digest']}`",
        "",
        "This is a report-only inventory of existing fuzz targets, deterministic",
        "fuzz-like smokes, required surfaces, execution profiles, and target gaps.",
        "It runs no campaign and creates no proof or invariant coverage.",
        "",
        "## Summary",
        "",
        f"- Inventory targets: {summary['inventory_targets']}",
        f"- Cargo-fuzz targets: {summary['cargo_fuzz_targets']}",
        f"- Bounded Rust smokes: {summary['bounded_rust_smokes']}",
        f"- Required surfaces: {summary['required_surfaces']}",
        f"- Gap surfaces: {summary['gap_surfaces']}",
        "",
        "## Required Surfaces",
        "",
        "| Surface | Area | Association state | Direct targets | Partial targets |",
        "| --- | --- | --- | --- | --- |",
    ]
    for row in payload["surfaces"]:
        lines.append(
            f"| `{row['surface_id']}` | `{row['area']}` | `{row['state']}` | "
            f"{_ids(row['direct_target_ids'])} | {_ids(row['partial_target_ids'])} |"
        )
    lines.extend(
        [
            "",
            "## Target Inventory",
            "",
            "| Target | Kind | Ignore state | Primary tier | Additional tiers | Enforcement | Source |",
            "| --- | --- | --- | --- | --- | --- | --- |",
        ]
    )
    for row in payload["targets"]:
        lines.append(
            f"| `{row['id']}` | `{row['target_kind']}` | "
            f"`{row.get('ignore_state', 'not_applicable')}` | `{row['primary_tier']}` | "
            f"{_ids(row['additional_tiers'])} | `{row['enforcement_status']}` | `{row['path']}` |"
        )
    lines.extend(
        [
            "",
            "## Surface Gaps",
            "",
            "| Surface | Current state | Gap reason | Associated targets |",
            "| --- | --- | --- | --- |",
        ]
    )
    for row in payload["gap_rows"]:
        lines.append(
            f"| `{row['surface_id']}` | `{row['state']}` | `{row['reason']}` | "
            f"{_ids(row['target_ids'])} |"
        )
    lines.extend(["", "## Primary Tier Counts", ""])
    for tier in TIER_ORDER:
        lines.append(f"- `{tier}`: {summary['by_primary_tier'][tier]}")
    lines.extend(["", "## Additional Tier Counts", ""])
    for tier in TIER_ORDER:
        lines.append(f"- `{tier}`: {summary['by_additional_tier'][tier]}")
    lines.extend(["", "## Surface State Counts", ""])
    for state in STATE_ORDER:
        lines.append(f"- `{state}`: {summary['by_surface_state'][state]}")
    lines.extend(["", "## Corpus And Crash Handoff", ""])
    lines.append(
        f"- Working corpus storage: `{payload['corpus_policy']['working_corpus_storage']}`"
    )
    lines.append(f"- Raw crash storage: `{payload['corpus_policy']['raw_crash_storage']}`")
    lines.append(
        f"- Corpus contents assessed: `{_bool(payload['corpus_policy']['contents_assessed'])}`"
    )
    lines.append(
        "- Crash-to-regression enforcement: "
        f"`{payload['crash_regression_handoff']['enforcement_status']}`"
    )
    lines.extend(["", "## Boundaries", ""])
    for name in BOUNDARIES:
        lines.append(f"- `{name}`: `{_bool(payload['boundaries'][name])}`")
    lines.extend(["", "## Limitations", ""])
    lines.extend(f"- {item}" for item in payload["limitations"])
    lines.append("")
    return "\n".join(lines)


def write_reports(
    report: FuzzProgramReport,
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
