"""Canonical Phase 11 hardware-lab report model and renderer."""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from .hardware_lab_live import HardwareLabState


GENERATOR = "hardware-lab-audit"
GENERATOR_VERSION = 1
BOUNDARIES = {
    "hardware_executed": False,
    "skipped_case_is_hardware_proof": False,
    "manual_script_is_hardware_proof": False,
    "public_hardware_claim_qualified": False,
    "report_emits_product_proof": False,
}
LIMITATIONS = (
    "The report defines reviewed hardware-lab cases and bindings; it does not execute hardware.",
    "Every case remains skipped/unproven until a strict named-topology result is recorded as durable lab evidence.",
    "The GPIO case binds an existing manual script and tracked example but is not part of the strict device-in-loop harness.",
    "Public hardware documentation remains preview/unverified and no physical target is qualified by this report.",
)


def build_payload(state: HardwareLabState) -> dict[str, Any]:
    command = [
        "python3", "scripts/report_hardware_lab.py",
        "--json-out", state.output_json,
        "--markdown-out", state.output_markdown,
        "--branch", state.branch,
        "--timestamp", state.timestamp,
    ]
    return {
        "schema_version": 1,
        "generator": GENERATOR,
        "generator_version": GENERATOR_VERSION,
        "report_status": "complete",
        "commit": state.commit,
        "branch": state.branch,
        "timestamp": state.timestamp,
        "platform": state.platform,
        "input_paths": list(state.input_paths),
        "input_digest": state.input_digest,
        "output_paths": {"json": state.output_json, "markdown": state.output_markdown},
        "command": command,
        "boundaries": dict(BOUNDARIES),
        "summary": dict(state.summary),
        "public_claim": dict(state.public_claim),
        "cases": [dict(row) for row in state.cases],
        "limitations": list(LIMITATIONS),
    }


def canonical_json(payload: Mapping[str, Any]) -> str:
    return json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False) + "\n"


def render_markdown(payload: Mapping[str, Any], *, json_digest: str) -> str:
    summary = payload["summary"]
    claim = payload["public_claim"]
    lines = [
        "# Phase 11 Hardware-Lab Program",
        "",
        f"- Source commit: `{payload['commit']}`",
        f"- Branch: `{payload['branch']}`",
        f"- Timestamp: `{payload['timestamp']}`",
        f"- Platform: `{payload['platform']}`",
        f"- JSON SHA-256: `{json_digest}`",
        f"- Input digest: `{payload['input_digest']}`",
        f"- Cases: {summary['cases']}",
        f"- Protocols: {summary['protocols']}",
        f"- Strict-harness cases: {summary['strict_harness_cases']}",
        f"- Manual-script cases: {summary['manual_script_cases']}",
        f"- Skipped/unproven: {summary['skipped_unproven']}",
        f"- Durable lab evidence records: {summary['evidence_records']}",
        "",
        "No hardware execution is claimed by this report.",
        "",
        "## Public Claim Boundary",
        "",
        f"Status: `{claim['status']}`. Hardware qualified: `{str(claim['hardware_qualified']).lower()}`.",
        "",
        claim["limitation"],
        "",
        "## Cases",
        "",
        "| Case | Board row | Protocol | Binding | Proof status | Evidence |",
        "|---|---|---|---|---|---:|",
    ]
    for row in payload["cases"]:
        lines.append(
            f"| `{row['id']}` | `{row['board_row']}` | `{row['protocol']}` | "
            f"`{row['binding_kind']}` | `{row['proof_status']}` | {len(row['evidence_ids'])} |"
        )
    lines.extend(["", "## Case Details", ""])
    for row in payload["cases"]:
        lines.extend(
            [
                f"### {row['id']} - {row['title']}",
                "",
                f"- Command: `{row['command']}`",
                f"- Required environment: {', '.join(f'`{item}`' for item in row['required_env_vars']) or 'none'}",
                f"- Topology: {row['topology']}",
                f"- Topology reference: `{row['topology_ref']}`",
                f"- Expected artifacts: {', '.join(f'`{item}`' for item in row['artifact_paths'])}",
                f"- Public-claim impact: {row['public_claim_impact']}",
                "- Assertions:",
                *[f"  - {item}" for item in row["assertions"]],
                "",
            ]
        )
    lines.extend(["## Limitations", "", *[f"- {item}" for item in payload["limitations"]], ""])
    return "\n".join(lines)


def write_report(payload: Mapping[str, Any], json_path: Path, markdown_path: Path) -> None:
    raw = canonical_json(payload).encode("utf-8")
    json_path.parent.mkdir(parents=True, exist_ok=True)
    markdown_path.parent.mkdir(parents=True, exist_ok=True)
    json_path.write_bytes(raw)
    markdown_path.write_text(
        render_markdown(payload, json_digest=hashlib.sha256(raw).hexdigest()),
        encoding="utf-8",
    )
