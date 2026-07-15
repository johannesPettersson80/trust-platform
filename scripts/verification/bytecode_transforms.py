"""Bytecode transform case generation for verification seeds."""

from __future__ import annotations

import hashlib
import json
import tomllib
from pathlib import Path
from typing import Any

from .case_digests import current_generator_digest, file_digest
from .metadata_validator.constants import CASE_FAMILIES, ROOT


class BytecodeTransformError(RuntimeError):
    pass


TRANSFORM_BEHAVIOR_PARTITIONS = {
    "container_truncate": "TRUNCATED_REQUIRED_CONTAINER_DATA",
    "unknown_opcode": "UNIMPLEMENTED_RESERVED_OR_UNKNOWN_OPCODE",
    "jump_target": "JUMP_OUTSIDE_POU_OR_INSTRUCTION_BOUNDARY",
    "stack_underflow": "INVALID_OPERAND_STACK_DATAFLOW",
}
EXPECTED_BEHAVIOR_FIELDS = (
    "outcome",
    "delta",
    "error_code",
    "no_partial_apply",
    "fault_surface",
    "oracle_ref",
)


def generate_bytecode_transform_case_file(
    invariant: dict[str, Any],
    *,
    root: Path = ROOT,
) -> dict[str, Any]:
    seed_ref = invariant.get("transform_seed")
    if not isinstance(seed_ref, dict):
        raise BytecodeTransformError(f"{invariant.get('id')} has no transform_seed table")
    seed_path_text = require_string(seed_ref, "path")
    runnable = specified_transform_oracle_ref(invariant, seed_ref)
    seed_gap_ref = transform_seed_gap_ref(invariant, seed_ref, runnable)

    seed_path = resolve_workspace_path(root, seed_path_text)
    seed = load_seed(seed_path)
    seed_bytes = parse_bytes_hex(require_string(seed, "bytes_hex"), context=f"{seed_path_text} bytes_hex")
    seed_digest = file_digest(seed_path)

    cases: list[dict[str, Any]] = []
    cases.extend(truncation_cases(invariant, seed, seed_path_text, seed_digest, seed_bytes, seed_gap_ref))
    cases.extend(unknown_opcode_cases(invariant, seed, seed_path_text, seed_digest, seed_bytes, seed_gap_ref))
    cases.extend(jump_target_cases(invariant, seed, seed_path_text, seed_digest, seed_bytes, seed_gap_ref))
    cases.extend(stack_underflow_cases(invariant, seed, seed_path_text, seed_digest, seed_bytes, seed_gap_ref))
    if not cases:
        raise BytecodeTransformError(f"{seed_path_text} generated no transform cases")

    if runnable is not None:
        for case in cases:
            case.pop("state", None)
            case.pop("spec_gap_ref", None)
            case["expect"] = rejection_expectation(invariant, case, runnable)

    return {
        "schema_version": 1,
        "id": f"CASES_{invariant['id']}",
        "title": f"Generated cases for {invariant['title']}",
        "area": invariant["area"],
        "owner": invariant["owner"],
        "status": "active" if runnable is not None else "planned",
        "invariant": invariant["id"],
        "generator": "gen_cases.py v1",
        "generator_digest": current_generator_digest(),
        "source_digest": file_digest(invariant["_path"]),
        "last_reviewed": invariant["last_reviewed"],
        "case": cases,
    }


def transform_seed_gap_ref(
    invariant: dict[str, Any],
    seed_ref: dict[str, Any],
    runnable_oracle_ref: str | None,
) -> str:
    if runnable_oracle_ref is not None:
        if "spec_gap_ref" in seed_ref:
            raise BytecodeTransformError(
                f"{invariant['id']} specified transform_seed forbids spec_gap_ref"
            )
        return runnable_oracle_ref
    if "oracle_ref" in seed_ref:
        raise BytecodeTransformError(
            f"{invariant['id']} unresolved transform_seed requires spec_gap_ref"
        )
    seed_gap_ref = require_string(seed_ref, "spec_gap_ref")
    if seed_gap_ref not in invariant.get("spec_gap_refs", []):
        raise BytecodeTransformError(
            f"{invariant['id']} transform_seed spec_gap_ref is not listed on the invariant"
        )
    return seed_gap_ref


def specified_transform_oracle_ref(
    invariant: dict[str, Any],
    seed_ref: dict[str, Any],
) -> str | None:
    spec = invariant.get("spec")
    if not isinstance(spec, dict) or spec.get("status") != "specified":
        return None
    oracle_ref = seed_ref.get("oracle_ref")
    if not isinstance(oracle_ref, str) or not oracle_ref:
        raise BytecodeTransformError(
            f"{invariant['id']} specified transform_seed requires a non-empty oracle_ref"
        )
    if oracle_ref.startswith("SPEC_GAP_"):
        raise BytecodeTransformError(
            f"{invariant['id']} specified transform cases cannot use a spec-gap oracle"
        )
    source_id = oracle_ref.split("#", 1)[0]
    if source_id not in spec.get("source_refs", []):
        raise BytecodeTransformError(
            f"{invariant['id']} transform_seed oracle_ref must name a listed spec source"
        )
    return oracle_ref


def rejection_expectation(
    invariant: dict[str, Any],
    case: dict[str, Any],
    oracle_ref: str,
) -> dict[str, Any]:
    transform = case["input"]["transform"]
    partition = TRANSFORM_BEHAVIOR_PARTITIONS.get(transform)
    if partition is None:
        raise BytecodeTransformError(f"unknown runnable bytecode transform {transform!r}")
    matches = [
        behavior
        for behavior in invariant.get("behavior", [])
        if isinstance(behavior, dict)
        and behavior.get("partition") == {"equals": partition}
        and behavior.get("oracle_ref") == oracle_ref
    ]
    if len(matches) != 1:
        raise BytecodeTransformError(
            f"{invariant['id']} transform {transform!r} has no matching oracle-backed behavior"
        )
    expectation = {
        field: matches[0][field]
        for field in EXPECTED_BEHAVIOR_FIELDS
        if field in matches[0]
    }
    if expectation.get("outcome") != "reject" or expectation.get("no_partial_apply") is not True:
        raise BytecodeTransformError(
            f"{invariant['id']} transform {transform!r} behavior must require transactional rejection"
        )
    return expectation


def load_seed(path: Path) -> dict[str, Any]:
    try:
        data = tomllib.loads(path.read_text())
    except Exception as exc:
        raise BytecodeTransformError(f"failed to read bytecode seed {path}: {exc}") from exc
    if not isinstance(data, dict):
        raise BytecodeTransformError(f"bytecode seed {path} is not a table")
    if data.get("schema_version") != 1:
        raise BytecodeTransformError(f"bytecode seed {path} must use schema_version = 1")
    if "id" not in data:
        raise BytecodeTransformError(f"bytecode seed {path} is missing id")
    return data


def resolve_workspace_path(root: Path, raw_path: str) -> Path:
    path = Path(raw_path)
    if path.is_absolute() or ".." in path.parts:
        raise BytecodeTransformError(f"seed path must be a relative workspace path: {raw_path}")
    root_resolved = root.resolve()
    resolved = (root_resolved / path).resolve()
    try:
        resolved.relative_to(root_resolved)
    except ValueError as exc:
        raise BytecodeTransformError(f"seed path must be a relative workspace path: {raw_path}") from exc
    if not resolved.exists():
        raise BytecodeTransformError(f"bytecode seed does not exist: {raw_path}")
    return resolved


def truncation_cases(
    invariant: dict[str, Any],
    seed: dict[str, Any],
    seed_path: str,
    seed_digest: str,
    seed_bytes: bytes,
    spec_gap_ref: str,
) -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = []
    for point in require_list(seed, "truncate_points"):
        offset = require_int(point, "offset")
        if offset < 0 or offset > len(seed_bytes):
            raise BytecodeTransformError(f"truncate point {point.get('id')} has offset outside seed bytes")
        mutated = seed_bytes[:offset]
        cases.append(
            transform_case(
                invariant=invariant,
                seed=seed,
                seed_path=seed_path,
                seed_digest=seed_digest,
                mutated=mutated,
                spec_gap_ref=spec_gap_ref,
                family=case_family(point, "missing_required"),
                transform="container_truncate",
                suffix=f"TRUNCATE_{require_string(point, 'id')}",
                details={
                    "truncate_point": point["id"],
                    "truncate_offset": offset,
                },
            )
        )
    return cases


def unknown_opcode_cases(
    invariant: dict[str, Any],
    seed: dict[str, Any],
    seed_path: str,
    seed_digest: str,
    seed_bytes: bytes,
    spec_gap_ref: str,
) -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = []
    for site in require_list(seed, "opcode_sites"):
        offset = require_int(site, "offset")
        if offset < 0 or offset >= len(seed_bytes):
            raise BytecodeTransformError(f"opcode site {site.get('id')} has offset outside seed bytes")
        opcodes = site.get("opcodes")
        if not isinstance(opcodes, list) or not opcodes:
            raise BytecodeTransformError(f"opcode site {site.get('id')} must list opcodes")
        for opcode in opcodes:
            if not isinstance(opcode, int) or opcode < 0 or opcode > 255:
                raise BytecodeTransformError(f"opcode site {site.get('id')} has invalid opcode {opcode!r}")
            mutated = bytearray(seed_bytes)
            mutated[offset] = opcode
            cases.append(
                transform_case(
                    invariant=invariant,
                    seed=seed,
                    seed_path=seed_path,
                    seed_digest=seed_digest,
                    mutated=bytes(mutated),
                    spec_gap_ref=spec_gap_ref,
                    family=case_family(site, "extra_or_unknown"),
                    transform="unknown_opcode",
                    suffix=f"UNKNOWN_OPCODE_{require_string(site, 'id')}_{opcode:02X}",
                    details={
                        "opcode_site": site["id"],
                        "opcode_offset": offset,
                        "opcode": opcode,
                    },
                )
            )
    return cases


def jump_target_cases(
    invariant: dict[str, Any],
    seed: dict[str, Any],
    seed_path: str,
    seed_digest: str,
    seed_bytes: bytes,
    spec_gap_ref: str,
) -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = []
    for site in require_list(seed, "jump_sites"):
        operand_offset = require_int(site, "operand_offset")
        if operand_offset < 0 or operand_offset + 4 > len(seed_bytes):
            raise BytecodeTransformError(f"jump site {site.get('id')} operand_offset outside seed bytes")
        deltas = site.get("deltas")
        if not isinstance(deltas, list) or not deltas:
            raise BytecodeTransformError(f"jump site {site.get('id')} must list deltas")
        for delta in deltas:
            if not isinstance(delta, int):
                raise BytecodeTransformError(f"jump site {site.get('id')} has non-integer delta")
            mutated = bytearray(seed_bytes)
            mutated[operand_offset : operand_offset + 4] = int(delta).to_bytes(
                4,
                byteorder="little",
                signed=True,
            )
            cases.append(
                transform_case(
                    invariant=invariant,
                    seed=seed,
                    seed_path=seed_path,
                    seed_digest=seed_digest,
                    mutated=bytes(mutated),
                    spec_gap_ref=spec_gap_ref,
                    family=case_family(site, "wrong_type_or_shape"),
                    transform="jump_target",
                    suffix=f"JUMP_TARGET_{require_string(site, 'id')}_{delta}",
                    details={
                        "jump_site": site["id"],
                        "operand_offset": operand_offset,
                        "delta": delta,
                    },
                )
            )
    return cases


def stack_underflow_cases(
    invariant: dict[str, Any],
    seed: dict[str, Any],
    seed_path: str,
    seed_digest: str,
    seed_bytes: bytes,
    spec_gap_ref: str,
) -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = []
    for site in require_list(seed, "stack_underflow_sites"):
        offset = require_int(site, "offset")
        length = require_int(site, "length")
        patch = parse_bytes_hex(require_string(site, "bytes_hex"), context=f"{site.get('id')} bytes_hex")
        if length != len(patch):
            raise BytecodeTransformError(f"stack site {site.get('id')} length does not match bytes_hex")
        if offset < 0 or length < 0 or offset + length > len(seed_bytes):
            raise BytecodeTransformError(f"stack site {site.get('id')} patch outside seed bytes")
        mutated = bytearray(seed_bytes)
        mutated[offset : offset + length] = patch
        cases.append(
            transform_case(
                invariant=invariant,
                seed=seed,
                seed_path=seed_path,
                seed_digest=seed_digest,
                mutated=bytes(mutated),
                spec_gap_ref=spec_gap_ref,
                family=case_family(site, "wrong_type_or_shape"),
                transform="stack_underflow",
                suffix=f"STACK_UNDERFLOW_{require_string(site, 'id')}",
                details={
                    "stack_site": site["id"],
                    "patch_offset": offset,
                    "patch_bytes_hex": patch.hex(),
                },
            )
        )
    return cases


def transform_case(
    *,
    invariant: dict[str, Any],
    seed: dict[str, Any],
    seed_path: str,
    seed_digest: str,
    mutated: bytes,
    spec_gap_ref: str,
    family: str,
    transform: str,
    suffix: str,
    details: dict[str, Any],
) -> dict[str, Any]:
    if family not in CASE_FAMILIES:
        raise BytecodeTransformError(f"transform produced unknown case family {family!r}")
    input_table = {
        "seed_artifact": seed_path,
        "seed_id": seed["id"],
        "seed_digest": seed_digest,
        "transform": transform,
        **details,
        "bytes_hex": mutated.hex(),
        "mutated_digest": bytes_digest(mutated),
    }
    return {
        "id": transform_case_id(invariant["id"], suffix, input_table),
        "family": family,
        "input": input_table,
        "state": "blocked",
        "spec_gap_ref": spec_gap_ref,
    }


def transform_case_id(invariant_id: str, suffix: str, details: dict[str, Any]) -> str:
    normalized = "".join(ch if ch.isalnum() else "_" for ch in suffix.upper()).strip("_")
    digest = hashlib.sha256(canonical_json(details).encode()).hexdigest()[:8].upper()
    return f"{invariant_id}_{normalized}_{digest}"


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def bytes_digest(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def parse_bytes_hex(value: str, *, context: str) -> bytes:
    compact = "".join(value.split())
    if len(compact) % 2:
        raise BytecodeTransformError(f"{context} has odd-length hex")
    try:
        return bytes.fromhex(compact)
    except ValueError as exc:
        raise BytecodeTransformError(f"{context} is not valid hex") from exc


def case_family(table: dict[str, Any], default: str) -> str:
    family = table.get("family", default)
    if not isinstance(family, str):
        raise BytecodeTransformError(f"{table.get('id')} family must be a string")
    return family


def require_list(table: dict[str, Any], key: str) -> list[Any]:
    value = table.get(key, [])
    if not isinstance(value, list):
        raise BytecodeTransformError(f"{key} must be a list")
    return value


def require_string(table: dict[str, Any], key: str) -> str:
    value = table.get(key)
    if not isinstance(value, str) or not value:
        raise BytecodeTransformError(f"{key} must be a non-empty string")
    return value


def require_int(table: dict[str, Any], key: str) -> int:
    value = table.get(key)
    if not isinstance(value, int):
        raise BytecodeTransformError(f"{table.get('id')} {key} must be an integer")
    return value
