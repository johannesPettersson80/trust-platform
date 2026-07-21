#!/usr/bin/env python3
"""Compatibility entrypoint for the Phase 17 code-to-spec coverage gate."""

from verification.code_spec_cli import main


if __name__ == "__main__":
    raise SystemExit(main())
