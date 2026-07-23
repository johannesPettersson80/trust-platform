"""Merge disjoint trace shards and optionally require the full live denominator."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

from .coverage_trace_merge import merge_trace_documents
from .test_catalog_scanner import scan_repository


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--input", type=Path, action="append", required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument(
        "--mode",
        choices=("focused_shard", "full_campaign"),
        default="focused_shard",
    )
    parser.add_argument("--require-complete-denominator", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        documents = [_load_json(_resolve(root, path)) for path in args.input]
        scan = scan_repository(root)
        live_facts = {
            fact.stable_id: {
                "discovery_id": fact.stable_id,
                "discovery_source_kind": fact.source_kind,
                "path": fact.path,
                "name": fact.name,
            }
            for fact in scan.inferred_facts
        }
        merged = merge_trace_documents(
            documents,
            live_facts=live_facts,
            mode=args.mode,
            require_complete_denominator=args.require_complete_denominator,
        )
        output = _resolve(root, args.out)
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(merged, indent=2, sort_keys=True) + "\n")
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"trace merge failed: {error}", file=sys.stderr)
        return 2
    print(f"trace shards merged: {len(merged['tests'])} tests")
    return 0


def _resolve(root: Path, path: Path) -> Path:
    return path if path.is_absolute() else root / path


def _load_json(path: Path) -> dict[str, Any]:
    document = json.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} root must be an object")
    return document


if __name__ == "__main__":
    raise SystemExit(main())
