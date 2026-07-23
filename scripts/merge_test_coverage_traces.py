#!/usr/bin/env python3
"""Thin entrypoint for deterministic test coverage trace merging."""

from verification.coverage_trace_merge_cli import main


if __name__ == "__main__":
    raise SystemExit(main())
