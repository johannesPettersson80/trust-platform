"""Canonical parsing and rendering support for code-spec refresh projection."""

from __future__ import annotations

import hashlib
import json
import re
import tomllib
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Any


CODE_ID_RE = re.compile(r"^CODE_[0-9A-F]{64}$")
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")


@dataclass(frozen=True)
class TableBlock:
    identifier: str
    start: int
    end: int
    content: bytes
    row: dict[str, Any]


def table_blocks(
    content: bytes,
    table: str,
    id_field: str,
    label: str,
) -> list[TableBlock]:
    pattern = re.compile(
        rb"(?m)^\[\[" + re.escape(table.encode()) + rb"\]\][ \t]*\r?\n"
    )
    matches = list(pattern.finditer(content))
    if not matches:
        raise ValueError(f"{label} contains no {table} tables")
    blocks = []
    seen = set()
    for index, match in enumerate(matches):
        end = (
            matches[index + 1].start()
            if index + 1 < len(matches)
            else len(content)
        )
        block_content = content[match.start() : end]
        document = tomllib.loads(block_content.decode())
        rows = document.get(table)
        if not isinstance(rows, list) or len(rows) != 1:
            raise ValueError(f"{label} table {index} is malformed")
        row = dict(rows[0])
        identifier = required_string(
            row, id_field, f"{label} table {index}"
        )
        if identifier in seen:
            raise ValueError(f"{label} duplicates {identifier}")
        seen.add(identifier)
        blocks.append(
            TableBlock(
                identifier=identifier,
                start=match.start(),
                end=end,
                content=block_content,
                row=row,
            )
        )
    return blocks


def replace_blocks(
    content: bytes,
    blocks: Sequence[TableBlock],
    replacements: Mapping[str, bytes | None],
    *,
    append_order: Sequence[str],
) -> bytes:
    chunks = []
    cursor = 0
    for block in blocks:
        chunks.append(content[cursor : block.start])
        replacement = replacements.get(block.identifier, block.content)
        if replacement is not None:
            chunks.append(replacement)
        cursor = block.end
    chunks.append(content[cursor:])
    result = b"".join(chunks)
    for identifier in append_order:
        replacement = replacements.get(identifier)
        if replacement is None:
            continue
        if result and not result.endswith(b"\n"):
            result += b"\n"
        if result and not result.endswith(b"\n\n"):
            result += b"\n"
        result += replacement
    return result


def render_table(
    table: str, row: Mapping[str, Any], field_order: Sequence[str]
) -> bytes:
    unknown = set(row) - set(field_order)
    if unknown:
        raise ValueError(
            f"{table} row contains unsupported fields: {sorted(unknown)}"
        )
    lines = [f"[[{table}]]"]
    for field in field_order:
        if field in row:
            lines.append(f"{field} = {toml_value(row[field])}")
    return ("\n".join(lines) + "\n\n").encode()


def toml_value(value: Any) -> str:
    if isinstance(value, str):
        return json.dumps(value, ensure_ascii=False)
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, list):
        if len(value) <= 3:
            return "[" + ", ".join(toml_value(item) for item in value) + "]"
        return "[\n" + "".join(
            f"  {toml_value(item)},\n" for item in value
        ) + "]"
    raise ValueError(f"unsupported TOML value {type(value).__name__}")


def validate_projected_documents(
    code_map: bytes, acknowledgements: bytes, gaps: bytes
) -> None:
    documents = (
        (code_map, "mappings", "code_id", "projected code map"),
        (
            acknowledgements,
            "acknowledgements",
            "id",
            "projected risk ledger",
        ),
        (gaps, "spec_gaps", "id", "projected spec-gap ledger"),
    )
    for content, key, id_field, label in documents:
        document = tomllib.loads(content.decode())
        rows = document.get(key)
        if not isinstance(rows, list):
            raise ValueError(f"{label} root is malformed")
        unique_by(rows, id_field, label)


def unique_by(
    rows: Sequence[Mapping[str, Any]],
    field: str,
    label: str,
) -> dict[str, Mapping[str, Any]]:
    result = {}
    for index, row in enumerate(rows):
        value = required_string(row, field, f"{label} row {index}")
        if value in result:
            raise ValueError(f"{label} duplicates {value}")
        result[value] = row
    return result


def load_json_object(content: bytes, label: str) -> dict[str, Any]:
    try:
        value = json.loads(content)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError(f"{label} is invalid JSON") from error
    if not isinstance(value, dict):
        raise ValueError(f"{label} root must be an object")
    return value


def required_string(
    row: Mapping[str, Any], field: str, label: str
) -> str:
    value = row.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires nonempty {field}")
    return value


def required_code_id(
    row: Mapping[str, Any], field: str, label: str
) -> str:
    value = required_string(row, field, label)
    if CODE_ID_RE.fullmatch(value) is None:
        raise ValueError(f"{label} has malformed {field}")
    return value


def required_digest(
    row: Mapping[str, Any], field: str, label: str
) -> str:
    value = required_string(row, field, label)
    if DIGEST_RE.fullmatch(value) is None:
        raise ValueError(f"{label} has malformed {field}")
    return value


def string_array(
    value: Any, label: str, *, allow_empty: bool = False
) -> list[str]:
    if (
        not isinstance(value, list)
        or (not allow_empty and not value)
        or any(not isinstance(item, str) or not item for item in value)
        or len(value) != len(set(value))
    ):
        raise ValueError(f"{label} must be a unique string array")
    return list(value)


def code_id_array(
    value: Any, label: str, *, allow_empty: bool = False
) -> list[str]:
    rows = string_array(value, label, allow_empty=allow_empty)
    if any(CODE_ID_RE.fullmatch(item) is None for item in rows):
        raise ValueError(f"{label} contains malformed code identity")
    return rows


def digest(content: bytes) -> str:
    return "sha256:" + hashlib.sha256(content).hexdigest()


def object_digest(value: Mapping[str, Any]) -> str:
    content = json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode()
    return digest(content)
