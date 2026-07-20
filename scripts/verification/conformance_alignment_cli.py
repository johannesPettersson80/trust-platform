"""CLI orchestration for the report-only Phase 7 conformance audit."""

from __future__ import annotations

import argparse
from pathlib import Path

from .conformance_alignment_live import build_live_conformance_alignment_state
from .conformance_alignment_report import (
    DEFAULT_JSON_PATH,
    DEFAULT_MARKDOWN_PATH,
    ConformanceAlignmentProvenance,
    ConformanceAlignmentReport,
    write_reports,
)
from .metadata_validator.constants import ROOT
from .report_input_contract import resolve_report_output_path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--json-out", type=Path, default=DEFAULT_JSON_PATH)
    parser.add_argument("--markdown-out", type=Path, default=DEFAULT_MARKDOWN_PATH)
    parser.add_argument("--timestamp")
    args = parser.parse_args()
    root = args.root.resolve()
    _, json_output = resolve_report_output_path(root, args.json_out, "JSON")
    _, markdown_output = resolve_report_output_path(root, args.markdown_out, "Markdown")
    if json_output == markdown_output:
        raise ValueError("JSON and Markdown output paths must be distinct")
    state = build_live_conformance_alignment_state(
        root,
        timestamp=args.timestamp,
        require_clean_commit=True,
    )
    command = (
        "python3",
        "scripts/report_conformance_alignment.py",
        "--json-out",
        args.json_out.as_posix(),
        "--markdown-out",
        args.markdown_out.as_posix(),
        "--timestamp",
        state.timestamp,
    )
    report = ConformanceAlignmentReport(
        provenance=ConformanceAlignmentProvenance(
            command=command,
            commit=state.commit,
            timestamp=state.timestamp,
            platform=state.platform,
            input_paths=state.input_paths,
            output_json=args.json_out.as_posix(),
            output_markdown=args.markdown_out.as_posix(),
        ),
        input_digest=state.input_digest,
        analysis=state.analysis,
    )
    write_reports(
        report,
        json_path=json_output,
        markdown_path=markdown_output,
    )
    print(
        "conformance alignment report generated: "
        f"{state.analysis['summary']['cases']} cases, "
        f"{state.analysis['summary']['explicitly_linked_cases']} explicitly linked"
    )
    return 0
