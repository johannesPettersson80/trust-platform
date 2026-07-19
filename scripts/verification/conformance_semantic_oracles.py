"""Explicit semantic-oracle assessment for cataloged conformance cases."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any

from .metadata_validator.oracle_refs import ORACLE_AUTHORITIES


def catalog_semantic_oracle(
    *,
    case_id: str,
    profile: str,
    catalog_record: Mapping[str, Any] | None,
    spec_sources: Mapping[str, Mapping[str, Any]],
) -> tuple[str | None, str | None]:
    """Return an explicit eligible oracle binding, requiring it for v2 cases."""

    if catalog_record is None:
        return None, None
    oracle_ref = catalog_record.get("oracle_ref")
    expected_result = (
        catalog_record.get("expected_result")
    )
    if oracle_ref is None and expected_result is None and profile != "v2":
        return None, None
    if not isinstance(oracle_ref, str) or not oracle_ref:
        raise ValueError(f"{case_id} requires an explicit catalog oracle_ref")
    if not isinstance(expected_result, str) or not expected_result:
        raise ValueError(f"{case_id} requires a non-empty catalog expected_result")

    source_id = oracle_ref.split("#", 1)[0]
    source = spec_sources.get(source_id)
    if not isinstance(source, Mapping):
        raise ValueError(f"{case_id} oracle_ref names unknown source {source_id}")
    if source.get("source_status") != "active":
        raise ValueError(f"{case_id} oracle_ref source must be active")
    if source.get("oracle_eligible") is not True:
        raise ValueError(f"{case_id} oracle_ref source must be oracle-eligible")
    if source.get("authority") not in ORACLE_AUTHORITIES:
        raise ValueError(f"{case_id} oracle_ref source authority is not eligible")
    return oracle_ref, expected_result


def unresolved_v2_oracle_gaps(
    categories: Sequence[str],
    cases: Sequence[Mapping[str, Any]],
) -> list[dict[str, Any]]:
    gaps: list[dict[str, Any]] = []
    for category in categories:
        selected = [row for row in cases if row["category"] == category]
        assessed = bool(selected) and all(
            row.get("invariant_ids")
            and row.get("oracle_ref")
            and row.get("expected_result")
            for row in selected
        )
        if assessed:
            continue
        gaps.append(
            {
                "category": category,
                "case_ids": [row["case_id"] for row in selected],
                "case_present": bool(selected),
                "expected_artifact_present": bool(selected)
                and all(row["expected_artifact_path"] for row in selected),
                "invariant_mapping_state": (
                    "linked"
                    if selected and all(row["invariant_ids"] for row in selected)
                    else "missing"
                ),
                "semantic_oracle_state": "not_assessed",
                "gap_status": "open",
            }
        )
    return gaps
