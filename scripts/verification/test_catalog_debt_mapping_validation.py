"""Focused validation for mapping-accelerator debt-report rows."""

from __future__ import annotations

import re
from typing import Any

from .test_catalog_validation import check_exact_fields


MAPPING_ACCELERATOR_FIELDS = {
    "pending_proposals",
    "by_candidate_tier",
    "specification_demands",
    "unmapped_code_facts",
    "demand_ranking",
    "structural_demand_ranking",
    "unmapped_code_demand_ranking",
}
CANDIDATE_TIER_COUNT_FIELDS = {"candidate_tier", "pending_proposals"}
SPEC_DEMAND_RANKING_FIELDS = {
    "spec_gap_ref",
    "affected_discovery_ids",
    "covered_code_ids",
    "tests_touching",
    "code_facts_touched",
}
STRUCTURAL_DEMAND_RANKING_FIELDS = {
    "cell_id",
    "parent_spec_gap_ref",
    "area",
    "owner",
    "source_kind",
    "path",
    "qualified_symbol_parent",
    "item_kind",
    "affected_discovery_ids",
    "covered_code_ids",
    "tests_touching",
    "tests_exclusive",
    "tests_dominant",
    "code_facts_touched",
}
UNMAPPED_CODE_DEMAND_RANKING_FIELDS = {
    "area",
    "affected_discovery_ids",
    "covered_code_ids",
    "tests_touching",
    "code_facts_touched",
}

DISCOVERY_ID_RE = re.compile(r"^DISC_[A-F0-9]{20}$")
CODE_ID_RE = re.compile(r"^CODE_[A-F0-9]{64}$")
SPEC_GAP_RE = re.compile(r"^SPEC_GAP_[A-Z0-9_]+$")
DEMAND_CELL_RE = re.compile(r"^DEMAND_CELL_[A-F0-9]{64}$")


def validate_mapping_accelerator(value: Any, failures: list[str]) -> None:
    if not isinstance(value, dict):
        failures.append("mapping_accelerator must be an object")
        return
    check_exact_fields(
        value,
        MAPPING_ACCELERATOR_FIELDS,
        "mapping_accelerator",
        failures,
    )
    for field in (
        "pending_proposals",
        "specification_demands",
        "unmapped_code_facts",
    ):
        count = value.get(field)
        if not isinstance(count, int) or isinstance(count, bool) or count < 0:
            failures.append(f"mapping_accelerator.{field} must be nonnegative")

    tier_rows = value.get("by_candidate_tier")
    tier_total = 0
    if not isinstance(tier_rows, list):
        failures.append("mapping_accelerator.by_candidate_tier must be an array")
    else:
        observed_tiers: list[Any] = []
        for index, row in enumerate(tier_rows):
            if not isinstance(row, dict):
                failures.append(
                    f"mapping_accelerator.by_candidate_tier[{index}] must be an object"
                )
                continue
            check_exact_fields(
                row,
                CANDIDATE_TIER_COUNT_FIELDS,
                f"mapping_accelerator.by_candidate_tier[{index}]",
                failures,
            )
            observed_tiers.append(row.get("candidate_tier"))
            count = row.get("pending_proposals")
            if not isinstance(count, int) or isinstance(count, bool) or count < 0:
                failures.append(
                    "mapping_accelerator.by_candidate_tier"
                    f"[{index}].pending_proposals must be nonnegative"
                )
            else:
                tier_total += count
        if observed_tiers != ["C1", "C2", "C3"]:
            failures.append(
                "mapping_accelerator.by_candidate_tier must use C1, C2, C3 order"
            )
    if value.get("pending_proposals") != tier_total:
        failures.append(
            "mapping_accelerator.pending_proposals does not match tier totals"
        )

    demand_rows = value.get("demand_ranking")
    if not isinstance(demand_rows, list):
        failures.append("mapping_accelerator.demand_ranking must be an array")
    else:
        for index, row in enumerate(demand_rows):
            _validate_spec_demand_row(row, index, failures)
        canonical = sorted(
            (row for row in demand_rows if isinstance(row, dict)),
            key=lambda row: (
                -_safe_int(row.get("tests_touching")),
                -_safe_int(row.get("code_facts_touched")),
                str(row.get("spec_gap_ref", "")),
            ),
        )
        if demand_rows != canonical:
            failures.append(
                "mapping_accelerator.demand_ranking must use canonical yield order"
            )

    structural_rows = value.get("structural_demand_ranking")
    if not isinstance(structural_rows, list):
        failures.append(
            "mapping_accelerator.structural_demand_ranking must be an array"
        )
    else:
        for index, row in enumerate(structural_rows):
            _validate_structural_demand_row(row, index, failures)
        canonical = sorted(
            (row for row in structural_rows if isinstance(row, dict)),
            key=lambda row: (
                -_safe_int(row.get("tests_exclusive")),
                -_safe_int(row.get("tests_dominant")),
                -_safe_int(row.get("tests_touching")),
                -_safe_int(row.get("code_facts_touched")),
                str(row.get("cell_id", "")),
            ),
        )
        if structural_rows != canonical:
            failures.append(
                "mapping_accelerator.structural_demand_ranking "
                "must use canonical locality order"
            )

    unmapped_rows = value.get("unmapped_code_demand_ranking")
    covered_unmapped_code: set[str] = set()
    if not isinstance(unmapped_rows, list):
        failures.append(
            "mapping_accelerator.unmapped_code_demand_ranking must be an array"
        )
    else:
        for index, row in enumerate(unmapped_rows):
            _validate_unmapped_code_demand_row(row, index, failures)
            if isinstance(row, dict) and isinstance(row.get("covered_code_ids"), list):
                covered_unmapped_code.update(
                    code_id
                    for code_id in row["covered_code_ids"]
                    if isinstance(code_id, str)
                )
        canonical = sorted(
            (row for row in unmapped_rows if isinstance(row, dict)),
            key=lambda row: (
                -_safe_int(row.get("tests_touching")),
                -_safe_int(row.get("code_facts_touched")),
                str(row.get("area", "")),
            ),
        )
        if unmapped_rows != canonical:
            failures.append(
                "mapping_accelerator.unmapped_code_demand_ranking "
                "must use canonical yield order"
            )
    if value.get("unmapped_code_facts") != len(covered_unmapped_code):
        failures.append(
            "mapping_accelerator.unmapped_code_facts does not match ranked CODE IDs"
        )


def _validate_spec_demand_row(
    value: Any,
    index: int,
    failures: list[str],
) -> None:
    label = f"mapping_accelerator.demand_ranking[{index}]"
    if not isinstance(value, dict):
        failures.append(f"{label} must be an object")
        return
    check_exact_fields(value, SPEC_DEMAND_RANKING_FIELDS, label, failures)
    if not SPEC_GAP_RE.fullmatch(str(value.get("spec_gap_ref", ""))):
        failures.append(f"{label}.spec_gap_ref is invalid")
    discovery_ids = _validate_identifier_array(
        value.get("affected_discovery_ids"),
        DISCOVERY_ID_RE,
        f"{label}.affected_discovery_ids",
        failures,
    )
    code_ids = _validate_identifier_array(
        value.get("covered_code_ids"),
        CODE_ID_RE,
        f"{label}.covered_code_ids",
        failures,
    )
    _validate_positive_count(
        value.get("tests_touching"),
        len(discovery_ids),
        f"{label}.tests_touching",
        failures,
    )
    _validate_positive_count(
        value.get("code_facts_touched"),
        len(code_ids),
        f"{label}.code_facts_touched",
        failures,
    )


def _validate_structural_demand_row(
    value: Any,
    index: int,
    failures: list[str],
) -> None:
    label = f"mapping_accelerator.structural_demand_ranking[{index}]"
    if not isinstance(value, dict):
        failures.append(f"{label} must be an object")
        return
    check_exact_fields(value, STRUCTURAL_DEMAND_RANKING_FIELDS, label, failures)
    if not DEMAND_CELL_RE.fullmatch(str(value.get("cell_id", ""))):
        failures.append(f"{label}.cell_id is invalid")
    if not SPEC_GAP_RE.fullmatch(str(value.get("parent_spec_gap_ref", ""))):
        failures.append(f"{label}.parent_spec_gap_ref is invalid")
    for field in ("area", "owner", "source_kind", "path", "item_kind"):
        if not isinstance(value.get(field), str) or not value[field]:
            failures.append(f"{label}.{field} must be non-empty")
    if not isinstance(value.get("qualified_symbol_parent"), str):
        failures.append(f"{label}.qualified_symbol_parent must be a string")
    discovery_ids = _validate_identifier_array(
        value.get("affected_discovery_ids"),
        DISCOVERY_ID_RE,
        f"{label}.affected_discovery_ids",
        failures,
    )
    code_ids = _validate_identifier_array(
        value.get("covered_code_ids"),
        CODE_ID_RE,
        f"{label}.covered_code_ids",
        failures,
    )
    touching = value.get("tests_touching")
    _validate_positive_count(
        touching,
        len(discovery_ids),
        f"{label}.tests_touching",
        failures,
    )
    _validate_positive_count(
        value.get("code_facts_touched"),
        len(code_ids),
        f"{label}.code_facts_touched",
        failures,
    )
    for field in ("tests_exclusive", "tests_dominant"):
        count = value.get(field)
        if (
            not isinstance(count, int)
            or isinstance(count, bool)
            or count < 0
            or not isinstance(touching, int)
            or count > touching
        ):
            failures.append(
                f"{label}.{field} must be between zero and tests_touching"
            )


def _validate_unmapped_code_demand_row(
    value: Any,
    index: int,
    failures: list[str],
) -> None:
    label = f"mapping_accelerator.unmapped_code_demand_ranking[{index}]"
    if not isinstance(value, dict):
        failures.append(f"{label} must be an object")
        return
    check_exact_fields(
        value,
        UNMAPPED_CODE_DEMAND_RANKING_FIELDS,
        label,
        failures,
    )
    if not isinstance(value.get("area"), str) or not value["area"]:
        failures.append(f"{label}.area must be non-empty")
    discovery_ids = _validate_identifier_array(
        value.get("affected_discovery_ids"),
        DISCOVERY_ID_RE,
        f"{label}.affected_discovery_ids",
        failures,
    )
    code_ids = _validate_identifier_array(
        value.get("covered_code_ids"),
        CODE_ID_RE,
        f"{label}.covered_code_ids",
        failures,
    )
    _validate_positive_count(
        value.get("tests_touching"),
        len(discovery_ids),
        f"{label}.tests_touching",
        failures,
    )
    _validate_positive_count(
        value.get("code_facts_touched"),
        len(code_ids),
        f"{label}.code_facts_touched",
        failures,
    )


def _validate_identifier_array(
    value: Any,
    pattern: re.Pattern[str],
    label: str,
    failures: list[str],
) -> list[str]:
    if not isinstance(value, list):
        failures.append(f"{label} must be an array")
        return []
    strings = [item for item in value if isinstance(item, str)]
    if len(strings) != len(value) or strings != sorted(set(strings)) or any(
        not pattern.fullmatch(item) for item in strings
    ):
        failures.append(f"{label} must contain sorted unique valid identifiers")
    return strings


def _validate_positive_count(
    value: Any,
    expected: int,
    label: str,
    failures: list[str],
) -> None:
    if not isinstance(value, int) or isinstance(value, bool) or value < 1:
        failures.append(f"{label} must be positive")
    elif value != expected:
        failures.append(f"{label} does not match its identifier array")


def _safe_int(value: Any) -> int:
    return value if isinstance(value, int) and not isinstance(value, bool) else -1
