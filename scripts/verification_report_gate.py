#!/usr/bin/env python3
"""Compatibility entrypoint for the report-only verification gate."""

from verification.report_gate import main


if __name__ == "__main__":
    raise SystemExit(main())
