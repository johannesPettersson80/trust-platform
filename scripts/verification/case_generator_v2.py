"""Versioned case generation for non-bytecode decision-table invariants."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

from .case_digests_v2 import current_generator_digest
from .case_generator import (
    EXIT_DIFF,
    EXIT_METADATA_INVALID,
    EXIT_OK,
    EXIT_USAGE,
    CaseGenerationError,
    MetadataInvalidError,
    cases_from_behavior,
    check_case_file,
    default_case_path,
    load_metadata,
    parse_args,
    render_toml,
    write_output,
)
from .execution_contract import invariant_execution_contract_digest
from .metadata_validator.core import Validator


GENERATOR_VERSION = "gen_cases_v2.py v1"


def generate_case_file(invariant_id: str, validator: Validator) -> dict[str, Any]:
    invariant = validator.invariants.get(invariant_id)
    if invariant is None:
        raise CaseGenerationError(f"unknown invariant {invariant_id!r}")
    if invariant.get("area") == "bytecode_vm":
        raise CaseGenerationError("bytecode_vm decision tables remain on gen_cases.py v1")
    if invariant.get("contract_kind") != "decision_table":
        raise CaseGenerationError(f"{invariant_id} is not a decision_table invariant")
    if "transform_seed" in invariant:
        raise CaseGenerationError("non-bytecode decision tables forbid transform_seed")

    behavior_rows = invariant.get("behavior", [])
    if not behavior_rows:
        raise CaseGenerationError(f"{invariant_id} has no behavior rows")
    cases: list[dict[str, Any]] = []
    for index, behavior in enumerate(behavior_rows, start=1):
        cases.extend(cases_from_behavior(invariant, behavior, index))
    if not cases:
        raise CaseGenerationError(f"{invariant_id} generated no cases")

    return {
        "schema_version": 1,
        "id": f"CASES_{invariant_id}",
        "title": f"Generated cases for {invariant['title']}",
        "area": invariant["area"],
        "owner": invariant["owner"],
        "status": "planned",
        "invariant": invariant_id,
        "generator": GENERATOR_VERSION,
        "generator_digest": current_generator_digest(),
        "source_digest": invariant_execution_contract_digest(invariant),
        "last_reviewed": invariant["last_reviewed"],
        "case": cases,
    }


def main(argv: list[str] | None = None) -> int:
    try:
        args = parse_args(argv or sys.argv[1:])
    except SystemExit as exc:
        return int(exc.code) if exc.code == 0 else EXIT_USAGE
    try:
        validator = load_metadata()
        record = generate_case_file(args.invariant, validator)
        rendered = render_toml(record)
        if args.format == "json":
            rendered = json.dumps(record, indent=2, sort_keys=True) + "\n"
        if args.check is not None:
            if args.format != "toml":
                raise CaseGenerationError("--check only supports TOML case-file output")
            check_path = Path(args.check) if args.check else default_case_path(record)
            check_case_file(check_path, rendered)
        elif args.out:
            write_output(Path(args.out), rendered)
        else:
            print(rendered, end="")
    except MetadataInvalidError as exc:
        print(f"gen_cases_v2 error: {exc}", file=sys.stderr)
        return EXIT_METADATA_INVALID
    except CaseGenerationError as exc:
        print(f"gen_cases_v2 error: {exc}", file=sys.stderr)
        return EXIT_DIFF
    return EXIT_OK
