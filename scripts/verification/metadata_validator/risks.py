"""Risk-register validation for the verification metadata control plane."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Callable

from .constants import RISKS

Fail = Callable[[Path, str], None]
Require = Callable[[Path, dict[str, Any], list[str], str], None]
CheckCommon = Callable[[Path, dict[str, Any]], None]
CheckRefs = Callable[
    [Path, list[str], dict[str, dict[str, Any]], str, str], None
]


def validate_risks(
    *,
    fail: Fail,
    require: Require,
    check_common: CheckCommon,
    check_refs: CheckRefs,
    risks: dict[str, dict[str, Any]],
    invariants: dict[str, dict[str, Any]],
    spec_gaps: dict[str, dict[str, Any]],
    spec_sources: dict[str, dict[str, Any]],
    evidence: dict[str, dict[str, Any]],
) -> None:
    """Validate risk records and all typed traceability references."""

    required = [
        "schema_version",
        "id",
        "title",
        "area",
        "risk",
        "owner",
        "status",
        "last_reviewed",
        "description",
        "mitigation",
        "related_invariants",
    ]
    for record in risks.values():
        path = record["_path"]
        require(path, record, required, "risk")
        check_common(path, record)
        if record.get("risk") not in RISKS:
            fail(path, f"{record['id']} has unknown risk {record.get('risk')!r}")
        check_refs(
            path,
            record.get("related_invariants", []),
            invariants,
            "invariant",
            record["id"],
        )
        check_refs(
            path,
            record.get("related_spec_gaps", []),
            spec_gaps,
            "spec gap",
            record["id"],
        )
        check_refs(
            path,
            record.get("source_refs", []),
            spec_sources,
            "spec source",
            record["id"],
        )
        check_refs(
            path,
            record.get("related_spec_sources", []),
            spec_sources,
            "spec source",
            record["id"],
        )
        check_refs(
            path,
            record.get("evidence_refs", []),
            evidence,
            "evidence",
            record["id"],
        )
