"""Canonical Phase 7 conformance-alignment report model and renderer."""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping


GENERATOR = "conformance-alignment-audit"
GENERATOR_VERSION = 1
DEFAULT_JSON_PATH = Path("target/gate-artifacts/verification/conformance-alignment.json")
DEFAULT_MARKDOWN_PATH = Path("target/gate-artifacts/verification/conformance-alignment.md")
SCOPE = {
    "case_basis": "live_conformance_manifests",
    "mapping_basis": "exact_test_catalog_discovery_id_only",
    "gap_basis": "ten_v2_categories",
    "debt_is_report_failure": False,
}
BOUNDARIES = {
    "report_creates_proof": False,
    "report_closes_spec_gaps": False,
    "semantic_oracles_assessed": False,
    "live_network_or_hardware_used": False,
    "p7_002_invariant_mapping_remains_open": True,
    "generated_reports_remain_ci_artifacts": True,
    "public_page_updated": False,
    "runtime_or_product_behavior_changed": False,
    "ci_enforcement_changed": False,
}
LIMITATIONS = (
    "Catalog associations come only from an exact discovery_id join; names, paths, and prose do not create mappings.",
    "All current conformance cases are explicitly reported as unlinked; VERIF-P7-002 remains open.",
    "Coverage-gap rows record missing invariant mappings and do not assess or invent semantic oracles.",
    "The comms-determinism audit checks the committed scripted in-process case shape; it performs no live socket or hardware execution.",
    "The public conformance page and registered contract source are bound as publication context, not as behavior proof or external conformance certification.",
    "Generated conformance results remain CI artifacts; tracked expected artifacts are inputs, not proof that a case passed in this audit.",
    "The report creates no proof, closes no specification gap, changes no CI enforcement, and changes no runtime or product behavior.",
    "The blocked-row posture is checked live from the implementation board, which is excluded from the digest because board and evidence closure follow report generation.",
)


@dataclass(frozen=True)
class ConformanceAlignmentProvenance:
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
class ConformanceAlignmentReport:
    provenance: ConformanceAlignmentProvenance
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
            "contract": dict(self.analysis["contract"]),
            "categories": list(self.analysis["categories"]),
            "cases": list(self.analysis["cases"]),
            "unlinked_case_ids": list(self.analysis["unlinked_case_ids"]),
            "coverage_gaps": list(self.analysis["coverage_gaps"]),
            "comms_determinism": dict(self.analysis["comms_determinism"]),
            "publication": dict(self.analysis["publication"]),
            "summary": dict(self.analysis["summary"]),
            "limitations": list(LIMITATIONS),
        }

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), indent=2, sort_keys=True) + "\n"

    def to_markdown(self, *, json_digest: str) -> str:
        return render_markdown(self.to_dict(), json_digest=json_digest)


def render_markdown(payload: Mapping[str, Any], *, json_digest: str) -> str:
    summary = payload["summary"]
    contract = payload["contract"]
    comms = payload["comms_determinism"]
    publication = payload["publication"]
    lines = [
        "# Phase 7 Conformance Program Alignment",
        "",
        f"Generator: `{GENERATOR} v{GENERATOR_VERSION}`",
        f"Source revision: `{payload['commit']}`",
        f"Generated: `{payload['timestamp']}`",
        f"Platform: `{payload['platform']}`",
        f"Generated JSON SHA-256: `{json_digest}`",
        f"Input SHA-256: `{payload['input_digest']}`",
        "",
        "This is a report-only audit of committed conformance manifests, expected",
        "artifacts, explicit catalog links, publication posture, and the scripted",
        "comms-determinism case. It executes no conformance case and creates no proof.",
        "",
        "## Summary",
        "",
        f"- Categories: {summary['categories']} ({summary['v1_categories']} v1, {summary['v2_categories']} v2)",
        f"- Cases: {summary['cases']} ({summary['v1_cases']} v1, {summary['v2_cases']} v2)",
        f"- Runtime cases: {summary['runtime_cases']}",
        f"- Compile-error cases: {summary['compile_error_cases']}",
        f"- Connector-status-trace cases: {summary['connector_status_trace_cases']}",
        f"- Program sources: {summary['program_sources']}",
        f"- Expected artifacts: {summary['expected_artifacts']}",
        f"- Missing expected artifacts: {summary['missing_expected_artifacts']}",
        f"- Orphan expected artifacts: {summary['orphan_expected_artifacts']}",
        f"- Explicitly linked cases: {summary['explicitly_linked_cases']}",
        f"- Unlinked cases: {summary['unlinked_cases']}",
        f"- Coverage gaps: {summary['coverage_gaps']}",
        "",
        "## Categories",
        "",
        "| Profile | Category | Cases | Case IDs |",
        "| --- | --- | ---: | --- |",
    ]
    for row in payload["categories"]:
        lines.append(
            f"| `{row['profile']}` | `{row['category']}` | {row['case_count']} | "
            f"{_ids(row['case_ids'])} |"
        )
    lines.extend(
        [
            "",
            "## Cases",
            "",
            "| Case | Profile | Category | Kind | Program | Expected | Catalog test | Invariants |",
            "| --- | --- | --- | --- | --- | --- | --- | --- |",
        ]
    )
    for row in payload["cases"]:
        lines.append(
            f"| `{row['case_id']}` | `{row['profile']}` | `{row['category']}` | "
            f"`{row['kind']}` | `{_optional(row['program_path'])}` | "
            f"`{row['expected_artifact_path']}` | `{_optional(row['catalog_test_id'])}` | "
            f"{_ids(row['invariant_ids'])} |"
        )
    lines.extend(
        [
            "",
            "## Coverage Gaps",
            "",
            "| Category | Case present | Expected artifact | Invariant mapping | Semantic oracle | Status |",
            "| --- | --- | --- | --- | --- | --- |",
        ]
    )
    for row in payload["coverage_gaps"]:
        lines.append(
            f"| `{row['category']}` | `{_bool(row['case_present'])}` | "
            f"`{_bool(row['expected_artifact_present'])}` | "
            f"`{row['invariant_mapping_state']}` | `{row['semantic_oracle_state']}` | "
            f"`{row['gap_status']}` |"
        )
    lines.extend(
        [
            "",
            "## Comms",
            "",
            f"- Case: `{comms['case_id']}`",
            f"- Kind: `{comms['kind']}`",
            f"- Execution mode: `{comms['execution_mode']}`",
            f"- Scripted steps: {comms['scripted_steps']}",
            f"- Program source present: `{_bool(comms['program_source_present'])}`",
            f"- Live socket dependency: `{_bool(comms['live_socket_dependency'])}`",
            f"- Reviewed call path: {_ids(comms['reviewed_call_path'])}",
            f"- Reviewed source paths: {_ids(comms['reviewed_source_paths'])}",
            f"- Reviewed source SHA-256: `{comms['reviewed_source_digest']}`",
            "",
            "## Contract",
            "",
            f"- Spec source: `{contract['spec_source_id']}`",
            f"- Area: `{contract['area']}`",
            f"- Owner: `{contract['owner']}`",
            f"- Metadata status: `{contract['metadata_status']}`",
            f"- Covers: {_ids(contract['covers'])}",
            f"- Authority: `{contract['authority']}`",
            f"- Oracle eligible: `{_bool(contract['oracle_eligible'])}`",
            f"- Contract path: `{contract['path']}`",
            f"- Contract SHA-256: `{contract['digest']}`",
            f"- Tracked: `{_bool(contract['tracked'])}`",
            f"- Visibility: `{contract['visibility']}`",
            f"- Public page bound: `{_bool(contract['public_page_bound'])}`",
            f"- Reviewed runner source paths: {_ids(contract['reviewed_runner_source_paths'])}",
            f"- Reviewed runner source SHA-256: `{contract['reviewed_runner_source_digest']}`",
            f"- Reviewed runner behaviors: {_ids(contract['reviewed_runner_behaviors'])}",
            "",
            "## Publication",
            "",
            f"- CI job: `{publication['ci_job']}`",
            f"- CI job SHA-256: `{publication['ci_job_digest']}`",
            f"- CI artifact name: `{publication['ci_artifact_name']}`",
            f"- Generated JSON glob: `{publication['generated_json_glob']}`",
            f"- Generated Markdown glob: `{publication['generated_markdown_glob']}`",
            f"- Generated report policy: `{publication['generated_report_policy']}`",
            f"- Tracked report files: {_ids(publication['tracked_report_files'])}",
            f"- Public page embeds generated result: `{_bool(publication['public_page_embeds_generated_result'])}`",
            f"- Public page SHA-256: `{publication['public_page_digest']}`",
            "",
            "## Boundaries",
            "",
        ]
    )
    for name in BOUNDARIES:
        lines.append(f"- `{name}`: `{_bool(payload['boundaries'][name])}`")
    lines.extend(["", "## Limitations", ""])
    lines.extend(f"- {item}" for item in payload["limitations"])
    lines.append("")
    return "\n".join(lines)


def write_reports(
    report: ConformanceAlignmentReport,
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


def _optional(value: object) -> str:
    return str(value) if value is not None else "none"


def _bool(value: object) -> str:
    return str(value).lower()
