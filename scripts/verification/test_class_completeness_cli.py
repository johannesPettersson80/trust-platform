"""CLI orchestration for the test-class completeness report."""

from __future__ import annotations

import argparse
import json
import sys
import tomllib
from pathlib import Path

from .report_input_contract import resolve_report_output_path

from .metadata_validator.constants import ROOT as METADATA_ROOT
from .metadata_validator.core import Validator
from .report_input_contract import validate_bound_input_paths, validator_code_input_paths
from .test_catalog_common import input_digest
from .test_catalog_json_schema import validate_json_schema_instance
from .test_catalog_scanner import scan_repository
from .test_catalog_staleness import validate_catalog_staleness
from .test_catalog_validation import validate_report_payload as validate_generated_catalog_payload
from .test_class_completeness import (
    DEFAULT_JSON_PATH,
    DEFAULT_MARKDOWN_PATH,
    REPORT_CONTRACT_PATHS,
    CompletenessProvenance,
    TestClassCompletenessReport,
    analyze_test_class_completeness,
    write_reports,
)
from .test_class_completeness_validation import (
    validate_report_files,
    validate_report_payload,
    validate_schema_contract,
)


SCHEMA_PATH = Path("verification/schemas/test-class-completeness-report.schema.json")


def default_command(
    json_path: Path,
    markdown_path: Path,
    timestamp: str | None,
) -> tuple[str, ...]:
    command = [
        "python3",
        "scripts/report_test_class_completeness.py",
        "--json-out",
        json_path.as_posix(),
        "--markdown-out",
        markdown_path.as_posix(),
    ]
    if timestamp:
        command.extend(("--timestamp", timestamp))
    return tuple(command)


def generate_report(
    root: Path,
    *,
    json_path: Path,
    markdown_path: Path,
    timestamp: str | None,
) -> TestClassCompletenessReport:
    root = root.resolve()
    if root != METADATA_ROOT.resolve():
        raise ValueError("--root must identify the repository that loaded the verification modules")
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
    scan = scan_repository(root, timestamp=timestamp)
    scan_failures = validate_generated_catalog_payload(scan.to_dict())
    if scan.to_dict().get("scan_status") != "complete":
        scan_failures.append("generated catalog scan_status is not complete")
    if scan_failures:
        raise ValueError("; ".join(scan_failures))

    matrix = tomllib.loads((root / "verification/matrix.toml").read_text())
    catalog = tomllib.loads((root / "verification/test-catalog.toml").read_text())
    tests = catalog.get("tests")
    if not isinstance(tests, list) or not all(isinstance(item, dict) for item in tests):
        raise ValueError("verification/test-catalog.toml must contain [[tests]] records")
    tests_by_id = {record["id"]: record for record in tests if isinstance(record.get("id"), str)}
    if len(tests_by_id) != len(tests):
        raise ValueError("test catalog ids must be present and unique")
    stale_failures = validate_catalog_staleness(
        root=root,
        tests=tests_by_id,
        facts=scan.inferred_facts,
    )
    if stale_failures:
        raise ValueError("; ".join(stale_failures))

    input_paths = sorted(
        set(scan.provenance.input_paths)
        | set(REPORT_CONTRACT_PATHS)
        | validator_code_input_paths(root)
        | {"verification/matrix.toml", "verification/test-catalog.toml"}
    )
    input_failures = validate_bound_input_paths(root, input_paths)
    if input_failures:
        raise ValueError("; ".join(input_failures))
    output_json = _workspace_relative(root, json_path)
    output_markdown = _workspace_relative(root, markdown_path)
    report_timestamp = timestamp or scan.provenance.timestamp
    report = TestClassCompletenessReport(
        provenance=CompletenessProvenance(
            command=default_command(Path(output_json), Path(output_markdown), report_timestamp),
            commit=scan.provenance.commit,
            timestamp=report_timestamp,
            platform=scan.provenance.platform,
            input_paths=tuple(input_paths),
            output_json=output_json,
            output_markdown=output_markdown,
        ),
        input_digest=input_digest(root, input_paths),
        analysis=analyze_test_class_completeness(
            matrix=matrix,
            tests=tests,
            facts=scan.inferred_facts,
        ),
    )
    failures = validate_report_payload(report.to_dict(), expected_analysis=report.analysis)
    schema = json.loads((root / SCHEMA_PATH).read_text())
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
        markdown_file = args.markdown_out if args.markdown_out.is_absolute() else root / args.markdown_out
        write_reports(report, json_path=json_file, markdown_path=markdown_file)
        failures = validate_report_files(
            root,
            json_file,
            markdown_file,
            root / SCHEMA_PATH,
        )
    except (OSError, ValueError, json.JSONDecodeError, tomllib.TOMLDecodeError) as exc:
        print(f"test-class completeness report failed: {exc}", file=sys.stderr)
        return 2
    if failures:
        for failure in failures:
            print(f"test-class completeness report failed at rest: {failure}", file=sys.stderr)
        return 2
    summary = report.analysis["summary"]
    print(
        "test-class completeness reported: "
        f"{summary['classified_scanner_facts']}/{summary['scanner_facts']} scanner facts classified; "
        f"{summary['complete_class_slots']}/{summary['required_class_slots']} required class slots complete"
    )
    return 0


def _workspace_relative(root: Path, path: Path) -> str:
    return resolve_report_output_path(root, path, "report")[0]


def _display_path(root: Path, path: Path) -> str:
    if not path.is_absolute():
        return path.as_posix()
    try:
        return path.relative_to(root).as_posix()
    except ValueError:
        return path.as_posix()
