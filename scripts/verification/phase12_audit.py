"""Canonical Phase 12 workflow and UI-journey audit model."""

from __future__ import annotations

import hashlib
import json
from collections import Counter
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any


GENERATOR = "phase12-workflow-ui-audit"
GENERATOR_VERSION = 1
DEFAULT_JSON_PATH = Path("target/gate-artifacts/verification/phase12-workflow-ui-audit.json")
DEFAULT_MARKDOWN_PATH = Path(
    "docs/internal/testing/evidence/plc-verification-program/2026-07-19/phase12-workflow-ui-audit.md"
)
BOUNDARIES = {
    "report_emits_proof": False,
    "report_promotes_ui_invariants": False,
    "backend_proof_replaces_visual_evidence": False,
    "source_transform_requires_silent_corruption_risk": True,
    "validated_ui_requires_accepted_journey": True,
}
LIMITATIONS = (
    "The report inventories reviewed workflow and UI-journey associations; it emits no product proof and promotes no invariant.",
    "A backend or extension test is supporting evidence only and cannot replace fresh visual journey evidence.",
    "Evidence-missing, stale, and provisional journeys remain visible debt; only ux_accepted is acceptance.",
    "Implementation-change attribution is limited to the implementation paths explicitly owned by each journey row.",
)


def build_rows(
    workflow_reviews: Sequence[Mapping[str, Any]],
    journeys: Sequence[Mapping[str, Any]],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    journey_rows = [_journey_row(row) for row in journeys]
    workflow_rows: list[dict[str, Any]] = []
    for review in workflow_reviews:
        workflow_id = str(review["discovery_id"])
        linked = [
            row
            for row in journey_rows
            if workflow_id in row["workflow_candidate_ids"]
        ]
        invariant_ids = sorted(
            {
                invariant_id
                for row in linked
                for invariant_id in row["invariant_ids"]
            }
        )
        acceptance_status = _workflow_acceptance_status(linked)
        is_workflow = review.get("disposition") == "workflow_spec"
        workflow_rows.append(
            {
                "discovery_id": workflow_id,
                "path": str(review["path"]),
                "heading_path": list(review["heading_path"]),
                "disposition": str(review["disposition"]),
                "spec_source_id": review.get("spec_source_id"),
                "linked_journey_ids": [row["id"] for row in linked],
                "invariant_ids": invariant_ids,
                "acceptance_status": acceptance_status if is_workflow else "not_applicable",
                "missing_spec_source": is_workflow and not bool(review.get("spec_source_id")),
                "missing_invariant_link": is_workflow and not invariant_ids,
                "missing_acceptance_evidence": is_workflow
                and acceptance_status == "missing",
            }
        )
    return workflow_rows, journey_rows


def build_summary(
    workflow_rows: Sequence[Mapping[str, Any]],
    journey_rows: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    workflows = [row for row in workflow_rows if row["disposition"] == "workflow_spec"]
    return {
        "workflow_candidates": len(workflow_rows),
        "workflow_specs": len(workflows),
        "reviewed_nonworkflows": len(workflow_rows) - len(workflows),
        "workflow_missing_spec_source": sum(row["missing_spec_source"] for row in workflows),
        "workflow_missing_invariant_link": sum(
            row["missing_invariant_link"] for row in workflows
        ),
        "workflow_missing_acceptance_evidence": sum(
            row["missing_acceptance_evidence"] for row in workflows
        ),
        "journeys": len(journey_rows),
        "journeys_with_invariants": sum(bool(row["invariant_ids"]) for row in journey_rows),
        "journeys_with_supporting_tests": sum(
            bool(row["supporting_test_ids"]) for row in journey_rows
        ),
        "journeys_with_fresh_visual_evidence": sum(
            row["fresh_visual_evidence"] for row in journey_rows
        ),
        "backend_support_without_fresh_visual": sum(
            row["backend_support_without_fresh_visual"] for row in journey_rows
        ),
        "journey_status_counts": [
            {"name": name, "count": count}
            for name, count in sorted(Counter(row["status"] for row in journey_rows).items())
        ],
    }


def build_payload(
    *,
    commit: str,
    timestamp: str,
    platform: str,
    input_paths: Sequence[str],
    input_digest: str,
    output_json: str,
    output_markdown: str,
    command: Sequence[str],
    workflow_rows: Sequence[Mapping[str, Any]],
    journey_rows: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    workflow_rows = [dict(row) for row in workflow_rows]
    journey_rows = [dict(row) for row in journey_rows]
    return {
        "schema_version": 1,
        "generator": GENERATOR,
        "generator_version": GENERATOR_VERSION,
        "report_status": "complete",
        "commit": commit,
        "timestamp": timestamp,
        "platform": platform,
        "input_paths": list(input_paths),
        "input_digest": input_digest,
        "output_paths": {"json": output_json, "markdown": output_markdown},
        "command": list(command),
        "boundaries": dict(BOUNDARIES),
        "workflow_rows": workflow_rows,
        "journey_rows": journey_rows,
        "summary": build_summary(workflow_rows, journey_rows),
        "limitations": list(LIMITATIONS),
    }


def canonical_json(payload: Mapping[str, Any]) -> str:
    return json.dumps(payload, indent=2, sort_keys=True) + "\n"


def render_markdown(payload: Mapping[str, Any], *, json_digest: str) -> str:
    summary = payload["summary"]
    lines = [
        "# Phase 12 Workflow and UI Journey Audit",
        "",
        f"Generator: `{GENERATOR} v{GENERATOR_VERSION}`",
        f"Source revision: `{payload['commit']}`",
        f"Generated: `{payload['timestamp']}`",
        f"Platform: `{payload['platform']}`",
        f"Generated JSON SHA-256: `{json_digest}`",
        f"Input SHA-256: `{payload['input_digest']}`",
        "",
        "This report inventories workflow specifications and UI journey evidence without",
        "converting backend tests or provisional screenshots into UI acceptance.",
        "",
        "## Summary",
        "",
        f"- Public workflow candidates: {summary['workflow_candidates']}",
        f"- Workflow specifications: {summary['workflow_specs']}",
        f"- Reviewed nonworkflows: {summary['reviewed_nonworkflows']}",
        f"- Workflow specs missing invariant links: {summary['workflow_missing_invariant_link']}",
        f"- Workflow specs missing acceptance evidence: {summary['workflow_missing_acceptance_evidence']}",
        f"- UI journeys: {summary['journeys']}",
        f"- Journeys with fresh visual evidence: {summary['journeys_with_fresh_visual_evidence']}",
        f"- Backend-supported journeys without fresh visual evidence: {summary['backend_support_without_fresh_visual']}",
        "",
        "## Workflow Review",
        "",
        "| Candidate | Disposition | Spec source | Journeys | Invariants | Acceptance |",
        "| --- | --- | --- | --- | --- | --- |",
    ]
    for row in payload["workflow_rows"]:
        lines.append(
            f"| `{row['discovery_id']}` | `{row['disposition']}` | "
            f"`{row['spec_source_id'] or 'none'}` | "
            f"`{', '.join(row['linked_journey_ids']) or 'none'}` | "
            f"`{', '.join(row['invariant_ids']) or 'none'}` | "
            f"`{row['acceptance_status']}` |"
        )
    lines.extend(
        [
            "",
            "## UI Journeys",
            "",
            "| Journey | Status | Workflows | Invariants | Supporting tests | Fresh visual |",
            "| --- | --- | --- | --- | --- | --- |",
        ]
    )
    for row in payload["journey_rows"]:
        lines.append(
            f"| `{row['id']}` | `{row['status']}` | "
            f"`{', '.join(row['workflow_candidate_ids']) or 'none'}` | "
            f"`{', '.join(row['invariant_ids']) or 'none'}` | "
            f"`{', '.join(row['supporting_test_ids']) or 'none'}` | "
            f"`{str(row['fresh_visual_evidence']).lower()}` |"
        )
    lines.extend(["", "## Boundaries", ""])
    lines.extend(
        f"- `{name}`: `{str(payload['boundaries'][name]).lower()}`"
        for name in BOUNDARIES
    )
    lines.extend(["", "## Limitations", ""])
    lines.extend(f"- {item}" for item in payload["limitations"])
    return "\n".join(lines) + "\n"


def write_report(payload: Mapping[str, Any], json_path: Path, markdown_path: Path) -> None:
    rendered = canonical_json(payload)
    digest = hashlib.sha256(rendered.encode()).hexdigest()
    json_path.parent.mkdir(parents=True, exist_ok=True)
    markdown_path.parent.mkdir(parents=True, exist_ok=True)
    json_path.write_text(rendered, encoding="utf-8")
    markdown_path.write_text(render_markdown(payload, json_digest=digest), encoding="utf-8")


def _journey_row(row: Mapping[str, Any]) -> dict[str, Any]:
    status = str(row["status"])
    fresh_visual = status in {"provisional", "ux_accepted"}
    supporting = list(row["supporting_test_ids"])
    return {
        "id": str(row["id"]),
        "title": str(row["title"]),
        "surface": str(row["surface"]),
        "status": status,
        "journey_source": str(row["journey_source"]),
        "workflow_candidate_ids": list(row["workflow_candidate_ids"]),
        "invariant_ids": list(row["invariant_ids"]),
        "supporting_test_ids": supporting,
        "source_transformation": bool(row["source_transformation"]),
        "fresh_visual_evidence": fresh_visual,
        "backend_support_without_fresh_visual": bool(supporting) and not fresh_visual,
    }


def _workflow_acceptance_status(rows: Sequence[Mapping[str, Any]]) -> str:
    statuses = {row["status"] for row in rows}
    for status in ("ux_accepted", "provisional", "stale", "evidence_missing"):
        if status in statuses:
            return "missing" if status == "evidence_missing" else status
    return "missing"
