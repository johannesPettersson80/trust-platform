"""CLI orchestration for the report-only specification-source audit."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .report_input_contract import resolve_report_output_path

from .spec_source_contract import validate_report_payload, validate_schema_contract
from .spec_source_live import (
    REPORT_SCHEMA_PATH,
    build_live_spec_source_state,
    validate_report_files,
)
from .spec_source_report import (
    DEFAULT_JSON_PATH,
    DEFAULT_MARKDOWN_PATH,
    SpecSourceAuditProvenance,
    SpecSourceAuditReport,
    write_reports,
)
from .test_catalog_json_schema import validate_json_schema_instance


def default_command(
    json_path: Path,
    markdown_path: Path,
    timestamp: str,
) -> tuple[str, ...]:
    if not timestamp:
        raise ValueError("timestamp is required")
    return (
        "python3",
        "scripts/report_spec_source_audit.py",
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
) -> SpecSourceAuditReport:
    root = root.resolve()
    output_json = _relative_output(root, json_path)
    output_markdown = _relative_output(root, markdown_path)
    if output_json == output_markdown:
        raise ValueError("report output paths must be distinct")
    state = build_live_spec_source_state(
        root,
        timestamp=timestamp,
        require_clean_commit=True,
    )
    report = SpecSourceAuditReport(
        provenance=SpecSourceAuditProvenance(
            command=default_command(
                Path(output_json),
                Path(output_markdown),
                state.timestamp,
            ),
            commit=state.commit,
            timestamp=state.timestamp,
            platform=state.platform,
            input_paths=state.input_paths,
            output_json=output_json,
            output_markdown=output_markdown,
        ),
        input_digest=state.input_digest,
        analysis=state.analysis,
    )
    schema = json.loads((root / REPORT_SCHEMA_PATH).read_text())
    payload = report.to_dict()
    failures = validate_report_payload(payload, expected_analysis=state.analysis)
    failures.extend(validate_schema_contract(schema))
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
        json_file = args.json_out if args.json_out.is_absolute() else root / args.json_out
        markdown_file = (
            args.markdown_out
            if args.markdown_out.is_absolute()
            else root / args.markdown_out
        )
        write_reports(report, json_path=json_file, markdown_path=markdown_file)
        failures = validate_report_files(
            root,
            json_file,
            markdown_file,
            root / REPORT_SCHEMA_PATH,
        )
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"specification-source audit failed: {exc}", file=sys.stderr)
        return 2
    if failures:
        for failure in failures:
            print(f"specification-source audit failed at rest: {failure}", file=sys.stderr)
        return 2
    summary = report.to_dict()["summary"]
    print(
        "specification-source audit reported: "
        f"{summary['documents_total']} documents, "
        f"{summary['public_prose_blocks']} public prose blocks, "
        f"{summary['unreviewed_public_blocks']} unreviewed, "
        f"{summary['warning_findings']} warnings; report-only exit 0"
    )
    return 0


def _relative_output(root: Path, path: Path) -> str:
    return resolve_report_output_path(root, path, "report")[0]
