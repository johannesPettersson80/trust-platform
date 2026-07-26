"""Deterministic specification-source and document-review registration."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .spec_source_models import stable_document_id


SPEC_REGISTRY = Path("verification/spec-sources.toml")
REVIEW_REGISTRY = Path("verification/spec-document-reviews.toml")
HEADING_RE = re.compile(r"^(#{1,2})\s+(.+?)\s*$")
PREFIX_RE = re.compile(r"^[0-9]+[-_ ]*")
NON_ALNUM_RE = re.compile(r"[^A-Za-z0-9]+")


@dataclass(frozen=True)
class RegistrationResult:
    changed: bool
    record: dict[str, Any]
    rendered: str
    document_reviews_rendered: str


def build_spec_source_record(
    *,
    root: Path,
    path: Path,
    area: str,
    owner: str,
    reviewed_date: str,
) -> dict[str, Any]:
    relative = _relative_path(root, path)
    source = (root / relative).read_text()
    title, covers = _title_and_covers(source)
    stem = PREFIX_RE.sub("", relative.stem)
    stable_name = NON_ALNUM_RE.sub("_", stem).strip("_").upper()
    if not stable_name:
        raise ValueError("specification path cannot produce a stable ID")
    return {
        "schema_version": 2,
        "id": f"SPEC_{stable_name}_001",
        "title": title,
        "area": _nonempty(area, "area"),
        "owner": _nonempty(owner, "owner"),
        "status": "mapped",
        "authority": "normative_product",
        "source_status": "active",
        "oracle_eligible": True,
        "visibility": "public" if relative.parts[:2] == ("docs", "specs") else "internal",
        "locator_kind": "tracked_file",
        "path": relative.as_posix(),
        "version": "current",
        "last_reviewed": _review_date(reviewed_date),
        "covers": covers,
        "known_limitations": [],
        "conflicts_with": [],
    }


def register_spec_source(
    *,
    root: Path,
    registry_path: Path,
    review_registry_path: Path,
    path: Path,
    area: str,
    owner: str,
    reviewed_date: str,
) -> RegistrationResult:
    root = root.resolve()
    record = build_spec_source_record(
        root=root,
        path=path,
        area=area,
        owner=owner,
        reviewed_date=reviewed_date,
    )
    source_registry = registry_path.read_text()
    source_records = _records(source_registry, "spec_sources", registry_path)
    source_match = _existing_record(
        source_records,
        id_field="id",
        record_id=record["id"],
        path=record["path"],
        label="specification source",
    )
    if source_match is not None and source_match != record:
        raise ValueError(
            f"specification source {record['id']} already exists with different metadata"
        )
    rendered_sources = (
        source_registry
        if source_match is not None
        else _append_record(source_registry, "spec_sources", record)
    )

    relative = Path(record["path"])
    content_digest = hashlib.sha256((root / relative).read_bytes()).hexdigest()
    review = {
        "schema_version": 1,
        "document_id": stable_document_id(relative.as_posix()),
        "path": relative.as_posix(),
        "content_sha256": content_digest,
        "areas": [record["area"]],
        "authority_levels": [record["authority"]],
        "owners": [record["owner"]],
        "freshness": "current",
        "visibility": record["visibility"],
        "oracle_usable": True,
        "classification_basis": "registered_metadata",
        "conflict_disposition": "registered_conflicts_reviewed",
        "checklist_staleness": "not_applicable",
        "removed_behavior_disposition": "reviewed_current",
        "last_reviewed": record["last_reviewed"],
    }
    review_registry = review_registry_path.read_text()
    review_records = _records(
        review_registry, "document_reviews", review_registry_path
    )
    review_match = _existing_record(
        review_records,
        id_field="document_id",
        record_id=review["document_id"],
        path=review["path"],
        label="document review",
    )
    if review_match is not None and review_match != review:
        raise ValueError(
            f"document review {review['document_id']} already exists with different metadata"
        )
    rendered_reviews = (
        review_registry
        if review_match is not None
        else _append_record(review_registry, "document_reviews", review)
    )
    return RegistrationResult(
        changed=source_match is None or review_match is None,
        record=record,
        rendered=rendered_sources,
        document_reviews_rendered=rendered_reviews,
    )


def _title_and_covers(source: str) -> tuple[str, list[str]]:
    title: str | None = None
    covers: list[str] = []
    for line in source.splitlines():
        match = HEADING_RE.fullmatch(line)
        if match is None:
            continue
        level, text = match.groups()
        if level == "#":
            if title is None:
                title = text.strip()
            continue
        slug = NON_ALNUM_RE.sub("_", text).strip("_").lower()
        if slug and slug not in covers:
            covers.append(slug)
    if title is None:
        raise ValueError("specification requires one H1 title")
    if not covers:
        raise ValueError("specification requires at least one H2 coverage section")
    return title, covers


def _existing_record(
    records: list[dict[str, Any]],
    *,
    id_field: str,
    record_id: str,
    path: str,
    label: str,
) -> dict[str, Any] | None:
    matches = [row for row in records if row.get(id_field) == record_id]
    if len(matches) > 1:
        raise ValueError(f"{label} ID is duplicated: {record_id}")
    if matches and matches[0].get("path") != path:
        raise ValueError(f"{label} ID collision: {record_id}")
    path_matches = [row for row in records if row.get("path") == path]
    if len(path_matches) > 1:
        raise ValueError(f"{label} path is duplicated: {path}")
    if path_matches and not matches:
        raise ValueError(
            f"{label} path {path} is already registered under another ID"
        )
    return matches[0] if matches else None


def _records(source: str, key: str, path: Path) -> list[dict[str, Any]]:
    document = tomllib.loads(source)
    records = document.get(key, [])
    if not isinstance(records, list) or any(
        not isinstance(row, dict) for row in records
    ):
        raise ValueError(f"{path} must contain [[{key}]] records")
    return records


def _append_record(source: str, table: str, record: dict[str, Any]) -> str:
    lines = ["", "", f"[[{table}]]"]
    for key, value in record.items():
        lines.append(f"{key} = {_toml_value(value)}")
    rendered = source.rstrip() + "\n" + "\n".join(lines) + "\n"
    parsed = tomllib.loads(rendered)
    if parsed.get(table, [])[-1] != record:
        raise ValueError(f"generated [[{table}]] record failed round-trip")
    return rendered


def _toml_value(value: Any) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, str):
        return json.dumps(value, ensure_ascii=False)
    if isinstance(value, list) and all(isinstance(item, str) for item in value):
        return "[" + ", ".join(json.dumps(item, ensure_ascii=False) for item in value) + "]"
    raise ValueError(f"unsupported registration value {value!r}")


def _relative_path(root: Path, path: Path) -> Path:
    root = root.resolve()
    candidate = path if path.is_absolute() else root / path
    resolved = candidate.resolve()
    try:
        relative = resolved.relative_to(root)
    except ValueError as error:
        raise ValueError("specification path escapes repository") from error
    if not resolved.is_file() or resolved.is_symlink():
        raise ValueError("specification path must be a regular file")
    return relative


def _nonempty(value: str, label: str) -> str:
    result = value.strip()
    if not result:
        raise ValueError(f"{label} must be non-empty")
    return result


def _review_date(value: str) -> str:
    if re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}", value) is None:
        raise ValueError("reviewed date must be YYYY-MM-DD")
    return value


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--path", type=Path, required=True)
    parser.add_argument("--area", required=True)
    parser.add_argument("--owner", required=True)
    parser.add_argument("--reviewed-date", required=True)
    parser.add_argument("--apply", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        result = register_spec_source(
            root=root,
            registry_path=root / SPEC_REGISTRY,
            review_registry_path=root / REVIEW_REGISTRY,
            path=args.path,
            area=args.area,
            owner=args.owner,
            reviewed_date=args.reviewed_date,
        )
        if args.apply and result.changed:
            (root / SPEC_REGISTRY).write_text(result.rendered)
            (root / REVIEW_REGISTRY).write_text(
                result.document_reviews_rendered
            )
    except (OSError, ValueError, tomllib.TOMLDecodeError) as error:
        print(f"specification registration failed: {error}", file=sys.stderr)
        return 2
    verb = "registered" if args.apply and result.changed else "ready"
    print(
        f"specification {verb}: {result.record['path']} "
        f"({result.record['id']})"
    )
    return 0
