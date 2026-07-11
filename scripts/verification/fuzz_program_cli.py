"""CLI orchestration for the report-only Phase 9 fuzz-program audit."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path, PurePosixPath

from .fuzz_program_live import REPORT_SCHEMA_PATH, build_live_fuzz_program_state
from .fuzz_program_report import (
    DEFAULT_JSON_PATH,
    DEFAULT_MARKDOWN_PATH,
    report_from_state,
    write_reports,
)
from .fuzz_program_report_contract import validate_report_payload, validate_schema_contract
from .fuzz_program_validation import validate_report_files
from .metadata_validator.constants import ROOT
from .test_catalog_json_schema import validate_json_schema_instance


def main(argv: list[str] | None = None) -> int:
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
    except ValueError as exc:
        print(f"fuzz-program report generation failed: {exc}", file=sys.stderr)
        return 2
    try:
        state = build_live_fuzz_program_state(
            root,
            timestamp=args.timestamp,
            require_clean_commit=True,
        )
    except (OSError, ValueError) as exc:
        print(f"fuzz-program report generation failed: {exc}", file=sys.stderr)
        return 2
    report = report_from_state(
        state,
        output_json=args.json_out.as_posix(),
        output_markdown=args.markdown_out.as_posix(),
    )
    try:
        schema = json.loads((root / REPORT_SCHEMA_PATH).read_text())
    except (OSError, json.JSONDecodeError) as exc:
        print(f"fuzz-program report schema cannot be read: {exc}", file=sys.stderr)
        return 2
    failures = validate_schema_contract(schema)
    failures.extend(validate_json_schema_instance(report.to_dict(), schema))
    failures.extend(validate_report_payload(report.to_dict(), expected_state=state))
    if failures:
        for failure in sorted(set(failures)):
            print(f"fuzz-program report generation failed: {failure}", file=sys.stderr)
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
            print(f"fuzz-program report failed at-rest validation: {failure}", file=sys.stderr)
        return 2
    summary = state.analysis["summary"]
    print(
        "fuzz-program report generated: "
        f"{summary['inventory_targets']} targets, "
        f"{summary['required_surfaces']} surfaces, "
        f"{summary['gap_surfaces']} gaps"
    )
    return 0


def _validated_output_path(root: Path, value: Path, label: str) -> Path:
    raw = value.as_posix()
    relative = PurePosixPath(raw)
    if (
        not relative.parts
        or value.is_absolute()
        or "\\" in raw
        or ".." in relative.parts
        or "." in relative.parts
    ):
        raise ValueError(f"{label} output path must be normalized and workspace-relative")
    candidate = root
    for part in relative.parts:
        candidate /= part
        if candidate.is_symlink():
            raise ValueError(f"{label} output path must not contain a symlink")
    try:
        candidate.resolve(strict=False).relative_to(root)
    except ValueError as exc:
        raise ValueError(f"{label} output path escapes the workspace") from exc
    return candidate
