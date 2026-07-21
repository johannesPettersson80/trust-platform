"""Shared structural checks that do not merge proof-class semantics."""

from __future__ import annotations

from typing import Any, Mapping


def validate_proof_row(
    row: Mapping[str, Any], budget: Mapping[str, Any], expected_id: str
) -> list[str]:
    failures: list[str] = []
    if row.get("id") != expected_id:
        failures.append(f"{expected_id} validator received {row.get('id')!r}")
    if budget.get("proof_class") != expected_id:
        failures.append(f"{expected_id} budget binding is wrong")
    denominator = row.get("denominator")
    numerator = row.get("numerator")
    debt = row.get("debt")
    if any(not isinstance(value, int) or isinstance(value, bool) or value < 0 for value in (denominator, numerator, debt)):
        failures.append(f"{expected_id} numerator/denominator/debt must be nonnegative integers")
    elif denominator != numerator + debt:
        failures.append(f"{expected_id} numerator plus debt does not equal denominator")
    for field in (
        "stale", "unowned", "skipped", "filtered", "unrun", "timeouts",
        "harness_failures", "execution_count", "producer_exit", "validator_exit",
    ):
        value = row.get(field)
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            failures.append(f"{expected_id} {field} must be a nonnegative integer")
    if row.get("status") == "closed":
        for field in (
            "debt", "stale", "unowned", "skipped", "filtered", "unrun",
            "timeouts", "harness_failures", "producer_exit", "validator_exit",
        ):
            if row.get(field) != 0:
                failures.append(f"closed {expected_id} has nonzero {field}")
        if isinstance(denominator, int) and denominator > 0 and row.get("execution_count", 0) < 1:
            failures.append(f"closed {expected_id} has zero execution")
    return failures
