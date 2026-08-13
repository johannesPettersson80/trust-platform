"""Refresh generated fields for one reviewed focused-mutation shard."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import tempfile
import tomllib
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable

from .metadata_validator.constants import ROOT
from .metadata_validator.mutation_contracts import MutationContractError
from .focused_mutation_runner import (
    _write_atomic as _runner_write_atomic,
    artifact_output_path,
    canonical_json,
    execute_source_shard,
    select_source_shard,
)
from .mutation_execution import discover_mutants, select_generated_mutant
from .mutation_program_contract import (
    MUTATION_PROGRAM_PATH,
    REQUIRED_SHARD_IDS,
    load_mutation_program,
    validate_mutation_program_for_shard_execution,
)


# The legacy bytecode-validator shard remains non-executable through the focused runner,
# but its source-derived digest and selector fields still need the same safe refresh path.
SOURCE_SHARD_IDS = frozenset(REQUIRED_SHARD_IDS[:5])
DERIVED_MUTATION_FIELDS = frozenset({"source_digest", "selector_name"})
_SHARD_HEADER_RE = re.compile(r"(?m)^\[\[shards\]\][ \t]*(?:#.*)?$")
_MUTATION_HEADER_RE = re.compile(r"(?m)^\[\[shards\.mutations\]\][ \t]*(?:#.*)?$")


@dataclass(frozen=True)
class MutationProgramRefresh:
    shard_id: str
    changed: bool
    mutation_ids: tuple[str, ...]


def refresh_mutation_shard(
    root: Path,
    shard_id: str,
    *,
    discover: Callable[[Path, Path, dict[str, str]], list[dict[str, Any]]] | None = None,
    write: bool = True,
) -> MutationProgramRefresh:
    """Derive and atomically persist one shard's live digest and selector fields."""

    root = root.resolve()
    if shard_id not in SOURCE_SHARD_IDS:
        raise MutationContractError(
            f"mutation shard {shard_id} is outside the reviewed source-refresh set"
        )
    manifest_path = root / MUTATION_PROGRAM_PATH
    original = manifest_path.read_text()
    try:
        program = tomllib.loads(original)
    except tomllib.TOMLDecodeError as exc:
        raise MutationContractError(f"mutation program cannot be parsed: {exc}") from exc
    shard = _select_shard(program, shard_id)
    mutations = _mutation_rows(shard)
    if not mutations:
        raise MutationContractError(f"mutation shard {shard_id} has no mutations")

    discover_fn = discover or discover_mutants
    environment = os.environ.copy()
    candidates_by_source: dict[str, list[dict[str, Any]]] = {}
    generated: dict[str, dict[str, str]] = {}
    for mutation in mutations:
        mutation_id = mutation.get("id")
        source_file = mutation.get("source_file")
        if not isinstance(mutation_id, str) or not mutation_id:
            raise MutationContractError(f"mutation shard {shard_id} has an invalid mutation id")
        if not isinstance(source_file, str) or not source_file:
            raise MutationContractError(f"mutation {mutation_id} has an invalid source_file")
        source_path = _safe_source_path(root, source_file)
        candidates = candidates_by_source.get(source_file)
        if candidates is None:
            candidates = discover_fn(source_path, root, environment)
            candidates_by_source[source_file] = candidates
        semantic_selector = {
            key: value
            for key, value in mutation.items()
            if key not in DERIVED_MUTATION_FIELDS
        }
        selected = select_generated_mutant(candidates, semantic_selector)
        selector_name = selected.get("name")
        if not isinstance(selector_name, str) or not selector_name:
            raise MutationContractError(
                f"mutation {mutation_id} generated selector name is missing"
            )
        generated[mutation_id] = {
            "source_digest": "sha256:" + hashlib.sha256(source_path.read_bytes()).hexdigest(),
            "selector_name": selector_name,
        }

    refreshed = _render_refreshed_manifest(original, shard_id, generated)
    _validate_only_derived_fields_changed(original, refreshed, shard_id, generated)
    changed = refreshed != original
    if changed and write:
        _write_atomic(manifest_path, refreshed)
    return MutationProgramRefresh(
        shard_id=shard_id,
        changed=changed,
        mutation_ids=tuple(generated),
    )


def _select_shard(program: Mapping[str, Any], shard_id: str) -> dict[str, Any]:
    shards = program.get("shards")
    if not isinstance(shards, list):
        raise MutationContractError("mutation program shards must be an array")
    matches = [
        dict(row)
        for row in shards
        if isinstance(row, Mapping) and row.get("id") == shard_id
    ]
    if len(matches) != 1:
        raise MutationContractError(
            f"mutation shard {shard_id} must occur exactly once, found {len(matches)}"
        )
    return matches[0]


def _mutation_rows(shard: Mapping[str, Any]) -> list[dict[str, Any]]:
    rows = shard.get("mutations")
    if not isinstance(rows, list) or not all(isinstance(row, Mapping) for row in rows):
        raise MutationContractError("focused mutation shard mutations must be an object array")
    return [dict(row) for row in rows]


def _safe_source_path(root: Path, source_file: str) -> Path:
    relative = Path(source_file)
    if (
        relative.is_absolute()
        or "\\" in source_file
        or ".." in relative.parts
        or relative.as_posix() != source_file
    ):
        raise MutationContractError(f"mutation source path is unsafe: {source_file}")
    candidate = root
    for part in relative.parts:
        candidate /= part
        if candidate.is_symlink():
            raise MutationContractError(f"mutation source path contains a symlink: {source_file}")
    try:
        candidate.resolve().relative_to(root)
    except ValueError as exc:
        raise MutationContractError(f"mutation source escapes the workspace: {source_file}") from exc
    if not candidate.is_file():
        raise MutationContractError(f"mutation source does not exist: {source_file}")
    return candidate


def _render_refreshed_manifest(
    text: str,
    shard_id: str,
    generated: Mapping[str, Mapping[str, str]],
) -> str:
    shard_matches = list(_SHARD_HEADER_RE.finditer(text))
    selected: list[tuple[int, int]] = []
    for index, header in enumerate(shard_matches):
        end = shard_matches[index + 1].start() if index + 1 < len(shard_matches) else len(text)
        body = text[header.start():end]
        first_mutation = _MUTATION_HEADER_RE.search(body)
        shard_prefix = body[: first_mutation.start() if first_mutation else len(body)]
        if _table_id(shard_prefix, "mutation shard") == shard_id:
            selected.append((header.start(), end))
    if len(selected) != 1:
        raise MutationContractError(
            f"mutation shard {shard_id} TOML block must occur exactly once, found {len(selected)}"
        )
    shard_start, shard_end = selected[0]
    shard_text = text[shard_start:shard_end]
    mutation_matches = list(_MUTATION_HEADER_RE.finditer(shard_text))
    blocks: dict[str, tuple[int, int]] = {}
    for index, header in enumerate(mutation_matches):
        end = (
            mutation_matches[index + 1].start()
            if index + 1 < len(mutation_matches)
            else len(shard_text)
        )
        mutation_id = _table_id(shard_text[header.start():end], "mutation")
        if mutation_id in blocks:
            raise MutationContractError(f"mutation TOML block is duplicated: {mutation_id}")
        blocks[mutation_id] = (header.start(), end)
    if set(blocks) != set(generated):
        raise MutationContractError(
            f"mutation shard {shard_id} parsed/TOML mutation ids disagree"
        )

    rendered = shard_text
    for mutation_id, (start, end) in sorted(
        blocks.items(), key=lambda item: item[1][0], reverse=True
    ):
        block = rendered[start:end]
        for field in sorted(DERIVED_MUTATION_FIELDS):
            block = _replace_string_field(block, field, generated[mutation_id][field])
        rendered = rendered[:start] + block + rendered[end:]
    return text[:shard_start] + rendered + text[shard_end:]


def _table_id(text: str, label: str) -> str:
    matches = re.findall(
        r'(?m)^[ \t]*id[ \t]*=[ \t]*("(?:\\.|[^"\\])*")[ \t]*(?:#.*)?$',
        text,
    )
    if not matches:
        raise MutationContractError(f"{label} TOML block lacks an id")
    try:
        value = json.loads(matches[0])
    except json.JSONDecodeError as exc:
        raise MutationContractError(f"{label} TOML id is invalid") from exc
    if not isinstance(value, str) or not value:
        raise MutationContractError(f"{label} TOML id must be non-empty text")
    return value


def _replace_string_field(block: str, field: str, value: str) -> str:
    pattern = re.compile(
        rf'(?m)^([ \t]*{re.escape(field)}[ \t]*=[ \t]*)'
        r'("(?:\\.|[^"\\])*")([ \t]*(?:#.*)?)$'
    )
    matches = list(pattern.finditer(block))
    if len(matches) != 1:
        raise MutationContractError(
            f"mutation TOML block must contain exactly one {field}, found {len(matches)}"
        )
    replacement = json.dumps(value, ensure_ascii=False)
    return pattern.sub(lambda match: match.group(1) + replacement + match.group(3), block)


def _validate_only_derived_fields_changed(
    before_text: str,
    after_text: str,
    shard_id: str,
    generated: Mapping[str, Mapping[str, str]],
) -> None:
    try:
        before = tomllib.loads(before_text)
        after = tomllib.loads(after_text)
    except tomllib.TOMLDecodeError as exc:
        raise MutationContractError(f"refreshed mutation program is invalid TOML: {exc}") from exc
    before_static = _without_derived_fields(before)
    after_static = _without_derived_fields(after)
    if before_static != after_static:
        raise MutationContractError("mutation refresh changed reviewed semantic fields")
    refreshed = _select_shard(after, shard_id)
    rows = {row["id"]: row for row in _mutation_rows(refreshed)}
    if set(rows) != set(generated):
        raise MutationContractError("refreshed mutation ids drifted")
    for mutation_id, fields in generated.items():
        if any(rows[mutation_id].get(field) != value for field, value in fields.items()):
            raise MutationContractError(
                f"mutation {mutation_id} generated fields were not written exactly"
            )


def _without_derived_fields(value: Mapping[str, Any]) -> dict[str, Any]:
    copied = dict(value)
    shards: list[Any] = []
    for shard in copied.get("shards", []):
        if not isinstance(shard, Mapping):
            shards.append(shard)
            continue
        shard_copy = dict(shard)
        mutations = []
        for mutation in shard_copy.get("mutations", []):
            if not isinstance(mutation, Mapping):
                mutations.append(mutation)
                continue
            mutations.append(
                {
                    key: item
                    for key, item in mutation.items()
                    if key not in DERIVED_MUTATION_FIELDS
                }
            )
        shard_copy["mutations"] = mutations
        shards.append(shard_copy)
    copied["shards"] = shards
    return copied


def _write_atomic(path: Path, text: str) -> None:
    mode = stat.S_IMODE(path.stat().st_mode)
    temporary: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            dir=path.parent,
            prefix=path.name + ".",
            suffix=".tmp",
            delete=False,
        ) as handle:
            temporary = Path(handle.name)
            handle.write(text)
            handle.flush()
            os.fsync(handle.fileno())
        temporary.chmod(mode)
        os.replace(temporary, path)
        if os.name != "nt":
            directory_fd = os.open(
                path.parent,
                os.O_RDONLY | getattr(os, "O_DIRECTORY", 0),
            )
            try:
                os.fsync(directory_fd)
            finally:
                os.close(directory_fd)
    finally:
        if temporary is not None and temporary.exists():
            temporary.unlink()


def _write_execution_artifact(path: Path, payload: Mapping[str, Any]) -> None:
    """Write an execution artifact after the focused runner returns it."""

    _runner_write_atomic(path, canonical_json(payload))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--shard-id", required=True)
    parser.add_argument(
        "--check",
        action="store_true",
        help="report stale generated fields without modifying the manifest",
    )
    parser.add_argument("--execute-artifact", action="store_true")
    parser.add_argument("--json-out", type=Path)
    parser.add_argument("--target-dir", type=Path)
    parser.add_argument("--build-timeout", type=float, default=1800.0)
    parser.add_argument("--test-timeout", type=float, default=900.0)
    args = parser.parse_args(argv)
    try:
        if args.execute_artifact:
            if args.check or args.json_out is None or args.target_dir is None:
                raise MutationContractError(
                    "--execute-artifact requires --json-out and --target-dir and forbids --check"
                )
            root = args.root.resolve()
            program = load_mutation_program(root)
            failures = validate_mutation_program_for_shard_execution(
                root,
                program,
                args.shard_id,
            )
            if failures:
                raise MutationContractError("; ".join(failures))
            shard = select_source_shard(program, args.shard_id)
            output = artifact_output_path(root, shard, args.json_out)
            payload = execute_source_shard(
                root=root,
                shard=shard,
                target_dir=args.target_dir,
                build_timeout=args.build_timeout,
                test_timeout=args.test_timeout,
            )
            _write_execution_artifact(output, payload)
            summary = payload["summary"]
            print(
                f"focused mutation shard {args.shard_id}: "
                f"{summary['caught']} caught, {summary['survived']} survived, "
                f"{summary['unviable']} unviable, {summary['timeout']} timeout"
            )
            return 0
        result = refresh_mutation_shard(
            args.root,
            args.shard_id,
            write=not args.check,
        )
    except (MutationContractError, OSError) as exc:
        print(f"focused mutation refresh failed: {exc}", file=os.sys.stderr)
        return 2
    if args.check and result.changed:
        print(f"focused mutation shard {result.shard_id} generated fields are stale")
        return 1
    disposition = "refreshed" if result.changed else "already current"
    print(
        f"focused mutation shard {result.shard_id} {disposition}: "
        f"{len(result.mutation_ids)} mutation(s)"
    )
    return 0
