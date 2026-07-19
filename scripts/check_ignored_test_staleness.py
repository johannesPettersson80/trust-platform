#!/usr/bin/env python3
"""Compatibility entrypoint for live ignored-test registry validation."""

from verification.ignored_test_staleness import main


if __name__ == "__main__":
    raise SystemExit(main())
