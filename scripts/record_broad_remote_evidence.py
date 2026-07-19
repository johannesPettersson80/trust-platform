#!/usr/bin/env python3
"""Execute the reviewed broad trust-builder command and index successful evidence."""

from __future__ import annotations

import argparse
import sys

from verification.broad_remote_gate import BroadRemoteGateError, BroadRemoteGateProducer


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Run the fixed PR broad command on trust-builder and append authentic evidence."
    )
    parser.add_argument(
        "--invariant",
        action="append",
        required=True,
        dest="invariants",
        help="Invariant to bind; repeat to bind multiple invariants in one area.",
    )
    args = parser.parse_args(argv)
    try:
        result = BroadRemoteGateProducer().run(args.invariants)
    except BroadRemoteGateError as exc:
        print(f"broad remote evidence failed: {exc}", file=sys.stderr)
        return 2
    print(f"wrote {result.record['id']} to {result.evidence_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
