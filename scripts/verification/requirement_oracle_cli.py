"""CLI orchestration for the report-only Phase 6 requirement/oracle audit."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .requirement_oracle_contract import validate_report_payload, validate_schema_contract
from .requirement_oracle_live import REPORT_SCHEMA_PATH, build_live_requirement_oracle_state
from .requirement_oracle_report import (
    DEFAULT_JSON_PATH,
    DEFAULT_MARKDOWN_PATH,
    RequirementOracleProvenance,
    RequirementOracleReport,
    write_reports,
)
from .requirement_oracle_validation import validate_report_files
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
        "scripts/report_requirement_oracle_audit.py",
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
) -> RequirementOracleReport:
    root = root.resolve()
    state = build_live_requirement_oracle_state(
        root,
        timestamp=timestamp,
        require_clean_commit=True,
    )
    output_json = _relative(root, json_path)
    output_markdown = _relative(root, markdown_path)
    report = RequirementOracleReport(
        provenance=RequirementOracleProvenance(
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
        print(f"requirement/oracle audit failed: {exc}", file=sys.stderr)
        return 2
    if failures:
        for failure in failures:
            print(f"requirement/oracle audit failed at rest: {failure}", file=sys.stderr)
        return 2
    summary = report.to_dict()["summary"]
    print(
        "requirement/oracle audit reported: "
        f"{summary['invariants_total']} invariants, "
        f"{summary['eligible_oracles']} eligible oracles, "
        f"{summary['missing_oracles']} missing; report-only exit 0"
    )
    return 0


def _relative(root: Path, path: Path) -> str:
    candidate = path if path.is_absolute() else root / path
    try:
        return candidate.resolve().relative_to(root).as_posix()
    except (OSError, ValueError) as exc:
        raise ValueError(f"report output escapes workspace: {path}") from exc
