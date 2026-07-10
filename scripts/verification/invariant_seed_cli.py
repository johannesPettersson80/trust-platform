"""CLI orchestration for the report-only Phase 4 invariant-seed audit."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .invariant_seed_live import REPORT_SCHEMA_PATH, build_live_seed_audit_state
from .invariant_seed_report import (
    DEFAULT_JSON_PATH,
    DEFAULT_MARKDOWN_PATH,
    SeedAuditProvenance,
    SeedAuditReport,
    write_reports,
)
from .invariant_seed_validation import (
    validate_report_files,
    validate_report_payload,
    validate_schema_contract,
)
from .test_catalog_json_schema import validate_json_schema_instance


def default_command(json_path: Path, markdown_path: Path, timestamp: str) -> tuple[str, ...]:
    if not timestamp:
        raise ValueError("timestamp is required")
    return (
        "python3",
        "scripts/report_invariant_seed_audit.py",
        "--json-out",
        json_path.as_posix(),
        "--markdown-out",
        markdown_path.as_posix(),
        "--timestamp",
        timestamp,
    )


def generate_report(
    root: Path,
    *,
    json_path: Path,
    markdown_path: Path,
    timestamp: str | None,
) -> SeedAuditReport:
    root = root.resolve()
    state = build_live_seed_audit_state(
        root, timestamp=timestamp, require_clean_commit=True
    )
    output_json = _workspace_relative(root, json_path)
    output_markdown = _workspace_relative(root, markdown_path)
    report = SeedAuditReport(
        provenance=SeedAuditProvenance(
            command=default_command(Path(output_json), Path(output_markdown), state.timestamp),
            commit=state.commit,
            timestamp=state.timestamp,
            platform=state.platform,
            input_paths=state.input_paths,
            output_json=output_json,
            output_markdown=output_markdown,
        ),
        input_digest=state.input_digest,
        rows=state.audit.rows,
    )
    schema = json.loads((root / REPORT_SCHEMA_PATH).read_text())
    manifest_schema = json.loads(
        (root / "verification/schemas/invariant-seed-manifest.schema.json").read_text()
    )
    payload = report.to_dict()
    failures = validate_report_payload(payload, expected_rows=state.audit.rows)
    failures.extend(validate_schema_contract(schema, manifest_schema=manifest_schema))
    failures.extend(validate_json_schema_instance(payload, schema))
    if failures:
        raise ValueError("; ".join(sorted(set(failures))))
    return report


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--json-out", type=Path, default=DEFAULT_JSON_PATH)
    parser.add_argument("--markdown-out", type=Path, default=DEFAULT_MARKDOWN_PATH)
    parser.add_argument("--timestamp", help="fixed ISO-8601 report timestamp")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        report = generate_report(
            args.root,
            json_path=args.json_out,
            markdown_path=args.markdown_out,
            timestamp=args.timestamp,
        )
        root = args.root.resolve()
        json_file = _absolute(root, args.json_out)
        markdown_file = _absolute(root, args.markdown_out)
        write_reports(report, json_path=json_file, markdown_path=markdown_file)
        failures = validate_report_files(
            root, json_file, markdown_file, root / REPORT_SCHEMA_PATH
        )
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"invariant-seed audit failed: {exc}", file=sys.stderr)
        return 2
    if failures:
        for failure in failures:
            print(f"invariant-seed audit failed at rest: {failure}", file=sys.stderr)
        return 2
    summary = report.to_dict()["summary"]
    print(
        "invariant-seed audit reported: "
        f"{summary['seeds']} seeds, {summary['canonical_invariants']} canonical invariants, "
        f"{summary['p4_000_risks']} imported review risks; report-only exit 0"
    )
    return 0


def _workspace_relative(root: Path, path: Path) -> str:
    candidate = path if path.is_absolute() else root / path
    try:
        return candidate.resolve().relative_to(root).as_posix()
    except (OSError, ValueError) as exc:
        raise ValueError(f"report output escapes workspace: {path}") from exc


def _absolute(root: Path, path: Path) -> Path:
    return path if path.is_absolute() else root / path
