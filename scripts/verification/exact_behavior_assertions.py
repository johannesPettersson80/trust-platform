"""Build Rust assertion-inventory requests from exact behavior assignments."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any

from .mapping_assertions import RUST_SOURCE_KINDS


def build_assertion_facts(
    assignments: Sequence[Mapping[str, Any]],
    *,
    label: str,
) -> list[dict[str, str]]:
    """Return one Rust assertion request for every exact assignment."""

    facts = []
    seen: set[str] = set()
    for assignment in assignments:
        discovery_id = _string(
            assignment,
            "discovery_id",
            f"{label} assignment",
        )
        if discovery_id in seen:
            raise ValueError(f"{label} assignments duplicate {discovery_id}")
        seen.add(discovery_id)
        source_kind = _string(assignment, "source_kind", discovery_id)
        if source_kind not in RUST_SOURCE_KINDS:
            raise ValueError(
                f"{discovery_id} requires unsupported assertion adapter "
                f"{source_kind}"
            )
        facts.append(
            {
                "discovery_id": discovery_id,
                "discovery_source_kind": source_kind,
                "path": _string(assignment, "path", discovery_id),
                "name": _string(assignment, "name", discovery_id),
            }
        )
    return sorted(facts, key=lambda row: (row["path"], row["name"]))


def _string(
    row: Mapping[str, Any],
    field: str,
    label: str,
) -> str:
    value = row.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} lacks {field}")
    return value
