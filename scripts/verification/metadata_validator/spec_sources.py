"""Closed metadata contract for registered specification sources."""

from __future__ import annotations

import re
import subprocess
from collections.abc import Callable, Mapping
from pathlib import Path, PurePosixPath
from typing import Any

from ..spec_source_markdown import MarkdownScan, scan_markdown_document
from ..spec_source_scope import is_primary_public_path
from .constants import (
    AREAS,
    SOURCE_AUTHORITIES,
    SOURCE_STATUSES,
    STATUSES,
)


Fail = Callable[[Path, str], None]

SPEC_SOURCE_SCHEMA_VERSION = 2
LOCATOR_KINDS = {"tracked_file", "external_reference"}
VISIBILITIES = {"public", "internal", "external"}

COMMON_REQUIRED_FIELDS = {
    "schema_version",
    "id",
    "title",
    "area",
    "owner",
    "status",
    "authority",
    "source_status",
    "oracle_eligible",
    "visibility",
    "locator_kind",
    "version",
    "last_reviewed",
    "covers",
    "known_limitations",
    "conflicts_with",
}
TRACKED_FILE_FIELDS = {"locator_kind", "path"}
EXTERNAL_REFERENCE_FIELDS = {
    "locator_kind",
    "external_ref",
    "expected_local_path",
    "retrieval_expectation",
    "publication_date",
    "redistributable",
    "absence_blocks_proof",
}
PUBLIC_CLAIM_FIELDS = {"claim_text", "surface_ref"}
WORKFLOW_FIELDS = {
    "actor",
    "entry_point",
    "preconditions",
    "success_state",
    "failure_status_behavior",
    "acceptance_evidence",
}
SPEC_SOURCE_FIELDS = (
    COMMON_REQUIRED_FIELDS
    | {"path"}
    | (EXTERNAL_REFERENCE_FIELDS - {"locator_kind"})
    | PUBLIC_CLAIM_FIELDS
    | WORKFLOW_FIELDS
    | {"context_refs"}
)

_DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
_PUBLICATION_DATE_RE = re.compile(r"^\d{4}-\d{2}(?:-\d{2})?$")


def validate_spec_source_records(
    *,
    root: Path,
    records: Mapping[str, Mapping[str, Any]],
    fail: Fail,
) -> None:
    """Validate every source without reading uncommitted external-standard bytes."""

    root = root.resolve()
    for source_id, record in records.items():
        path = _record_path(root, record)
        fields = set(record) - {"_path"}
        missing = sorted(COMMON_REQUIRED_FIELDS - fields)
        extra = sorted(fields - SPEC_SOURCE_FIELDS)
        if missing:
            fail(path, f"spec source {source_id} missing fields: {', '.join(missing)}")
        if extra:
            fail(path, f"spec source {source_id} has unexpected fields: {', '.join(extra)}")

        if record.get("schema_version") != SPEC_SOURCE_SCHEMA_VERSION:
            fail(
                path,
                f"record {source_id} must use schema_version = "
                f"{SPEC_SOURCE_SCHEMA_VERSION}",
            )
        _nonempty_strings(record, source_id, path, fail)
        if record.get("area") not in AREAS:
            fail(path, f"{source_id} uses unknown area {record.get('area')!r}")
        if record.get("status") not in STATUSES:
            fail(path, f"{source_id} uses unknown status {record.get('status')!r}")
        if record.get("authority") not in SOURCE_AUTHORITIES:
            fail(path, f"{source_id} has unknown authority {record.get('authority')!r}")
        if record.get("source_status") not in SOURCE_STATUSES:
            fail(path, f"{source_id} has unknown source_status {record.get('source_status')!r}")
        if record.get("visibility") not in VISIBILITIES:
            fail(path, f"{source_id} has unknown visibility {record.get('visibility')!r}")
        if not isinstance(record.get("oracle_eligible"), bool):
            fail(path, f"{source_id} oracle_eligible must be boolean")
        if not _DATE_RE.fullmatch(str(record.get("last_reviewed", ""))):
            fail(path, f"{source_id} last_reviewed must use YYYY-MM-DD")

        _string_list(record, "covers", source_id, path, fail, require_nonempty=True)
        _string_list(record, "known_limitations", source_id, path, fail)
        _string_list(record, "conflicts_with", source_id, path, fail)
        if "context_refs" in record:
            _string_list(record, "context_refs", source_id, path, fail)
        for field in (
            "actor",
            "entry_point",
            "success_state",
            "failure_status_behavior",
        ):
            if field in record:
                _optional_nonempty_string(record, field, source_id, path, fail)
        for field in ("preconditions", "acceptance_evidence"):
            if field in record:
                _string_list(record, field, source_id, path, fail)

        locator_kind = record.get("locator_kind")
        if locator_kind not in LOCATOR_KINDS:
            fail(path, f"{source_id} has unknown locator_kind {locator_kind!r}")
        elif locator_kind == "tracked_file":
            _validate_tracked_file(root, source_id, record, path, fail)
        else:
            _validate_external_reference(root, source_id, record, path, fail)

        if record.get("authority") == "public_claim":
            if record.get("oracle_eligible") is not False:
                fail(path, f"public claim {source_id} must set oracle_eligible = false")
            for field in sorted(PUBLIC_CLAIM_FIELDS):
                if not isinstance(record.get(field), str) or not record[field]:
                    fail(path, f"public claim {source_id} missing {field}")

    _validate_conflict_refs(records, fail, root)
    _validate_public_claim_surface_refs(root, records, fail)


def _validate_tracked_file(
    root: Path,
    source_id: str,
    record: Mapping[str, Any],
    metadata_path: Path,
    fail: Fail,
) -> None:
    if not isinstance(record.get("path"), str) or not record["path"]:
        fail(metadata_path, f"{source_id} tracked_file requires path")
        return
    forbidden = sorted((EXTERNAL_REFERENCE_FIELDS - {"locator_kind"}) & set(record))
    if forbidden:
        fail(metadata_path, f"{source_id} tracked_file forbids external-reference fields")
    relative = str(record["path"])
    if not _safe_relative_path(relative):
        fail(metadata_path, f"{source_id} tracked path must be normalized and workspace-relative")
        return
    candidate = root / relative
    if _has_symlink_component(root, relative):
        fail(metadata_path, f"{source_id} tracked path contains a symlink component")
    if not candidate.is_file():
        fail(metadata_path, f"{source_id} path does not exist: {relative}")
    elif not _is_tracked(root, relative):
        fail(metadata_path, f"{source_id} tracked_file path is not tracked: {relative}")


def _validate_external_reference(
    root: Path,
    source_id: str,
    record: Mapping[str, Any],
    metadata_path: Path,
    fail: Fail,
) -> None:
    missing = sorted(EXTERNAL_REFERENCE_FIELDS - set(record))
    if missing:
        fail(metadata_path, f"{source_id} external_reference missing fields: {', '.join(missing)}")
    if "path" in record:
        fail(metadata_path, f"{source_id} external_reference forbids path")
    if record.get("authority") != "normative_external":
        fail(metadata_path, f"{source_id} external_reference requires normative_external authority")
    if record.get("oracle_eligible") is not False:
        fail(metadata_path, f"{source_id} external_reference must set oracle_eligible = false")
    if record.get("redistributable") is not False:
        fail(metadata_path, f"{source_id} external_reference must set redistributable = false")
    if record.get("absence_blocks_proof") is not True:
        fail(metadata_path, f"{source_id} external_reference must set absence_blocks_proof = true")
    for field in ("external_ref", "retrieval_expectation"):
        _optional_nonempty_string(record, field, source_id, metadata_path, fail)
    if not _PUBLICATION_DATE_RE.fullmatch(str(record.get("publication_date", ""))):
        fail(metadata_path, f"{source_id} publication_date must use YYYY-MM or YYYY-MM-DD")
    expected = record.get("expected_local_path")
    if not isinstance(expected, str) or not _safe_relative_path(expected):
        fail(metadata_path, f"{source_id} expected_local_path must be normalized and workspace-relative")
    elif not expected.startswith("docs/internal/standards/"):
        fail(metadata_path, f"{source_id} expected_local_path must be under docs/internal/standards")
    elif not _is_ignored_untracked_path(root, expected):
        fail(metadata_path, f"{source_id} expected_local_path must be gitignored and untracked")


def _validate_conflict_refs(
    records: Mapping[str, Mapping[str, Any]],
    fail: Fail,
    root: Path,
) -> None:
    for source_id, record in records.items():
        path = _record_path(root, record)
        refs = record.get("conflicts_with")
        if not isinstance(refs, list):
            continue
        for target in refs:
            if target == source_id:
                fail(path, f"{source_id} conflicts_with cannot reference itself")
            elif target not in records:
                fail(path, f"{source_id} conflicts_with references unknown source {target}")


def _validate_public_claim_surface_refs(
    root: Path,
    records: Mapping[str, Mapping[str, Any]],
    fail: Fail,
) -> None:
    claims = {
        source_id: record
        for source_id, record in records.items()
        if record.get("authority") == "public_claim"
    }
    if not claims:
        return

    parsed: dict[str, tuple[str, str | None]] = {}
    for source_id, record in claims.items():
        metadata_path = _record_path(root, record)
        surface_ref = record.get("surface_ref")
        if not isinstance(surface_ref, str) or not surface_ref:
            continue
        surface_path, separator, fragment = surface_ref.partition("#")
        if not _safe_relative_path(surface_path):
            fail(
                metadata_path,
                f"public claim {source_id} surface_ref path must be normalized "
                "and workspace-relative",
            )
            continue
        if separator and (not fragment or "#" in fragment):
            fail(
                metadata_path,
                f"public claim {source_id} surface_ref fragment must be one non-empty anchor",
            )
            continue
        if not is_primary_public_path(surface_path):
            fail(
                metadata_path,
                f"public claim {source_id} surface_ref path is not a reviewed public surface",
            )
            continue
        if _has_symlink_component(root, surface_path):
            fail(
                metadata_path,
                f"public claim {source_id} surface_ref path contains a symlink component",
            )
            continue
        if not (root / surface_path).is_file():
            fail(
                metadata_path,
                f"public claim {source_id} surface_ref path does not exist: {surface_path}",
            )
            continue
        if not _is_tracked(root, surface_path):
            fail(
                metadata_path,
                f"public claim {source_id} surface_ref path is not tracked: {surface_path}",
            )
            continue
        parsed[source_id] = (surface_path, fragment or None)

    scans: dict[str, MarkdownScan | None] = {}
    for source_id, (surface_path, fragment) in parsed.items():
        metadata_path = _record_path(root, claims[source_id])
        if surface_path not in scans:
            try:
                text = (root / surface_path).read_text(encoding="utf-8")
            except (OSError, UnicodeDecodeError) as exc:
                fail(
                    metadata_path,
                    f"public claim {source_id} cannot read surface_ref path: {exc}",
                )
                scans[surface_path] = None
            else:
                scans[surface_path] = scan_markdown_document(surface_path, text)
        scan = scans[surface_path]
        if scan is None:
            continue
        errors = [item for item in scan.diagnostics if item.severity == "error"]
        if errors:
            detail = "; ".join(
                f"{item.kind} at line {item.line}" for item in errors[:5]
            )
            fail(
                metadata_path,
                f"public claim {source_id} surface_ref path is not valid Markdown: {detail}",
            )
            continue
        if fragment is not None:
            headings = [heading for heading in scan.headings if heading.anchor == fragment]
            if len(headings) != 1:
                fail(
                    metadata_path,
                    f"public claim {source_id} surface_ref fragment does not identify a heading",
                )


def _nonempty_strings(
    record: Mapping[str, Any],
    source_id: str,
    path: Path,
    fail: Fail,
) -> None:
    for field in (
        "id",
        "title",
        "area",
        "owner",
        "status",
        "authority",
        "source_status",
        "visibility",
        "locator_kind",
        "version",
        "last_reviewed",
    ):
        if not isinstance(record.get(field), str) or not record[field]:
            fail(path, f"spec source {source_id} {field} must be a non-empty string")


def _string_list(
    record: Mapping[str, Any],
    field: str,
    source_id: str,
    path: Path,
    fail: Fail,
    *,
    require_nonempty: bool = False,
) -> None:
    value = record.get(field)
    if not isinstance(value, list) or not all(isinstance(item, str) and item for item in value):
        fail(path, f"{source_id} {field} must be a string array")
    elif require_nonempty and not value:
        fail(path, f"{source_id} must cover at least one tag")
    elif len(value) != len(set(value)):
        fail(path, f"{source_id} {field} must not contain duplicates")


def _optional_nonempty_string(
    record: Mapping[str, Any],
    field: str,
    source_id: str,
    path: Path,
    fail: Fail,
) -> None:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        fail(path, f"{source_id} {field} must be a non-empty string")


def _record_path(root: Path, record: Mapping[str, Any]) -> Path:
    value = record.get("_path")
    if isinstance(value, Path):
        return value if value.is_absolute() else root / value
    return root / "verification/spec-sources.toml"


def _safe_relative_path(value: str) -> bool:
    if not value or "\\" in value:
        return False
    path = PurePosixPath(value)
    return (
        bool(path.parts)
        and not path.is_absolute()
        and "." not in path.parts
        and ".." not in path.parts
        and path.as_posix() == value
    )


def _has_symlink_component(root: Path, relative: str) -> bool:
    candidate = root
    for part in PurePosixPath(relative).parts:
        candidate /= part
        if candidate.is_symlink():
            return True
    return False


def _is_tracked(root: Path, relative: str) -> bool:
    result = subprocess.run(
        ["git", "-C", str(root), "ls-files", "--error-unmatch", "--", relative],
        check=False,
        capture_output=True,
    )
    return result.returncode == 0


def _is_ignored_untracked_path(root: Path, relative: str) -> bool:
    ignored = subprocess.run(
        ["git", "-C", str(root), "check-ignore", "-q", "--", relative],
        check=False,
        capture_output=True,
    )
    if ignored.returncode != 0:
        return False
    return not _is_tracked(root, relative)
