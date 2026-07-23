#!/usr/bin/env python3
"""Thin entrypoint for batched VS Code per-test coverage tracing."""

from verification.coverage_trace_vscode_cli import main


if __name__ == "__main__":
    raise SystemExit(main())
