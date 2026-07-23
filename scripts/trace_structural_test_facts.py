#!/usr/bin/env python3
"""Thin entrypoint for structural/ignored test trace generation."""

from verification.coverage_trace_structural_cli import main


if __name__ == "__main__":
    raise SystemExit(main())
