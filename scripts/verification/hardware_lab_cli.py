"""CLI orchestration for the Phase 11 hardware-lab report."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .hardware_lab import DEFAULT_JSON_PATH, DEFAULT_MARKDOWN_PATH, REPORT_SCHEMA_PATH
from .hardware_lab_live import build_live_hardware_lab_state
from .hardware_lab_report import build_payload, write_report
from .hardware_lab_validation import validate_files, validate_payload, validate_schema
from .test_catalog_json_schema import validate_json_schema_instance


def generate(
    root: Path,
    *,
    json_path: Path,
    markdown_path: Path,
    branch: str,
    timestamp: str,
) -> dict:
    root = root.resolve()
    state = build_live_hardware_lab_state(
        root,
        branch=branch,
        timestamp=timestamp,
        json_path=json_path,
        markdown_path=markdown_path,
        require_clean_commit=True,
    )
    payload = build_payload(state)
    schema = json.loads((root / REPORT_SCHEMA_PATH).read_text(encoding="utf-8"))
    failures = validate_payload(payload, expected_state=state)
    failures.extend(validate_schema(schema))
    failures.extend(validate_json_schema_instance(payload, schema))
    if failures:
        raise ValueError("; ".join(sorted(set(failures))))
    return payload


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--json-out", type=Path, default=DEFAULT_JSON_PATH)
    parser.add_argument("--markdown-out", type=Path, default=DEFAULT_MARKDOWN_PATH)
    parser.add_argument("--branch", required=True)
    parser.add_argument("--timestamp", required=True)
    args = parser.parse_args(argv)
    root = args.root.resolve()
    try:
        payload = generate(
            root,
            json_path=args.json_out,
            markdown_path=args.markdown_out,
            branch=args.branch,
            timestamp=args.timestamp,
        )
        json_file = root / payload["output_paths"]["json"]
        markdown_file = root / payload["output_paths"]["markdown"]
        write_report(payload, json_file, markdown_file)
        failures = validate_files(root, json_file, markdown_file, root / REPORT_SCHEMA_PATH)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"Hardware-lab report failed: {exc}", file=sys.stderr)
        return 2
    if failures:
        for failure in failures:
            print(f"Hardware-lab report failed at rest: {failure}", file=sys.stderr)
        return 2
    print(
        "Hardware-lab report generated: "
        f"cases={payload['summary']['cases']}, "
        f"skipped_unproven={payload['summary']['skipped_unproven']}, "
        f"evidence={payload['summary']['evidence_records']}"
    )
    return 0
