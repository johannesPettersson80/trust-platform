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
    fallback_covers: list[str] | None = None,
) -> dict[str, Any]:
    relative = _relative_path(root, path)
    source = (root / relative).read_text()
    title, covers = _title_and_covers(source, fallback_covers=fallback_covers)
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
    promote_review: bool = False,
) -> RegistrationResult:
    root = root.resolve()
    relative = _relative_path(root, path)
    source_registry = registry_path.read_text()
    source_records = _records(source_registry, "spec_sources", registry_path)
    path_matches = [
        row for row in source_records if row.get("path") == relative.as_posix()
    ]
    if len(path_matches) > 1:
        raise ValueError(f"specification source path is duplicated: {relative}")
    fallback_covers = path_matches[0].get("covers") if path_matches else None
    if fallback_covers is not None and (
        not isinstance(fallback_covers, list)
        or any(not isinstance(value, str) or not value for value in fallback_covers)
    ):
        raise ValueError("existing specification source covers must be nonempty strings")
    record = build_spec_source_record(
        root=root,
        path=relative,
        area=area,
        owner=owner,
        reviewed_date=reviewed_date,
        fallback_covers=fallback_covers,
    )
    if path_matches:
        # Legacy reviewed sources may predate filename-derived IDs. The path is
        # already unique, so refresh the existing authority without renaming it.
        record = {**record, "id": path_matches[0]["id"]}
    source_match = _existing_record(
        source_records,
        id_field="id",
        record_id=record["id"],
        path=record["path"],
        label="specification source",
    )
    if source_match is not None:
        for field in ("area", "owner"):
            if source_match.get(field) != record[field]:
                raise ValueError(
                    f"specification source {record['id']} {field} is reviewed "
                    "metadata and cannot be changed by refresh"
                )
        record = {
            **source_match,
            "last_reviewed": record["last_reviewed"],
        }
    rendered_sources = (
        _append_record(source_registry, "spec_sources", record)
        if source_match is None
        else (
            source_registry
            if source_match == record
            else _replace_record(
                source_registry,
                table="spec_sources",
                index=source_records.index(source_match),
                record=record,
            )
        )
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
    if review_match is not None:
        if (
            source_match is None
            or (
                promote_review
                and review_match.get("classification_basis")
                != "registered_metadata"
            )
        ):
            # A document may already have a conservative, non-oracle audit row.
            # First registration is the acceptance act; a later promotion must
            # be explicitly authorized rather than hidden in a digest refresh.
            review = {**review}
        else:
            review = {
                **review_match,
                "content_sha256": review["content_sha256"],
                "freshness": "current",
                "last_reviewed": record["last_reviewed"],
            }
    rendered_reviews = (
        _append_record(review_registry, "document_reviews", review)
        if review_match is None
        else (
            review_registry
            if review_match == review
            else _replace_record(
                review_registry,
                table="document_reviews",
                index=review_records.index(review_match),
                record=review,
            )
        )
    )
    return RegistrationResult(
        changed=source_match != record or review_match != review,
        record=record,
        rendered=rendered_sources,
        document_reviews_rendered=rendered_reviews,
    )


def _title_and_covers(
    source: str,
    *,
    fallback_covers: list[str] | None = None,
) -> tuple[str, list[str]]:
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
    if not covers and fallback_covers:
        covers = list(fallback_covers)
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


def _replace_record(
    source: str,
    *,
    table: str,
    index: int,
    record: dict[str, Any],
) -> str:
    header = re.compile(rf"(?m)^\[\[{re.escape(table)}\]\]\s*$")
    matches = list(header.finditer(source))
    if index < 0 or index >= len(matches):
        raise ValueError(f"generated [[{table}]] replacement index is invalid")
    start = matches[index].start()
    next_table = re.search(r"(?m)^\[\[[^\]]+\]\]\s*$", source[matches[index].end() :])
    end = (
        matches[index].end() + next_table.start()
        if next_table is not None
        else len(source)
    )
    block = "\n".join(
        [f"[[{table}]]"]
        + [f"{key} = {_toml_value(value)}" for key, value in record.items()]
    )
    suffix = source[end:]
    separator = "\n\n" if suffix else "\n"
    rendered = source[:start].rstrip() + "\n\n" + block + separator + suffix.lstrip()
    parsed = tomllib.loads(rendered)
    if parsed.get(table, [])[index] != record:
        raise ValueError(f"generated [[{table}]] replacement failed round-trip")
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
    parser.add_argument(
        "--promote-review",
        action="store_true",
        help=(
            "explicitly accept and promote an existing conservative document "
            "review to the registered source authority"
        ),
    )
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
            promote_review=args.promote_review,
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
