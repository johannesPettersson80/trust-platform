"""Live path and generated-identity validation for committed test-catalog rows."""

from __future__ import annotations

import tomllib
from collections import Counter, defaultdict
from collections.abc import Mapping, Sequence
from pathlib import Path, PurePosixPath
from typing import Any

from .test_catalog_models import InferredTestFact
from .test_catalog_scanner import scan_repository
from .test_catalog_validation import validate_report_payload


DISCOVERY_FIELDS = {"discovery_id", "discovery_source_kind", "name"}


def validate_catalog_staleness(
    *,
    root: Path,
    tests: Mapping[str, Mapping[str, Any]],
    facts: Sequence[InferredTestFact],
) -> list[str]:
    failures: list[str] = []
    facts_by_id: dict[str, list[InferredTestFact]] = defaultdict(list)
    for fact in facts:
        facts_by_id[fact.stable_id].append(fact)

    for record_id in sorted(tests):
        record = tests[record_id]
        _validate_path(root, record_id, record.get("path"), failures)
        subject_kind = record.get("subject_kind")
        if subject_kind == "generated_test":
            _validate_generated_binding(record_id, record, facts_by_id, failures)
        elif subject_kind in {"case_table_artifact", "mutation_shard_runner"}:
            unexpected = sorted(DISCOVERY_FIELDS & set(record))
            if unexpected:
                failures.append(
                    f"{record_id} {subject_kind} must not carry scanner identity: {', '.join(unexpected)}"
                )
        else:
            failures.append(f"{record_id} has unsupported subject_kind {subject_kind!r}")
    return failures


def validate_live_catalog(root: Path) -> tuple[list[str], int, int]:
    """Scan current sources and join them to the committed catalog."""

    root = root.resolve()
    report = scan_repository(root)
    payload = report.to_dict()
    failures = [f"generated catalog: {item}" for item in validate_report_payload(payload)]
    if payload.get("scan_status") != "complete":
        failures.append("generated catalog scan_status is not complete")
    try:
        catalog = tomllib.loads((root / "verification/test-catalog.toml").read_text())
    except Exception as exc:
        return [*failures, f"committed catalog cannot be read: {exc}"], 0, len(report.inferred_facts)
    records = catalog.get("tests")
    if not isinstance(records, list):
        return [*failures, "committed catalog must contain [[tests]] records"], 0, len(report.inferred_facts)
    tests: dict[str, Mapping[str, Any]] = {}
    counts = Counter(record.get("id") for record in records if isinstance(record, Mapping))
    for record in records:
        if not isinstance(record, Mapping) or not isinstance(record.get("id"), str):
            failures.append("committed catalog contains a test without a string id")
            continue
        record_id = record["id"]
        if counts[record_id] != 1:
            failures.append(f"committed catalog has duplicate test id {record_id}")
            continue
        tests[record_id] = record
    failures.extend(
        validate_catalog_staleness(root=root, tests=tests, facts=report.inferred_facts)
    )
    return sorted(set(failures)), len(tests), len(report.inferred_facts)


def _validate_path(root: Path, record_id: str, value: Any, failures: list[str]) -> None:
    if not isinstance(value, str) or not _safe_relative_path(value):
        failures.append(f"{record_id} path is not a safe workspace-relative path: {value!r}")
        return
    root_resolved = root.resolve()
    candidate = root_resolved / value
    try:
        resolved = candidate.resolve()
        resolved.relative_to(root_resolved)
    except (OSError, ValueError):
        failures.append(f"{record_id} path escapes the workspace: {value}")
        return
    if not candidate.is_file():
        failures.append(f"{record_id} path does not exist: {value}")


def _safe_relative_path(value: str) -> bool:
    path = PurePosixPath(value)
    return bool(value) and "\\" not in value and not path.is_absolute() and ".." not in path.parts


def _validate_generated_binding(
    record_id: str,
    record: Mapping[str, Any],
    facts_by_id: Mapping[str, list[InferredTestFact]],
    failures: list[str],
) -> None:
    discovery_id = record.get("discovery_id")
    if not isinstance(discovery_id, str):
        failures.append(f"{record_id} generated_test lacks discovery_id")
        return
    matches = facts_by_id.get(discovery_id, [])
    if len(matches) != 1:
        if not matches:
            failures.append(f"{record_id} discovery_id is absent from current scanner facts: {discovery_id}")
        else:
            failures.append(f"{record_id} discovery_id resolved to {len(matches)} scanner facts: {discovery_id}")
        return
    fact = matches[0]
    for field, actual in (
        ("path", fact.path),
        ("name", fact.name),
        ("discovery_source_kind", fact.source_kind),
    ):
        if record.get(field) != actual:
            failures.append(
                f"{record_id} {field} is stale: catalog {record.get(field)!r}, scanner {actual!r}"
            )
