#!/usr/bin/env python3
"""Validate a generated Phase 10 mutation-program report at rest."""

from scripts.verification.mutation_program_cli import validate_main


if __name__ == "__main__":
    raise SystemExit(validate_main())
