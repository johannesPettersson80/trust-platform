"""Refresh existing specification document-review digests without hand editing."""

from __future__ import annotations

import argparse
import hashlib
import os
import re
import subprocess
import tempfile
import tomllib
from dataclasses import dataclass
from datetime import date
from pathlib import Path, PurePosixPath
from typing import Sequence

from .spec_source_reviews import FRESHNESS_STATES


REVIEW_REGISTRY = Path("verification/spec-document-reviews.toml")
ROW_HEADER_RE = re.compile(r"(?m)^\[\[document_reviews\]\][ \t]*(?:#.*)?(?:\r?\n|$)")
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")


class DocumentReviewRefreshError(ValueError):
    """A requested refresh is unsafe or cannot bind to exactly one review row."""


@dataclass(frozen=True)
class DocumentRefresh:
    path: str
    old_content_sha256: str
    new_content_sha256: str
    old_last_reviewed: str
    new_last_reviewed: str


@dataclass(frozen=True)
class DocumentReviewRefreshResult:
    changed: bool
    applied: bool
    rendered: str
    documents: tuple[DocumentRefresh, ...]


def refresh_document_reviews(
    *,
    root: Path,
    paths: Sequence[str],
    reviewed_date: str,
    apply: bool = False,
) -> DocumentReviewRefreshResult:
    """Refresh exactly the requested existing document-review rows."""

    root = root.resolve()
    reviewed_date = _review_date(reviewed_date)
    relative_paths = _validated_paths(root, paths)
    registry_path = root / REVIEW_REGISTRY
    _require_regular_nonsymlink(registry_path, "document-review registry")
    source = registry_path.read_text()
    rows = _load_rows(source, registry_path)
    row_spans = _row_spans(source)
    if len(rows) != len(row_spans):
        raise DocumentReviewRefreshError(
            "document-review registry row layout does not match parsed rows"
        )

    rows_by_path: dict[str, list[int]] = {}
    for index, row in enumerate(rows):
        row_path = row.get("path")
        if isinstance(row_path, str):
            rows_by_path.setdefault(row_path, []).append(index)

    requested_indexes: dict[int, tuple[str, str]] = {}
    refreshes: list[DocumentRefresh] = []
    for relative in relative_paths:
        relative_text = relative.as_posix()
        matches = rows_by_path.get(relative_text, [])
        if not matches:
            raise DocumentReviewRefreshError(
                f"no document review row exists for {relative_text}"
            )
        if len(matches) != 1:
            raise DocumentReviewRefreshError(
                f"multiple document review rows exist for {relative_text}"
            )
        row_index = matches[0]
        row = rows[row_index]
        old_digest = _required_string(row, "content_sha256", relative_text)
        old_date = _required_string(row, "last_reviewed", relative_text)
        freshness = row.get("freshness")
        if freshness is not None and freshness not in FRESHNESS_STATES:
            raise DocumentReviewRefreshError(
                f"{relative_text} has invalid freshness {freshness!r}"
            )
        new_digest = hashlib.sha256((root / relative).read_bytes()).hexdigest()
        requested_indexes[row_index] = (new_digest, reviewed_date)
        refreshes.append(
            DocumentRefresh(
                path=relative_text,
                old_content_sha256=old_digest,
                new_content_sha256=new_digest,
                old_last_reviewed=old_date,
                new_last_reviewed=reviewed_date,
            )
        )

    rendered_parts: list[str] = []
    cursor = 0
    for index, (start, end) in enumerate(row_spans):
        rendered_parts.append(source[cursor:start])
        row_source = source[start:end]
        if index in requested_indexes:
            digest, refresh_date = requested_indexes[index]
            row_source = _replace_string_field(
                row_source, "content_sha256", digest, required=True
            )
            row_source = _replace_string_field(
                row_source, "last_reviewed", refresh_date, required=True
            )
            if "freshness" in rows[index]:
                row_source = _replace_string_field(
                    row_source, "freshness", "current", required=True
                )
        rendered_parts.append(row_source)
        cursor = end
    rendered_parts.append(source[cursor:])
    rendered = "".join(rendered_parts)
    changed = rendered != source
    if apply and changed:
        _atomic_write(registry_path, rendered)

    return DocumentReviewRefreshResult(
        changed=changed,
        applied=apply and changed,
        rendered=rendered,
        documents=tuple(refreshes),
    )


def _validated_paths(root: Path, paths: Sequence[str]) -> list[PurePosixPath]:
    if not paths:
        raise DocumentReviewRefreshError("at least one document path is required")
    validated: list[PurePosixPath] = []
    seen: set[str] = set()
    for raw in paths:
        if (
            not raw
            or "\\" in raw
            or PurePosixPath(raw).is_absolute()
            or raw != PurePosixPath(raw).as_posix()
            or any(part in ("", ".", "..") for part in PurePosixPath(raw).parts)
        ):
            raise DocumentReviewRefreshError(
                f"{raw!r} is not a safe repo-relative path"
            )
        relative = PurePosixPath(raw)
        normalized = relative.as_posix()
        if normalized in seen:
            raise DocumentReviewRefreshError(
                f"duplicate requested path: {normalized}"
            )
        seen.add(normalized)
        live_path = root.joinpath(*relative.parts)
        _reject_symlink_components(root, relative)
        _require_regular_nonsymlink(live_path, f"document {normalized}")
        if not _is_tracked(root, normalized):
            raise DocumentReviewRefreshError(
                f"document path is not tracked by Git: {normalized}"
            )
        validated.append(relative)
    return validated


def _reject_symlink_components(root: Path, relative: PurePosixPath) -> None:
    candidate = root
    for part in relative.parts:
        candidate = candidate / part
        if candidate.is_symlink():
            raise DocumentReviewRefreshError(
                f"document path contains a symlink: {relative.as_posix()}"
            )


def _require_regular_nonsymlink(path: Path, label: str) -> None:
    if path.is_symlink():
        raise DocumentReviewRefreshError(f"{label} must not be a symlink")
    if not path.is_file():
        raise DocumentReviewRefreshError(f"{label} is not a regular file")


def _is_tracked(root: Path, relative: str) -> bool:
    result = subprocess.run(
        ["git", "-C", str(root), "ls-files", "--error-unmatch", "--", relative],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return result.returncode == 0


def _review_date(value: str) -> str:
    if not DATE_RE.fullmatch(value):
        raise DocumentReviewRefreshError(
            "reviewed date must be a valid ISO date in YYYY-MM-DD form"
        )
    try:
        parsed = date.fromisoformat(value)
    except ValueError as error:
        raise DocumentReviewRefreshError(
            "reviewed date must be a valid ISO date in YYYY-MM-DD form"
        ) from error
    if parsed.isoformat() != value:
        raise DocumentReviewRefreshError(
            "reviewed date must be a valid ISO date in YYYY-MM-DD form"
        )
    return value


def _load_rows(source: str, registry_path: Path) -> list[dict[str, object]]:
    try:
        loaded = tomllib.loads(source)
    except tomllib.TOMLDecodeError as error:
        raise DocumentReviewRefreshError(
            f"cannot parse document-review registry {registry_path}: {error}"
        ) from error
    rows = loaded.get("document_reviews")
    if not isinstance(rows, list) or any(not isinstance(row, dict) for row in rows):
        raise DocumentReviewRefreshError(
            "document-review registry must contain [[document_reviews]] rows"
        )
    return rows


def _row_spans(source: str) -> list[tuple[int, int]]:
    headers = list(ROW_HEADER_RE.finditer(source))
    return [
        (
            header.start(),
            headers[index + 1].start() if index + 1 < len(headers) else len(source),
        )
        for index, header in enumerate(headers)
    ]


def _required_string(row: dict[str, object], field: str, path: str) -> str:
    value = row.get(field)
    if not isinstance(value, str):
        raise DocumentReviewRefreshError(
            f"{path} document review has no valid {field} string"
        )
    return value


def _replace_string_field(
    row_source: str,
    field: str,
    value: str,
    *,
    required: bool,
) -> str:
    pattern = re.compile(
        rf'(?m)^(?P<prefix>[ \t]*{re.escape(field)}[ \t]*=[ \t]*")'
        r'(?P<value>[^"\r\n]*)'
        r'(?P<suffix>"[ \t]*(?:#.*)?(?:\r?\n|$))'
    )
    matches = list(pattern.finditer(row_source))
    expected = 1 if required else 0
    if len(matches) != expected:
        raise DocumentReviewRefreshError(
            f"document review row must contain exactly one scalar {field} field"
        )
    if not matches:
        return row_source
    return pattern.sub(
        lambda match: f"{match.group('prefix')}{value}{match.group('suffix')}",
        row_source,
        count=1,
    )


def _atomic_write(path: Path, rendered: str) -> None:
    descriptor, temporary_name = tempfile.mkstemp(
        dir=path.parent,
        prefix=f".{path.name}.",
        suffix=".tmp",
    )
    temporary_path = Path(temporary_name)
    try:
        os.fchmod(descriptor, path.stat().st_mode)
        with os.fdopen(descriptor, "w") as handle:
            handle.write(rendered)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary_path, path)
    except BaseException:
        try:
            os.close(descriptor)
        except OSError:
            pass
        temporary_path.unlink(missing_ok=True)
        raise


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Refresh existing document-review digests for explicit tracked paths; "
            "dry-run unless --apply is supplied."
        )
    )
    parser.add_argument(
        "--reviewed-date",
        required=True,
        help="review date in YYYY-MM-DD form",
    )
    parser.add_argument(
        "--apply",
        action="store_true",
        help="atomically write the refreshed registry (default: dry-run)",
    )
    parser.add_argument(
        "paths",
        nargs="+",
        help="explicit tracked repo-relative document paths",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    root = Path(__file__).resolve().parents[2]
    try:
        result = refresh_document_reviews(
            root=root,
            paths=args.paths,
            reviewed_date=args.reviewed_date,
            apply=args.apply,
        )
    except DocumentReviewRefreshError as error:
        print(f"error: {error}")
        return 2
    mode = "applied" if result.applied else "dry-run"
    for document in result.documents:
        print(f"{mode}: {document.path}")
    if not result.changed:
        print(f"{mode}: no changes")
    return 0
