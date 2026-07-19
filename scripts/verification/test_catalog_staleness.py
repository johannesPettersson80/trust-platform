"""Live path and generated-identity validation for committed test-catalog rows."""

from __future__ import annotations

from collections import defaultdict
from collections.abc import Mapping, Sequence
from pathlib import Path, PurePosixPath
from typing import Any

from .test_catalog_models import InferredTestFact
from .test_refactor_contract import validate_test_refactor_records


DISCOVERY_FIELDS = {"discovery_id", "discovery_source_kind", "name"}


def validate_catalog_staleness(
    *,
    root: Path,
    tests: Mapping[str, Mapping[str, Any]],
    facts: Sequence[InferredTestFact],
    proposals: Mapping[str, Mapping[str, Any]] | None = None,
    redirects: Mapping[str, Mapping[str, Any]] | None = None,
    evidence: Mapping[str, Mapping[str, Any]] | None = None,
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
    if any(value is not None for value in (proposals, redirects, evidence)):
        failures.extend(
            validate_test_refactor_records(
                root=root,
                proposals=proposals or {},
                redirects=redirects or {},
                tests=tests,
                evidence=evidence or {},
                facts=facts,
            )
        )
    return failures


def validate_live_catalog(root: Path) -> tuple[list[str], int, int]:
    """Recompute the complete live P2A join, including proposal assessment."""

    from .test_refactor_live import build_live_test_refactor_state

    try:
        state = build_live_test_refactor_state(root)
    except ValueError as exc:
        return [str(exc)], 0, 0
    return [], state.catalog_count, state.fact_count


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
