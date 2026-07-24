"""Input, rendering, and canonical-path support for authority selection."""

from __future__ import annotations

import hashlib
import json
import re
import tomllib
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .quarantine_paths import quarantine_path


INVARIANT_ROOT = "verification/invariants"
MAPPING_BLOCK_RE = re.compile(br"(?m)^\[\[mappings\]\][ \t]*\r?\n")
DISCOVERY_ID_RE = re.compile(r"^DISC_([0-9A-F]{20})$")
SHA256_PREFIX = "sha256:"


@dataclass(frozen=True)
class MappingBlock:
    code_id: str
    start: int
    end: int
    content: bytes
    row: dict[str, Any]


def load_toml(root: Path, path: Path, label: str) -> dict[str, Any]:
    canonical = canonical_file(root, path, label)
    value = tomllib.loads(canonical.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path}: TOML root must be a table")
    return value


def load_indexed_toml(
    root: Path, path: Path, key: str, label: str
) -> dict[str, Mapping[str, Any]]:
    document = load_toml(root, path, label)
    return unique_by_id(
        object_array(document.get(key), f"{label} rows"), "id", label
    )


def load_toml_captured(
    root: Path,
    path: Path,
    label: str,
    expected_originals: dict[str, bytes | None],
) -> dict[str, Any]:
    canonical = canonical_file(root, path, label)
    content = canonical.read_bytes()
    relative = canonical.relative_to(root).as_posix()
    expected_originals[relative] = content
    value = tomllib.loads(content.decode())
    if not isinstance(value, dict):
        raise ValueError(f"{path}: TOML root must be a table")
    return value


def load_indexed_toml_captured(
    root: Path,
    path: Path,
    key: str,
    label: str,
    expected_originals: dict[str, bytes | None],
) -> dict[str, Mapping[str, Any]]:
    document = load_toml_captured(
        root, path, label, expected_originals
    )
    return unique_by_id(
        object_array(document.get(key), f"{label} rows"), "id", label
    )


def load_invariants(
    root: Path,
    expected_originals: dict[str, bytes | None],
) -> dict[str, Mapping[str, Any]]:
    invariant_root = canonical_directory(
        root, root / INVARIANT_ROOT, "invariant root"
    )
    rows = []
    for candidate in sorted(invariant_root.rglob("*")):
        if candidate.is_symlink():
            raise ValueError(
                f"invariant ancestor or file must not be a symlink: {candidate}"
            )
    for path in sorted(invariant_root.rglob("*.toml")):
        if path.is_symlink():
            raise ValueError(f"invariant path must not be a symlink: {path}")
        rows.append(
            load_toml_captured(
                root, path, "invariant", expected_originals
            )
        )
    return unique_by_id(rows, "id", "invariants")


def capture_review_provenance_inputs(
    root: Path,
    provenance: Any,
    expected_originals: dict[str, bytes | None],
) -> None:
    if not isinstance(provenance, Mapping):
        raise ValueError("authority selection review provenance is malformed")
    paths = [
        provenance.get("proposal_path"),
        provenance.get("review_card_path"),
        provenance.get("human_selection_path"),
    ]
    candidate = provenance.get("candidate_provenance")
    if isinstance(candidate, Mapping):
        paths.extend(
            [
                candidate.get("candidate_card_path"),
                candidate.get("candidate_human_selection_path"),
                candidate.get("trace_path"),
            ]
        )
    for value in paths:
        if not isinstance(value, str):
            raise ValueError("authority provenance path is malformed")
        relative, resolved = quarantine_path(
            root,
            root / value,
            label="authority provenance input",
            must_exist=True,
        )
        expected_originals[relative] = resolved.read_bytes()


def capture_spec_document(
    root: Path,
    source: Mapping[str, Any],
    expected_originals: dict[str, bytes | None],
    *,
    code_id: str,
) -> None:
    path = source.get("path")
    if not isinstance(path, str):
        raise ValueError(f"{code_id} spec source path is malformed")
    canonical = canonical_file(
        root, root / path, f"{code_id} spec document"
    )
    expected_originals[canonical.relative_to(root).as_posix()] = (
        canonical.read_bytes()
    )


def mapping_blocks(content: bytes) -> list[MappingBlock]:
    matches = list(MAPPING_BLOCK_RE.finditer(content))
    if not matches:
        raise ValueError("code-spec map contains no mapping blocks")
    blocks = []
    seen: set[str] = set()
    for index, match in enumerate(matches):
        start = match.start()
        end = (
            matches[index + 1].start()
            if index + 1 < len(matches)
            else len(content)
        )
        block_content = content[start:end]
        row = parse_mapping_block(block_content)
        code_id = required_string(row, "code_id", f"mapping block {index}")
        if code_id in seen:
            raise ValueError(f"code-spec map duplicates code_id {code_id}")
        seen.add(code_id)
        blocks.append(
            MappingBlock(
                code_id=code_id,
                start=start,
                end=end,
                content=block_content,
                row=row,
            )
        )
    return blocks


def parse_mapping_block(content: bytes) -> dict[str, Any]:
    document = tomllib.loads(content.decode())
    if set(document) != {"mappings"}:
        raise ValueError("mapping block contains non-mapping top-level fields")
    rows = document.get("mappings")
    if (
        not isinstance(rows, list)
        or len(rows) != 1
        or not isinstance(rows[0], dict)
    ):
        raise ValueError("mapping block must contain exactly one table")
    return dict(rows[0])


def render_selected_block(
    content: bytes,
    *,
    spec_ref: str,
    invariant_ids: list[str],
    test_ids: list[str],
    reviewed_date: str,
) -> bytes:
    source = content.decode()
    replacements = (
        (
            re.compile(r'(?m)^disposition = "spec_gap"$'),
            'disposition = "specified"',
            "disposition",
        ),
        (
            re.compile(r'(?m)^spec_gap_ref = "[^"\r\n]+"$'),
            (
                f"spec_ref = {json.dumps(spec_ref)}\n"
                f"invariant_ids = {toml_string_array(invariant_ids)}\n"
                f"test_ids = {toml_string_array(test_ids)}"
            ),
            "spec_gap_ref",
        ),
        (
            re.compile(r'(?m)^last_reviewed = "\d{4}-\d{2}-\d{2}"$'),
            f'last_reviewed = "{reviewed_date}"',
            "last_reviewed",
        ),
    )
    updated = source
    for pattern, replacement, label in replacements:
        updated, count = pattern.subn(replacement, updated, count=1)
        if count != 1:
            raise ValueError(
                f"selected mapping requires one canonical {label} field"
            )
    return updated.encode()


def replace_mapping_blocks(
    content: bytes,
    blocks: Sequence[MappingBlock],
    replacements: Mapping[str, bytes],
) -> bytes:
    chunks = []
    cursor = 0
    for block in blocks:
        chunks.append(content[cursor:block.start])
        chunks.append(replacements.get(block.code_id, block.content))
        cursor = block.end
    chunks.append(content[cursor:])
    return b"".join(chunks)


def validate_only_semantic_transition(
    *,
    code_id: str,
    before: Mapping[str, Any],
    after: Mapping[str, Any],
    spec_ref: str,
    invariant_ids: list[str],
    test_ids: list[str],
    reviewed_date: str,
) -> None:
    expected = dict(before)
    expected.pop("spec_gap_ref", None)
    expected.update(
        {
            "disposition": "specified",
            "spec_ref": spec_ref,
            "invariant_ids": invariant_ids,
            "test_ids": test_ids,
            "last_reviewed": reviewed_date,
        }
    )
    if dict(after) != expected:
        raise ValueError(
            f"{code_id} transition changed fields outside reviewed authority"
        )


def unique_by_id(
    rows: Sequence[Mapping[str, Any]] | Any,
    field: str,
    label: str,
) -> dict[str, Mapping[str, Any]]:
    if not isinstance(rows, Sequence) or isinstance(rows, (str, bytes)):
        raise ValueError(f"{label} rows must be an array")
    indexed = {}
    for index, row in enumerate(rows):
        if not isinstance(row, Mapping):
            raise ValueError(f"{label} row {index} must be a table")
        identifier = required_string(row, field, f"{label} row {index}")
        if identifier in indexed:
            raise ValueError(f"{label} duplicates {field} {identifier}")
        indexed[identifier] = row
    return indexed


def object_array(value: Any, label: str) -> list[Mapping[str, Any]]:
    if (
        not isinstance(value, Sequence)
        or isinstance(value, (str, bytes))
        or any(not isinstance(row, Mapping) for row in value)
    ):
        raise ValueError(f"{label} must be an array of objects")
    return list(value)


def closed_string_array(
    value: Any,
    label: str,
    *,
    pattern: re.Pattern[str] | None,
) -> list[str]:
    if (
        not isinstance(value, list)
        or not value
        or any(not isinstance(item, str) or not item for item in value)
    ):
        raise ValueError(f"{label} must be a non-empty string array")
    if len(set(value)) != len(value):
        singular = "discovery_id" if "discovery" in label else "value"
        raise ValueError(f"{label} duplicates {singular}")
    if pattern is not None and any(
        pattern.fullmatch(item) is None for item in value
    ):
        raise ValueError(f"{label} contains malformed identity")
    return sorted(value)


def required_string(
    value: Mapping[str, Any], field: str, label: str
) -> str:
    result = value.get(field)
    if not isinstance(result, str) or not result:
        raise ValueError(f"{label} requires non-empty {field}")
    return result


def test_id_for_discovery(discovery_id: str) -> str:
    match = DISCOVERY_ID_RE.fullmatch(discovery_id)
    if match is None:
        raise ValueError(f"malformed discovery_id {discovery_id}")
    return "TEST_AUTO_" + match.group(1)


def toml_string_array(values: Sequence[str]) -> str:
    return "[" + ", ".join(json.dumps(value) for value in values) + "]"


def digest(content: bytes) -> str:
    return SHA256_PREFIX + hashlib.sha256(content).hexdigest()


def rooted_path(root: Path, value: str) -> Path:
    path = Path(value)
    return path if path.is_absolute() else root / path


def canonical_file(root: Path, path: Path, label: str) -> Path:
    return canonical_path(root, path, label, require_directory=False)


def canonical_directory(root: Path, path: Path, label: str) -> Path:
    return canonical_path(root, path, label, require_directory=True)


def canonical_path(
    root: Path,
    path: Path,
    label: str,
    *,
    require_directory: bool,
) -> Path:
    root = root.resolve()
    candidate = path if path.is_absolute() else root / path
    try:
        relative = candidate.relative_to(root)
    except ValueError as error:
        raise ValueError(f"{label} must resolve under repository root") from error
    if any(part in {"", ".", ".."} for part in relative.parts):
        raise ValueError(f"{label} path is not canonical")
    current = root
    for part in relative.parts:
        current /= part
        if current.is_symlink():
            raise ValueError(f"{label} path contains symlink: {current}")
    try:
        resolved = candidate.resolve(strict=True)
    except OSError as error:
        raise ValueError(f"{label} path is absent") from error
    try:
        resolved.relative_to(root)
    except ValueError as error:
        raise ValueError(f"{label} must resolve under repository root") from error
    valid_kind = resolved.is_dir() if require_directory else resolved.is_file()
    if not valid_kind:
        kind = "directory" if require_directory else "regular file"
        raise ValueError(f"{label} must be a {kind}")
    return resolved
