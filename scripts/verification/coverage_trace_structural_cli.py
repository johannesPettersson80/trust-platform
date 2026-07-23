"""Generate the exact structural/ignored trace shard from the live scanner."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .code_spec_live import load_code_spec_metadata
from .coverage_trace_structural import (
    build_structural_trace_document,
    current_platform,
    git_head,
    structural_route_reason,
)
from .test_catalog_scanner import scan_repository


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument(
        "--out",
        type=Path,
        default=Path(
            "target/gate-artifacts/verification/structural-test-trace.json"
        ),
    )
    parser.add_argument(
        "--mode",
        choices=("focused_shard", "full_campaign"),
        default="focused_shard",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        scan = scan_repository(root)
        facts = [
            {
                "discovery_id": fact.stable_id,
                "discovery_source_kind": fact.source_kind,
                "path": fact.path,
                "name": fact.name,
                "command_hint": fact.command_hint,
                "ignore_state": fact.ignore_state,
            }
            for fact in scan.inferred_facts
        ]
        selected = [
            fact for fact in facts if structural_route_reason(fact) is not None
        ]
        metadata = load_code_spec_metadata(root)
        document = build_structural_trace_document(
            facts=selected,
            scanner_input_digest=scan.input_digest,
            code_map=metadata.code_map,
            source_commit=git_head(root),
            platform_name=current_platform(),
            mode=args.mode,
        )
        output = args.out if args.out.is_absolute() else root / args.out
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"structural coverage trace failed: {error}", file=sys.stderr)
        return 2
    print(f"structural coverage trace complete: {len(document['tests'])} facts routed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
