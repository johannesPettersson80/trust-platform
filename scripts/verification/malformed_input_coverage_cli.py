"""CLI orchestration for malformed-input coverage reporting."""

from __future__ import annotations

import argparse
import json
import sys
import tomllib
from pathlib import Path

from .malformed_input_contract import (
    load_malformed_input_taxonomy,
    validate_catalog_malformed_bindings,
    validate_malformed_input_contract,
)
from .malformed_input_coverage import (
    DEFAULT_JSON_PATH,
    DEFAULT_MARKDOWN_PATH,
    REPORT_CONTRACT_PATHS,
    MalformedCoverageProvenance,
    MalformedInputCoverageReport,
    analyze_malformed_input_coverage,
    write_reports,
)
from .malformed_input_coverage_validation import (
    validate_report_files,
    validate_report_payload,
    validate_schema_contract,
)
from .metadata_validator.constants import ROOT as METADATA_ROOT
from .metadata_validator.core import Validator
from .report_input_contract import validate_bound_input_paths, validator_code_input_paths
from .test_catalog_common import input_digest
from .test_catalog_json_schema import validate_json_schema_instance
from .test_catalog_scanner import scan_repository
from .test_catalog_staleness import validate_catalog_staleness
from .test_catalog_validation import validate_report_payload as validate_generated_catalog_payload


SCHEMA_PATH = Path("verification/schemas/malformed-input-coverage-report.schema.json")


def default_command(json_path: Path, markdown_path: Path, timestamp: str) -> tuple[str, ...]:
    return (
        "python3",
        "scripts/report_malformed_input_coverage.py",
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
) -> MalformedInputCoverageReport:
    root = root.resolve()
    if root != METADATA_ROOT.resolve():
        raise ValueError("--root must identify the repository that loaded verification modules")
    validator = Validator()
    validator.load_records()
    validator.validate()
    if validator.failures:
        raise ValueError(
            "; ".join(f"{failure.path}: {failure.message}" for failure in validator.failures)
        )

    scan = scan_repository(root, timestamp=timestamp)
    scan_payload = scan.to_dict()
    scan_failures = validate_generated_catalog_payload(scan_payload)
    if scan_payload.get("scan_status") != "complete":
        scan_failures.append("generated catalog scan_status is not complete")
    if scan_failures:
        raise ValueError("; ".join(scan_failures))

    taxonomy = load_malformed_input_taxonomy(root)
    contract_failures = validate_malformed_input_contract(root, taxonomy)
    catalog = tomllib.loads((root / "verification/test-catalog.toml").read_text())
    tests = catalog.get("tests")
    if not isinstance(tests, list) or not all(isinstance(item, dict) for item in tests):
        raise ValueError("verification/test-catalog.toml must contain [[tests]] records")
    tests_by_id = {record["id"]: record for record in tests if isinstance(record.get("id"), str)}
    if len(tests_by_id) != len(tests):
        raise ValueError("test catalog IDs must be present and unique")
    contract_failures.extend(
        validate_catalog_malformed_bindings(tests=tests_by_id, taxonomy=taxonomy)
    )
    contract_failures.extend(
        validate_catalog_staleness(root=root, tests=tests_by_id, facts=scan.inferred_facts)
    )
    if contract_failures:
        raise ValueError("; ".join(contract_failures))

    input_paths = sorted(
        set(scan.provenance.input_paths)
        | set(REPORT_CONTRACT_PATHS)
        | validator_code_input_paths(root)
    )
    input_failures = validate_bound_input_paths(root, input_paths)
    if input_failures:
        raise ValueError("; ".join(input_failures))
    output_json = _workspace_relative(root, json_path)
    output_markdown = _workspace_relative(root, markdown_path)
    report_timestamp = timestamp or scan.provenance.timestamp
    report = MalformedInputCoverageReport(
        provenance=MalformedCoverageProvenance(
            command=default_command(Path(output_json), Path(output_markdown), report_timestamp),
            commit=scan.provenance.commit,
            timestamp=report_timestamp,
            platform=scan.provenance.platform,
            input_paths=tuple(input_paths),
            output_json=output_json,
            output_markdown=output_markdown,
        ),
        input_digest=input_digest(root, input_paths),
        analysis=analyze_malformed_input_coverage(
            taxonomy=taxonomy,
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
        failures = validate_report_files(root, json_file, markdown_file, root / SCHEMA_PATH)
    except (OSError, ValueError, json.JSONDecodeError, tomllib.TOMLDecodeError) as exc:
        print(f"malformed-input coverage report failed: {exc}", file=sys.stderr)
        return 2
    if failures:
        for failure in failures:
            print(f"malformed-input coverage report failed at rest: {failure}", file=sys.stderr)
        return 2
    summary = report.analysis["summary"]
    print(
        "malformed-input coverage reported: "
        f"{summary['mapped_classes']}/{summary['taxonomy_classes']} classes mapped; "
        f"{summary['by_state']['covered']} covered, "
        f"{summary['by_state']['gap_open']} gap_open, "
        f"{summary['by_state']['spec_gap']} spec_gap"
    )
    return 0


def _workspace_relative(root: Path, path: Path) -> str:
    candidate = path if path.is_absolute() else root / path
    try:
        return candidate.resolve().relative_to(root).as_posix()
    except (OSError, ValueError) as exc:
        raise ValueError(f"report output escapes workspace: {path}") from exc
