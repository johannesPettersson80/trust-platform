"""CLI orchestration for the Phase 12 workflow and UI audit."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .phase12_audit import (
    DEFAULT_JSON_PATH,
    DEFAULT_MARKDOWN_PATH,
    build_payload,
    write_report,
)
from .phase12_audit_live import SCHEMA_PATH, build_live_phase12_state
from .phase12_audit_validation import validate_files, validate_payload, validate_schema
from .report_input_contract import resolve_report_output_path
from .test_catalog_json_schema import validate_json_schema_instance


def default_command(json_path: str, markdown_path: str, timestamp: str) -> tuple[str, ...]:
    return (
        "python3",
        "scripts/report_phase12_workflow_ui_audit.py",
        "--json-out",
        json_path,
        "--markdown-out",
        markdown_path,
        "--timestamp",
        timestamp,
    )


def generate(
    root: Path, *, json_path: Path, markdown_path: Path, timestamp: str
) -> dict:
    if not timestamp:
        raise ValueError("timestamp is required")
    root = root.resolve()
    output_json = resolve_report_output_path(root, json_path, "report JSON")[0]
    output_markdown = resolve_report_output_path(root, markdown_path, "report Markdown")[0]
    state = build_live_phase12_state(root, timestamp=timestamp, require_clean_commit=True)
    payload = build_payload(
        commit=state.commit,
        timestamp=state.timestamp,
        platform=state.platform,
        input_paths=state.input_paths,
        input_digest=state.input_digest,
        output_json=output_json,
        output_markdown=output_markdown,
        command=default_command(output_json, output_markdown, state.timestamp),
        workflow_rows=state.workflow_rows,
        journey_rows=state.journey_rows,
    )
    schema = json.loads((root / SCHEMA_PATH).read_text(encoding="utf-8"))
    failures = validate_payload(payload, expected_state=state)
    failures.extend(validate_schema(schema))
    failures.extend(validate_json_schema_instance(payload, schema))
    if failures:
        raise ValueError("; ".join(sorted(set(failures))))
    return payload


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--json-out", type=Path, default=DEFAULT_JSON_PATH)
    parser.add_argument("--markdown-out", type=Path, default=DEFAULT_MARKDOWN_PATH)
    parser.add_argument("--timestamp", required=True)
    args = parser.parse_args(argv)
    root = args.root.resolve()
    try:
        payload = generate(
            root,
            json_path=args.json_out,
            markdown_path=args.markdown_out,
            timestamp=args.timestamp,
        )
        json_path = args.json_out if args.json_out.is_absolute() else root / args.json_out
        markdown_path = (
            args.markdown_out
            if args.markdown_out.is_absolute()
            else root / args.markdown_out
        )
        write_report(payload, json_path, markdown_path)
        failures = validate_files(root, json_path, markdown_path, root / SCHEMA_PATH)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"Phase 12 workflow/UI audit failed: {exc}", file=sys.stderr)
        return 2
    if failures:
        for failure in failures:
            print(f"Phase 12 workflow/UI audit failed at rest: {failure}", file=sys.stderr)
        return 2
    summary = payload["summary"]
    print(
        "Phase 12 workflow/UI audit reported: "
        f"{summary['workflow_candidates']} candidates, {summary['workflow_specs']} workflows, "
        f"{summary['journeys']} journeys, "
        f"{summary['journeys_with_fresh_visual_evidence']} with fresh visual evidence"
    )
    return 0
