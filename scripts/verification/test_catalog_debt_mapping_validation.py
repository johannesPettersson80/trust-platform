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
    "unmapped_code_demand_ranking",
}
CANDIDATE_TIER_COUNT_FIELDS = {"candidate_tier", "pending_proposals"}
SPEC_DEMAND_RANKING_FIELDS = {
    "spec_gap_ref",
    "affected_discovery_ids",
    "covered_code_ids",
    "tests_unblocked",
    "code_facts_unblocked",
}
UNMAPPED_CODE_DEMAND_RANKING_FIELDS = {
    "area",
    "affected_discovery_ids",
    "covered_code_ids",
    "tests_affected",
    "code_facts_missing",
}

DISCOVERY_ID_RE = re.compile(r"^DISC_[A-F0-9]{20}$")
CODE_ID_RE = re.compile(r"^CODE_[A-F0-9]{64}$")
SPEC_GAP_RE = re.compile(r"^SPEC_GAP_[A-Z0-9_]+$")


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
                -_safe_int(row.get("tests_unblocked")),
                -_safe_int(row.get("code_facts_unblocked")),
                str(row.get("spec_gap_ref", "")),
            ),
        )
        if demand_rows != canonical:
            failures.append(
                "mapping_accelerator.demand_ranking must use canonical yield order"
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
                -_safe_int(row.get("tests_affected")),
                -_safe_int(row.get("code_facts_missing")),
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
        value.get("tests_unblocked"),
        len(discovery_ids),
        f"{label}.tests_unblocked",
        failures,
    )
    _validate_positive_count(
        value.get("code_facts_unblocked"),
        len(code_ids),
        f"{label}.code_facts_unblocked",
        failures,
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
        value.get("tests_affected"),
        len(discovery_ids),
        f"{label}.tests_affected",
        failures,
    )
    _validate_positive_count(
        value.get("code_facts_missing"),
        len(code_ids),
        f"{label}.code_facts_missing",
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
