"""Canonical report and Markdown renderer for the Phase 10 mutation program."""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping

from .mutation_program_live import LiveMutationProgramState


GENERATOR = "mutation-program-audit"
GENERATOR_VERSION = 1
DEFAULT_JSON_PATH = Path("target/gate-artifacts/verification/mutation-survivor-report.json")
DEFAULT_MARKDOWN_PATH = Path("target/gate-artifacts/verification/mutation-survivor-report.md")
SCOPE = {
    "mutation_basis": "six_exact_focused_shards_and_seven_single_file_selectors",
    "measured_basis": "validated_legacy_bytecode_pilot_only",
    "planned_basis": "selector_and_live_test_binding_without_execution",
    "survivor_basis": "derived_survived_outcomes_with_resolved_durable_action",
    "coverage_basis": "zero_runs_no_fabricated_percentage",
}
BOUNDARIES = {
    "report_creates_proof": False,
    "report_creates_invariant_coverage": False,
    "report_closes_spec_gaps": False,
    "report_is_release_evidence": False,
    "new_mutation_or_coverage_run_executed_by_report": False,
    "runtime_or_product_behavior_changed": False,
    "ci_enforcement_changed": False,
}
LIMITATIONS = (
    "Only the existing bytecode-validator pilot is measured; five other focused shards are definitions with empty result arrays.",
    "Cargo-mutants single-file listing resolves each selector but does not execute a baseline, build, test, mutation, or coverage command.",
    "Caught and survived are derived from raw build/test exit and timeout fields; infrastructure failures are errors and cannot count as caught or unviable.",
    "Associated scanner and case identities are traceability labels, not claims that a specific test or blocked case killed a mutant.",
    "Mutation and coverage results are test-adequacy signals, never release safety proof, invariant coverage, or spec-gap closure.",
    "A future measured connector-projection shard must bind a delivered artifact SHA-256 and direct execution confirmation.",
    "The implementation board is checked live but excluded from the digest because board and evidence closure follow report generation.",
)


@dataclass(frozen=True)
class MutationProgramReport:
    provenance: Mapping[str, Any]
    input_digest: str
    tool_version: str
    shards: tuple[dict[str, Any], ...]
    survivors: tuple[dict[str, Any], ...]
    coverage: Mapping[str, Any]
    summary: Mapping[str, int]

    @property
    def payload(self) -> dict[str, Any]:
        return {
            "schema_version": 1,
            "generator": GENERATOR,
            "generator_version": GENERATOR_VERSION,
            "report_status": "complete",
            "input_digest": self.input_digest,
            **dict(self.provenance),
            "scope": dict(SCOPE),
            "boundaries": dict(BOUNDARIES),
            "tool": {
                "name": "cargo-mutants",
                "version": self.tool_version,
                "selection_mode": "single_file_list_only",
            },
            "shards": [dict(item) for item in self.shards],
            "survivors": [dict(item) for item in self.survivors],
            "coverage": dict(self.coverage),
            "summary": dict(self.summary),
            "limitations": list(LIMITATIONS),
        }

    @classmethod
    def from_state(
        cls,
        state: LiveMutationProgramState,
        *,
        output_json: str = DEFAULT_JSON_PATH.as_posix(),
        output_markdown: str = DEFAULT_MARKDOWN_PATH.as_posix(),
    ) -> "MutationProgramReport":
        command = [
            "python3",
            "scripts/report_mutation_program.py",
            "--json-out",
            output_json,
            "--markdown-out",
            output_markdown,
            "--timestamp",
            state.timestamp,
        ]
        provenance = {
            "command": command,
            "commit": state.commit,
            "timestamp": state.timestamp,
            "platform": state.platform,
            "input_paths": list(state.input_paths),
            "output_paths": {"json": output_json, "markdown": output_markdown},
        }
        return cls(
            provenance=provenance,
            input_digest=state.input_digest,
            tool_version=state.tool_version,
            shards=state.shards,
            survivors=state.survivors,
            coverage=state.coverage,
            summary=state.summary,
        )

    def to_json(self) -> str:
        return json.dumps(self.payload, indent=2, sort_keys=True) + "\n"

    def to_markdown(self, *, json_digest: str) -> str:
        return render_markdown(self.payload, json_digest=json_digest)


def render_markdown(payload: Mapping[str, Any], *, json_digest: str) -> str:
    summary = payload["summary"]
    lines = [
        "# Phase 10 Focused Mutation Program",
        "",
        f"Generator: `{GENERATOR} v{GENERATOR_VERSION}`",
        f"Source revision: `{payload['commit']}`",
        f"Generated: `{payload['timestamp']}`",
        f"Platform: `{payload['platform']}`",
        f"Generated JSON SHA-256: `{json_digest}`",
        f"Input SHA-256: `{payload['input_digest']}`",
        "",
        "This report separates one validated measured pilot from five planned focused",
        "shards. It creates no proof, invariant coverage, spec-gap closure, release",
        "evidence, product behavior, or CI enforcement change.",
        "",
        "## Summary",
        "",
        f"- Shards: {summary['shards']}",
        f"- Measured shards: {summary['measured_shards']}",
        f"- Planned shards: {summary['planned_shards']}",
        f"- Defined mutants: {summary['defined_mutants']}",
        f"- Measured mutants: {summary['measured_mutants']}",
        f"- Caught: {summary['caught']}",
        f"- Survived: {summary['survived']}",
        f"- Unviable: {summary['unviable']}",
        f"- Timeout: {summary['timeout']}",
        f"- Error: {summary['error']}",
        f"- Coverage runs: {payload['coverage']['runs']}",
        "",
        "## Shards",
        "",
        "| Shard | Area | Status | Defined | Measured | Result artifact |",
        "| --- | --- | --- | ---: | ---: | --- |",
    ]
    for row in payload["shards"]:
        artifact = row.get("result_artifact")
        artifact_text = (
            f"`{artifact['path']}` (`{artifact['sha256']}`)"
            if isinstance(artifact, Mapping)
            else "none"
        )
        lines.append(
            f"| `{row['id']}` | `{row['area']}` | `{row['execution_status']}` | "
            f"{len(row['mutations'])} | {len(row['results'])} | {artifact_text} |"
        )
    lines.extend(["", "## Outcomes", ""])
    measured = [
        (shard["id"], result)
        for shard in payload["shards"]
        for result in shard["results"]
    ]
    if measured:
        lines.extend(
            [
                "| Shard | Mutant | Result |",
                "| --- | --- | --- |",
            ]
        )
        lines.extend(
            f"| `{shard_id}` | `{result['id']}` | `{result['result']}` |"
            for shard_id, result in measured
        )
    else:
        lines.append("No measured outcomes.")
    lines.extend(["", "## Survivors", ""])
    if payload["survivors"]:
        lines.extend(
            f"- `{row['shard_id']}/{row['mutation_id']}`: `{row['action']}` via `{row['resolution_ref']}`"
            for row in payload["survivors"]
        )
    else:
        lines.append("No survivors are present in the measured pilot.")
    lines.extend(["", "## Boundaries", ""])
    lines.extend(
        f"- `{field}`: `{str(payload['boundaries'][field]).lower()}`"
        for field in BOUNDARIES
    )
    lines.extend(["", "## Limitations", ""])
    lines.extend(f"- {item}" for item in payload["limitations"])
    lines.append("")
    return "\n".join(lines)


def write_reports(
    report: MutationProgramReport,
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
