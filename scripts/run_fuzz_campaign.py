#!/usr/bin/env python3
"""Run the complete bounded fuzz program and emit canonical crash-handoff input."""

from verification.fuzz_campaign_cli import main


if __name__ == "__main__":
    raise SystemExit(main())
