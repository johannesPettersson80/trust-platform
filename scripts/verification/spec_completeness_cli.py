"""CLI orchestration for the report-only specification-completeness audit."""

from __future__ import annotations

import argparse
import json
import platform
import sys
import tomllib
from pathlib import Path

from .metadata_validator.constants import ROOT as METADATA_ROOT
from .metadata_validator.core import Validator
from .spec_completeness_contract import validate_report_payload, validate_schema_contract
from .spec_completeness_live import load_repository_inputs, validate_report_files
from .spec_completeness_report import (
    DEFAULT_JSON_PATH,
    DEFAULT_MARKDOWN_PATH,
    SpecCompletenessProvenance,
    SpecCompletenessReport,
    write_reports,
)
from .test_catalog_common import input_digest
from .test_catalog_json_schema import validate_json_schema_instance
from .test_catalog_scanner import repository_revision


SCHEMA_PATH = Path("verification/schemas/spec-completeness-report.schema.json")


def default_command(
    json_path: Path,
    markdown_path: Path,
    timestamp: str,
) -> tuple[str, ...]:
    return (
        "python3",
        "scripts/report_spec_completeness.py",
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
) -> SpecCompletenessReport:
    root = root.resolve()
    if root != METADATA_ROOT.resolve():
        raise ValueError("--root must identify the repository that loaded verification modules")
    validator = Validator()
    validator.load_records()
    validator.validate()
    if validator.failures:
        raise ValueError(
            "; ".join(
                f"{_display_path(root, failure.path)}: {failure.message}"
                for failure in validator.failures
            )
        )
    analysis, input_paths = load_repository_inputs(root, validator)
    revision, commit_timestamp = repository_revision(root)
    if len(revision) != 40 or revision.startswith("dirty:"):
        raise ValueError("source commit must identify a clean full Git SHA")
    report_timestamp = timestamp or commit_timestamp
    output_json = _workspace_relative(root, json_path)
    output_markdown = _workspace_relative(root, markdown_path)
    report = SpecCompletenessReport(
        provenance=SpecCompletenessProvenance(
            command=default_command(
                Path(output_json),
                Path(output_markdown),
                report_timestamp,
            ),
            commit=revision,
            timestamp=report_timestamp,
            platform=f"{platform.system().lower()}-{platform.machine().lower()}",
            input_paths=tuple(input_paths),
            output_json=output_json,
            output_markdown=output_markdown,
        ),
        input_digest=input_digest(root, input_paths),
        analysis=analysis,
    )
    schema = json.loads((root / SCHEMA_PATH).read_text())
    failures = validate_report_payload(report.to_dict(), expected_analysis=analysis)
    failures.extend(validate_schema_contract(schema))
    failures.extend(validate_json_schema_instance(report.to_dict(), schema))
    if failures:
        raise ValueError("; ".join(failures))
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
            root / SCHEMA_PATH,
        )
    except (OSError, ValueError, json.JSONDecodeError, tomllib.TOMLDecodeError) as exc:
        print(f"specification-completeness report failed: {exc}", file=sys.stderr)
        return 2
    if failures:
        for failure in failures:
            print(f"specification-completeness report failed at rest: {failure}", file=sys.stderr)
        return 2
    summary = report.analysis["summary"]
    pilot = report.analysis["bytecode_pilot"]["summary"]
    print(
        "specification completeness reported: "
        f"{summary['invariants_without_spec']}/{summary['invariants_total']} invariants not specified, "
        f"{summary['tests_without_oracle']} expected-result tests unbound, "
        f"{summary['spec_gap_cells']} spec-gap cells, "
        f"{pilot['total']} bytecode pilot gaps (report-only)"
    )
    return 0


def _workspace_relative(root: Path, path: Path) -> str:
    candidate = path if path.is_absolute() else root / path
    try:
        return candidate.resolve().relative_to(root).as_posix()
    except (OSError, ValueError) as exc:
        raise ValueError(f"report output escapes workspace: {path}") from exc


def _display_path(root: Path, path: Path) -> str:
    if not path.is_absolute():
        return path.as_posix()
    try:
        return path.relative_to(root).as_posix()
    except ValueError:
        return path.as_posix()
