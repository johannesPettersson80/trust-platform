"""CLI orchestration for the Phase 13 release-evidence audit."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .phase13_release import DEFAULT_JSON_PATH, DEFAULT_MARKDOWN_PATH, build_payload, write_report
from .phase13_release_live import REPORT_SCHEMA_PATH, build_live_phase13_state
from .phase13_release_validation import validate_files, validate_payload, validate_schema
from .report_input_contract import resolve_report_output_path
from .test_catalog_json_schema import validate_json_schema_instance


def default_command(json_path: str, markdown_path: str, branch: str, timestamp: str) -> tuple[str, ...]:
    return (
        "python3", "scripts/report_phase13_release_evidence.py",
        "--json-out", json_path, "--markdown-out", markdown_path,
        "--branch", branch, "--timestamp", timestamp,
    )


def generate(root: Path, *, json_path: Path, markdown_path: Path, branch: str, timestamp: str) -> dict:
    if not timestamp:
        raise ValueError("timestamp is required")
    root = root.resolve()
    output_json = resolve_report_output_path(root, json_path, "report JSON")[0]
    output_markdown = resolve_report_output_path(root, markdown_path, "report Markdown")[0]
    state = build_live_phase13_state(root, branch=branch, timestamp=timestamp, require_clean_commit=True)
    payload = build_payload(
        commit=state.commit, branch=state.branch, timestamp=state.timestamp,
        platform=state.platform, input_paths=state.input_paths, input_digest=state.input_digest,
        output_json=output_json, output_markdown=output_markdown,
        command=default_command(output_json, output_markdown, state.branch, state.timestamp),
        candidate=state.candidate, public_release=state.public_release,
        proof_origins=state.proof_origins, security=state.security, platforms=state.platforms,
        conformance=state.conformance, hardware_labs=state.hardware_labs,
        ui_acceptance=state.ui_acceptance, known_gaps=state.known_gaps,
    )
    schema = json.loads((root / REPORT_SCHEMA_PATH).read_text(encoding="utf-8"))
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
    parser.add_argument("--branch", required=True)
    parser.add_argument("--timestamp", required=True)
    args = parser.parse_args(argv)
    root = args.root.resolve()
    try:
        payload = generate(
            root, json_path=args.json_out, markdown_path=args.markdown_out,
            branch=args.branch, timestamp=args.timestamp,
        )
        json_path = args.json_out if args.json_out.is_absolute() else root / args.json_out
        markdown_path = args.markdown_out if args.markdown_out.is_absolute() else root / args.markdown_out
        write_report(payload, json_path, markdown_path)
        failures = validate_files(root, json_path, markdown_path, root / REPORT_SCHEMA_PATH)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"Phase 13 release-evidence audit failed: {exc}", file=sys.stderr)
        return 2
    if failures:
        for failure in failures:
            print(f"Phase 13 release-evidence audit failed at rest: {failure}", file=sys.stderr)
        return 2
    print(
        "Phase 13 release-evidence audit reported: "
        f"candidate={payload['candidate']['expected_tag']} complete={payload['candidate']['release_complete']}, "
        f"latest={payload['public_release']['tag']}, hardware_unproven={len(payload['hardware_labs'])}"
    )
    return 0
