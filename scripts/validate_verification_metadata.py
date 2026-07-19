#!/usr/bin/env python3
"""Compatibility entrypoint for the verification metadata validator."""

from verification.metadata_validator.core import main


if __name__ == "__main__":
    raise SystemExit(main())
