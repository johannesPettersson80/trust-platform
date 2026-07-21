"""Canonical Phase 18 report serialization."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any, Mapping


def canonical_json_bytes(payload: Mapping[str, Any]) -> bytes:
    return (json.dumps(payload, sort_keys=True, separators=(",", ":")) + "\n").encode()


def render_markdown(payload: Mapping[str, Any]) -> str:
    summary = payload.get("summary", {})
    findings = payload.get("findings", [])
    lines = [
        "# Phase 18 Zero-Debt Report",
        "",
        f"Status: `{payload.get('status', 'unknown')}`",
        f"Producer source: `{payload.get('source_sha', 'unavailable')}`",
        f"Contract source: `{payload.get('contract_source_sha', 'unavailable')}`",
        "",
        "## Zero-debt counters",
        "",
        "| Counter | Value |",
        "| --- | ---: |",
    ]
    if isinstance(summary, Mapping):
        for key in sorted(summary):
            lines.append(f"| `{key}` | {summary[key]} |")
    lines.extend(("", "## Findings", ""))
    if isinstance(findings, list) and findings:
        lines.extend(f"- {finding}" for finding in findings)
    else:
        lines.append("- none")
    return "\n".join(lines) + "\n"


def write_artifacts(
    out_dir: Path,
    report: Mapping[str, Any],
    *,
    input_paths: list[Path],
    source_sha: str,
    exit_status: int,
    display_root: Path | None = None,
) -> dict[str, Any]:
    out_dir.mkdir(parents=True, exist_ok=True)
    json_path = out_dir / "p18-zero-debt.json"
    markdown_path = out_dir / "p18-zero-debt.md"
    json_path.write_bytes(canonical_json_bytes(report))
    markdown_path.write_text(render_markdown(report))
    rows = []
    for path, kind in (
        *((path, "input") for path in input_paths),
        (json_path, "report_json"),
        (markdown_path, "report_markdown"),
    ):
        display_path = path
        if display_root is not None:
            try:
                display_path = path.relative_to(display_root)
            except ValueError:
                pass
        rows.append(
            {
                "path": path.name if path.parent == out_dir else display_path.as_posix(),
                "kind": kind,
                "sha256": "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest(),
            }
        )
    rows.sort(key=lambda row: row["path"])
    closure = canonical_json_bytes({"artifacts": rows})
    index = {
        "schema_version": 1,
        "source_sha": str(report.get("source_sha", source_sha)),
        "contract_source_sha": source_sha,
        "producer": "scripts/p18_zero_debt_gate.py",
        "validator": "scripts.verification.p18_zero_debt.contracts",
        "exit_status": exit_status,
        "input_closure_sha256": "sha256:" + hashlib.sha256(closure).hexdigest(),
        "artifacts": rows,
    }
    (out_dir / "artifact-index.json").write_bytes(canonical_json_bytes(index))
    return index
