"""CLI orchestration for the report-only Phase 2A refactor assessment."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .report_input_contract import resolve_report_output_path

from .test_catalog_common import input_digest
from .test_catalog_json_schema import validate_json_schema_instance
from .test_refactor_live import REPORT_SCHEMA_PATH, build_live_test_refactor_state
from .test_refactor_report import (
    DEFAULT_JSON_PATH,
    DEFAULT_MARKDOWN_PATH,
    RefactorAssessmentProvenance,
    TestRefactorAssessmentReport,
    write_reports,
)
from .test_refactor_validation import (
    validate_report_files,
    validate_report_payload,
    validate_schema_contract,
)


def default_command(
    json_path: Path,
    markdown_path: Path,
    timestamp: str,
) -> tuple[str, ...]:
    if not timestamp:
        raise ValueError("timestamp is required")
    return (
        "python3",
        "scripts/report_test_refactor_assessment.py",
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
) -> TestRefactorAssessmentReport:
    root = root.resolve()
    state = build_live_test_refactor_state(root, timestamp=timestamp)
    output_json = _workspace_relative(root, json_path)
    output_markdown = _workspace_relative(root, markdown_path)
    report = TestRefactorAssessmentReport(
        provenance=RefactorAssessmentProvenance(
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
        input_digest=input_digest(root, state.input_paths),
        scope=state.scope,
        assessment=state.assessment,
        limitations=state.limitations,
    )
    schema = json.loads((root / REPORT_SCHEMA_PATH).read_text())
    failures = validate_report_payload(
        report.to_dict(),
        expected_assessment=state.assessment,
        expected_scope=state.scope,
        expected_limitations=state.limitations,
    )
    failures.extend(validate_schema_contract(schema))
    failures.extend(validate_json_schema_instance(report.to_dict(), schema))
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
            args.markdown_out if args.markdown_out.is_absolute() else root / args.markdown_out
        )
        write_reports(report, json_path=json_file, markdown_path=markdown_file)
        failures = validate_report_files(
            root,
            json_file,
            markdown_file,
            root / REPORT_SCHEMA_PATH,
        )
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"test-refactor assessment failed: {exc}", file=sys.stderr)
        return 2
    if failures:
        for failure in failures:
            print(f"test-refactor assessment failed at rest: {failure}", file=sys.stderr)
        return 2
    summary = report.assessment["summary"]
    print(
        "test-refactor assessment reported: "
        f"{summary['large_file_candidates']} large-file signals, "
        f"{summary['broad_claim_candidates']} broad-claim signals, "
        f"{summary['supported_proposals']}/{summary['proposals']} supported decisions; "
        "report-only exit 0"
    )
    return 0


def _workspace_relative(root: Path, path: Path) -> str:
    return resolve_report_output_path(root, path, "report")[0]
