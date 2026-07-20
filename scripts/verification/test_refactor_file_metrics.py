"""Mechanical file metrics joined only to reviewed test-catalog intent."""

from __future__ import annotations

from collections import defaultdict
from collections.abc import Mapping, Sequence
from pathlib import Path, PurePosixPath
from typing import Any


def analyze_test_files(
    *,
    root: Path,
    scanner_facts: Sequence[Mapping[str, Any]],
    catalog_records: Sequence[Mapping[str, Any]],
    large_file_threshold: int,
) -> list[dict[str, Any]]:
    """Return canonical per-source metrics without inferring purpose from source text."""

    if isinstance(large_file_threshold, bool) or not isinstance(large_file_threshold, int):
        raise ValueError("large_file_threshold must be an integer")
    if large_file_threshold < 1:
        raise ValueError("large_file_threshold must be positive")
    root = root.resolve()
    facts_by_id: dict[str, Mapping[str, Any]] = {}
    facts_by_path: dict[str, list[Mapping[str, Any]]] = defaultdict(list)
    for fact in scanner_facts:
        discovery_id = _required_string(fact, ("stable_id", "discovery_id"), "scanner fact identity")
        if discovery_id in facts_by_id:
            raise ValueError(f"scanner duplicates discovery identity {discovery_id}")
        path = _required_string(fact, ("path",), f"scanner fact {discovery_id} path")
        read_workspace_bytes(root, path)
        source_kind = _required_string(
            fact,
            ("source_kind", "discovery_source_kind"),
            f"scanner fact {discovery_id} source_kind",
        )
        facts_by_id[discovery_id] = fact
        facts_by_path[path].append({**fact, "_identity": discovery_id, "_source_kind": source_kind})

    reviewed_by_path: dict[str, list[Mapping[str, Any]]] = defaultdict(list)
    catalog_ids: set[str] = set()
    classified_discovery_ids: set[str] = set()
    for record in catalog_records:
        test_id = _required_string(record, ("id",), "catalog test id")
        if test_id in catalog_ids:
            raise ValueError(f"catalog duplicates test id {test_id}")
        catalog_ids.add(test_id)
        if record.get("subject_kind") != "generated_test":
            continue
        discovery_id = _required_string(
            record, ("discovery_id",), f"catalog test {test_id} discovery_id"
        )
        if discovery_id in classified_discovery_ids:
            raise ValueError(f"discovery identity {discovery_id} has multiple catalog records")
        classified_discovery_ids.add(discovery_id)
        fact = facts_by_id.get(discovery_id)
        if fact is None:
            raise ValueError(f"catalog test {test_id} discovery identity is absent: {discovery_id}")
        path = _required_string(record, ("path",), f"catalog test {test_id} path")
        fact_path = _required_string(fact, ("path",), f"scanner fact {discovery_id} path")
        if path != fact_path:
            raise ValueError(
                f"catalog test {test_id} path does not match scanner fact: {path!r} != {fact_path!r}"
            )
        reviewed_by_path[path].append(record)

    rows: list[dict[str, Any]] = []
    for path in sorted(facts_by_path):
        raw = read_workspace_bytes(root, path)
        try:
            line_count = len(raw.decode("utf-8").splitlines())
        except UnicodeError as exc:
            raise ValueError(f"scanner source is not UTF-8 text: {path}: {exc}") from exc
        facts = facts_by_path[path]
        records = reviewed_by_path.get(path, [])
        areas = sorted({_required_string(row, ("area",), "catalog area") for row in records})
        classes = sorted(
            {_required_string(row, ("test_class",), "catalog test_class") for row in records}
        )
        test_ids = sorted(_required_string(row, ("id",), "catalog test id") for row in records)
        invariant_ids = sorted(
            {
                invariant
                for row in records
                for invariant in _required_string_list(row.get("invariants"), "catalog invariants")
            }
        )
        large = line_count >= large_file_threshold
        mixed = len(areas) > 1 or len(classes) > 1
        candidate_reasons = []
        if large:
            candidate_reasons.append("large_file")
        if mixed:
            candidate_reasons.append("reviewed_mapping_diversity")
        rows.append(
            {
                "candidate_reasons": candidate_reasons,
                "conditional_count": sum(
                    1 for row in facts if row.get("ignore_state") == "conditional"
                ),
                "ignored_count": sum(1 for row in facts if row.get("ignore_state") == "ignored"),
                "mapped_test_ids": test_ids,
                "packages": sorted(
                    {
                        package
                        for row in facts
                        if isinstance((package := row.get("package")), str) and package
                    }
                ),
                "path": path,
                "physical_lines": line_count,
                "reviewed_areas": areas,
                "reviewed_invariant_ids": invariant_ids,
                "reviewed_test_classes": classes,
                "scanner_fact_count": len(facts),
                "source_kinds": sorted({str(row["_source_kind"]) for row in facts}),
                "unmapped_fact_count": len(facts) - len(records),
            }
        )
    return rows


def analyze_vscode_registration(
    audit: Mapping[str, Any],
    file_rows: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    """Project the explicit registration audit onto mechanical file metrics."""

    registered = _optional_string_list(audit.get("registered_files", []), "registered_files")
    test_files = _optional_string_list(audit.get("test_files", []), "test_files")
    issues = {
        "unregistered_files": _optional_string_list(
            audit.get("unregistered_files", []), "unregistered_files"
        ),
        "missing_targets": _optional_string_list(
            audit.get("missing_targets", []), "missing_targets"
        ),
        "duplicate_targets": _optional_string_list(
            audit.get("duplicate_targets", []), "duplicate_targets"
        ),
        "unregistered_fact_files": _optional_string_list(
            audit.get("unregistered_fact_files", []), "unregistered_fact_files"
        ),
    }
    diagnostics = audit.get("diagnostics", [])
    if not isinstance(diagnostics, (list, tuple)):
        raise ValueError("VS Code registration diagnostics must be a list")
    diagnostic_rows: list[dict[str, Any]] = []
    for item in diagnostics:
        if not isinstance(item, Mapping):
            raise ValueError("VS Code registration diagnostic must be a mapping")
        diagnostic_rows.append(
            {
                "kind": _required_string(item, ("kind",), "registration diagnostic kind"),
                "path": _required_string(item, ("path",), "registration diagnostic path"),
                "severity": _required_string(
                    item, ("severity",), "registration diagnostic severity"
                ),
            }
        )
    entries_value = audit.get("entries", [])
    if not isinstance(entries_value, (list, tuple)):
        raise ValueError("VS Code registration entries must be a list")
    entries_by_path: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for item in entries_value:
        if not isinstance(item, Mapping):
            raise ValueError("VS Code registration entry must be a mapping")
        source_line = item.get("source_line", item.get("registration_line"))
        if isinstance(source_line, bool) or not isinstance(source_line, int) or source_line < 1:
            raise ValueError("VS Code registration source_line must be a positive integer")
        path = _required_string(
            item,
            ("resolved_path", "path"),
            "VS Code registration resolved path",
        )
        entries_by_path[path].append(
            {
                "registration_line": source_line,
                "specifier": _required_string(
                    item, ("specifier",), "VS Code registration specifier"
                ),
            }
        )
    file_by_path = {str(row["path"]): row for row in file_rows}
    file_details: list[dict[str, Any]] = []
    for path in sorted(registered):
        row = file_by_path.get(path)
        if row is None:
            raise ValueError(f"registered VS Code file is absent from scanner file metrics: {path}")
        path_entries = entries_by_path.get(path, [])
        if len(path_entries) != 1:
            raise ValueError(
                f"registered VS Code file must have one literal registration entry: {path}"
            )
        entry = path_entries[0]
        file_details.append(
            {
                "fact_count": row["scanner_fact_count"],
                "ignored_count": row["ignored_count"],
                "large_candidate": "large_file" in row["candidate_reasons"],
                "mapped_count": len(row["mapped_test_ids"]),
                "path": path,
                "physical_lines": row["physical_lines"],
                "registration_line": entry["registration_line"],
                "specifier": entry["specifier"],
            }
        )
    explicit_clean = audit.get("is_clean")
    if explicit_clean is not None and not isinstance(explicit_clean, bool):
        raise ValueError("VS Code registration is_clean must be boolean")
    derived_clean = not (
        any(issues.values())
        or any(row["severity"] == "error" for row in diagnostic_rows)
    )
    if explicit_clean is not None and explicit_clean != derived_clean:
        raise ValueError("VS Code registration is_clean disagrees with audit details")
    fact_count = audit.get("fact_count", 0)
    if isinstance(fact_count, bool) or not isinstance(fact_count, int) or fact_count < 0:
        raise ValueError("VS Code registration fact_count must be a nonnegative integer")
    index_path = audit.get("index_path")
    if index_path is not None and not isinstance(index_path, str):
        raise ValueError("VS Code registration index_path must be a string or null")
    return {
        "diagnostics": sorted(
            diagnostic_rows, key=lambda row: (row["path"], row["severity"], row["kind"])
        ),
        "fact_count": fact_count,
        "files": file_details,
        "index_path": index_path,
        "registration_count": sum(len(items) for items in entries_by_path.values()),
        "test_file_count": len(test_files),
        "registration_issues": {key: sorted(value) for key, value in sorted(issues.items())},
    }


def read_workspace_bytes(root: Path, relative_path: str) -> bytes:
    """Read a regular contained file and reject path or symlink ambiguity."""

    if not isinstance(relative_path, str) or not _safe_relative_path(relative_path):
        raise ValueError(f"path is not a safe workspace-relative path: {relative_path!r}")
    root = root.resolve()
    candidate = root / relative_path
    try:
        resolved = candidate.resolve(strict=True)
        resolved.relative_to(root)
    except (OSError, ValueError) as exc:
        raise ValueError(f"path is not a contained existing file: {relative_path}") from exc
    if candidate.is_symlink() or not resolved.is_file():
        raise ValueError(f"path must be a regular non-symlink file: {relative_path}")
    try:
        return resolved.read_bytes()
    except OSError as exc:
        raise ValueError(f"path cannot be read: {relative_path}: {exc}") from exc


def _safe_relative_path(value: str) -> bool:
    path = PurePosixPath(value)
    return (
        bool(value)
        and "\\" not in value
        and not path.is_absolute()
        and ".." not in path.parts
        and path.as_posix() == value
    )


def _required_string(
    record: Mapping[str, Any],
    fields: tuple[str, ...],
    label: str,
) -> str:
    for field in fields:
        value = record.get(field)
        if isinstance(value, str) and value:
            return value
    raise ValueError(f"{label} must be a non-empty string")


def _required_string_list(value: Any, label: str) -> list[str]:
    if not isinstance(value, list) or any(not isinstance(item, str) or not item for item in value):
        raise ValueError(f"{label} must be a string list")
    if len(value) != len(set(value)):
        raise ValueError(f"{label} must not contain duplicates")
    return value


def _optional_string_list(value: Any, label: str) -> list[str]:
    if not isinstance(value, (list, tuple)) or any(
        not isinstance(item, str) or not item for item in value
    ):
        raise ValueError(f"VS Code registration {label} must be a string list")
    if len(value) != len(set(value)):
        raise ValueError(f"VS Code registration {label} must not contain duplicates")
    return list(value)
