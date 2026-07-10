"""CLI orchestration for the report-only Phase 3 ignored-test inventory."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .ignored_test_live import REPORT_SCHEMA_PATH, build_live_inventory_state
from .ignored_test_models import (
    DEFAULT_JSON_PATH,
    DEFAULT_MARKDOWN_PATH,
    IgnoredTestInventoryReport,
    InventoryProvenance,
    write_reports,
)
from .ignored_test_validation import (
    validate_report_files,
    validate_report_payload,
    validate_schema_contract,
)
from .test_catalog_common import input_digest
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
        "scripts/report_ignored_test_inventory.py",
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
) -> IgnoredTestInventoryReport:
    root = root.resolve()
    state = build_live_inventory_state(
        root, timestamp=timestamp, require_clean_commit=True
    )
    output_json = _workspace_relative(root, json_path)
    output_markdown = _workspace_relative(root, markdown_path)
    report = IgnoredTestInventoryReport(
        provenance=InventoryProvenance(
            command=default_command(
                Path(output_json), Path(output_markdown), state.timestamp
            ),
            commit=state.commit,
            timestamp=state.timestamp,
            platform=state.platform,
            input_paths=state.input_paths,
            output_json=output_json,
            output_markdown=output_markdown,
        ),
        input_digest=input_digest(root, state.input_paths),
        records=state.analysis.records,
        diagnostics=state.analysis.diagnostics,
        surface_summary=state.analysis.surface_summary,
        limitations=state.analysis.limitations,
    )
    schema = json.loads((root / REPORT_SCHEMA_PATH).read_text())
    failures = validate_report_payload(report.to_dict())
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
        json_file = _absolute(root, args.json_out)
        markdown_file = _absolute(root, args.markdown_out)
        write_reports(report, json_path=json_file, markdown_path=markdown_file)
        failures = validate_report_files(
            root, json_file, markdown_file, root / REPORT_SCHEMA_PATH
        )
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"ignored-test inventory failed: {exc}", file=sys.stderr)
        return 2
    if failures:
        for failure in failures:
            print(f"ignored-test inventory failed at rest: {failure}", file=sys.stderr)
        return 2
    summary = report.to_dict()["summary"]
    print(
        "ignored-test inventory reported: "
        f"{summary['ignored']} ignored, {summary['conditional']} conditional, "
        f"{summary['warnings']} warning(s); report-only exit 0"
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
