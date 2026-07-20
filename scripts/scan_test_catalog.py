#!/usr/bin/env python3
"""Compatibility entrypoint for the existing-test catalog scanner."""

from verification.test_catalog_scanner import main


if __name__ == "__main__":
    raise SystemExit(main())
