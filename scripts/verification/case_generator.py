"""Decision-table case generation for the verification pilot."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

from .bytecode_transforms import BytecodeTransformError, generate_bytecode_transform_case_file
from .case_digests import current_generator_digest, file_digest
from .metadata_validator.constants import CASE_FAMILIES, ROOT, VERIFICATION
from .metadata_validator.core import Validator


EXIT_OK = 0
EXIT_DIFF = 1
EXIT_USAGE = 5
EXIT_METADATA_INVALID = 6
GENERATOR_VERSION = "gen_cases.py v1"


class CaseGenerationError(RuntimeError):
    pass


class MetadataInvalidError(CaseGenerationError):
    pass


def load_metadata() -> Validator:
    validator = Validator()
    validator.load_records()
    validator.validate()
    if validator.failures:
        detail = "\n".join(
            f"{failure.path}: {failure.message}" for failure in validator.failures[:20]
        )
        if len(validator.failures) > 20:
            detail += f"\n... {len(validator.failures) - 20} more"
        raise MetadataInvalidError(f"verification metadata invalid:\n{detail}")
    return validator


def generate_case_file(invariant_id: str, validator: Validator) -> dict[str, Any]:
    invariant = validator.invariants.get(invariant_id)
    if invariant is None:
        raise CaseGenerationError(f"unknown invariant {invariant_id!r}")
    if invariant.get("area") != "bytecode_vm":
        raise CaseGenerationError("P1B case generation is scoped to bytecode_vm only")
    if invariant.get("contract_kind") != "decision_table":
        raise CaseGenerationError(f"{invariant_id} is not a decision_table invariant")
    if "transform_seed" in invariant:
        try:
            return generate_bytecode_transform_case_file(invariant, root=invariant.get("_root", ROOT))
        except BytecodeTransformError as exc:
            raise CaseGenerationError(str(exc)) from exc

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
        "source_digest": file_digest(invariant["_path"]),
        "last_reviewed": invariant["last_reviewed"],
        "case": cases,
    }


def cases_from_behavior(
    invariant: dict[str, Any],
    behavior: dict[str, Any],
    behavior_index: int,
) -> list[dict[str, Any]]:
    partition = behavior.get("partition")
    if not isinstance(partition, dict) or not partition:
        raise CaseGenerationError(f"{invariant['id']} behavior {behavior_index} has no partition")

    generated: list[dict[str, Any]] = []
    if "min" in partition and "max" in partition:
        generated.append(case_from_partition(invariant, behavior, behavior_index, "boundary_low", "MIN", partition["min"]))
        generated.append(case_from_partition(invariant, behavior, behavior_index, "boundary_high", "MAX", partition["max"]))
    elif "min" in partition:
        generated.append(case_from_partition(invariant, behavior, behavior_index, "boundary_low", "MIN", partition["min"]))
    elif "max" in partition:
        generated.append(case_from_partition(invariant, behavior, behavior_index, "boundary_high", "MAX", partition["max"]))
    elif "below" in partition:
        generated.append(case_from_partition(invariant, behavior, behavior_index, "below_min", "BELOW_MIN", just_below(partition["below"])))
    elif "above" in partition:
        generated.append(case_from_partition(invariant, behavior, behavior_index, "above_max", "ABOVE_MAX", just_above(partition["above"])))
    elif "wrong_type" in partition:
        generated.append(case_from_shape_descriptor(invariant, behavior, "WRONG_TYPE", partition["wrong_type"]))
    elif "malformed" in partition:
        generated.append(case_from_shape_descriptor(invariant, behavior, "MALFORMED", partition["malformed"]))
    elif "equals" in partition:
        generated.append(case_from_scenario(invariant, behavior, str(partition["equals"])))
    else:
        keys = ", ".join(sorted(partition))
        raise CaseGenerationError(f"{invariant['id']} behavior {behavior_index} has unsupported partition keys: {keys}")
    return generated


def case_from_partition(
    invariant: dict[str, Any],
    behavior: dict[str, Any],
    behavior_index: int,
    family: str,
    suffix: str,
    value: Any,
) -> dict[str, Any]:
    if family not in CASE_FAMILIES:
        raise CaseGenerationError(f"generator produced unknown case family {family!r}")
    case = {
        "id": case_id(invariant["id"], suffix, behavior["partition"]),
        "family": family,
        "input": {
            "source_partition": behavior["partition"],
            invariant.get("input", {}).get("name", "value"): materialize_input_value(invariant, value),
        },
    }
    attach_expected_or_blocked(case, invariant, behavior)
    return case


def case_from_shape_descriptor(
    invariant: dict[str, Any],
    behavior: dict[str, Any],
    suffix: str,
    descriptor: Any,
) -> dict[str, Any]:
    family = "wrong_type_or_shape"
    if family not in CASE_FAMILIES:
        raise CaseGenerationError(f"generator produced unknown case family {family!r}")
    case = {
        "id": case_id(invariant["id"], suffix, behavior["partition"]),
        "family": family,
        "input": {
            "source_partition": behavior["partition"],
            "shape_descriptor": descriptor,
        },
    }
    attach_expected_or_blocked(case, invariant, behavior)
    return case


def case_from_scenario(
    invariant: dict[str, Any],
    behavior: dict[str, Any],
    scenario: str,
) -> dict[str, Any]:
    family = behavior.get("case_family")
    if family not in CASE_FAMILIES:
        raise CaseGenerationError(f"{invariant['id']} equals partition requires canonical case_family")
    case = {
        "id": case_id(invariant["id"], scenario, behavior["partition"]),
        "family": family,
        "input": {
            "source_partition": behavior["partition"],
            "scenario": scenario,
        },
    }
    attach_expected_or_blocked(case, invariant, behavior)
    return case


def attach_expected_or_blocked(
    case: dict[str, Any],
    invariant: dict[str, Any],
    behavior: dict[str, Any],
) -> None:
    if "spec_gap_ref" in behavior:
        case["state"] = "blocked"
        case["spec_gap_ref"] = behavior["spec_gap_ref"]
    elif "oracle_ref" in behavior:
        case["expect"] = copy_expected_behavior(behavior)
    else:
        raise CaseGenerationError(f"{invariant['id']} behavior has no oracle_ref or spec_gap_ref")


def copy_expected_behavior(behavior: dict[str, Any]) -> dict[str, Any]:
    fields = [
        "outcome",
        "delta",
        "error_code",
        "no_partial_apply",
        "fault_surface",
        "oracle_ref",
    ]
    expected = {field: behavior[field] for field in fields if field in behavior}
    if "oracle_ref" not in expected:
        raise CaseGenerationError("oracle-backed behavior row is missing oracle_ref")
    return expected


def just_below(value: Any) -> Any:
    return value - 1 if isinstance(value, int) else f"below:{value}"


def just_above(value: Any) -> Any:
    return value + 1 if isinstance(value, int) else f"above:{value}"


def materialize_input_value(invariant: dict[str, Any], value: Any) -> Any:
    input_type = invariant.get("input", {}).get("type")
    if input_type == "STRING" and isinstance(value, int) and value >= 0:
        return "x" * value
    return value


def case_id(invariant_id: str, suffix: str, partition: dict[str, Any]) -> str:
    normalized = "".join(ch if ch.isalnum() else "_" for ch in suffix.upper()).strip("_")
    digest = hashlib.sha256(canonical_json(partition).encode()).hexdigest()[:8].upper()
    return f"{invariant_id}_{normalized}_{digest}"


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def render_toml(record: dict[str, Any]) -> str:
    lines: list[str] = []
    for key in [
        "schema_version",
        "id",
        "title",
        "area",
        "owner",
        "status",
        "invariant",
        "generator",
        "generator_digest",
        "source_digest",
        "last_reviewed",
    ]:
        lines.append(f"{key} = {toml_value(record[key])}")
    for case in record["case"]:
        lines.extend(["", "[[case]]"])
        for key in ["id", "family", "input", "state", "spec_gap_ref", "expect"]:
            if key in case:
                lines.append(f"{key} = {toml_value(case[key])}")
    return "\n".join(lines) + "\n"


def toml_value(value: Any) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, str):
        return json.dumps(value)
    if isinstance(value, list):
        return "[" + ", ".join(toml_value(item) for item in value) + "]"
    if isinstance(value, dict):
        items = ", ".join(f"{key} = {toml_value(item)}" for key, item in value.items())
        return "{ " + items + " }"
    raise CaseGenerationError(f"cannot render TOML value {value!r}")


def default_case_path(record: dict[str, Any]) -> Path:
    return VERIFICATION / "cases" / record["area"] / f"{record['invariant']}.toml"


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate or check verification case files.")
    parser.add_argument("--invariant", required=True, help="Invariant ID to derive cases from")
    parser.add_argument("--out", help="Write generated TOML to this path instead of stdout")
    parser.add_argument("--check", nargs="?", const="", help="Compare generated TOML to a case file")
    parser.add_argument("--format", choices=["toml", "json"], default="toml")
    return parser.parse_args(argv)


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
        print(f"gen_cases error: {exc}", file=sys.stderr)
        return EXIT_METADATA_INVALID
    except CaseGenerationError as exc:
        print(f"gen_cases error: {exc}", file=sys.stderr)
        return EXIT_DIFF
    return EXIT_OK


def check_case_file(path: Path, rendered: str) -> None:
    expected_path = path if path.is_absolute() else ROOT / path
    if not expected_path.exists():
        raise CaseGenerationError(f"case file does not exist: {path}")
    actual = expected_path.read_text()
    if actual != rendered:
        raise CaseGenerationError(f"case file drift: {path}")


def write_output(path: Path, rendered: str) -> None:
    target = path if path.is_absolute() else ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(rendered)
