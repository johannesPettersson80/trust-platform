"""CLI orchestration for the report-only Phase 10 mutation-program audit."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .metadata_validator.constants import ROOT
from .report_input_contract import resolve_report_output_path
from .mutation_program_live import REPORT_SCHEMA_PATH, build_live_mutation_program_state
from .mutation_program_report import (
    DEFAULT_JSON_PATH,
    DEFAULT_MARKDOWN_PATH,
    MutationProgramReport,
    write_reports,
)
from .mutation_program_report_contract import validate_report_payload, validate_schema_contract
from .mutation_program_validation import validate_report_files
from .test_catalog_json_schema import validate_json_schema_instance


def report_main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--json-out", type=Path, default=DEFAULT_JSON_PATH)
    parser.add_argument("--markdown-out", type=Path, default=DEFAULT_MARKDOWN_PATH)
    parser.add_argument("--timestamp")
    args = parser.parse_args(argv)
    root = args.root.resolve()
    try:
        json_output = _validated_output_path(root, args.json_out, "JSON")
        markdown_output = _validated_output_path(root, args.markdown_out, "Markdown")
        if json_output == markdown_output:
            raise ValueError("JSON and Markdown output paths must be distinct")
        state = build_live_mutation_program_state(
            root,
            timestamp=args.timestamp,
            require_clean_commit=True,
        )
        collisions = sorted(
            {args.json_out.as_posix(), args.markdown_out.as_posix()}
            & set(state.input_paths)
        )
        if collisions:
            raise ValueError(
                "report outputs cannot overwrite bound inputs: " + ", ".join(collisions)
            )
        report = MutationProgramReport.from_state(state)
        schema = json.loads((root / REPORT_SCHEMA_PATH).read_text())
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"mutation-program report generation failed: {exc}", file=sys.stderr)
        return 2
    failures = validate_schema_contract(schema)
    failures.extend(validate_json_schema_instance(report.payload, schema))
    failures.extend(validate_report_payload(report.payload, expected_state=state))
    if failures:
        for failure in sorted(set(failures)):
            print(f"mutation-program report generation failed: {failure}", file=sys.stderr)
        return 2
    write_reports(report, json_path=json_output, markdown_path=markdown_output)
    failures = validate_report_files(
        root,
        args.json_out,
        args.markdown_out,
        Path(REPORT_SCHEMA_PATH),
    )
    if failures:
        for failure in failures:
            print(f"mutation-program report failed at-rest validation: {failure}", file=sys.stderr)
        return 2
    print(
        "mutation-program report generated: "
        f"{state.summary['shards']} shards, "
        f"{state.summary['measured_mutants']} measured mutants, "
        f"{state.summary['survived']} survivors"
    )
    return 0


def validate_main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate a Phase 10 mutation-program report")
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--json", type=Path, default=DEFAULT_JSON_PATH)
    parser.add_argument("--markdown", type=Path, default=DEFAULT_MARKDOWN_PATH)
    args = parser.parse_args(argv)
    failures = validate_report_files(
        args.root.resolve(),
        args.json,
        args.markdown,
        Path(REPORT_SCHEMA_PATH),
    )
    if failures:
        for failure in failures:
            print(f"mutation-program report validation failed: {failure}", file=sys.stderr)
        return 1
    print("mutation-program report validation passed")
    return 0


def _validated_output_path(root: Path, value: Path, label: str) -> Path:
    return resolve_report_output_path(root, value, label)[1]
